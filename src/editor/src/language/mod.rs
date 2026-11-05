//! Language support in the editor. The document is kept in sync with a
//! [`language::LanguageService`], whose diagnostics are underlined.
//! Hover, completions and go to definition ask the same service. Only
//! what the service's capabilities allow is offered.

pub(crate) mod completion;
pub(crate) mod diagnostics;
pub(crate) mod hover;
pub(crate) mod markdown;
mod navigation;
pub(crate) mod popup;

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures::StreamExt as _;
use gpui::{AnyElement, Context, Task};
use language::{Capabilities, LanguageService, ServiceEvent};
use text::{Buffer, TextChanges};
use theme::Theme;

use crate::editor::{Editor, EditorEvent};

use self::completion::CompletionMenu;
use self::diagnostics::PlacedDiagnostic;
use self::hover::HoverState;

/// The editor's link to a language service.
pub(crate) struct LanguageState {
    pub(crate) service: Arc<dyn LanguageService>,
    /// The document as the service knows it.
    pub(crate) path: PathBuf,
    language_id: String,
    /// Version of the text last sent.
    version: i32,
    pub(crate) capabilities: Capabilities,
    /// Problems of the text, sorted by start, in current offsets.
    pub(crate) diagnostics: Vec<PlacedDiagnostic>,
    pub(crate) hover: HoverState,
    pub(crate) completion: Option<CompletionMenu>,
    /// The completion request in flight.
    pub(crate) completion_task: Option<Task<()>>,
    /// The go to definition request in flight.
    pub(crate) definition_task: Option<Task<()>>,
    _events: Task<()>,
}

impl Drop for LanguageState {
    fn drop(&mut self) {
        self.service.close_document(&self.path);
    }
}

impl Editor {
    /// Connects the text to `service` as a document of language
    /// `language_id`. Only editors of files can be connected.
    pub fn set_language_service(
        &mut self,
        service: Arc<dyn LanguageService>,
        language_id: &str,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = self.path().map(Path::to_path_buf) else {
            return;
        };
        // Dropping the previous state closes its document.
        self.language = None;
        self.document.track_changes();
        service.open_document(&path, language_id, 1, self.document.buffer().clone());
        let mut events = service.events();
        let task = cx.spawn(async move |editor, cx| {
            while let Some(event) = events.next().await {
                if editor
                    .update(cx, |editor, cx| editor.on_service_event(event, cx))
                    .is_err()
                {
                    break;
                }
            }
        });
        self.language = Some(LanguageState {
            capabilities: service.capabilities(),
            service,
            path,
            language_id: language_id.to_owned(),
            version: 1,
            diagnostics: Vec::new(),
            hover: HoverState::default(),
            completion: None,
            completion_task: None,
            definition_task: None,
            _events: task,
        });
        cx.notify();
    }

    /// The language service the text is connected to.
    pub fn language_service(&self) -> Option<&Arc<dyn LanguageService>> {
        self.language.as_ref().map(|language| &language.service)
    }

    /// Tells the service about edits, and moves diagnostics with them.
    pub(crate) fn follow_language_changes(
        &mut self,
        changes: &TextChanges,
        cx: &mut Context<Self>,
    ) {
        let Some(language) = &mut self.language else {
            return;
        };
        if changes.is_empty() {
            return;
        }
        language.version += 1;
        language.service.change_document(
            &language.path,
            language.version,
            changes.clone(),
            self.document.buffer().clone(),
        );
        diagnostics::follow(&mut language.diagnostics, changes, self.document.buffer());
        self.dismiss_hover(cx);
        self.update_completion_after_edit(changes, cx);
    }

    /// The file was saved.
    pub(crate) fn language_saved(&self) {
        if let Some(language) = &self.language {
            language.service.save_document(&language.path);
        }
    }

    /// The file was renamed on disk: the service gets the document under
    /// its new name.
    pub(crate) fn language_renamed(&mut self, cx: &mut Context<Self>) {
        if let Some(language) = &self.language {
            let service = language.service.clone();
            let language_id = language.language_id.clone();
            self.set_language_service(service, &language_id, cx);
        }
    }

    fn on_service_event(&mut self, event: ServiceEvent, cx: &mut Context<Self>) {
        let Some(language) = &mut self.language else {
            return;
        };
        match event {
            ServiceEvent::Diagnostics {
                path, diagnostics, ..
            } if path == language.path => {
                language.diagnostics = diagnostics::place(&diagnostics, self.document.buffer());
                cx.emit(EditorEvent::DiagnosticsChanged);
                cx.notify();
            }
            ServiceEvent::CapabilitiesChanged => {
                language.capabilities = language.service.capabilities();
                cx.notify();
            }
            _ => {}
        }
    }

    /// Closes the hover and the completion list; `true` if one was open.
    pub(crate) fn dismiss_popups(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(language) = &mut self.language else {
            return false;
        };
        let open = language.hover.popup.is_some() || language.completion.is_some();
        language.hover = HoverState::default();
        language.completion = None;
        language.completion_task = None;
        if open {
            cx.notify();
        }
        open
    }

    /// The open popups: the completion list, else the hover.
    pub(crate) fn render_popups(
        &self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        self.render_completion(theme, cx)
            .or_else(|| self.render_hover(theme, cx))
    }

    /// Colors of the grammar's highlights in `theme` (empty without one).
    pub(crate) fn syntax_colors(&self, theme: &Theme) -> Vec<Option<gpui::Hsla>> {
        self.syntax
            .as_ref()
            .map(|syntax| syntax.colors(&theme.syntax))
            .unwrap_or_default()
    }
}

/// The identifier around `offset`, if there is one.
pub(crate) fn identifier_at(buffer: &Buffer, offset: usize) -> Option<Range<usize>> {
    let start = completion::identifier_start(buffer, offset);
    let line_end = buffer.line_range(buffer.offset_to_point(offset).row).end;
    let after = buffer.slice(offset..line_end);
    let length: usize = after
        .chars()
        .take_while(|c| completion::is_identifier_char(*c))
        .map(char::len_utf8)
        .sum();
    let range = start..offset + length;
    (!range.is_empty()).then_some(range)
}
