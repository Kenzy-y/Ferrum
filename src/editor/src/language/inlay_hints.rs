//! Inlay hints: text the language service puts inside lines (types of
//! bindings, names of parameters). Only the visible rows and a screen
//! above and below are asked for, after `[editor]
//! language_decorations_delay_ms` once the view settles. Hints follow
//! edits until new ones arrive; a hint inside replaced text goes.

use std::ops::Range;
use std::time::Duration;

use gpui::{Context, Task};
use language::InlayHint;
use text::{Buffer, Point, TextChanges};

use crate::editor::Editor;
use crate::highlighting::patch::map_anchor;

/// A hint placed in the text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlacedHint {
    /// Drawn before the character at this byte offset.
    pub(crate) offset: usize,
    /// The label with its padding spaces.
    pub(crate) text: String,
}

/// The hints of a document and the request for more.
#[derive(Default)]
pub(crate) struct InlayHints {
    /// Sorted by offset.
    pub(crate) hints: Vec<PlacedHint>,
    /// Rows the hints are current for; `None` asks again.
    covered: Option<Range<usize>>,
    /// Rows of the request in flight.
    pending: Option<Range<usize>>,
    task: Option<Task<()>>,
}

impl InlayHints {
    /// Moves hints through edits and asks again; a whole-text change
    /// drops them.
    pub(crate) fn follow(&mut self, changes: &TextChanges, buffer: &Buffer) {
        self.invalidate();
        if changes.replaced {
            self.hints.clear();
            return;
        }
        self.hints.retain_mut(|hint| {
            for edit in &changes.edits {
                let Some(offset) = map_anchor(hint.offset, edit) else {
                    return false;
                };
                hint.offset = offset;
            }
            hint.offset = buffer.clamp(hint.offset);
            true
        });
        self.hints.sort_by_key(|hint| hint.offset);
    }

    /// The service has new hints, or the settings changed: the next frame
    /// asks again.
    pub(crate) fn invalidate(&mut self) {
        self.covered = None;
        self.pending = None;
        self.task = None;
    }

    /// The hints in `range` (buffer offsets, both ends included), as
    /// offsets from `range.start` and text, for [`DisplayLine`].
    ///
    /// [`DisplayLine`]: crate::render::display::DisplayLine
    pub(crate) fn in_range(&self, range: Range<usize>) -> Vec<(usize, &str)> {
        let from = self.hints.partition_point(|hint| hint.offset < range.start);
        self.hints[from..]
            .iter()
            .take_while(|hint| hint.offset <= range.end)
            .map(|hint| (hint.offset - range.start, hint.text.as_str()))
            .collect()
    }
}

/// Places `hints` in `buffer`, sorted by offset.
pub(crate) fn place(hints: &[InlayHint], buffer: &Buffer) -> Vec<PlacedHint> {
    let mut placed: Vec<PlacedHint> = hints
        .iter()
        .filter(|hint| !hint.label.is_empty())
        .map(|hint| PlacedHint {
            offset: buffer.point_to_offset(hint.position),
            text: format!(
                "{}{}{}",
                if hint.padding_left { " " } else { "" },
                hint.label.replace(['\n', '\r', '\t'], " "),
                if hint.padding_right { " " } else { "" },
            ),
        })
        .collect();
    placed.sort_by_key(|hint| hint.offset);
    placed
}

/// The rows to ask for: `visible` and as many rows above and below.
fn wanted_rows(visible: &Range<usize>, line_count: usize) -> Range<usize> {
    let margin = visible.len();
    visible.start.saturating_sub(margin)..(visible.end + margin).min(line_count)
}

impl Editor {
    /// Hints of `range` to draw (see [`InlayHints::in_range`]); none when
    /// they are off.
    pub(crate) fn inlay_hints_in(&self, range: Range<usize>) -> Vec<(usize, &str)> {
        match &self.language {
            Some(language) if self.settings.inlay_hints => language.inlay_hints.in_range(range),
            _ => Vec::new(),
        }
    }

