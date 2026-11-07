//! The completion list: asked for with `editor.triggerSuggest`, after a
//! trigger character, or when a word starts (`[editor] suggest_on_type`).
//! Typing filters it (fuzzy, like the palette); an incomplete list is
//! asked for again. While it is open, Up, Down, Page Up and Page Down
//! choose, Enter and Tab accept, Escape closes.

use std::ops::Range;

use gpui::{
    AnyElement, Context, FontWeight, HighlightStyle, InteractiveElement as _, MouseButton,
    ParentElement as _, ScrollEvent, SharedString, Styled as _, StyledText, div, px,
};
use language::{Completion, CompletionKind, CompletionTrigger};
use text::Selection;
use theme::Theme;

use crate::editor::Editor;
use crate::language::popup;

/// An open completion list.
pub(crate) struct CompletionMenu {
    items: Vec<Completion>,
    /// Typing more may bring other items.
    incomplete: bool,
    /// Start of the word being completed.
    word_start: usize,
    /// Items matching the typed word, best first: index and matched
    /// character positions of the filter text.
    matches: Vec<(usize, Vec<usize>)>,
    selected: usize,
    /// First row shown.
    first_row: usize,
}

/// How the selection moves in the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    Up,
    Down,
    PageUp,
    PageDown,
}

impl CompletionMenu {
    /// Keeps the items matching `word`, best first.
    fn filter(&mut self, word: &str) {
        let mut matches: Vec<(i32, usize, Vec<usize>)> = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                let text = item.filter_text.as_deref().unwrap_or(&item.label);
                if word.is_empty() {
                    return Some((0, index, Vec::new()));
                }
                let found = commands::fuzzy_match(word, text)?;
                Some((found.score, index, found.positions))
            })
            .collect();
        let items = &self.items;
        let sort_key = |index: usize| {
            let item = &items[index];
            item.sort_text.clone().unwrap_or_else(|| item.label.clone())
        };
        matches.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| sort_key(a.1).cmp(&sort_key(b.1)))
        });
        self.matches = matches
            .into_iter()
            .map(|(_, index, positions)| (index, positions))
            .collect();
        self.selected = 0;
        self.first_row = 0;
    }

    /// The selected item and its index in `items`.
    pub(crate) fn selected_item(&self) -> Option<(usize, &Completion)> {
        let (index, _) = self.matches.get(self.selected)?;
        Some((*index, &self.items[*index]))
    }

    /// Whether item `index` is still the one labelled `label`.
    pub(crate) fn has_item(&self, index: usize, label: &str) -> bool {
        self.items
            .get(index)
            .is_some_and(|item| item.label == label)
    }

    /// Puts the resolved form of item `index` in its place.
    pub(crate) fn replace_item(&mut self, index: usize, item: Completion) {
        if let Some(slot) = self.items.get_mut(index) {
            *slot = item;
        }
    }

    fn step(&mut self, step: Step, rows: usize) {
        let count = self.matches.len();
        if count == 0 {
            return;
        }
        let page = rows.max(1);
        self.selected = match step {
            Step::Up => (self.selected + count - 1) % count,
            Step::Down => (self.selected + 1) % count,
            Step::PageUp => self.selected.saturating_sub(page),
            Step::PageDown => (self.selected + page).min(count - 1),
        };
        self.reveal(rows);
    }

    /// Scrolls so the selected row shows.
    fn reveal(&mut self, rows: usize) {
        if self.selected < self.first_row {
            self.first_row = self.selected;
        } else if self.selected >= self.first_row + rows {
            self.first_row = self.selected + 1 - rows;
        }
    }
}

impl Editor {
    /// Whether the completion list is open.
    pub(crate) fn completion_open(&self) -> bool {
        self.language
            .as_ref()
            .is_some_and(|language| language.completion.is_some())
    }

    /// Asks the service for completions at the primary cursor.
    pub(crate) fn trigger_completion(
        &mut self,
        trigger: CompletionTrigger,
        cx: &mut Context<Self>,
    ) {
        if self.document.selections().len() != 1 {
            return;
        }
        let head = self.document.selections().primary().head;
        let word_start = identifier_start(self.document.buffer(), head);
        let point = self.document.buffer().offset_to_point(head);
        let Some(language) = &mut self.language else {
            return;
        };
        if language.capabilities.completion.is_none() {
            return;
        }
        let request = language.service.completions(&language.path, point, trigger);
        language.completion_task = Some(cx.spawn(async move |editor, cx| {
            let Ok(list) = request.await else {
                return;
            };
            let _ = editor.update(cx, |editor, cx| {
                let mut menu = CompletionMenu {
                    items: list.items,
                    incomplete: list.incomplete,
                    word_start,
                    matches: Vec::new(),
                    selected: 0,
                    first_row: 0,
                };
                let Some(word) = editor.completion_word(word_start) else {
                    return;
                };
                menu.filter(&word);
                if let Some(language) = &mut editor.language {
                    language.completion = (!menu.matches.is_empty()).then_some(menu);
                    cx.notify();
                }
                editor.sync_completion_docs(cx);
            });
        }));
    }

