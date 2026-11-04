use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

use futures::StreamExt as _;

use super::*;
use crate::FsEvent;

fn ready<T>(future: impl Future<Output = T>) -> T {
    match pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("FakeFs futures complete immediately"),
    }
}

fn sample() -> FakeFs {
    let fs = FakeFs::new();
    fs.insert_file("/project/src/main.rs", "fn main() {}");
    fs.insert_file("/project/Cargo.toml", "[package]");
    fs.insert_dir("/project/target/debug");
    fs
}

#[test]
fn reads_lists_and_writes() {
    let fs = sample();
    assert_eq!(
        ready(fs.read_to_string(Path::new("/project/src/main.rs"))).unwrap(),
        "fn main() {}"
    );

    let mut names: Vec<_> = ready(fs.read_dir(Path::new("/project")))
        .unwrap()
        .into_iter()
        .map(|entry| entry.name)
        .collect();
    names.sort();
    assert_eq!(names, ["Cargo.toml", "src", "target"]);

    ready(fs.write(Path::new("/project/src/lib.rs"), b"pub fn f() {}")).unwrap();
    assert!(
        ready(fs.metadata(Path::new("/project/src/lib.rs")))
            .unwrap()
            .unwrap()
            .is_file()
    );
    assert!(
        ready(fs.write(Path::new("/missing/file.rs"), b"x"))
            .unwrap_err()
            .is_not_found()
    );
}

#[test]
fn writing_through_a_link_changes_the_target() {
    let fs = sample();
    fs.insert_symlink("/project/link.toml", "Cargo.toml");
    ready(fs.write(Path::new("/project/link.toml"), b"[workspace]")).unwrap();

    assert!(fs.is_symlink("/project/link.toml"));
    assert_eq!(
        ready(fs.read_to_string(Path::new("/project/Cargo.toml"))).unwrap(),
        "[workspace]"
    );
}

#[test]
fn read_only_files_are_not_written() {
    let fs = sample();
    fs.set_read_only("/project/Cargo.toml", true);
    assert!(ready(fs.write(Path::new("/project/Cargo.toml"), b"x")).is_err());
}

#[test]
fn canonicalize_detects_link_cycles() {
    let fs = sample();
    fs.insert_symlink("/project/a", "b");
    fs.insert_symlink("/project/b", "a");
    assert!(ready(fs.canonicalize(Path::new("/project/a"))).is_err());

    fs.insert_symlink("/project/src/up", "..");
    assert_eq!(
        ready(fs.canonicalize(Path::new("/project/src/up"))).unwrap(),
        PathBuf::from("/project")
    );
}

#[test]
fn non_recursive_watches_see_direct_entries_only() {
    let fs = sample();
    let filter: Arc<dyn WatchFilter> = Arc::new(|_: &Path| false);
    let mut watcher = fs
        .watch(
            Path::new("/project/target"),
            WatchMode::NonRecursive,
            filter,
        )
        .unwrap();
    fs.insert_file("/project/target/debug/deep.o", "x");
    fs.insert_file("/project/target/.rustc_info.json", "{}");
    let batch = ready(watcher.events().next()).unwrap();
    assert_eq!(
        batch[0].path,
        PathBuf::from("/project/target/.rustc_info.json")
    );
}

#[test]
fn watchers_skip_ignored_paths() {
    let fs = sample();
    let filter: Arc<dyn WatchFilter> = Arc::new(|path: &Path| path.starts_with("/project/target"));
    let mut watcher = fs
        .watch(Path::new("/project"), WatchMode::Recursive, filter)
        .unwrap();

    fs.insert_file("/project/target/debug/app.exe", "binary");
    fs.insert_file("/project/src/main.rs", "fn main() { }");

    let batch = ready(watcher.events().next()).unwrap();
    assert_eq!(
        batch,
        vec![FsEvent {
            path: PathBuf::from("/project/src/main.rs"),
            kind: FsEventKind::Changed,
        }]
    );
}
