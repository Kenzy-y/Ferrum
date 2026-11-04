//! In-memory [`Fs`] for tests.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::SystemTime;

use futures::channel::mpsc::unbounded;

mod paths;
#[cfg(test)]
mod tests;
mod watch;

use self::paths::{canonical, metadata_of};
use self::watch::{FakeWatch, notify};
use crate::{
    BoxFuture, DirEntry, Fs, FsError, FsEventKind, Metadata, WatchFilter, WatchMode, Watcher,
};

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

    fn watch(
        &self,
        root: &Path,
        mode: WatchMode,
        filter: Arc<dyn WatchFilter>,
    ) -> Result<Watcher, FsError> {
        let (sender, receiver) = unbounded();
        self.state().watches.push(FakeWatch {
            root: root.to_path_buf(),
            mode,
            filter,
            sender,
        });
        Ok(Watcher::new(Box::pin(receiver), Box::new(())))
    }
}