    /// The text typed since `word_start` while the primary cursor is still
    /// in that word; `None` when the cursor left it.
    fn completion_word(&self, word_start: usize) -> Option<String> {
        if self.document.selections().len() != 1 {
            return None;
        }
        let head = self.document.selections().primary().head;
        if head < word_start {
            return None;
        }
        let word = self.document.buffer().slice(word_start..head);
        word.chars().all(is_identifier_char).then_some(word)
    }

    /// Follows an edit: the open list is filtered (or asked for again when
    /// incomplete); a typed trigger character or a new word opens it.
    pub(crate) fn update_completion_after_edit(
        &mut self,
        changes: &text::TextChanges,
        cx: &mut Context<Self>,
    ) {
        let Some(language) = &self.language else {
            return;
        };
        let Some(capabilities) = language.capabilities.completion.clone() else {
            return;
        };
        if let Some(menu) = &language.completion {
            let word_start = menu.word_start;
            let incomplete = menu.incomplete;
            match self.completion_word(word_start) {
                Some(_) if incomplete => {
                    self.trigger_completion(CompletionTrigger::Incomplete, cx);
                    return;
                }
                Some(word) => {
                    if let Some(menu) = self.language.as_mut().and_then(|l| l.completion.as_mut()) {
                        menu.filter(&word);
                        if menu.matches.is_empty() {
                            self.close_completion(cx);
                        }
                    }
                    self.sync_completion_docs(cx);
                    cx.notify();
                    return;
                }
                None => self.close_completion(cx),
            }
        }
        // Only typing opens the list, not deleting or pasting.
        let Some(typed) = typed_char(changes) else {
            return;
        };
        let head = self.document.selections().primary().head;
        if capabilities
            .trigger_characters
            .iter()
            .any(|trigger| trigger.chars().eq(std::iter::once(typed)))
        {
            self.trigger_completion(CompletionTrigger::Character(typed.to_string()), cx);
        } else if self.settings.suggest_on_type
            && is_identifier_char(typed)
            && identifier_start(self.document.buffer(), head) + typed.len_utf8() == head
        {
            self.trigger_completion(CompletionTrigger::Invoked, cx);
        }
    }

    /// Closes the list.
    pub(crate) fn close_completion(&mut self, cx: &mut Context<Self>) {
        if let Some(language) = &mut self.language {
            language.completion_task = None;
            language.completion_docs = super::completion_docs::CompletionDocs::default();
            if language.completion.take().is_some() {
                cx.notify();
            }
        }
    }

    /// Moves the selection in the open list.
    pub(crate) fn step_completion(
        &mut self,
        step: Step,
        theme_rows: usize,
        cx: &mut Context<Self>,
    ) {
        if let Some(menu) = self.language.as_mut().and_then(|l| l.completion.as_mut()) {
            menu.step(step, theme_rows);
            cx.notify();
        }
        self.sync_completion_docs(cx);
    }

    /// Inserts the selected item (or item `pick`) and closes the list.
    pub(crate) fn accept_completion(&mut self, pick: Option<usize>, cx: &mut Context<Self>) {
        let Some(language) = &mut self.language else {
            return;
        };
        let Some(menu) = language.completion.take() else {
            return;
        };
        language.completion_task = None;
        let Some((index, _)) = menu.matches.get(pick.unwrap_or(menu.selected)) else {
            return;
        };
        let item = menu.items[*index].clone();
        let resolve = language
            .capabilities
            .completion
            .as_ref()
            .is_some_and(|completion| completion.resolve.resolve)
            && item.resolve.is_some()
            && item.additional_edits.is_empty();
        let service = language.service.clone();
        let head = self.document.selections().primary().head;
        let buffer = self.document.buffer();
        let range = match &item.edit.range {
            Some(range) => {
                buffer.point_to_offset(range.start)..buffer.point_to_offset(range.end).max(head)
            }
            None => menu.word_start..head,
        };
        let additional = item.additional_edits.clone();
        let mut base = range.start;
        self.change(cx, |document| {
            let inserted = document.replace(range, &item.edit.new_text);
            base = apply_additional(document, &additional, inserted.start);
        });
        if !item.edit.tab_stops.is_empty() {
            let length = item.edit.new_text.len();
            let stops = item.edit.tab_stops.clone();
            self.start_snippet(base, length, &stops, cx);
        }
        if resolve {
            // The service fills in edits elsewhere (an import) on request.
            let version = self.document.version();
            let request = service.resolve_completion(item);
            cx.spawn(async move |editor, cx| {
                let Ok(resolved) = request.await else {
                    return;
                };
                let _ = editor.update(cx, |editor, cx| {
                    if editor.document.version() == version && !resolved.additional_edits.is_empty()
                    {
                        editor.end_snippet();
                        editor.change(cx, |document| {
                            let head = document.selections().primary().head;
                            apply_additional(document, &resolved.additional_edits, head);
                        });
                    }
                });
            })
            .detach();
        }
    }

