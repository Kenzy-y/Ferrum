//! Hover: what the service knows about the symbol under the pointer (after
//! `[editor] hover_delay_ms`) or at the cursor (`editor.showHover`), with
//! the diagnostics there. A popup opened by the pointer hides once the
//! pointer has left both its word and the popup for the same delay.

use std::ops::Range;
use std::time::Duration;

use gpui::{
    AnyElement, Context, InteractiveElement as _, ParentElement as _, Pixels, Point,
    StatefulInteractiveElement as _, Styled as _, Task, div, px,
};
use language::Severity;
use theme::Theme;

use crate::editor::Editor;
use crate::language::markdown::{self, Block};
use crate::language::popup;

/// Hover state of an editor with a language service.
#[derive(Default)]
pub(crate) struct HoverState {
    pub(crate) popup: Option<HoverPopup>,
    /// Text the pointer rests on while the delay runs.
    target: Option<Range<usize>>,
    /// The delay or the request in flight; dropping it cancels.
    task: Option<Task<()>>,
    /// Hides a pointer popup; dropping it keeps the popup.
    hide: Option<Task<()>>,
    /// The pointer is over the popup.
    popup_hovered: bool,
}

/// An open hover.
pub(crate) struct HoverPopup {
    /// The text it is about.
    pub(crate) range: Range<usize>,
    /// Problems there, most severe first.
    diagnostics: Vec<(Severity, String)>,
    blocks: Vec<Block>,
    /// Opened by the pointer: moving away hides it.
    by_pointer: bool,
}

impl Editor {
    /// The pointer moved over the text (`None`: away from it).
    pub(crate) fn hover_pointer(
        &mut self,
        position: Option<Point<Pixels>>,
        cx: &mut Context<Self>,
    ) {
        let offset = position.and_then(|position| self.text_offset_at(position));
        let target = offset.and_then(|offset| self.hover_range(offset));
        let delay = Duration::from_millis(self.settings.hover_delay_ms);
        let Some(language) = &mut self.language else {
            return;
        };
        let hover = &mut language.hover;
        if let Some(open) = &hover.popup
            && open.by_pointer
        {
            let inside = offset
                .is_some_and(|offset| open.range.contains(&offset) || open.range.end == offset);
            if inside {
                hover.hide = None;
                return;
            }
            if hover.hide.is_none() {
                hover.hide = Some(hide_after(delay, cx));
            }
        }
        if hover.target == target {
            return;
        }
        hover.target.clone_from(&target);
        hover.task = None;
        let (Some(offset), Some(_)) = (offset, target) else {
            return;
        };
        hover.task = Some(cx.spawn(async move |editor, cx| {
            cx.background_executor().timer(delay).await;
            let _ = editor.update(cx, |editor, cx| editor.request_hover(offset, true, cx));
        }));
    }

    /// The pointer entered or left the popup.
    pub(crate) fn hover_popup_hovered(&mut self, hovered: bool, cx: &mut Context<Self>) {
        let delay = Duration::from_millis(self.settings.hover_delay_ms);
        let Some(language) = &mut self.language else {
            return;
        };
        let hover = &mut language.hover;
        hover.popup_hovered = hovered;
        if hovered {
            hover.hide = None;
        } else if hover.popup.as_ref().is_some_and(|open| open.by_pointer) {
            hover.hide = Some(hide_after(delay, cx));
        }
    }

    /// `editor.showHover`: the hover of the primary cursor.
    pub(crate) fn show_hover_at_cursor(&mut self, cx: &mut Context<Self>) {
        let head = self.document.selections().primary().head;
        self.request_hover(head, false, cx);
    }

    /// Closes the hover; `true` if it was open.
    pub(crate) fn dismiss_hover(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(language) = &mut self.language else {
            return false;
        };
        let open = language.hover.popup.is_some();
        language.hover = HoverState::default();
        if open {
            cx.notify();
        }
        open
    }

    /// Asks for the hover at `offset` and shows it with the diagnostics
    /// there once the answer arrives.
    fn request_hover(&mut self, offset: usize, by_pointer: bool, cx: &mut Context<Self>) {
        let Some(range) = self.hover_range(offset) else {
            return;
        };
        let diagnostics: Vec<(Severity, String)> = self
            .diagnostics_in(offset..offset + 1)
            .into_iter()
            .map(|diagnostic| (diagnostic.severity, diagnostic_text(diagnostic)))
            .collect();
        let grammar = self.loaded_grammar();
        let language_id = self.language_id().map(str::to_owned);
        let version = self.document.version();
        let point = self.document.buffer().offset_to_point(offset);
        let Some(language) = &mut self.language else {
            return;
        };
        let request = language
            .capabilities
            .hover
            .then(|| language.service.hover(&language.path, point));
        language.hover.task = Some(cx.spawn(async move |editor, cx| {
            let answer = match request {
                Some(request) => request.await.ok().flatten(),
                None => None,
            };
            let server_range = answer.as_ref().and_then(|hover| hover.range.clone());
            let blocks = match answer {
                Some(hover) => {
                    cx.background_executor()
                        .spawn(async move {
                            let mut blocks = markdown::parse(&hover.contents);
                            if let (Some(grammar), Some(id)) = (grammar, language_id) {
                                markdown::highlight_code(&mut blocks, &grammar, &id);
                            }
                            blocks
                        })
                        .await
                }
                None => Vec::new(),
            };
            let _ = editor.update(cx, |editor, cx| {
                if editor.document.version() != version
                    || (blocks.is_empty() && diagnostics.is_empty())
                {
                    return;
                }
                let buffer = editor.document.buffer();
                let range = server_range.map_or(range, |range| {
                    buffer.point_to_offset(range.start)..buffer.point_to_offset(range.end)
                });
                if let Some(language) = &mut editor.language {
                    language.hover.hide = None;
                    language.hover.popup = Some(HoverPopup {
                        range,
                        diagnostics,
                        blocks,
                        by_pointer,
                    });
                    cx.notify();
                }
            });
        }));
    }

