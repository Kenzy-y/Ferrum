//! Editing at every cursor: typing, pasting, new lines, deletion, and
//! applying a set of replacements as one undo step.

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation as _;

use crate::boundaries::{next_grapheme, next_word_end, previous_grapheme, previous_word_start};
use crate::history::Edit;
use crate::{Buffer, EditKind, LineEnding, Selection, Selections};

use super::{Document, Placement};

impl Document {
    /// Types `text` at every cursor, replacing selected text.
    pub fn insert(&mut self, text: &str) {
        let text = self.buffer.with_line_ending(text);
        let text = text.as_str();
        let single_grapheme = !text.contains('\n') && text.graphemes(true).count() == 1;
        let kind = if single_grapheme && self.selections.iter().all(Selection::is_empty) {
            EditKind::Typing
        } else {
            EditKind::Other
        };
        let replacements = self
            .selections
            .iter()
            .map(|selection| (selection.range(), text.to_owned()))
            .collect();
        let typed = if kind == EditKind::Typing {
            text.chars().next()
        } else {
            None
        };
        self.apply(
            replacements,
            kind,
            typed,
            &Placement::PerEdit(&|edit| Selection::cursor(edit.new_range.end)),
        );
    }

    /// Pastes `text`. With as many lines as cursors, each cursor gets its
    /// line (as copied from several selections).
    pub fn paste(&mut self, text: &str) {
        let normalized = text.replace("\r\n", "\n");
        let lines: Vec<&str> = normalized.split('\n').collect();
        let replacements = if self.selections.len() > 1 && lines.len() == self.selections.len() {
            self.selections
                .iter()
                .zip(lines)
                .map(|(selection, line)| (selection.range(), line.to_owned()))
                .collect()
        } else {
            let text = self.buffer.with_line_ending(&normalized);
            self.selections
                .iter()
                .map(|selection| (selection.range(), text.clone()))
                .collect()
        };
        self.apply(
            replacements,
            EditKind::Other,
            None,
            &Placement::PerEdit(&|edit| Selection::cursor(edit.new_range.end)),
        );
    }

    /// Breaks the line at every cursor, keeping the indentation of the line.
    pub fn newline(&mut self) {
        let replacements = self
            .selections
            .iter()
            .map(|selection| {
                let row = self.buffer.offset_to_point(selection.start()).row;
                let line = self.buffer.line(row);
                let indentation: String = line
                    .chars()
                    .take_while(|character| *character == ' ' || *character == '\t')
                    .collect();
                (
                    selection.range(),
                    format!("{}{indentation}", self.buffer.line_ending().as_str()),
                )
            })
            .collect();
        self.apply(
            replacements,
            EditKind::Other,
            None,
            &Placement::PerEdit(&|edit| Selection::cursor(edit.new_range.end)),
        );
    }

    /// Deletes the selections, or the grapheme before each cursor.
    pub fn delete_backward(&mut self) {
        self.delete_with(|buffer, head| previous_grapheme(buffer.rope(), head)..head);
    }

    /// Deletes the selections, or the grapheme after each cursor.
    pub fn delete_forward(&mut self) {
        self.delete_with(|buffer, head| head..next_grapheme(buffer.rope(), head));
    }

    /// Deletes the selections, or the word before each cursor.
    pub fn delete_word_backward(&mut self) {
        self.delete_with(|buffer, head| previous_word_start(buffer.rope(), head)..head);
    }

    /// Deletes the selections, or the word after each cursor.
    pub fn delete_word_forward(&mut self) {
        self.delete_with(|buffer, head| head..next_word_end(buffer.rope(), head));
    }