    /// The open completion list, below or above the word.
    pub(crate) fn render_completion(
        &self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let language = self.language.as_ref()?;
        let menu = language.completion.as_ref()?;
        let sizes = &theme.metrics.size;
        let rows = completion_rows(theme);
        let (anchor, side) = self.popup_anchor(menu.word_start, sizes.editor_popup_gap)?;
        let metrics = &theme.metrics;
        let row_height = sizes.outline_row_height;
        let mono = SharedString::from(theme.fonts.mono.clone());
        let list = menu
            .matches
            .iter()
            .enumerate()
            .skip(menu.first_row)
            .take(rows)
            .map(|(position, (index, matched))| {
                let item = &menu.items[*index];
                let selected = position == menu.selected;
                let label = item.label.clone();
                let filter = item.filter_text.as_deref().unwrap_or(&item.label);
                let highlights = label_highlights(&label, filter, matched, theme);
                div()
                    .id(position)
                    .flex()
                    .items_center()
                    .gap(px(metrics.space.sm))
                    .h(px(row_height))
                    .px(px(metrics.space.sm))
                    .rounded(px(metrics.radius.sm))
                    .when_selected(selected, theme)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |editor, _, _, cx| {
                            cx.stop_propagation();
                            editor.accept_completion(Some(position), cx);
                        }),
                    )
                    .child(
                        div()
                            .w(px(metrics.font_size.md))
                            .flex_none()
                            .text_color(theme.colors.text_subtle)
                            .child(kind_glyph(item.kind)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .font_family(mono.clone())
                            .text_color(theme.colors.text_primary)
                            .child(StyledText::new(label).with_highlights(highlights)),
                    )
                    .children(item.detail.clone().map(|detail| {
                        div()
                            .flex_none()
                            .max_w(px(sizes.completion_width / 2.0))
                            .overflow_hidden()
                            .text_color(theme.colors.text_muted)
                            .text_size(px(metrics.font_size.sm))
                            .child(detail)
                    }))
            });
        let body = popup::container(theme)
            .id("completion-list")
            .w(px(sizes.completion_width))
            .p(px(metrics.space.xs))
            .on_scroll(cx.listener(move |editor, event: &ScrollEvent, window, cx| {
                cx.stop_propagation();
                let delta = event.delta.pixel_delta(window.line_height()).y.as_f32();
                let step = if delta < 0.0 { Step::Down } else { Step::Up };
                editor.step_completion(step, rows, cx);
            }))
            .children(list);
        // The documentation of the selected item, beside the list.
        let docs = self.render_completion_docs(theme);
        let row = div()
            .flex()
            .gap(px(sizes.editor_popup_gap))
            .child(body)
            .children(docs);
        let row = match side {
            popup::Side::Below => row.items_start(),
            popup::Side::Above => row.items_end(),
        };
        Some(popup::place(row, anchor, side))
    }
}

/// Rows the list shows at once.
pub(crate) fn completion_rows(theme: &Theme) -> usize {
    // A small whole number stored as a float token.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "completion.rows is a small positive count"
    )]
    let rows = theme.metrics.size.completion_rows.max(1.0) as usize;
    rows
}

trait SelectedRow {
    fn when_selected(self, selected: bool, theme: &Theme) -> Self;
}

impl SelectedRow for gpui::Stateful<gpui::Div> {
    fn when_selected(self, selected: bool, theme: &Theme) -> Self {
        if selected {
            self.bg(theme.colors.bg_list_selected)
        } else {
            self.hover(|style| style.bg(theme.colors.bg_list_hover))
        }
    }
}

/// Highlights of the matched characters in `label`; positions refer to
/// `filter`, which is usually the label itself.
fn label_highlights(
    label: &str,
    filter: &str,
    matched: &[usize],
    theme: &Theme,
) -> Vec<(Range<usize>, HighlightStyle)> {
    if label != filter {
        return Vec::new();
    }
    let style = HighlightStyle {
        color: Some(theme.colors.accent.into()),
        font_weight: Some(FontWeight::MEDIUM),
        ..HighlightStyle::default()
    };
    matched
        .iter()
        .filter_map(|&start| {
            let length = label.get(start..)?.chars().next()?.len_utf8();
            Some((start..start + length, style))
        })
        .collect()
}

