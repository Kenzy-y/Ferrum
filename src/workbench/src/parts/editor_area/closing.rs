//! Saving with questions, and closing the window or quitting with unsaved
//! files: the window closes only after the user chose and every chosen
//! save worked.

use editor::{DiskState, Editor};
use gpui::{Context, Entity, PromptLevel, Task, Window};
use i18n::FluentArgs;

use crate::Workbench;
use crate::parts::editor_area::tabs::file_title;

/// What happens once the user agreed to close.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AfterClose {
    /// The window closes.
    CloseWindow,
    /// The application quits.
    Quit,
}

/// State of closing the window.
#[derive(Debug, Default)]
pub(crate) struct Closing {
    /// The user agreed; the platform may close the window now.
    confirmed: bool,
    /// The question is on screen; another close request waits for it.
    asking: bool,
}

impl Workbench {
    /// Saves `editor`. If its file changed on disk since, asks first
    /// whether to overwrite it. The task tells whether the file was saved.
    pub(crate) fn save_editor(
        &mut self,
        editor: &Entity<Editor>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Task<bool> {
        if editor.read(cx).disk_state() != DiskState::Changed {
            return editor.update(cx, Editor::save);
        }
        let name = editor
            .read(cx)
            .path()
            .map(|path| file_title(path).to_string())
            .unwrap_or_default();
        let mut args = FluentArgs::new();
        args.set("name", name);
        let message = self.i18n.text_with("editor-overwrite", &args);
        let detail = self.i18n.text("editor-overwrite-detail");
        let answers = [
            self.i18n.text("editor-overwrite-confirm"),
            self.i18n.text("editor-close-cancel"),
        ];
        let answer = window.prompt(
            PromptLevel::Warning,
            &message,
            Some(&detail),
            &answers.iter().map(String::as_str).collect::<Vec<_>>(),
            cx,
        );
        let editor = editor.clone();
        cx.spawn_in(window, async move |_, cx| {
            if !matches!(answer.await, Ok(0)) {
                return false;
            }
            editor.update(cx, Editor::save).await
        })
    }

    /// Whether the platform may close the window now. With unsaved files it
    /// asks first and closes the window itself once the user agreed.
    pub fn should_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.closing.confirmed {
            return true;
        }
        self.close(AfterClose::CloseWindow, window, cx);
        false
    }

    /// Closes the window (`workbench.closeWindow`), or quits (`app.quit`),
    /// after asking about unsaved files.
    pub(crate) fn close(&mut self, after: AfterClose, window: &mut Window, cx: &mut Context<Self>) {
        if self.closing.asking {
            return;
        }
        let dirty: Vec<Entity<Editor>> = self
            .editors()
            .filter(|editor| editor.read(cx).is_dirty())
            .cloned()
            .collect();
        if dirty.is_empty() {
            self.finish_close(after, window, cx);
            return;
        }
        let names: Vec<String> = dirty
            .iter()
            .filter_map(|editor| {
                editor
                    .read(cx)
                    .path()
                    .map(|path| file_title(path).to_string())
            })
            .collect();
        let mut args = FluentArgs::new();
        args.set("count", dirty.len());
        let message = self.i18n.text_with("window-close-unsaved", &args);
        let detail = names.join("\n");
        let answers = [
            self.i18n.text("window-close-save-all"),
            self.i18n.text("editor-close-discard"),
            self.i18n.text("editor-close-cancel"),
        ];
        let answer = window.prompt(
            PromptLevel::Warning,
            &message,
            Some(&detail),
            &answers.iter().map(String::as_str).collect::<Vec<_>>(),
            cx,
        );
        self.closing.asking = true;
        cx.spawn_in(window, async move |workbench, cx| {
            let answer = answer.await;
            let close = match answer {
                Ok(0) => {
                    // One by one: a save may ask about a changed file.
                    let mut saved_all = true;
                    for editor in dirty {
                        let Ok(save) = workbench.update_in(cx, |workbench, window, cx| {
                            workbench.save_editor(&editor, window, cx)
                        }) else {
                            return;
                        };
                        saved_all &= save.await;
                    }
                    saved_all
                }
                Ok(1) => true,
                _ => false,
            };
            let _ = workbench.update_in(cx, |workbench, window, cx| {
                workbench.closing.asking = false;
                if close {
                    workbench.finish_close(after, window, cx);
                }
            });
        })
        .detach();
    }

    /// Quits the application (`app.quit`) after asking about unsaved
    /// files.
    pub fn quit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close(AfterClose::Quit, window, cx);
    }

    fn finish_close(&mut self, after: AfterClose, window: &mut Window, cx: &mut Context<Self>) {
        match after {
            AfterClose::CloseWindow => {
                self.closing.confirmed = true;
                window.remove_window();
            }
            AfterClose::Quit => {
                self.closing.confirmed = true;
                cx.quit();
            }
        }
    }
}
