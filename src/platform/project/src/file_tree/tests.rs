use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

use fs::FakeFs;

use super::*;

fn ready<T>(future: impl Future<Output = T>) -> T {
    match pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("FakeFs futures complete immediately"),
    }
}

const ROOT: &str = "/work/forge";

fn project() -> FakeFs {
    let fs = FakeFs::new();
    fs.insert_file("/work/forge/.gitignore", "target/\n");
    fs.insert_file("/work/forge/Cargo.toml", "[package]");
    fs.insert_file("/work/forge/README.md", "# forge");
    fs.insert_file("/work/forge/src/main.rs", "fn main() {}");
    fs.insert_file("/work/forge/src/lib.rs", "");
    fs.insert_dir("/work/forge/target/debug");
    fs.insert_file("/work/forge/.git/HEAD", "ref: refs/heads/main");
    fs.insert_dir("/work/forge/.github");
    fs
}

/// Loads `directory` into `tree` like the view does.
fn load(fs: &FakeFs, tree: &mut FileTree, directory: &str) {
    let rules = tree.rules();
    match ready(load_dir(fs, Path::new(directory), &rules)) {
        Ok(listing) => tree.apply(listing),
        Err(error) => tree.fail(Path::new(directory), &error),
    }
}

fn names(tree: &FileTree) -> Vec<String> {
    tree.rows()
        .iter()
        .map(|row| format!("{}{}", "  ".repeat(row.depth), row.entry.name))
        .collect()
}

#[test]
fn lists_folders_first_and_hides_version_control() {
    let fs = project();
    let mut tree = FileTree::new(PathBuf::from(ROOT));
    load(&fs, &mut tree, ROOT);
    assert_eq!(
        names(&tree),
        [
            ".github",
            "src",
            "target",
            ".gitignore",
            "Cargo.toml",
            "README.md"
        ]
    );
    let target = tree
        .rows()
        .into_iter()
        .find(|row| row.entry.name == "target")
        .unwrap();
    assert_eq!(target.entry.visibility, Visibility::Ignored);
    assert_eq!(target.expanded, Some(false));
}

#[test]
fn expands_lazily_and_collapses() {
    let fs = project();
    let mut tree = FileTree::new(PathBuf::from(ROOT));
    load(&fs, &mut tree, ROOT);

    let to_load = tree.toggle(Path::new("/work/forge/src")).unwrap();
    assert_eq!(to_load, PathBuf::from("/work/forge/src"));
    let loading = tree
        .rows()
        .into_iter()
        .find(|row| row.entry.name == "src")
        .unwrap();
    assert_eq!(loading.state, RowState::Loading);

    load(&fs, &mut tree, "/work/forge/src");
    assert_eq!(&names(&tree)[1..4], ["src", "  lib.rs", "  main.rs"]);

    assert_eq!(tree.toggle(Path::new("/work/forge/src")), None);
    assert!(!names(&tree).contains(&"  lib.rs".to_owned()));
    assert_eq!(
        tree.toggle(Path::new("/work/forge/src")),
        None,
        "already loaded"
    );
    assert!(names(&tree).contains(&"  lib.rs".to_owned()));
}

#[test]
fn link_cycles_are_not_expanded() {
    let fs = project();
    fs.insert_symlink("/work/forge/src/loop", "..");
    let mut tree = FileTree::new(PathBuf::from(ROOT));
    load(&fs, &mut tree, ROOT);
    tree.expand(Path::new("/work/forge/src"));
    load(&fs, &mut tree, "/work/forge/src");

    assert!(tree.expand(Path::new("/work/forge/src/loop")).is_some());
    load(&fs, &mut tree, "/work/forge/src/loop");
    let row = tree
        .rows()
        .into_iter()
        .find(|row| row.entry.name == "loop")
        .unwrap();
    assert_eq!(row.state, RowState::Cycle);
    assert_eq!(row.expanded, Some(false));
    assert_eq!(
        tree.expand(Path::new("/work/forge/src/loop")),
        None,
        "cycles stay collapsed"
    );
}

#[test]
fn watcher_events_reload_the_parent_folder() {
    let fs = project();
    let mut tree = FileTree::new(PathBuf::from(ROOT));
    load(&fs, &mut tree, ROOT);
    tree.expand(Path::new("/work/forge/src"));
    load(&fs, &mut tree, "/work/forge/src");

    let event = |path: &str, kind| FsEvent {
        path: PathBuf::from(path),
        kind,
    };
    let reload = tree.dirs_to_reload(&[
        event("/work/forge/src/new.rs", FsEventKind::Created),
        event("/work/forge/src/main.rs", FsEventKind::Changed),
        event("/work/forge/.gitignore", FsEventKind::Changed),
        event("/work/forge/unloaded/file.rs", FsEventKind::Created),
    ]);
    assert_eq!(
        reload.into_iter().collect::<Vec<_>>(),
        [
            PathBuf::from("/work/forge"),
            PathBuf::from("/work/forge/src")
        ]
    );

    let rescan = tree.dirs_to_reload(&[event(ROOT, FsEventKind::Rescan)]);
    assert_eq!(rescan.len(), 2);
}

#[test]
fn removed_folders_are_forgotten() {
    let fs = project();
    let mut tree = FileTree::new(PathBuf::from(ROOT));
    load(&fs, &mut tree, ROOT);
    tree.expand(Path::new("/work/forge/src"));
    load(&fs, &mut tree, "/work/forge/src");

    fs.remove("/work/forge/src");
    load(&fs, &mut tree, ROOT);
    assert!(!names(&tree).iter().any(|name| name.contains("src")));
    assert!(
        !tree
            .loaded_dirs()
            .contains(&PathBuf::from("/work/forge/src"))
    );
}

#[test]
fn changing_gitignore_changes_dimming() {
    let fs = project();
    let mut tree = FileTree::new(PathBuf::from(ROOT));
    load(&fs, &mut tree, ROOT);
    fs.insert_file("/work/forge/.gitignore", "README.md\n");
    load(&fs, &mut tree, ROOT);

    let visibility = |name: &str| {
        tree.rows()
            .into_iter()
            .find(|row| row.entry.name == name)
            .map(|row| row.entry.visibility)
    };
    assert_eq!(visibility("target"), Some(Visibility::Visible));
    assert_eq!(visibility("README.md"), Some(Visibility::Ignored));
}

#[test]
fn open_ignored_folders_are_listed_while_visible() {
    let fs = project();
    fs.insert_dir("/work/forge/target/debug/deps");
    let mut tree = FileTree::new(PathBuf::from(ROOT));
    load(&fs, &mut tree, ROOT);
    assert!(tree.open_ignored_dirs().is_empty());

    tree.expand(Path::new("/work/forge/target"));
    load(&fs, &mut tree, "/work/forge/target");
    tree.expand(Path::new("/work/forge/target/debug"));
    load(&fs, &mut tree, "/work/forge/target/debug");
    assert_eq!(
        tree.open_ignored_dirs(),
        [
            PathBuf::from("/work/forge/target"),
            PathBuf::from("/work/forge/target/debug")
        ]
    );

    tree.collapse(Path::new("/work/forge/target"));
    assert!(
        tree.open_ignored_dirs().is_empty(),
        "hidden by the collapsed parent"
    );
}
