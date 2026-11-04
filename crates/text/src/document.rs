//! A buffer with selections and history: the editing operations.

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation as _;

use crate::boundaries::{next_grapheme, next_word_end, previous_grapheme, previous_word_start};
use crate::history::Edit;
use crate::{Buffer, EditKind, History, Selection, Selections};

/// Where a cursor moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Movement {
    /// One grapheme back.
    Left,
    /// One grapheme forward.
    Right,
    /// To the start of the previous word.
    WordLeft,
    /// To the end of the next word.
    WordRight,
    /// One line up, keeping the column.
    Up,
    /// One line down, keeping the column.
    Down,
    /// Several lines up (a page).
    PageUp(usize),
    /// Several lines down (a page).
    PageDown(usize),
    /// To the first non-blank character of the line, or to the column 0 if
    /// already there.
    LineStart,
    /// To the end of the line.
    LineEnd,
    /// To the start of the document.
    DocumentStart,
    /// To the end of the document.
    DocumentEnd,
}

/// Text being edited.
#[derive(Debug, Clone, Default)]
pub struct Document {
    buffer: Buffer,
    selections: Selections,
    history: History,
    /// Increases with every change of the text; for caches.
    version: u64,
}

impl Document {
    /// A document with `buffer` and a cursor at the start.
    pub fn new(buffer: Buffer) -> Self {
        Self {
            buffer,
            selections: Selections::default(),
            history: History::default(),
            version: 0,
        }
    }

    /// A document with `text` (see [`Buffer::from_text`]).
    pub fn from_text(text: &str) -> Self {
        Self::new(Buffer::from_text(text))
    }

    /// The text.
    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    /// The selections.
    pub fn selections(&self) -> &Selections {
        &self.selections
    }

    /// The undo history.
    pub fn history(&self) -> &History {
        &self.history
    }

    /// Changes with every edit of the text.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Replaces the selections (mouse clicks, select all). Ends the current
    /// undo group.
    pub fn set_selections(&mut self, selections: &Selections) {
        self.history.break_group();
        self.selections = clamp_selections(selections, &self.buffer);
    }

    /// Adds a selection and makes it primary (Alt+click).
    pub fn add_selection(&mut self, selection: Selection) {
        self.history.break_group();
        let mut selections = self.selections.clone();
        selections.add(selection);
        self.selections = clamp_selections(&selections, &self.buffer);
    }

    /// Selects the whole text.
    pub fn select_all(&mut self) {
        self.set_selections(&Selections::single(Selection::new(0, self.buffer.len())));
    }

    /// Moves every cursor; with `extend` the selections grow instead.
    pub fn move_cursors(&mut self, movement: Movement, extend: bool) {
        self.history.break_group();
        let buffer = &self.buffer;
        self.selections = self.selections.map(|selection| {
            // Without extend, a selection collapses towards the movement.
            if !extend && !selection.is_empty() {
                match movement {
                    Movement::Left => return Selection::cursor(selection.start()),
                    Movement::Right => return Selection::cursor(selection.end()),
                    _ => {}
                }
            }
            let (head, goal_column) = moved(buffer, selection, movement);
            let anchor = if extend { selection.anchor } else { head };
            Selection {
                anchor,
                head,
                goal_column,
            }
        });
    }

    /// Adds a cursor on the line above (`up`) or below each primary cursor,
    /// at the same column.
    pub fn add_cursor_vertically(&mut self, up: bool) {
        let primary = self.selections.primary();
        let movement = if up { Movement::Up } else { Movement::Down };
        let (head, goal_column) = moved(&self.buffer, primary, movement);
        if head != primary.head {
            self.add_selection(Selection {
                anchor: head,
                head,
                goal_column,
            });
        }
    }

    /// Types `text` at every cursor, replacing selected text.
    pub fn insert(&mut self, text: &str) {
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
        self.apply(replacements, kind, |edit| {
            Selection::cursor(edit.new_range.end)
        });
    }

