//! Opening files in editors: read in the background, then shown in a tab
//! and connected to the language service of the folder; files that are not
//! UTF-8 text get a tab that says why.

use std::path::{Path, PathBuf};

use editor::{Editor, EditorEvent};
use gpui::{AppContext as _, Context, Entity, Window};
use i18n::FluentArgs;
use language::LanguageEntry;
use text::{Document, Point, TextError};

use super::Shell;

impl Shell {
    /// Shows `path` in an editor: its tab if it is open, otherwise a new
    /// one once the file is read. With `target`, the cursor goes there.
    pub(super) fn open_file(
        &mut self,
        path: PathBuf,
        target: Option<Point>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(editor) = self.workbench.read(cx).editor_for(&path, cx) {
            self.workbench.update(cx, |workbench, cx| {
                workbench.open_editor(editor.clone(), window, cx);
            });
            go_to(&editor, target, cx);
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
                    let entry = shell.languages.for_path(&path).cloned();
                    let grammar = entry.as_ref().and_then(|entry| entry.grammar().cloned());
                    let language = entry.as_ref().map(|entry| entry.id().to_owned());
                    let language = language.as_deref();
                    let settings = shell.settings.get::<editor::EditorSettings>(language);
                    let syntax_settings = shell.settings.get::<syntax::SyntaxSettings>(language);
                    let file = Some((fs, path));
                    let editor = cx.new(|cx| {
                        let mut editor =
                            Editor::new(Document::new(buffer), file, settings, window, cx);
                        editor.set_grammar(grammar, syntax_settings, cx);
                        if let Some(entry) = &entry {
                            editor.set_auto_pairs(entry.pairs().clone());
                        }
                        editor
                    });
                    shell.connect_language(&editor, entry.as_deref(), window, cx);
                    shell.workbench.update(cx, |workbench, cx| {
                        workbench.open_editor(editor.clone(), window, cx);
                    });
                    go_to(&editor, target, cx);
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

    /// Connects `editor` to the language service of its language in the
    /// open folder, and opens the files its go to definition finds.
    fn connect_language(
        &mut self,
        editor: &Entity<Editor>,
        entry: Option<&LanguageEntry>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.subscribe_in(
            editor,
            window,
            |shell, _, event: &EditorEvent, window, cx| {
                if let EditorEvent::OpenLocation(location) = event {
                    shell.open_file(
                        location.path.clone(),
                        Some(location.range.start),
                        window,
                        cx,
                    );
                }
            },
        )
        .detach();
        let (Some(entry), Some(workspace)) = (entry, &self.workspace) else {
            return;
        };
        let Some((service, started)) =
            self.language_servers
                .service(entry, workspace.root(), &self.settings)
        else {
            return;
        };
        if started {
            self.watch_problems(&service, cx);
        }
        editor.update(cx, |editor, cx| {
            editor.set_language_service(service, entry.id(), cx);
        });
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

/// Puts the cursor of `editor` at `target`, if given.
fn go_to(editor: &Entity<Editor>, target: Option<Point>, cx: &mut gpui::App) {
    if let Some(target) = target {
        editor.update(cx, |editor, cx| {
            let offset = editor.document().buffer().point_to_offset(target);
            editor.go_to(offset, cx);
        });
    }
}