/// A one-character mark of what an item inserts.
fn kind_glyph(kind: Option<CompletionKind>) -> &'static str {
    match kind {
        Some(CompletionKind::Function | CompletionKind::Method) => "ƒ",
        Some(CompletionKind::Field) => "▪",
        Some(CompletionKind::Variable) => "x",
        Some(CompletionKind::Constant) => "c",
        Some(CompletionKind::Struct) => "S",
        Some(CompletionKind::Enum) => "E",
        Some(CompletionKind::EnumMember) => "e",
        Some(CompletionKind::Interface) => "T",
        Some(CompletionKind::Module) => "M",
        Some(CompletionKind::Keyword) => "k",
        Some(CompletionKind::Snippet) => "◇",
        Some(CompletionKind::TypeParameter) => "τ",
        Some(CompletionKind::Text) | None => "·",
    }
}

/// Applies edits made elsewhere (an import); the cursor keeps its place in
/// the text.
/// Returns where `anchor` (the start of the main edit) moved.
fn apply_additional(
    document: &mut text::Document,
    edits: &[language::TextReplacement],
    anchor: usize,
) -> usize {
    if edits.is_empty() {
        return anchor;
    }
    let mut ranges: Vec<(Range<usize>, &str)> = edits
        .iter()
        .map(|edit| {
            let buffer = document.buffer();
            (
                buffer.point_to_offset(edit.range.start)..buffer.point_to_offset(edit.range.end),
                edit.new_text.as_str(),
            )
        })
        .collect();
    // From the end, so earlier ranges stay valid.
    ranges.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
    let mut head = document.selections().primary().head;
    let mut anchor = anchor;
    for (range, text) in ranges {
        document.replace(range.clone(), text);
        if range.end <= head {
            head = head - (range.end - range.start) + text.len();
        }
        if range.end <= anchor {
            anchor = anchor - (range.end - range.start) + text.len();
        }
    }
    document.set_primary_selection(Selection::cursor(head));
    anchor
}

/// The one character the edit typed, if it typed exactly one.
fn typed_char(changes: &text::TextChanges) -> Option<char> {
    let [edit] = changes.edits.as_slice() else {
        return None;
    };
    if edit.old_end != edit.start {
        return None;
    }
    let mut chars = edit.inserted.chars();
    let typed = chars.next()?;
    chars.next().is_none().then_some(typed)
}

/// Whether `c` can be part of an identifier.
pub(crate) fn is_identifier_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Start of the identifier that ends at `offset` (`offset` if none).
pub(crate) fn identifier_start(buffer: &text::Buffer, offset: usize) -> usize {
    let line_start = buffer.line_range(buffer.offset_to_point(offset).row).start;
    let before = buffer.slice(line_start..offset);
    let length: usize = before
        .chars()
        .rev()
        .take_while(|c| is_identifier_char(*c))
        .map(char::len_utf8)
        .sum();
    offset - length
}

#[cfg(test)]
mod tests {
    use language::CompletionEdit;

    use super::*;

    fn item(label: &str) -> Completion {
        Completion {
            label: label.to_owned(),
            kind: None,
            detail: None,
            documentation: None,
            filter_text: None,
            sort_text: None,
            edit: CompletionEdit {
                range: None,
                new_text: label.to_owned(),
                tab_stops: Vec::new(),
            },
            additional_edits: Vec::new(),
            resolve: None,
        }
    }

    #[test]
    fn filters_fuzzily_and_wraps_the_selection() {
        let mut menu = CompletionMenu {
            items: vec![item("println"), item("print"), item("push_str")],
            incomplete: false,
            word_start: 0,
            matches: Vec::new(),
            selected: 0,
            first_row: 0,
        };
        menu.filter("pr");
        let labels: Vec<_> = menu
            .matches
            .iter()
            .map(|(index, _)| menu.items[*index].label.as_str())
            .collect();
        // A subsequence matches too, but word starts and runs score higher.
        assert_eq!(labels.len(), 3);
        assert_eq!(labels[2], "push_str");
        menu.step(Step::Up, 10);
        assert_eq!(menu.selected, 2, "up from the first wraps to the last");
    }

    #[test]
    fn identifiers_end_at_the_cursor() {
        let buffer = text::Buffer::from_text("let foo_bar = x.ba");
        assert_eq!(identifier_start(&buffer, 11), 4);
        assert_eq!(identifier_start(&buffer, 18), 16);
        assert_eq!(identifier_start(&buffer, 15), 14);
        assert_eq!(identifier_start(&buffer, 16), 16, "after a dot");
    }
}
