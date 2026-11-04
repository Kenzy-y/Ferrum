//! Watching a real directory.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use fs::{Fs, FsEvent, RealFs, WatchFilter};
use futures::StreamExt as _;

/// Generous: file system notifications are slow on some CI machines.
const TIMEOUT: Duration = Duration::from_secs(10);

struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn reports_changes_and_drops_ignored_paths() {
    let dir = TempDir(std::env::temp_dir().join(format!("fs-watch-test-{}", std::process::id())));
    let _ = std::fs::remove_dir_all(&dir.0);
    std::fs::create_dir_all(dir.0.join("target")).unwrap();
    std::fs::create_dir_all(dir.0.join("src")).unwrap();
    // Watch the canonical path: on macOS the temp dir is behind a symlink
    // and events carry the resolved path.
    let root = std::fs::canonicalize(&dir.0).unwrap();

    let ignored_root = root.join("target");
    let filter: Arc<dyn WatchFilter> = Arc::new(move |path: &Path| path.starts_with(&ignored_root));
    let mut watcher = RealFs.watch(&root, filter).unwrap();

    let (sender, receiver) = mpsc::channel::<Vec<FsEvent>>();
    let collector = std::thread::spawn(move || {
        futures::executor::block_on(async {
            while let Some(batch) = watcher.events().next().await {
                if sender.send(batch).is_err() {
                    break;
                }
            }
        });
    });

    std::fs::write(root.join("target").join("build.log"), "noise").unwrap();
    std::fs::write(root.join("src").join("main.rs"), "fn main() {}").unwrap();

    let expected = root.join("src").join("main.rs");
    let deadline = Instant::now() + TIMEOUT;
    let mut seen = Vec::new();
    while !seen.iter().any(|event: &FsEvent| event.path == expected) {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let batch = receiver.recv_timeout(remaining).unwrap_or_else(|_| {
            panic!(
                "no event for {} within {TIMEOUT:?}; got {seen:?}",
                expected.display()
            )
        });
        seen.extend(batch);
    }
    assert!(
        seen.iter()
            .all(|event| !event.path.starts_with(root.join("target"))),
        "ignored paths must be dropped: {seen:?}"
    );
    drop(receiver);
    // The collector ends with the watcher; do not wait for it here.
    drop(collector);
}