    /// The selected texts joined by line breaks (for copy).
    pub fn selected_text(&self) -> String {
        self.selections
            .iter()
            .map(|selection| self.buffer.slice(selection.range()))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Deletes the selected text (for cut).
    pub fn delete_selections(&mut self) {
        let replacements = self
            .selections
            .iter()
            .filter(|selection| !selection.is_empty())
            .map(|selection| (selection.range(), String::new()))
            .collect();
        self.apply(
            replacements,
            EditKind::Other,
            None,
            &Placement::PerEdit(&|edit| Selection::cursor(edit.new_range.start)),
        );
    }

    /// Makes every line end with `ending` (one undo step); cursors stay
    /// where they were.
    pub fn set_line_endings(&mut self, ending: LineEnding) {
        let replacements = (0..self.buffer.line_count())
            .filter_map(|row| {
                let range = self.buffer.line_break_range(row);
                let current = self.buffer.slice(range.clone());
                (!current.is_empty() && current != ending.as_str())
                    .then(|| (range, ending.as_str().to_owned()))
            })
            .collect();
        self.apply(replacements, EditKind::Other, None, &Placement::Keep);
    }

    fn delete_with(&mut self, range_for_cursor: impl Fn(&Buffer, usize) -> Range<usize>) {
        let all_cursors = self.selections.iter().all(Selection::is_empty);
        let replacements = self
            .selections
            .iter()
            .map(|selection| {
                let range = if selection.is_empty() {
                    range_for_cursor(&self.buffer, selection.head)
                } else {
                    selection.range()
                };
                (range, String::new())
            })
            .collect();
        let kind = if all_cursors {
            EditKind::Deletion
        } else {
            EditKind::Other
        };
        self.apply(
            replacements,
            kind,
            None,
            &Placement::PerEdit(&|edit| Selection::cursor(edit.new_range.start)),
        );
    }

    /// Applies `replacements` (one per selection, any order) as one step:
    /// overlapping ranges are merged, the buffer is changed from the end so
    /// earlier ranges stay valid, every edit becomes a selection through
    /// `place` and the step is recorded for undo with the selections before
    /// and after.
    pub(super) fn apply(
        &mut self,
        replacements: Vec<(Range<usize>, String)>,
        kind: EditKind,
        typed: Option<char>,
        placement: &Placement<'_>,
    ) {
        let replacements = merge_replacements(replacements);
        if replacements
            .iter()
            .all(|(range, text)| range.is_empty() && text.is_empty())
        {
            return;
        }
        let mut edits = Vec::with_capacity(replacements.len());
        let (mut added, mut removed) = (0usize, 0usize);
        for (range, text) in replacements {
            let new_start = range.start + added - removed;
            removed += range.len();
            added += text.len();
            edits.push(Edit {
                old_text: self.buffer.slice(range.clone()),
                new_range: new_start..new_start + text.len(),
                old_range: range,
                new_text: text,
            });
        }
        self.changes.replace_step(
            &mut self.buffer,
            edits
                .iter()
                .map(|edit| (edit.old_range.clone(), edit.new_text.as_str())),
        );
        let before = self.selections.clone();
        let after = match placement {
            Placement::PerEdit(place) => Selections::from_vec(edits.iter().map(place).collect()),
            Placement::Keep | Placement::Shift => {
                let mapper = OffsetMapper::new(&edits, matches!(placement, Placement::Shift));
                before.map(|selection| Selection {
                    anchor: mapper.map(selection.anchor),
                    head: mapper.map(selection.head),
                    goal_column: selection.goal_column,
                })
            }
        };
        let now = self.clock.now();
        self.history
            .record(edits, kind, before, after.clone(), now, typed);
        self.selections = after;
        self.version += 1;
    }
}

/// Moves offsets of the old text to the new text after a step of edits,
/// in logarithmic time per offset.
struct OffsetMapper<'a> {
    edits: &'a [Edit],
    /// Offsets at the start of an edit move after its new text.
    after_insertions: bool,
}

impl<'a> OffsetMapper<'a> {
    fn new(edits: &'a [Edit], after_insertions: bool) -> Self {
        Self {
            edits,
            after_insertions,
        }
    }

    fn map(&self, offset: usize) -> usize {
        // The last edit that starts before `offset` (or at it, when offsets
        // move after insertions).
        let index = self.edits.partition_point(|edit| {
            edit.old_range.start < offset
                || (self.after_insertions && edit.old_range.start == offset)
        });
        let Some(edit) = index.checked_sub(1).map(|index| &self.edits[index]) else {
            return offset;
        };
        if offset < edit.old_range.end {
            // Inside replaced text: to the end of the replacement.
            edit.new_range.end
        } else {
            edit.new_range.end + (offset - edit.old_range.end)
        }
    }
}

/// Sorts replacements and merges overlapping ones (two cursors deleting the
/// same character delete it once).
fn merge_replacements(
    mut replacements: Vec<(Range<usize>, String)>,
) -> Vec<(Range<usize>, String)> {
    replacements.sort_by_key(|(range, _)| (range.start, range.end));
    let mut merged: Vec<(Range<usize>, String)> = Vec::with_capacity(replacements.len());
    for (range, text) in replacements {
        match merged.last_mut() {
            Some((last, last_text))
                if range.start < last.end
                    || (range.start == last.end && range.is_empty() && text.is_empty()) =>
            {
                last.end = last.end.max(range.end);
                if last_text.is_empty() {
                    *last_text = text;
                }
            }
            _ => merged.push((range, text)),
        }
    }
    merged
}
