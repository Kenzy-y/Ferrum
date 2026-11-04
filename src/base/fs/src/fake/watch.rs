//! Watching the fake file system: every change is reported at once, one
//! batch per operation.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures::channel::mpsc::UnboundedSender;

use super::State;
use crate::{FsEvent, FsEventKind, WatchFilter, WatchMode};

/// A registered watch.
pub(super) struct FakeWatch {
    pub(super) root: PathBuf,
    pub(super) mode: WatchMode,
    pub(super) filter: Arc<dyn WatchFilter>,
    pub(super) sender: UnboundedSender<Vec<FsEvent>>,
}

/// Reports a change of `path` to every watch that covers it.
pub(super) fn notify(state: &mut State, path: &Path, kind: FsEventKind) {
    state.watches.retain(|watch| !watch.sender.is_closed());
    for watch in &state.watches {
        let in_scope = match watch.mode {
            WatchMode::Recursive => path.starts_with(&watch.root),
            WatchMode::NonRecursive => {
                path == watch.root || path.parent() == Some(watch.root.as_path())
            }
        };
        if in_scope && !watch.filter.is_ignored(path) {
            let _ = watch.sender.unbounded_send(vec![FsEvent {
                path: path.to_path_buf(),
                kind,
            }]);
        }
    }
}