    /// After a frame that showed `visible` rows: asks for the hints there
    /// unless they are current or already asked for.
    pub(crate) fn request_inlay_hints(&mut self, visible: Range<usize>, cx: &mut Context<Self>) {
        let enabled = self.settings.inlay_hints;
        let delay = Duration::from_millis(self.settings.language_decorations_delay_ms);
        let buffer = self.document.buffer();
        let Some(language) = &mut self.language else {
            return;
        };
        if !enabled || !language.capabilities.inlay_hints || visible.is_empty() {
            return;
        }
        let state = &mut language.inlay_hints;
        let current = |rows: &Option<Range<usize>>| {
            rows.as_ref()
                .is_some_and(|rows| rows.start <= visible.start && visible.end <= rows.end)
        };
        if current(&state.covered) || current(&state.pending) {
            return;
        }
        let rows = wanted_rows(&visible, buffer.line_count());
        let end = buffer.offset_to_point(buffer.line_range(rows.end - 1).end);
        let range = Point::new(rows.start, 0)..end;
        let version = language.version;
        let service = language.service.clone();
        let path = language.path.clone();
        state.pending = Some(rows.clone());
        state.task = Some(cx.spawn(async move |editor, cx| {
            cx.background_executor().timer(delay).await;
            let answer = service.inlay_hints(&path, range).await;
            let _ = editor.update(cx, |editor, cx| {
                let buffer = editor.document.buffer();
                let Some(language) = &mut editor.language else {
                    return;
                };
                // A failed request is not repeated for these rows until
                // an edit, a refresh or a status change, so a failing
                // server costs nothing while idle.
                let (Ok(hints), true) = (answer, language.version == version) else {
                    return;
                };
                let state = &mut language.inlay_hints;
                state.pending = None;
                state.hints = place(&hints, buffer);
                state.covered = Some(rows);
                cx.notify();
            });
        }));
    }
}

#[cfg(test)]
mod tests {
    use language::InlayHintKind;

    use super::*;

    fn hint(row: usize, column: usize, label: &str) -> InlayHint {
        InlayHint {
            position: Point::new(row, column),
            label: label.to_owned(),
            kind: InlayHintKind::Type,
            padding_left: false,
            padding_right: false,
        }
    }

    #[test]
    fn hints_are_placed_with_padding_and_found_by_row() {
        let buffer = Buffer::from_text("let x = 1;\nf(a);\n");
        let mut padded = hint(1, 2, "value:");
        padded.padding_right = true;
        let hints = InlayHints {
            hints: place(&[padded, hint(0, 5, ": i32")], &buffer),
            ..InlayHints::default()
        };
        assert_eq!(hints.in_range(0..10), vec![(5, ": i32")]);
        assert_eq!(hints.in_range(11..16), vec![(2, "value: ")]);
    }

    #[test]
    fn hints_follow_edits_and_go_with_replaced_text() {
        let mut document = text::Document::from_text(
            "let x = 1;
let y = 2;
",
        );
        let mut hints = InlayHints {
            hints: place(
                &[hint(0, 5, ": i32"), hint(1, 5, ": u8")],
                document.buffer(),
            ),
            ..InlayHints::default()
        };
        document.track_changes();
        // "y " (offsets 15..17, around the second hint) replaced, then
        // "let " typed at the start.
        document.replace(15..17, "z ");
        document.replace(0..0, "let ");
        let changes = document.take_changes();
        hints.follow(&changes, document.buffer());
        let offsets: Vec<usize> = hints.hints.iter().map(|hint| hint.offset).collect();
        assert_eq!(offsets, vec![9], "the first moved, the second was inside");
    }

    #[test]
    fn a_screen_above_and_below_is_asked_for() {
        assert_eq!(wanted_rows(&(100..140), 1000), 60..180);
        assert_eq!(wanted_rows(&(0..40), 50), 0..50);
    }
}
