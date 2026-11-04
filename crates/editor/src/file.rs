//! The file behind an editor: saving, and reading it again when it changes
//! on disk while there are no unsaved edits.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use fs::{Fs, WatchFilter, WatchMode};
use futures::StreamExt as _;
use gpui::{Context, Task};

use crate::editor::{Editor, EditorEvent};

/// Where an editor's text is saved.
pub(crate) struct FileState {
    fs: Arc<dyn Fs>,
    path: PathBuf,
    /// Document version that matches the file.
    saved_version: u64,
    /// A save is running; changes on disk meanwhile are our own.
    saving: bool,
    _watch: Task<()>,
}

impl FileState {
    pub(crate) fn new(
        fs: Arc<dyn Fs>,
        path: PathBuf,
        saved_version: u64,
        cx: &mut Context<Editor>,
    ) -> Self {
        let watch = watch(fs.clone(), path.clone(), cx);
        Self {
            fs,
            path,
            saved_version,
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

/// Watches the folder of `path` (not recursively) for changes of the file.
fn watch(fs: Arc<dyn Fs>, path: PathBuf, cx: &mut Context<Editor>) -> Task<()> {
    let Some(folder) = path.parent().map(Path::to_path_buf) else {
        return Task::ready(());
    };
    cx.spawn(async move |editor, cx| {
        let target = path.clone();
        let filter: Arc<dyn WatchFilter> = Arc::new(move |changed: &Path| changed != target);
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
        while watcher.events().next().await.is_some() {
            if editor.update(cx, Editor::file_changed_on_disk).is_err() {
                break;
            }
        }
    })
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

    /// The file changed on disk: without unsaved edits, read it again; the
    /// text is compared in the background and replaced only if it differs
    /// (so our own saves change nothing).
    fn file_changed_on_disk(&mut self, cx: &mut Context<Self>) {
        let dirty = self.is_dirty();
        let Some(file) = &self.file else {
            return;
        };
        if dirty || file.saving {
            return;
        }
        let fs = file.fs.clone();
        let path = file.path.clone();
        let current = self.document.buffer().clone();
        let version = self.document.version();
        cx.spawn(async move |editor, cx| {
            let loaded = cx
                .background_executor()
                .spawn(async move {
                    let buffer = text::load(fs.as_ref(), &path).await?;
                    let same =
                        buffer.rope() == current.rope() && buffer.has_bom() == current.has_bom();
                    Ok::<_, text::TextError>((!same).then_some(buffer))
                })
                .await;
            let _ = editor.update(cx, |editor, cx| match loaded {
                Ok(Some(buffer)) if editor.document.version() == version => {
                    editor.document.reload(buffer);
                    editor.follow_text_changes(cx);
                    if let Some(file) = &mut editor.file {
                        file.saved_version = editor.document.version();
                    }
                    cx.emit(EditorEvent::Reloaded);
                    cx.emit(EditorEvent::SelectionsChanged);
                    cx.notify();
                }
                Ok(_) => {}
                Err(error) => tracing::debug!(%error, "file changed but cannot be read"),
            });
        })
        .detach();
    }
}
