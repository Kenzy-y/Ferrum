//! Undo and redo.

use std::ops::Range;

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

/// Edits applied together (one per cursor), in document order.
type Step = Vec<Edit>;

/// One undo step: the steps it groups and the selections around it.
#[derive(Debug, Clone)]
struct Transaction {
    steps: Vec<Step>,
    kind: EditKind,
    selections_before: Selections,
    selections_after: Selections,
}

/// Undo and redo stacks.
#[derive(Debug, Clone)]
pub struct History {
    undo: Vec<Transaction>,
    redo: Vec<Transaction>,
    limit: usize,
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
            undo: Vec::new(),
            redo: Vec::new(),
            limit,
        }
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
    /// typing (or both deletion) and the cursors did not move in between.
    pub(crate) fn record(
        &mut self,
        step: Step,
        kind: EditKind,
        before: Selections,
        after: Selections,
    ) {
        self.redo.clear();
        if kind != EditKind::Other
            && let Some(last) = self.undo.last_mut()
            && last.kind == kind
            && last.selections_after == before
        {
            last.steps.push(step);
            last.selections_after = after;
            return;
        }
        self.undo.push(Transaction {
            steps: vec![step],
            kind,
            selections_before: before,
            selections_after: after,
        });
        if self.undo.len() > self.limit {
            self.undo.remove(0);
        }
    }

    /// Ends the current group: the next edit starts a new undo step even if
    /// it is of the same kind (used after cursor moves, saves, focus loss).
    pub fn break_group(&mut self) {
        if let Some(last) = self.undo.last_mut() {
            last.kind = EditKind::Other;
        }
    }

    /// Reverts the last transaction; returns the selections it had before.
    pub(crate) fn undo(&mut self, buffer: &mut Buffer) -> Option<Selections> {
        let transaction = self.undo.pop()?;
        for step in transaction.steps.iter().rev() {
            // Later edits first, so earlier ranges stay valid.
            for edit in step.iter().rev() {
                buffer.replace(edit.new_range.clone(), &edit.old_text);
            }
        }
        let selections = transaction.selections_before.clone();
        self.redo.push(transaction);
        Some(selections)
    }

    /// Re-applies the last undone transaction; returns the selections it had
    /// after.
    pub(crate) fn redo(&mut self, buffer: &mut Buffer) -> Option<Selections> {
        let transaction = self.redo.pop()?;
        for step in &transaction.steps {
            for edit in step.iter().rev() {
                buffer.replace(edit.old_range.clone(), &edit.new_text);
            }
        }
        let selections = transaction.selections_after.clone();
        self.undo.push(transaction);
        Some(selections)
    }
}
