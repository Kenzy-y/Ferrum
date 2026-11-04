//! In-memory [`Fs`] for tests.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::SystemTime;

use futures::channel::mpsc::{UnboundedSender, unbounded};

use crate::{
    BoxFuture, DirEntry, EntryKind, Fs, FsError, FsEvent, FsEventKind, Metadata, WatchFilter,
    Watcher,
};

/// Maximum number of symbolic links followed while resolving a path.
const MAX_LINK_DEPTH: usize = 40;

#[derive(Debug, Clone)]
enum Node {
    File {
        contents: Vec<u8>,
        read_only: bool,
        modified: SystemTime,
    },
    Dir,
    Symlink(PathBuf),
}

struct FakeWatch {
    root: PathBuf,
    filter: Arc<dyn WatchFilter>,
    sender: UnboundedSender<Vec<FsEvent>>,
}

#[derive(Default)]
struct State {
    nodes: BTreeMap<PathBuf, Node>,
    watches: Vec<FakeWatch>,
}

/// A file system held in memory. Paths are used as given (use absolute
/// paths such as `/project/src/main.rs`); parents are created by the
/// `insert_*` helpers. Changes made through it are reported to watchers
/// immediately, one batch per operation.
#[derive(Default)]
pub struct FakeFs {
    state: Mutex<State>,
}

impl std::fmt::Debug for FakeFs {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("FakeFs").finish_non_exhaustive()
    }
}

impl FakeFs {
    /// Creates an empty file system.
    pub fn new() -> Self {
        Self::default()
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Creates a file (and its parent directories) with `contents`.
    pub fn insert_file(&self, path: impl AsRef<Path>, contents: impl Into<Vec<u8>>) {
        let path = path.as_ref();
        let mut state = self.state();
        let existed = state.nodes.contains_key(path);
        insert_parents(&mut state, path);
        state.nodes.insert(
            path.to_path_buf(),
            Node::File {
                contents: contents.into(),
                read_only: false,
                modified: SystemTime::now(),
            },
        );
        let kind = if existed {
            FsEventKind::Changed
        } else {
            FsEventKind::Created
        };
        notify(&mut state, path, kind);
    }

    /// Creates a directory and its parents.
    pub fn insert_dir(&self, path: impl AsRef<Path>) {
        let path = path.as_ref();
        let mut state = self.state();
        insert_parents(&mut state, path);
        if state.nodes.insert(path.to_path_buf(), Node::Dir).is_none() {
            notify(&mut state, path, FsEventKind::Created);
        }
    }

    /// Creates a symbolic link at `path` pointing to `target` (relative
    /// targets are resolved against the link's directory).
    pub fn insert_symlink(&self, path: impl AsRef<Path>, target: impl Into<PathBuf>) {
        let path = path.as_ref();
        let mut state = self.state();
        insert_parents(&mut state, path);
        state
            .nodes
            .insert(path.to_path_buf(), Node::Symlink(target.into()));
        notify(&mut state, path, FsEventKind::Created);
    }

    /// Marks a file read-only (or writable again).
    pub fn set_read_only(&self, path: impl AsRef<Path>, read_only: bool) {
        if let Some(Node::File {
            read_only: flag, ..
        }) = self.state().nodes.get_mut(path.as_ref())
        {
            *flag = read_only;
        }
    }

    /// Removes `path` and everything below it.
    pub fn remove(&self, path: impl AsRef<Path>) {
        let path = path.as_ref();
        let mut state = self.state();
        state
            .nodes
            .retain(|node_path, _| !node_path.starts_with(path));
        notify(&mut state, path, FsEventKind::Removed);
    }

    /// Whether `path` itself (not its target) is a symbolic link.
    pub fn is_symlink(&self, path: impl AsRef<Path>) -> bool {
        matches!(
            self.state().nodes.get(path.as_ref()),
            Some(Node::Symlink(_))
        )
    }
}

fn insert_parents(state: &mut State, path: &Path) {
    let mut parent = path.parent();
    while let Some(directory) = parent {
        if directory.as_os_str().is_empty() {
            break;
        }
        state
            .nodes
            .entry(directory.to_path_buf())
            .or_insert(Node::Dir);
        parent = directory.parent();
    }
}

fn notify(state: &mut State, path: &Path, kind: FsEventKind) {
    state.watches.retain(|watch| !watch.sender.is_closed());
    for watch in &state.watches {
        if path.starts_with(&watch.root) && !watch.filter.is_ignored(path) {
            let _ = watch.sender.unbounded_send(vec![FsEvent {
                path: path.to_path_buf(),
                kind,
            }]);
        }
    }
}

/// Joins a link target to the link's directory and removes `.` / `..`.
fn resolve_target(link: &Path, target: &Path) -> PathBuf {
    let joined = if target.is_relative() {
        link.parent().unwrap_or(Path::new("")).join(target)
    } else {
        target.to_path_buf()
    };
    let mut normalized = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other),
        }
    }
    normalized
}

