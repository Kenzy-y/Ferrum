//! Operations on explicit ranges: replacing text the platform names (input
//! methods), selecting words and lines (mouse), copying.

use std::ops::Range;

use crate::boundaries::word_range;
use crate::{Buffer, EditKind, Selection};

use super::{Document, Placement};

/// Text for the clipboard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopiedText {
    /// The text.
    pub text: String,
    /// Whole lines were copied because nothing was selected; pasting such
    /// text at a cursor inserts it above the cursor's line.
    pub whole_lines: bool,
}

impl Document {
    /// Replaces `range` with `text` as one edit and puts a single cursor
    /// after it. For input methods, which name the range themselves.
    pub fn replace(&mut self, range: Range<usize>, text: &str) -> Range<usize> {
        let start = self.buffer.clamp(range.start);
        let end = self.buffer.clamp(range.end.max(start));
        self.apply(
            vec![(start..end, text.to_owned())],
            EditKind::Other,
            None,
            &Placement::PerEdit(&|edit| Selection::cursor(edit.new_range.end)),
        );
        start..start + text.len()
    }

    /// Replaces the whole text with `buffer` (the file changed on disk):
    /// selections are kept where they fit, history starts over. `buffer`
    /// is built in the background; this is constant time.
    pub fn reload(&mut self, buffer: Buffer) {
        self.buffer = buffer;
        self.selections = super::clamp_selections(&self.selections, &self.buffer);
        self.history.clear();
        self.changes.replaced();
        self.version += 1;
    }

    /// Replaces the whole text with `buffer` as one undoable edit (the file
    /// was loaded again on purpose): undo brings the previous text back.
    /// Selections are kept where they fit.
    pub fn replace_all(&mut self, buffer: &Buffer) {
        let before = self.selections.clone();
        let text = buffer.rope().to_string();
        self.apply(
            vec![(0..self.buffer.len(), text)],
            EditKind::Other,
            None,
            &Placement::Keep,
        );
        self.buffer.set_bom(buffer.has_bom());
        self.selections = super::clamp_selections(&before, &self.buffer);
    }

    /// Replaces the primary selection (and keeps the others).
    pub fn set_primary_selection(&mut self, selection: Selection) {
        self.history.break_group();
        let mut selections = self.selections.clone();
        selections.replace_primary(Selection {
            anchor: self.buffer.clamp(selection.anchor),
            head: self.buffer.clamp(selection.head),
            goal_column: selection.goal_column,
        });
        self.selections = selections;
    }

    /// The word (or run of punctuation or whitespace) around `offset`.
    pub fn word_at(&self, offset: usize) -> Range<usize> {
        word_range(self.buffer.rope(), self.buffer.clamp(offset))
    }

    /// Line `row` with its line break.
    pub fn line_with_break(&self, row: usize) -> Range<usize> {
        let row = row.min(self.buffer.line_count() - 1);
        self.buffer.line_range(row).start..self.buffer.line_break_range(row).end
    }

    /// What copy puts on the clipboard: the selected texts joined by line
    /// breaks, or the lines of the cursors when nothing is selected.
    pub fn copy_text(&self) -> CopiedText {
        if self
            .selections
            .iter()
            .any(|selection| !selection.is_empty())
        {
            return CopiedText {
                text: self.selected_text(),
                whole_lines: false,
            };
        }
        let mut rows: Vec<usize> = self
            .selections
            .iter()
            .map(|selection| self.buffer.offset_to_point(selection.head).row)
            .collect();
        rows.dedup();
        let mut text = String::new();
        for row in rows {
            text.push_str(&self.buffer.slice(self.buffer.line_range(row)));
            text.push('\n');
        }
        CopiedText {
            text,
            whole_lines: true,
        }
    }

    /// Cut: copies like [`Self::copy_text`] and deletes what was copied
    /// (whole lines when nothing is selected).
    pub fn cut(&mut self) -> CopiedText {
        let copied = self.copy_text();
        if copied.whole_lines {
            let replacements = self
                .selections
                .iter()
                .map(|selection| {
                    let row = self.buffer.offset_to_point(selection.head).row;
                    (self.line_with_break(row), String::new())
                })
                .collect();
            self.apply(
                replacements,
                EditKind::Other,
                None,
                &Placement::PerEdit(&|edit| Selection::cursor(edit.new_range.start)),
            );
        } else {
            self.delete_selections();
        }
        copied
    }

    /// Pastes whole lines copied without a selection: above the line of
    /// every cursor, which stays where it was in its line.
    pub fn paste_lines(&mut self, text: &str) {
        let text = self.buffer.with_line_ending(text);
        let mut starts: Vec<usize> = self
            .selections
            .iter()
            .map(|selection| {
                let row = self.buffer.offset_to_point(selection.head).row;
                self.buffer.line_range(row).start
            })
            .collect();
        starts.dedup();
        let replacements = starts
            .into_iter()
            .map(|start| (start..start, text.clone()))
            .collect();
        self.apply(replacements, EditKind::Other, None, &Placement::Shift);
    }
}
