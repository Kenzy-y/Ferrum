//! A buffer with selections and history: the editing operations.

use std::fmt;
use std::ops::Range;
use std::sync::Arc;
use std::time::Instant;

use unicode_segmentation::UnicodeSegmentation as _;

use crate::boundaries::{next_grapheme, next_word_end, previous_grapheme, previous_word_start};
use crate::changes::ChangeLog;
use crate::history::Edit;
use crate::{Buffer, EditKind, History, LineEnding, Selection, Selections, TextChanges};

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

/// Where a document gets the time from (undo grouping by pauses);
/// replaceable in tests.
#[derive(Clone)]
pub struct Clock(Arc<dyn Fn() -> Instant + Send + Sync>);

impl Clock {
    /// A clock reading `now`.
    pub fn new(now: impl Fn() -> Instant + Send + Sync + 'static) -> Self {
        Self(Arc::new(now))
    }

    fn now(&self) -> Instant {
        (self.0)()
    }
}

impl Default for Clock {
    fn default() -> Self {
        Self::new(Instant::now)
    }
}

impl fmt::Debug for Clock {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Clock")
    }
}

/// Text being edited.
#[derive(Debug, Clone, Default)]
pub struct Document {
    buffer: Buffer,
    selections: Selections,
    history: History,
    clock: Clock,
    /// Increases with every change of the text; for caches.
    version: u64,
    /// Edits for consumers that follow the text incrementally.
    changes: ChangeLog,
}

/// How the selections after an edit are made.
enum Placement<'a> {
    /// One selection per edit.
    PerEdit(&'a dyn Fn(&Edit) -> Selection),
    /// The existing selections, moved by the edits; an offset where text
    /// is inserted stays in front of it.
    Keep,
    /// Like `Keep`, but an offset where text is inserted moves after it.
    Shift,
}

impl Document {
    /// A document with `buffer` and a cursor at the start.
    pub fn new(buffer: Buffer) -> Self {
        Self {
            buffer,
            selections: Selections::default(),
            history: History::default(),
            clock: Clock::default(),
            version: 0,
            changes: ChangeLog::default(),
        }
    }

    /// Uses `clock` for the time (tests).
    #[must_use]
    pub fn with_clock(mut self, clock: Clock) -> Self {
        self.clock = clock;
        self
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

    /// Identifies the text for comparing with a saved state: changes with
    /// every edit and returns to an earlier value when undo or redo bring
    /// that text back (see [`History::state_id`]).
    pub fn state_id(&self) -> u64 {
        self.history.state_id()
    }

    /// The text was saved: the next edit starts a new undo step, so the
    /// returned [`Self::state_id`] stays the saved text's value.
    pub fn mark_saved(&mut self) -> u64 {
        self.history.break_group();
        self.history.state_id()
    }

    /// Starts logging every edit for [`Self::take_changes`] (syntax trees
    /// follow the text with them). Off by default, because each logged
    /// edit costs a few extra lookups.
    pub fn track_changes(&mut self) {
        self.changes.enable();
    }

    /// The edits since the last call (none unless [`Self::track_changes`]
    /// was called).
    pub fn take_changes(&mut self) -> TextChanges {
        self.changes.take()
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

    /// Reverts the last undo step and restores the selections it had.
    pub fn undo(&mut self) -> bool {
        match self.history.undo(&mut self.buffer, &mut self.changes) {
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
        match self.history.redo(&mut self.buffer, &mut self.changes) {
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
    fn apply(
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

mod indentation;
mod ranges;

pub use indentation::Indent;
pub use ranges::CopiedText;

#[cfg(test)]
mod tests;
