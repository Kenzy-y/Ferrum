//! The file behind an editor: saving, following changes on disk (reading
//! the file again, conflicts with unsaved edits, deletion) and renames.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use fs::{EntryKind, Fs, FsEvent, WatchFilter, WatchMode};
use futures::StreamExt as _;
use gpui::{Context, Task};
use text::Buffer;

use crate::editor::{Editor, EditorEvent};

/// How the file on disk relates to the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiskState {
    /// The file is where the editor saves; nothing to resolve.
    #[default]
    InSync,
    /// The file changed on disk while the editor has unsaved edits: load
    /// it (`editor.reloadFromDisk`) or keep the edits
    /// (`editor.keepLocalChanges`); saving asks before overwriting.
    Changed,
    /// The file was deleted on disk: save it again or close it.
    Deleted,
}

/// A saved state that matches no text: after keeping local changes over a
/// changed file, nothing in the history is what the file contains.
const NO_STATE: u64 = u64::MAX;

/// Where an editor's text is saved.
pub(crate) struct FileState {
    fs: Arc<dyn Fs>,
    path: PathBuf,
    /// `Document::state_id` of the text the file contains.
    saved_state: u64,
    /// The text as it is in the file (a rope clone, sharing its nodes), to
    /// tell real changes on disk from our own saves and to recognize the
    /// file after a rename even with unsaved edits.
    saved: Buffer,
    /// What the disk says now.
    disk: DiskState,
    /// The changed file's text while `disk` is `Changed`.
    disk_text: Option<Buffer>,
    /// A save is running; changes on disk meanwhile are our own.
    saving: bool,
    _watch: Task<()>,
}

impl FileState {
    pub(crate) fn new(
        fs: Arc<dyn Fs>,
        path: PathBuf,
        saved_state: u64,
        saved: Buffer,
        cx: &mut Context<Editor>,
    ) -> Self {
        let watch = watch(fs.clone(), &path, cx);
        Self {
            fs,
            path,
            saved_state,
            saved,
            disk: DiskState::InSync,
            disk_text: None,
            saving: false,
            _watch: watch,
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn saved_state(&self) -> u64 {
        self.saved_state
    }
}

/// Watches the folder of `path` (not recursively). Every entry of the
/// folder is reported, not only the file: a rename shows up as the file
/// disappearing and another name appearing.
fn watch(fs: Arc<dyn Fs>, path: &Path, cx: &mut Context<Editor>) -> Task<()> {
    let Some(folder) = path.parent().map(Path::to_path_buf) else {
        return Task::ready(());
    };
    let path = path.to_path_buf();
    cx.spawn(async move |editor, cx| {
        let filter: Arc<dyn WatchFilter> = Arc::new(|_: &Path| false);
        let watcher = cx
            .background_executor()
            .spawn(async move { fs.watch(&folder, WatchMode::NonRecursive, filter) })
            .await;
        let mut watcher = match watcher {
            Ok(watcher) => watcher,
            Err(error) => {
                tracing::warn!(%error, path = %path.display(), "cannot watch the file");
                return;
            }
        };
        while let Some(batch) = watcher.events().next().await {
            if editor
                .update(cx, |editor, cx| editor.files_changed_on_disk(&batch, cx))
                .is_err()
            {
                break;
            }
        }
    })
}

/// What became of the file after events touched it.
enum OnDisk {
    /// It is there with this text (`None`: unreadable).
    Present(Option<Buffer>),
    /// It is gone and another name has exactly its saved text: renamed.
    Renamed(PathBuf),
    /// It is gone.
    Gone,
}

fn same_text(left: &Buffer, right: &Buffer) -> bool {
    left.rope() == right.rope() && left.has_bom() == right.has_bom()
}

/// Finds where the file went: the one other file of `folder` whose
/// contents are exactly `saved` (renames are atomic, so the new name
/// already exists when the old one is reported gone).
async fn find_renamed(fs: &dyn Fs, folder: &Path, old: &Path, saved: &Buffer) -> Option<PathBuf> {
    let entries = fs.read_dir(folder).await.ok()?;
    let bom = if saved.has_bom() { 3 } else { 0 };
    let saved_len = u64::try_from(saved.len() + bom).ok()?;
    let mut found = None;
    for entry in entries {
        if entry.path == old
            || entry.metadata.kind != EntryKind::File
            || entry.metadata.len != saved_len
        {
            continue;
        }
        let Ok(buffer) = text::load(fs, &entry.path).await else {
            continue;
        };
        if same_text(&buffer, saved) {
            if found.is_some() {
                // Two copies: no way to tell which one is the file.
                return None;
            }
            found = Some(entry.path);
        }
    }
    found
}

impl Editor {
    /// How the file on disk relates to the editor.
    pub fn disk_state(&self) -> DiskState {
        self.file
            .as_ref()
            .map_or(DiskState::InSync, |file| file.disk)
    }

    /// Writes the text to its file in the background; the task tells
    /// whether it worked. Saving resolves a conflict with the disk (the
    /// caller asks before overwriting a changed file).
    pub fn save(&mut self, cx: &mut Context<Self>) -> Task<bool> {
        let Some(file) = &mut self.file else {
            return Task::ready(true);
        };
        file.saving = true;
        let fs = file.fs.clone();
        let path = file.path.clone();
        // A rope clone shares its nodes: constant time.
        let buffer = self.document.buffer().clone();
        let saved = buffer.clone();
        let state = self.document.mark_saved();
        cx.spawn(async move |editor, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { text::save(fs.as_ref(), &path, &buffer).await })
                .await;
            editor
                .update(cx, |editor, cx| {
                    let Some(file) = &mut editor.file else {
                        return false;
                    };
                    file.saving = false;
                    let saved_ok = match result {
                        Ok(()) => {
                            file.saved_state = state;
                            file.saved = saved;
                            file.disk = DiskState::InSync;
                            file.disk_text = None;
                            cx.emit(EditorEvent::Saved);
                            true
                        }
                        Err(error) => {
                            tracing::error!(%error, "cannot save");
                            cx.emit(EditorEvent::SaveFailed(error.to_string()));
                            false
                        }
                    };
                    cx.notify();
                    saved_ok
                })
                .unwrap_or(false)
        })
    }