/// Resolves every symbolic link in `path`, component by component.
fn canonical(state: &State, path: &Path) -> Result<PathBuf, FsError> {
    let mut resolved = PathBuf::new();
    let mut hops = 0;
    let mut pending: Vec<PathBuf> = path
        .components()
        .map(|component| PathBuf::from(component.as_os_str()))
        .collect();
    pending.reverse();
    while let Some(component) = pending.pop() {
        let candidate = resolved.join(&component);
        match state.nodes.get(&candidate) {
            Some(Node::Symlink(target)) => {
                hops += 1;
                if hops > MAX_LINK_DEPTH {
                    return Err(FsError::Io {
                        path: path.to_path_buf(),
                        source: std::io::Error::other("too many levels of symbolic links"),
                    });
                }
                let target = resolve_target(&candidate, target);
                let mut parts: Vec<PathBuf> = target
                    .components()
                    .map(|part| PathBuf::from(part.as_os_str()))
                    .collect();
                parts.reverse();
                resolved = PathBuf::new();
                pending.extend(parts);
            }
            // Missing components are kept: callers decide whether the
            // resolved path must exist.
            Some(_) | None => resolved = candidate,
        }
    }
    Ok(resolved)
}

fn metadata_of(state: &State, path: &Path) -> Option<Metadata> {
    let node = state.nodes.get(path)?;
    let is_symlink = matches!(node, Node::Symlink(_));
    let target = if is_symlink {
        canonical(state, path)
            .ok()
            .and_then(|resolved| state.nodes.get(&resolved))
    } else {
        Some(node)
    };
    let metadata = match target {
        Some(Node::File {
            contents,
            read_only,
            modified,
        }) => Metadata {
            kind: EntryKind::File,
            is_symlink,
            len: u64::try_from(contents.len()).unwrap_or(u64::MAX),
            modified: Some(*modified),
            read_only: *read_only,
        },
        Some(Node::Dir) => Metadata {
            kind: EntryKind::Dir,
            is_symlink,
            len: 0,
            modified: None,
            read_only: false,
        },
        Some(Node::Symlink(_)) | None => Metadata {
            kind: EntryKind::Other,
            is_symlink,
            len: 0,
            modified: None,
            read_only: false,
        },
    };
    Some(metadata)
}

impl FakeFs {
    fn read_sync(&self, path: &Path) -> Result<Vec<u8>, FsError> {
        let state = self.state();
        let resolved = canonical(&state, path)?;
        match state.nodes.get(&resolved) {
            Some(Node::File { contents, .. }) => Ok(contents.clone()),
            Some(_) => Err(FsError::WrongKind {
                path: path.to_path_buf(),
                expected: "file",
            }),
            None => Err(FsError::NotFound {
                path: path.to_path_buf(),
            }),
        }
    }