    /// The text a hover at `offset` is about: a diagnostic there or the
    /// identifier.
    fn hover_range(&self, offset: usize) -> Option<Range<usize>> {
        let language = self.language.as_ref()?;
        let word = super::identifier_at(self.document.buffer(), offset);
        let diagnostic = language
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.range.contains(&offset))
            .map(|diagnostic| diagnostic.range.clone());
        word.or(diagnostic)
    }

    /// The character under window position `position`, if it is over text
    /// (not past the end of its line or below the last line).
    fn text_offset_at(&self, position: Point<Pixels>) -> Option<usize> {
        let layout = self.layout.as_ref()?;
        let buffer = self.document.buffer();
        let row = layout.row_at(buffer, position.y.as_f32());
        let top = layout.text_top + crate::render::viewport::to_f32(row) * layout.line_height;
        if position.y.as_f32() > top + layout.line_height {
            return None;
        }
        let offset = layout.offset_at(buffer, position);
        let line_end = buffer.line_range(row).end;
        let end_x = layout.text_left + layout.x_of(buffer, line_end);
        (position.x.as_f32() < end_x && position.x.as_f32() >= layout.text_left).then_some(offset)
    }

    /// The hover popup, if one is open and its text is visible.
    pub(crate) fn render_hover(&self, theme: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        let language = self.language.as_ref()?;
        let open = language.hover.popup.as_ref()?;
        let sizes = &theme.metrics.size;
        let (anchor, side) = self.popup_anchor(open.range.start, sizes.editor_popup_gap)?;
        let colors = self.syntax_colors(theme);
        let mut content = popup::scrolling("hover", sizes.hover_max_height);
        for (severity, message) in &open.diagnostics {
            let color = match severity {
                Severity::Error => theme.colors.diagnostic_error,
                Severity::Warning => theme.colors.diagnostic_warning,
                Severity::Information => theme.colors.diagnostic_info,
                Severity::Hint => theme.colors.diagnostic_hint,
            };
            content = content.child(
                div()
                    .flex()
                    .gap(px(theme.metrics.space.sm))
                    .px(px(theme.metrics.space.md))
                    .py(px(theme.metrics.space.xs))
                    .child(div().w(px(2.0)).flex_none().bg(color))
                    .child(div().child(message.clone())),
            );
        }
        if !open.diagnostics.is_empty() && !open.blocks.is_empty() {
            content = content.child(
                div()
                    .my(px(theme.metrics.space.xs))
                    .h(px(1.0))
                    .bg(theme.colors.border_divider),
            );
        }
        content = content.children(popup::blocks(&open.blocks, &colors, theme));
        let body = popup::container(theme)
            .id("hover-popup")
            .max_w(px(sizes.hover_max_width))
            .py(px(theme.metrics.space.xs))
            .on_hover(cx.listener(|editor, hovered: &bool, _, cx| {
                editor.hover_popup_hovered(*hovered, cx);
            }))
            .child(content);
        Some(popup::place(body, anchor, side))
    }
}

/// A timer that closes a pointer hover unless the pointer comes back.
fn hide_after(delay: Duration, cx: &mut Context<Editor>) -> Task<()> {
    cx.spawn(async move |editor, cx| {
        cx.background_executor().timer(delay).await;
        let _ = editor.update(cx, |editor, cx| {
            let hovered = editor
                .language
                .as_ref()
                .is_some_and(|language| language.hover.popup_hovered);
            if !hovered {
                editor.dismiss_hover(cx);
            }
        });
    })
}

/// A diagnostic as shown in the hover: the message with its source and
/// code (`rustc(E0308)`).
fn diagnostic_text(diagnostic: &super::diagnostics::PlacedDiagnostic) -> String {
    match (&diagnostic.source, &diagnostic.code) {
        (Some(source), Some(code)) => format!("{} {source}({code})", diagnostic.message),
        (Some(source), None) => format!("{} {source}", diagnostic.message),
        (None, Some(code)) => format!("{} ({code})", diagnostic.message),
        (None, None) => diagnostic.message.clone(),
    }
}
