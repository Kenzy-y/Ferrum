//! Undo and redo.

use std::collections::VecDeque;
use std::ops::Range;
use std::time::{Duration, Instant};

use crate::changes::ChangeLog;
use crate::{Buffer, Selections};

/// One replacement, with its range before and after the change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Edit {
    /// Range in the text before the change.
    pub(crate) old_range: Range<usize>,
    /// Range of the inserted text after the change.
    pub(crate) new_range: Range<usize>,
    pub(crate) old_text: String,
    pub(crate) new_text: String,
}

/// What kind of change a transaction is; consecutive typing (or deleting)
/// becomes one undo step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditKind {
    /// Characters typed one after another.
    Typing,
    /// Characters deleted one after another.
    Deletion,
    /// Anything else (paste, newline, cut): always its own step.
    Other,
}

/// A pause in typing longer than this starts a new undo step.
pub const UNDO_GROUP_PAUSE: Duration = Duration::from_secs(1);

/// Edits applied together (one per cursor), in document order.
type Step = Vec<Edit>;

/// One undo step: the steps it groups and the selections around it.
#[derive(Debug, Clone)]
struct Transaction {
    /// Identifies the text right after this transaction.
    id: u64,
    steps: Vec<Step>,
    kind: EditKind,
    selections_before: Selections,
    selections_after: Selections,
    /// When the last step was added.
    last_edit: Instant,
    /// The last character typed, to end the group at word ends.
    last_typed: Option<char>,
}

/// Undo and redo stacks.
#[derive(Debug, Clone)]
pub struct History {
    undo: VecDeque<Transaction>,
    redo: Vec<Transaction>,
    limit: usize,
    /// Identifies the text before the oldest transaction kept.
    base_id: u64,
    next_id: u64,
}

/// Undo steps kept by default.
const DEFAULT_LIMIT: usize = 10_000;

impl Default for History {
    fn default() -> Self {
        Self::new(DEFAULT_LIMIT)
    }
}

impl History {
    /// A history keeping at most `limit` undo steps.
    pub fn new(limit: usize) -> Self {
        Self {
            undo: VecDeque::new(),
            redo: Vec::new(),
            limit,
            base_id: 0,
            next_id: 1,
        }
    }

    /// Identifies the current text: a new value after every new edit, an
    /// earlier value again when undo or redo bring that text back. Edits
    /// that join a transaction keep its value, so whoever compares values
    /// (the saved state) calls [`Self::break_group`] when taking one.
    pub fn state_id(&self) -> u64 {
        self.undo.back().map_or(self.base_id, |last| last.id)
    }

    /// Forgets every step (the text was replaced); the text gets a value
    /// never used before.
    pub(crate) fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.base_id = self.next_id;
        self.next_id += 1;
    }

    /// Whether there is something to undo.
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Whether there is something to redo.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Records a step. It joins the previous transaction when both are
    /// typing (or both deletion), the cursors did not move in between, less
    /// than [`UNDO_GROUP_PAUSE`] passed, and the typing did not start a new
    /// word (a non-space after a space ends the previous word).
    pub(crate) fn record(
        &mut self,
        step: Step,
        kind: EditKind,
        before: Selections,
        after: Selections,
        at: Instant,
        typed: Option<char>,
    ) {
        self.redo.clear();
        if kind != EditKind::Other
            && let Some(last) = self.undo.back_mut()
            && last.kind == kind
            && last.selections_after == before
            && at.saturating_duration_since(last.last_edit) < UNDO_GROUP_PAUSE
            && !starts_new_word(last.last_typed, typed)
        {
            last.steps.push(step);
            last.selections_after = after;
            last.last_edit = at;
            last.last_typed = typed;
            return;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.undo.push_back(Transaction {
            id,
            steps: vec![step],
            kind,
            selections_before: before,
            selections_after: after,
            last_edit: at,
            last_typed: typed,
        });
        if self.undo.len() > self.limit
            && let Some(oldest) = self.undo.pop_front()
        {
            self.base_id = oldest.id;
        }
    }

    /// Ends the current group: the next edit starts a new undo step even if
    /// it is of the same kind (used after cursor moves, saves, focus loss).
    pub fn break_group(&mut self) {
        if let Some(last) = self.undo.back_mut() {
            last.kind = EditKind::Other;
        }
    }

    /// Reverts the last transaction; returns the selections it had before.
    pub(crate) fn undo(&mut self, buffer: &mut Buffer, log: &mut ChangeLog) -> Option<Selections> {
        let transaction = self.undo.pop_back()?;
        for step in transaction.steps.iter().rev() {
            log.replace_step(
                buffer,
                step.iter()
                    .map(|edit| (edit.new_range.clone(), edit.old_text.as_str())),
            );
        }
        let selections = transaction.selections_before.clone();
        self.redo.push(transaction);
        Some(selections)
    }

    /// Re-applies the last undone transaction; returns the selections it had
    /// after.
    pub(crate) fn redo(&mut self, buffer: &mut Buffer, log: &mut ChangeLog) -> Option<Selections> {
        let transaction = self.redo.pop()?;
        for step in &transaction.steps {
            log.replace_step(
                buffer,
                step.iter()
                    .map(|edit| (edit.old_range.clone(), edit.new_text.as_str())),
            );
        }
        let selections = transaction.selections_after.clone();
        self.undo.push_back(transaction);
        Some(selections)
    }
}

/// Whether typing `typed` after `previous` starts a new word: the first
/// non-space after a space. "hello world" becomes "hello " and "world".
fn starts_new_word(previous: Option<char>, typed: Option<char>) -> bool {
    matches!((previous, typed), (Some(previous), Some(typed)) if previous.is_whitespace() && !typed.is_whitespace())
}
