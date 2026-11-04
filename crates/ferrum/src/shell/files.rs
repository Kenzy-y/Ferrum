//! Opening files in editors: read in the background, then shown in a tab;
//! files that are not UTF-8 text get a tab that says why.

use std::path::{Path, PathBuf};

use gpui::{AppContext as _, Context, Window};
use i18n::FluentArgs;
use text::{Document, TextError};

use super::Shell;

impl Shell {
    /// Shows `path` in an editor: its tab if it is open, otherwise a new
    /// one once the file is read.
    pub(super) fn open_file(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(editor) = self.workbench.read(cx).editor_for(&path, cx) {
            self.workbench.update(cx, |workbench, cx| {
                workbench.open_editor(editor, window, cx);
            });
            return;
        }
        let fs = self.fs.clone();
        cx.spawn_in(window, async move |shell, cx| {
            let read_path = path.clone();
            let read_fs = fs.clone();
            let loaded = cx
                .background_executor()
                .spawn(async move { text::load(read_fs.as_ref(), &read_path).await })
                .await;
            let _ = shell.update_in(cx, |shell, window, cx| match loaded {
                Ok(buffer) => {
                    tracing::info!(path = %path.display(), "opened file");
                    let settings = shell.settings.get::<editor::EditorSettings>(None);
                    let file = Some((fs, path));
                    let editor = cx.new(|cx| {
                        editor::Editor::new(Document::new(buffer), file, settings, window, cx)
                    });
                    shell.workbench.update(cx, |workbench, cx| {
                        workbench.open_editor(editor, window, cx);
                    });
                }
                Err(error) => {
                    tracing::warn!(%error, "cannot open file");
                    let message = shell.open_error(&path, &error);
                    shell.workbench.update(cx, |workbench, cx| {
                        workbench.show_file_message(path, message.into(), window, cx);
                    });
                }
            });
        })
        .detach();
    }

    fn open_error(&self, path: &Path, error: &TextError) -> String {
        let mut args = FluentArgs::new();
        let name = path.file_name().map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        args.set("name", name);
        match error {
            TextError::Binary { .. } => self.i18n.text_with("editor-open-binary", &args),
            TextError::NotUtf8 { .. } => self.i18n.text_with("editor-open-not-utf8", &args),
            TextError::Fs(error) => {
                args.set("error", error.to_string());
                self.i18n.text_with("editor-open-failed", &args)
            }
        }
    }
}