    /// Pastes `text`. With as many lines as cursors, each cursor gets its
    /// line (as copied from several selections).
    pub fn paste(&mut self, text: &str) {
        let lines: Vec<&str> = text.split('\n').collect();
        if self.selections.len() > 1 && lines.len() == self.selections.len() {
            let replacements = self
                .selections
                .iter()
                .zip(lines)
                .map(|(selection, line)| (selection.range(), line.to_owned()))
                .collect();
            self.apply(replacements, EditKind::Other, |edit| {
                Selection::cursor(edit.new_range.end)
            });
        } else {
            let replacements = self
                .selections
                .iter()
                .map(|selection| (selection.range(), text.to_owned()))
                .collect();
            self.apply(replacements, EditKind::Other, |edit| {
                Selection::cursor(edit.new_range.end)
            });
        }
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
                (selection.range(), format!("\n{indentation}"))
            })
            .collect();
        self.apply(replacements, EditKind::Other, |edit| {
            Selection::cursor(edit.new_range.end)
        });
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
        self.apply(replacements, EditKind::Other, |edit| {
            Selection::cursor(edit.new_range.start)
        });
    }

    /// Reverts the last undo step and restores the selections it had.
    pub fn undo(&mut self) -> bool {
        match self.history.undo(&mut self.buffer) {
            Some(selections) => {
                self.selections = selections;
                self.version += 1;
                true
            }
            None => false,
        }
    }

    /// Re-applies the last undone step and its selections.
    pub fn redo(&mut self) -> bool {
        match self.history.redo(&mut self.buffer) {
            Some(selections) => {
                self.selections = selections;
                self.version += 1;
                true
            }
            None => false,
        }
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
        self.apply(replacements, kind, |edit| {
            Selection::cursor(edit.new_range.start)
        });
    }

    /// Applies `replacements` (one per selection, any order) as one step:
    /// overlapping ranges are merged, the buffer is changed from the end so
    /// earlier ranges stay valid, every edit becomes a selection through
    /// `place` and the step is recorded for undo with the selections before
    /// and after.
    fn apply(
        &mut self,
        replacements: Vec<(Range<usize>, String)>,
        kind: EditKind,
        place: impl Fn(&Edit) -> Selection,
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
        for edit in edits.iter().rev() {
            self.buffer.replace(edit.old_range.clone(), &edit.new_text);
        }
        let before = self.selections.clone();
        let after = Selections::from_vec(edits.iter().map(&place).collect());
        self.history.record(edits, kind, before, after.clone());
        self.selections = after;
        self.version += 1;
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

fn clamp_selections(selections: &Selections, buffer: &Buffer) -> Selections {
    selections.map(|selection| Selection {
        anchor: buffer.clamp(selection.anchor),
        head: buffer.clamp(selection.head),
        goal_column: selection.goal_column,
    })
}

/// The new head of `selection` after `movement`, with the goal column to
/// keep for vertical moves.
fn moved(buffer: &Buffer, selection: Selection, movement: Movement) -> (usize, Option<usize>) {
    let head = selection.head;
    let rope = buffer.rope();
    let vertical = |rows_up: Option<usize>, rows_down: Option<usize>| {
        let goal = selection
            .goal_column
            .unwrap_or_else(|| buffer.char_column(head));
        let row = buffer.offset_to_point(head).row;
        let last_row = buffer.line_count() - 1;
        let target = match (rows_up, rows_down) {
            // Up on the first line goes to its start, down on the last
            // line to the end of the text, as in other editors.
            (Some(_), _) if row == 0 => return (0, Some(goal)),
            (Some(rows), _) => row.saturating_sub(rows),
            (_, Some(_)) if row == last_row => return (buffer.len(), Some(goal)),
            (_, Some(rows)) => (row + rows).min(last_row),
            (None, None) => row,
        };
        (buffer.offset_at_char_column(target, goal), Some(goal))
    };
    match movement {
        Movement::Left => (previous_grapheme(rope, head), None),
        Movement::Right => (next_grapheme(rope, head), None),
        Movement::WordLeft => (previous_word_start(rope, head), None),
        Movement::WordRight => (next_word_end(rope, head), None),
        Movement::Up => vertical(Some(1), None),
        Movement::Down => vertical(None, Some(1)),
        Movement::PageUp(rows) => vertical(Some(rows.max(1)), None),
        Movement::PageDown(rows) => vertical(None, Some(rows.max(1))),
        Movement::LineStart => {
            let row = buffer.offset_to_point(head).row;
            let range = buffer.line_range(row);
            let blank = buffer
                .line(row)
                .chars()
                .take_while(|character| *character == ' ' || *character == '\t')
                .count();
            let first_non_blank = buffer.offset_at_char_column(row, blank);
            let target = if head == first_non_blank {
                range.start
            } else {
                first_non_blank
            };
            (target, None)
        }
        Movement::LineEnd => (
            buffer.line_range(buffer.offset_to_point(head).row).end,
            None,
        ),
        Movement::DocumentStart => (0, None),
        Movement::DocumentEnd => (buffer.len(), None),
    }
}

#[cfg(test)]
mod tests;
