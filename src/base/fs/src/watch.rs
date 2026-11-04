//! Watching directories for changes.

use std::any::Any;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use futures::channel::mpsc::{UnboundedSender, unbounded};
use notify::event::{ModifyKind, RenameMode};
use notify::{EventKind, RecursiveMode, Watcher as _};

use crate::{BoxStream, FsError};

/// How long events are collected into one batch. Saving a file or a build
/// step produces bursts of events; a short window turns them into one
/// update without a noticeable delay.
pub const WATCH_DEBOUNCE: Duration = Duration::from_millis(50);

/// Decides which paths a watcher reports.
pub trait WatchFilter: Send + Sync {
    /// Whether events for `path` should be dropped.
    fn is_ignored(&self, path: &Path) -> bool;
}

impl<F: Fn(&Path) -> bool + Send + Sync> WatchFilter for F {
    fn is_ignored(&self, path: &Path) -> bool {
        self(path)
    }
}

/// How deep a watch reaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WatchMode {
    /// The folder and everything below it.
    Recursive,
    /// Only the folder's direct entries.
    NonRecursive,
}

/// What happened to a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FsEventKind {
    /// The path appeared (created or renamed to).
    Created,
    /// The contents or metadata changed.
    Changed,
    /// The path disappeared (deleted or renamed from).
    Removed,
    /// Events were lost; everything under the path must be re-read.
    Rescan,
}

/// A change of one path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FsEvent {
    /// The changed path.
    pub path: PathBuf,
    /// What happened.
    pub kind: FsEventKind,
}

/// An active watch. Dropping it stops watching.
pub struct Watcher {
    events: BoxStream<Vec<FsEvent>>,
    _guard: Box<dyn Any + Send>,
}

impl std::fmt::Debug for Watcher {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Watcher").finish_non_exhaustive()
    }
}

impl Watcher {
    pub(crate) fn new(events: BoxStream<Vec<FsEvent>>, guard: Box<dyn Any + Send>) -> Self {
        Self {
            events,
            _guard: guard,
        }
    }

    /// Batches of events; ends when the watch fails for good.
    pub fn events(&mut self) -> &mut BoxStream<Vec<FsEvent>> {
        &mut self.events
    }
}

/// Starts a platform watcher on `root`.
pub(crate) fn watch_real(
    root: &Path,
    mode: WatchMode,
    filter: Arc<dyn WatchFilter>,
) -> Result<Watcher, FsError> {
    let watch_error = |error: &dyn std::fmt::Display| FsError::Watch {
        path: root.to_path_buf(),
        message: error.to_string(),
    };
    let (raw_sender, raw_receiver) = mpsc::channel::<FsEvent>();
    let rescan_root = root.to_path_buf();
    let mut watcher = notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
        // Runs on the watcher thread: filter here so that ignored paths
        // never reach the debouncer or the UI.
        let events = match result {
            Ok(event) => convert(&event),
            Err(_) => vec![FsEvent {
                path: rescan_root.clone(),
                kind: FsEventKind::Rescan,
            }],
        };
        for event in events {
            if event.kind == FsEventKind::Rescan || !filter.is_ignored(&event.path) {
                // The receiver is gone only while the watch is shutting down.
                let _ = raw_sender.send(event);
            }
        }
    })
    .map_err(|error| watch_error(&error))?;
    watcher
        .watch(
            root,
            match mode {
                WatchMode::Recursive => RecursiveMode::Recursive,
                WatchMode::NonRecursive => RecursiveMode::NonRecursive,
            },
        )
        .map_err(|error| watch_error(&error))?;

    let (sender, receiver) = unbounded();
    std::thread::Builder::new()
        .name("fs-watch-debounce".into())
        .spawn(move || debounce(&raw_receiver, &sender))
        .map_err(|error| watch_error(&error))?;
    Ok(Watcher::new(Box::pin(receiver), Box::new(watcher)))
}

/// Maps a platform event to our events.
fn convert(event: &notify::Event) -> Vec<FsEvent> {
    let make = |path: &PathBuf, kind| FsEvent {
        path: path.clone(),
        kind,
    };
    if event.need_rescan() {
        return event
            .paths
            .iter()
            .map(|path| make(path, FsEventKind::Rescan))
            .collect();
    }
    match event.kind {
        EventKind::Access(_) => Vec::new(),
        EventKind::Create(_) => event
            .paths
            .iter()
            .map(|path| make(path, FsEventKind::Created))
            .collect(),
        EventKind::Remove(_) | EventKind::Modify(ModifyKind::Name(RenameMode::From)) => event
            .paths
            .iter()
            .map(|path| make(path, FsEventKind::Removed))
            .collect(),
        EventKind::Modify(ModifyKind::Name(RenameMode::To)) => event
            .paths
            .iter()
            .map(|path| make(path, FsEventKind::Created))
            .collect(),
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
            let mut paths = event.paths.iter();
            paths
                .next()
                .map(|from| make(from, FsEventKind::Removed))
                .into_iter()
                .chain(paths.map(|to| make(to, FsEventKind::Created)))
                .collect()
        }
        _ => event
            .paths
            .iter()
            .map(|path| make(path, FsEventKind::Changed))
            .collect(),
    }
}

/// Collects events into batches of [`WATCH_DEBOUNCE`] and forwards them.
/// Ends when either side is dropped.
fn debounce(raw: &mpsc::Receiver<FsEvent>, sender: &UnboundedSender<Vec<FsEvent>>) {
    while let Ok(first) = raw.recv() {
        let mut batch = vec![first];
        let deadline = Instant::now() + WATCH_DEBOUNCE;
        let disconnected = loop {
            match raw.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(event) => batch.push(event),
                Err(RecvTimeoutError::Timeout) => break false,
                Err(RecvTimeoutError::Disconnected) => break true,
            }
        };
        if sender.unbounded_send(deduplicate(batch)).is_err() || disconnected {
            return;
        }
    }
}

/// Drops repeated events, keeping the first occurrence of each.
pub(crate) fn deduplicate(batch: Vec<FsEvent>) -> Vec<FsEvent> {
    let mut seen = HashSet::new();
    batch
        .into_iter()
        .filter(|event| seen.insert(event.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deduplicates_keeping_order() {
        let event = |path: &str, kind| FsEvent {
            path: PathBuf::from(path),
            kind,
        };
        let batch = vec![
            event("a", FsEventKind::Changed),
            event("b", FsEventKind::Created),
            event("a", FsEventKind::Changed),
            event("a", FsEventKind::Removed),
        ];
        assert_eq!(
            deduplicate(batch),
            vec![
                event("a", FsEventKind::Changed),
                event("b", FsEventKind::Created),
                event("a", FsEventKind::Removed),
            ]
        );
    }

    #[test]
    fn renames_become_remove_and_create() {
        let event = notify::Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::Both)))
            .add_path(PathBuf::from("old"))
            .add_path(PathBuf::from("new"));
        let converted = convert(&event);
        assert_eq!(converted[0].kind, FsEventKind::Removed);
        assert_eq!(converted[1].path, PathBuf::from("new"));
        assert_eq!(converted[1].kind, FsEventKind::Created);
    }
}
