//! Keys that open popups take before the text: the completion list, the
//! list of places, snippet tab stops and the hover.

use gpui::Context;

use crate::actions::{Action, Motion};
use crate::editor::Editor;
use crate::language::completion::{Step, completion_rows};

/// How `action` moves the selection of a list, if it does.
fn list_step(action: Action) -> Option<Step> {
    match action {
        Action::Move(Motion::Up, false) => Some(Step::Up),
        Action::Move(Motion::Down, false) => Some(Step::Down),
        Action::Move(Motion::PageUp, false) => Some(Step::PageUp),
        Action::Move(Motion::PageDown, false) => Some(Step::PageDown),
        _ => None,
    }
}

impl Editor {
    /// Keys an open popup handles instead of the text: the completion list
    /// and the list of places take moves up and down, Enter, Tab and
    /// Escape; Escape also closes the hover. Returns `true` if the action
    /// was taken.
    pub(crate) fn popup_takes(&mut self, action: Action, cx: &mut Context<Self>) -> bool {
        if self.completion_open() {
            let rows = completion_rows(&theme::shared_theme(cx));
            if let Some(step) = list_step(action) {
                self.step_completion(step, rows, cx);
                return true;
            }
            match action {
                Action::Newline | Action::Indent => {
                    self.accept_completion(None, cx);
                    return true;
                }
                Action::SingleCursor => {
                    self.close_completion(cx);
                    return true;
                }
                Action::Move(..) => self.close_completion(cx),
                _ => {}
            }
        }
        if self.locations_open() {
            let rows = completion_rows(&theme::shared_theme(cx));
            if let Some(step) = list_step(action) {
                self.step_locations(step, rows, cx);
                return true;
            }
            match action {
                Action::Newline | Action::Indent => {
                    self.accept_location(None, cx);
                    return true;
                }
                Action::SingleCursor => {
                    self.dismiss_popups(cx);
                    return true;
                }
                _ => {
                    self.dismiss_popups(cx);
                }
            }
        }
        match action {
            Action::Indent => return self.snippet_step(true, cx),
            Action::Outdent => return self.snippet_step(false, cx),
            Action::SingleCursor if self.end_snippet() => return true,
            _ => {}
        }
        action == Action::SingleCursor && self.dismiss_hover(cx)
    }
}
