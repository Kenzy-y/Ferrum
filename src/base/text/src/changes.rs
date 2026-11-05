//! What changed in the text, for consumers that follow it incrementally
//! (syntax trees): every replacement in the order it was applied.

use std::ops::Range;
use std::sync::Arc;

use crate::{Buffer, Point};

/// One replacement, in the coordinates of the text right before it was
/// applied. Edits must be replayed in the order they are listed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    /// Byte offset where the replaced text started.
    pub start: usize,
    /// Byte offset where the replaced text ended.
    pub old_end: usize,
    /// Byte offset where the inserted text ends.
    pub new_end: usize,
    /// `start` as a row and a byte column.
    pub start_point: Point,
    /// `old_end` as a row and a byte column, before the edit.
    pub old_end_point: Point,
    /// `new_end` as a row and a byte column, after the edit.
    pub new_end_point: Point,
    /// The inserted text, `start..new_end` right after the edit; language
    /// servers are sent it.
    pub inserted: Arc<str>,
}

/// Changes since they were last taken.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextChanges {
    /// The replacements, in the order they were applied.
    pub edits: Vec<TextEdit>,
    /// The whole text was replaced (reloaded, or too many edits to list):
    /// `edits` is empty and everything derived from the text is stale.
    pub replaced: bool,
}

impl TextChanges {
    /// Whether nothing changed.
    pub fn is_empty(&self) -> bool {
        self.edits.is_empty() && !self.replaced
    }
}

/// Edits kept before the log gives up and reports the whole text as
/// replaced, so a document nobody drains stays small (about 0.5 MB).
const MAX_LOGGED_EDITS: usize = 10_000;

/// A step with more edits than this (over a thousand cursors) is logged as
/// one edit spanning all of them: logging each costs three point lookups
/// and would double the cost of the step, while a spanning edit only makes
/// the next parse look at more text.
const MAX_EDITS_PER_STEP: usize = 1024;

/// The log of a document. Off by default: then replacing costs nothing
/// extra.
#[derive(Debug, Clone, Default)]
pub(crate) struct ChangeLog {
    enabled: bool,
    changes: TextChanges,
}

impl ChangeLog {
    pub(crate) fn enable(&mut self) {
        self.enabled = true;
    }

    /// Replaces `range` with `text` in `buffer` and logs it. Logging takes
    /// three logarithmic lookups of points.
    pub(crate) fn replace(&mut self, buffer: &mut Buffer, range: Range<usize>, text: &str) {
        if !self.enabled || self.changes.replaced {
            buffer.replace(range, text);
            return;
        }
        let start_point = buffer.offset_to_point(range.start);
        let old_end_point = buffer.offset_to_point(range.end);
        let (start, old_end) = (range.start, range.end);
        buffer.replace(range, text);
        let new_end = start + text.len();
        self.changes.edits.push(TextEdit {
            start,
            old_end,
            new_end,
            start_point,
            old_end_point,
            new_end_point: buffer.offset_to_point(new_end),
            inserted: Arc::from(text),
        });
        if self.changes.edits.len() > MAX_LOGGED_EDITS {
            self.replaced();
        }
    }

    /// Applies one step: `replacements` in document order, disjoint, in
    /// the coordinates of the text before the step. They are applied from
    /// the last to the first, so earlier ranges stay valid.
    pub(crate) fn replace_step<'a>(
        &mut self,
        buffer: &mut Buffer,
        replacements: impl DoubleEndedIterator<Item = (Range<usize>, &'a str)> + ExactSizeIterator,
    ) {
        if replacements.len() <= MAX_EDITS_PER_STEP || !self.enabled || self.changes.replaced {
            for (range, text) in replacements.rev() {
                self.replace(buffer, range, text);
            }
            return;
        }
        let mut span: Option<(usize, usize)> = None;
        let (mut added, mut removed) = (0, 0);
        let mut steps = Vec::with_capacity(replacements.len());
        for (range, text) in replacements {
            span = Some(span.map_or((range.start, range.end), |(start, _)| (start, range.end)));
            added += text.len();
            removed += range.len();
            steps.push((range, text));
        }
        let Some((start, old_end)) = span else {
            return;
        };
        let start_point = buffer.offset_to_point(start);
        let old_end_point = buffer.offset_to_point(old_end);
        for (range, text) in steps.into_iter().rev() {
            buffer.replace(range, text);
        }
        let new_end = old_end + added - removed;
        self.changes.edits.push(TextEdit {
            start,
            old_end,
            new_end,
            start_point,
            old_end_point,
            new_end_point: buffer.offset_to_point(new_end),
            inserted: Arc::from(buffer.slice(start..new_end).as_str()),
        });
    }

    /// The whole text changed.
    pub(crate) fn replaced(&mut self) {
        if self.enabled {
            self.changes.edits = Vec::new();
            self.changes.replaced = true;
        }
    }

    pub(crate) fn take(&mut self) -> TextChanges {
        std::mem::take(&mut self.changes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logs_edits_with_points_only_when_enabled() {
        let mut buffer = Buffer::from_text("ab\ncd");
        let mut log = ChangeLog::default();
        log.replace(&mut buffer, 0..1, "x");
        assert!(log.take().is_empty());

        log.enable();
        log.replace(&mut buffer, 3..4, "1\n2");
        let changes = log.take();
        assert_eq!(
            changes.edits,
            vec![TextEdit {
                start: 3,
                old_end: 4,
                new_end: 6,
                start_point: Point::new(1, 0),
                old_end_point: Point::new(1, 1),
                new_end_point: Point::new(2, 1),
                inserted: Arc::from("1\n2"),
            }]
        );
        assert!(log.take().is_empty(), "taking empties the log");
    }

    #[test]
    fn a_step_of_many_edits_is_logged_as_one_spanning_edit() {
        let mut buffer = Buffer::from_text(&"ab\n".repeat(2000));
        let mut log = ChangeLog::default();
        log.enable();
        // Insert "x" after the "a" of every line.
        let replacements = (0..2000).map(|row| (row * 3 + 1..row * 3 + 1, "x"));
        log.replace_step(&mut buffer, replacements);
        assert_eq!(buffer.slice(0..8), "axb\naxb\n");
        let changes = log.take();
        assert_eq!(
            changes.edits,
            vec![TextEdit {
                start: 1,
                old_end: 5998,
                new_end: 7998,
                start_point: Point::new(0, 1),
                old_end_point: Point::new(1999, 1),
                new_end_point: Point::new(1999, 2),
                inserted: Arc::from(buffer.slice(1..7998).as_str()),
            }]
        );

        // A few edits are logged one by one, last first.
        log.replace_step(&mut buffer, [(0..1, "1"), (5..6, "2")].into_iter());
        let starts: Vec<usize> = log.take().edits.iter().map(|edit| edit.start).collect();
        assert_eq!(starts, vec![5, 0]);
    }

    #[test]
    fn too_many_edits_become_a_replacement() {
        let mut buffer = Buffer::new();
        let mut log = ChangeLog::default();
        log.enable();
        for _ in 0..=MAX_LOGGED_EDITS {
            log.replace(&mut buffer, 0..0, "a");
        }
        let changes = log.take();
        assert!(changes.replaced && changes.edits.is_empty());
        assert_eq!(buffer.len(), MAX_LOGGED_EDITS + 1);
    }
}
