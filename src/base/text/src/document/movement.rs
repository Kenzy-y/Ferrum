//! Moving cursors: by character, word, line, page and document.

use crate::boundaries::{next_grapheme, next_word_end, previous_grapheme, previous_word_start};
use crate::{Buffer, Selection};

use super::Document;

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

impl Document {
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