    /// Replaces the edits with the file's text from disk, as one undoable
    /// edit (`editor.reloadFromDisk`).
    pub(crate) fn reload_from_disk(&mut self, cx: &mut Context<Self>) {
        let Some(disk_text) = self.file.as_mut().and_then(|file| file.disk_text.take()) else {
            return;
        };
        self.change(cx, |document| document.replace_all(&disk_text));
        let state = self.document.mark_saved();
        if let Some(file) = &mut self.file {
            file.saved_state = state;
            file.saved = disk_text;
            file.disk = DiskState::InSync;
        }
        cx.emit(EditorEvent::DiskStateChanged);
    }

    /// Keeps the edits over a file changed on disk
    /// (`editor.keepLocalChanges`); the next save overwrites the file.
    pub(crate) fn keep_local_changes(&mut self, cx: &mut Context<Self>) {
        let Some(file) = &mut self.file else {
            return;
        };
        if file.disk != DiskState::Changed {
            return;
        }
        if let Some(disk_text) = file.disk_text.take() {
            file.saved = disk_text;
        }
        file.saved_state = NO_STATE;
        file.disk = DiskState::InSync;
        cx.emit(EditorEvent::DiskStateChanged);
        cx.notify();
    }

    /// Entries of the file's folder changed on disk.
    /// - The file has the saved text (or the editor's): nothing to do.
    /// - Another text, no unsaved edits: it is read again.
    /// - Another text, unsaved edits: a conflict ([`DiskState::Changed`]).
    /// - The file is gone, another name has its saved text: the editor
    ///   follows the rename, unsaved edits and all.
    /// - The file is gone: [`DiskState::Deleted`].
    fn files_changed_on_disk(&mut self, batch: &[FsEvent], cx: &mut Context<Self>) {
        let Some(file) = &self.file else {
            return;
        };
        if file.saving || !batch.iter().any(|event| event.path == file.path) {
            return;
        }
        let Some(folder) = file.path.parent().map(Path::to_path_buf) else {
            return;
        };
        let fs = file.fs.clone();
        let path = file.path.clone();
        let saved = file.saved.clone();
        let version = self.document.version();
        cx.spawn(async move |editor, cx| {
            let on_disk = cx
                .background_executor()
                .spawn(async move {
                    if !matches!(fs.metadata(&path).await, Ok(Some(_))) {
                        return match find_renamed(fs.as_ref(), &folder, &path, &saved).await {
                            Some(renamed) => OnDisk::Renamed(renamed),
                            None => OnDisk::Gone,
                        };
                    }
                    match text::load(fs.as_ref(), &path).await {
                        Ok(buffer) => OnDisk::Present(Some(buffer)),
                        Err(error) => {
                            tracing::debug!(%error, "file changed but cannot be read");
                            OnDisk::Present(None)
                        }
                    }
                })
                .await;
            let _ = editor.update(cx, |editor, cx| {
                // Edits made while the file was read: never replace them.
                let edited_meanwhile = editor.document.version() != version;
                editor.apply_disk_change(on_disk, edited_meanwhile, cx);
            });
        })
        .detach();
    }

    fn apply_disk_change(
        &mut self,
        on_disk: OnDisk,
        edited_meanwhile: bool,
        cx: &mut Context<Self>,
    ) {
        let dirty = self.is_dirty() || edited_meanwhile;
        let Some(file) = &mut self.file else {
            return;
        };
        let before = file.disk;
        match on_disk {
            OnDisk::Present(Some(buffer)) => {
                if same_text(&buffer, &file.saved) {
                    // Our own save, or touched without a change.
                    file.disk = DiskState::InSync;
                    file.disk_text = None;
                } else if same_text(&buffer, self.document.buffer()) {
                    // The disk now has exactly the editor's text.
                    file.saved = buffer;
                    file.saved_state = self.document.mark_saved();
                    file.disk = DiskState::InSync;
                    file.disk_text = None;
                } else if dirty {
                    file.disk = DiskState::Changed;
                    file.disk_text = Some(buffer);
                } else {
                    file.saved = buffer.clone();
                    file.disk = DiskState::InSync;
                    file.disk_text = None;
                    self.document.reload(buffer);
                    self.follow_text_changes(cx);
                    if let Some(file) = &mut self.file {
                        file.saved_state = self.document.state_id();
                    }
                    cx.emit(EditorEvent::Reloaded);
                    cx.emit(EditorEvent::SelectionsChanged);
                }
            }
            OnDisk::Present(None) => {}
            OnDisk::Renamed(renamed) => {
                tracing::info!(from = %file.path.display(), to = %renamed.display(), "file renamed");
                file.path = renamed;
                file.disk = DiskState::InSync;
                cx.emit(EditorEvent::Renamed);
            }
            OnDisk::Gone => {
                file.disk = DiskState::Deleted;
                file.disk_text = None;
            }
        }
        if self.disk_state() != before {
            cx.emit(EditorEvent::DiskStateChanged);
        }
        cx.notify();
    }
}
