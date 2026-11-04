//! The file behind an editor: saving, reading it again when it changes on
//! disk while there are no unsaved edits, and following it when it is
//! renamed.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use fs::{EntryKind, Fs, FsEvent, WatchFilter, WatchMode};
use futures::StreamExt as _;
use gpui::{Context, Task};
use text::Buffer;

use crate::editor::{Editor, EditorEvent};

/// Where an editor's text is saved.
pub(crate) struct FileState {
    fs: Arc<dyn Fs>,
    path: PathBuf,
    /// Document version that matches the file.
    saved_version: u64,
    /// The text as it is in the file (a rope clone, sharing its nodes), to
    /// recognize the file after a rename even with unsaved edits.
    saved: Buffer,
    /// A save is running; changes on disk meanwhile are our own.
    saving: bool,
    _watch: Task<()>,
}

impl FileState {
    pub(crate) fn new(
        fs: Arc<dyn Fs>,
        path: PathBuf,
        saved_version: u64,
        saved: Buffer,
        cx: &mut Context<Editor>,
    ) -> Self {
        let watch = watch(fs.clone(), &path, cx);
        Self {
            fs,
            path,
            saved_version,
            saved,
            saving: false,
            _watch: watch,
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn saved_version(&self) -> u64 {
        self.saved_version
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
    /// It is still there, with this text if it differs from the editor's.
    Present(Option<Buffer>),
    /// It is gone and `PathBuf` has exactly its saved text: renamed.
    Renamed(PathBuf),
    /// It is gone.
    Gone,
}

/// Finds where the file went: the one other file of `folder` whose
/// contents are exactly `saved` (renames are atomic, so the new name
/// already exists when the old one is reported gone).
async fn find_renamed(fs: &dyn Fs, folder: &Path, old: &Path, saved: &Buffer) -> Option<PathBuf> {
    let entries = fs.read_dir(folder).await.ok()?;
    let saved_len = (saved.len() + if saved.has_bom() { 3 } else { 0 }) as u64;
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
        if buffer.rope() == saved.rope() && buffer.has_bom() == saved.has_bom() {
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
    /// Writes the text to its file in the background.
    pub fn save(&mut self, cx: &mut Context<Self>) {
        let Some(file) = &mut self.file else {
            return;
        };
        file.saving = true;
        let fs = file.fs.clone();
        let path = file.path.clone();
        // A rope clone shares its nodes: constant time.
        let buffer = self.document.buffer().clone();
        let saved = buffer.clone();
        let version = self.document.version();
        cx.spawn(async move |editor, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { text::save(fs.as_ref(), &path, &buffer).await })
                .await;
            let _ = editor.update(cx, |editor, cx| {
                if let Some(file) = &mut editor.file {
                    file.saving = false;
                    match result {
                        Ok(()) => {
                            file.saved_version = version;
                            file.saved = saved;
                            cx.emit(EditorEvent::Saved);
                        }
                        Err(error) => {
                            tracing::error!(%error, "cannot save");
                            cx.emit(EditorEvent::SaveFailed(error.to_string()));
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Entries of the file's folder changed on disk. If the file is still
    /// there and there are no unsaved edits, it is read again (and the text
    /// replaced only if it differs, so our own saves change nothing). If it
    /// is gone and another name has its saved text, the editor follows the
    /// rename, unsaved edits and all.
    fn files_changed_on_disk(&mut self, batch: &[FsEvent], cx: &mut Context<Self>) {
        let dirty = self.is_dirty();
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
        let current = self.document.buffer().clone();
        let version = self.document.version();
        cx.spawn(async move |editor, cx| {
            let on_disk = cx
                .background_executor()
                .spawn(async move {
                    let exists = matches!(fs.metadata(&path).await, Ok(Some(_)));
                    if !exists {
                        return match find_renamed(fs.as_ref(), &folder, &path, &saved).await {
                            Some(renamed) => OnDisk::Renamed(renamed),
                            None => OnDisk::Gone,
                        };
                    }
                    if dirty {
                        return OnDisk::Present(None);
                    }
                    match text::load(fs.as_ref(), &path).await {
                        Ok(buffer) => {
                            let same = buffer.rope() == current.rope()
                                && buffer.has_bom() == current.has_bom();
                            OnDisk::Present((!same).then_some(buffer))
                        }
                        Err(error) => {
                            tracing::debug!(%error, "file changed but cannot be read");
                            OnDisk::Present(None)
                        }
                    }
                })
                .await;
            let _ = editor.update(cx, |editor, cx| match on_disk {
                OnDisk::Present(Some(buffer)) if editor.document.version() == version => {
                    editor.document.reload(buffer.clone());
                    editor.follow_text_changes(cx);
                    if let Some(file) = &mut editor.file {
                        file.saved_version = editor.document.version();
                        file.saved = buffer;
                    }
                    cx.emit(EditorEvent::Reloaded);
                    cx.emit(EditorEvent::SelectionsChanged);
                    cx.notify();
                }
                OnDisk::Renamed(renamed) => {
                    if let Some(file) = &mut editor.file {
                        tracing::info!(from = %file.path.display(), to = %renamed.display(), "file renamed");
                        file.path = renamed;
                    }
                    cx.emit(EditorEvent::Renamed);
                    cx.notify();
                }
                OnDisk::Present(_) | OnDisk::Gone => {}
            });
        })
        .detach();
    }
}