    fn write_sync(&self, path: &Path, contents: &[u8]) -> Result<(), FsError> {
        let mut state = self.state();
        // Writing through a link changes the target and keeps the link.
        let target = canonical(&state, path).unwrap_or_else(|_| path.to_path_buf());
        if let Some(parent) = target
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            && !matches!(state.nodes.get(parent), Some(Node::Dir))
        {
            return Err(FsError::NotFound {
                path: parent.to_path_buf(),
            });
        }
        let kind = match state.nodes.get_mut(&target) {
            Some(Node::File {
                contents: existing,
                read_only,
                modified,
            }) => {
                if *read_only {
                    return Err(FsError::Io {
                        path: target,
                        source: std::io::ErrorKind::PermissionDenied.into(),
                    });
                }
                existing.clear();
                existing.extend_from_slice(contents);
                *modified = SystemTime::now();
                FsEventKind::Changed
            }
            Some(_) => {
                return Err(FsError::WrongKind {
                    path: target,
                    expected: "file",
                });
            }
            None => {
                state.nodes.insert(
                    target.clone(),
                    Node::File {
                        contents: contents.to_vec(),
                        read_only: false,
                        modified: SystemTime::now(),
                    },
                );
                FsEventKind::Created
            }
        };
        notify(&mut state, &target, kind);
        Ok(())
    }

    fn read_dir_sync(&self, path: &Path) -> Result<Vec<DirEntry>, FsError> {
        let state = self.state();
        let resolved = canonical(&state, path)?;
        match state.nodes.get(&resolved) {
            Some(Node::Dir) => {}
            Some(_) => {
                return Err(FsError::WrongKind {
                    path: path.to_path_buf(),
                    expected: "directory",
                });
            }
            None => {
                return Err(FsError::NotFound {
                    path: path.to_path_buf(),
                });
            }
        }
        let mut entries = Vec::new();
        for node_path in state.nodes.keys() {
            if node_path.parent() != Some(resolved.as_path()) {
                continue;
            }
            let Some(name) = node_path.file_name() else {
                continue;
            };
            let entry_path = path.join(name);
            if let Some(metadata) = metadata_of(&state, node_path) {
                entries.push(DirEntry {
                    name: name.to_string_lossy().into_owned(),
                    path: entry_path,
                    metadata,
                });
            }
        }
        Ok(entries)
    }
}

impl Fs for FakeFs {
    fn read_to_string<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<String, FsError>> {
        let result = self.read_sync(path).and_then(|bytes| {
            String::from_utf8(bytes).map_err(|_| FsError::InvalidUtf8 {
                path: path.to_path_buf(),
            })
        });
        Box::pin(async move { result })
    }

    fn read<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<Vec<u8>, FsError>> {
        let result = self.read_sync(path);
        Box::pin(async move { result })
    }

    fn write<'a>(
        &'a self,
        path: &'a Path,
        contents: &'a [u8],
    ) -> BoxFuture<'a, Result<(), FsError>> {
        let result = self.write_sync(path, contents);
        Box::pin(async move { result })
    }

    fn create_dir_all<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<(), FsError>> {
        self.insert_dir(path);
        Box::pin(async move { Ok(()) })
    }

    fn metadata<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<Option<Metadata>, FsError>> {
        let result = Ok(metadata_of(&self.state(), path));
        Box::pin(async move { result })
    }

    fn read_dir<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<Vec<DirEntry>, FsError>> {
        let result = self.read_dir_sync(path);
        Box::pin(async move { result })
    }

    fn canonicalize<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<PathBuf, FsError>> {
        let state = self.state();
        let result = canonical(&state, path).and_then(|resolved| {
            if state.nodes.contains_key(&resolved) {
                Ok(resolved)
            } else {
                Err(FsError::NotFound {
                    path: path.to_path_buf(),
                })
            }
        });
        drop(state);
        Box::pin(async move { result })
    }

    fn watch(&self, root: &Path, filter: Arc<dyn WatchFilter>) -> Result<Watcher, FsError> {
        let (sender, receiver) = unbounded();
        self.state().watches.push(FakeWatch {
            root: root.to_path_buf(),
            filter,
            sender,
        });
        Ok(Watcher::new(Box::pin(receiver), Box::new(())))
    }
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    use futures::StreamExt as _;

    use super::*;

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
    fn watchers_skip_ignored_paths() {
        let fs = sample();
        let filter: Arc<dyn WatchFilter> =
            Arc::new(|path: &Path| path.starts_with("/project/target"));
        let mut watcher = fs.watch(Path::new("/project"), filter).unwrap();

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
}
