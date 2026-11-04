//! A buffer with selections and history. Editing operations live in
//! `editing`, cursor movement in `movement`, indentation and line
//! operations in `indentation` and `ranges`.

use std::fmt;
use std::sync::Arc;
use std::time::Instant;

use crate::changes::ChangeLog;
use crate::history::Edit;
use crate::{Buffer, History, Selection, Selections, TextChanges};

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
}

fn clamp_selections(selections: &Selections, buffer: &Buffer) -> Selections {
    selections.map(|selection| Selection {
        anchor: buffer.clamp(selection.anchor),
        head: buffer.clamp(selection.head),
        goal_column: selection.goal_column,
    })
}

mod editing;
mod indentation;
mod movement;
mod ranges;

pub use indentation::Indent;
pub use movement::Movement;
pub use ranges::CopiedText;

#[cfg(test)]
mod tests;
