//! Selections and cursors. A cursor is an empty selection.

use std::ops::Range;

/// One selection: from `anchor` (where it started) to `head` (where the
/// cursor is). Offsets are bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Selection {
    /// The fixed end.
    pub anchor: usize,
    /// The moving end, where the cursor is drawn.
    pub head: usize,
    /// Column in characters to keep when moving up and down through
    /// shorter lines.
    pub goal_column: Option<usize>,
}

impl Selection {
    /// A cursor at `offset`.
    pub fn cursor(offset: usize) -> Self {
        Self {
            anchor: offset,
            head: offset,
            goal_column: None,
        }
    }

    /// A selection from `anchor` to `head`.
    pub fn new(anchor: usize, head: usize) -> Self {
        Self {
            anchor,
            head,
            goal_column: None,
        }
    }

    /// Whether nothing is selected.
    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    /// The smaller end.
    pub fn start(&self) -> usize {
        self.anchor.min(self.head)
    }

    /// The larger end.
    pub fn end(&self) -> usize {
        self.anchor.max(self.head)
    }

    /// The selected bytes.
    pub fn range(&self) -> Range<usize> {
        self.start()..self.end()
    }

    /// Whether the head is before the anchor.
    pub fn is_reversed(&self) -> bool {
        self.head < self.anchor
    }
}

/// All selections of a document, sorted and without overlaps, never empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selections {
    selections: Vec<Selection>,
    /// Index of the selection added last (scrolled into view).
    primary: usize,
}

impl Default for Selections {
    fn default() -> Self {
        Self::single(Selection::cursor(0))
    }
}

impl Selections {
    /// One selection.
    pub fn single(selection: Selection) -> Self {
        Self {
            selections: vec![selection],
            primary: 0,
        }
    }

    /// Several selections; the last one is primary. Overlapping ones are
    /// merged. An empty list becomes a cursor at 0.
    pub fn from_vec(selections: Vec<Selection>) -> Self {
        if selections.is_empty() {
            return Self::default();
        }
        let primary = selections.len() - 1;
        let mut set = Self {
            selections,
            primary,
        };
        set.normalize();
        set
    }

    /// The selections, in document order.
    pub fn iter(&self) -> impl Iterator<Item = &Selection> {
        self.selections.iter()
    }

    /// Number of selections.
    pub fn len(&self) -> usize {
        self.selections.len()
    }

    /// Never true; present for symmetry with `len`.
    pub fn is_empty(&self) -> bool {
        self.selections.is_empty()
    }

    /// The selection added last.
    pub fn primary(&self) -> Selection {
        self.selections[self.primary]
    }

    /// Applies `change` to every selection and merges the results.
    #[must_use]
    pub fn map(&self, mut change: impl FnMut(Selection) -> Selection) -> Self {
        let mut mapped = Self {
            selections: self
                .selections
                .iter()
                .map(|selection| change(*selection))
                .collect(),
            primary: self.primary,
        };
        mapped.normalize();
        mapped
    }

    /// Adds a selection and makes it primary.
    pub fn add(&mut self, selection: Selection) {
        self.selections.push(selection);
        self.primary = self.selections.len() - 1;
        self.normalize();
    }

    /// Sorts by start and merges overlapping selections (and cursors at the
    /// same place), keeping track of the primary one.
    fn normalize(&mut self) {
        let mut order: Vec<usize> = (0..self.selections.len()).collect();
        order.sort_by_key(|index| {
            (
                self.selections[*index].start(),
                self.selections[*index].end(),
            )
        });
        let mut merged: Vec<Selection> = Vec::with_capacity(self.selections.len());
        let mut new_primary = 0;
        for index in order {
            let selection = self.selections[index];
            let is_primary = index == self.primary;
            match merged.last_mut() {
                Some(last) if overlaps(last, &selection) => {
                    let start = last.start().min(selection.start());
                    let end = last.end().max(selection.end());
                    *last = if last.is_reversed() {
                        Selection::new(end, start)
                    } else {
                        Selection::new(start, end)
                    };
                    if is_primary {
                        new_primary = merged.len() - 1;
                    }
                }
                _ => {
                    merged.push(selection);
                    if is_primary {
                        new_primary = merged.len() - 1;
                    }
                }
            }
        }
        self.selections = merged;
        self.primary = new_primary;
    }
}

/// Whether `right` (which starts at or after `left`) merges into `left`:
/// non-empty selections merge when they share bytes (touching ones stay
/// apart), a cursor merges when it is inside or at an edge.
fn overlaps(left: &Selection, right: &Selection) -> bool {
    if left.is_empty() || right.is_empty() {
        right.start() <= left.end()
    } else {
        right.start() < left.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorts_and_merges() {
        let selections = Selections::from_vec(vec![
            Selection::new(10, 15),
            Selection::cursor(3),
            Selection::new(12, 20),
            Selection::cursor(3),
        ]);
        let ranges: Vec<_> = selections.iter().map(Selection::range).collect();
        assert_eq!(ranges, [3..3, 10..20]);
        assert_eq!(
            selections.primary().range(),
            3..3,
            "the last added was the cursor at 3"
        );
    }

    #[test]
    fn touching_selections_stay_apart_but_a_cursor_inside_merges() {
        let touching = Selections::from_vec(vec![Selection::new(0, 5), Selection::new(5, 9)]);
        assert_eq!(touching.len(), 2);
        let inside = Selections::from_vec(vec![Selection::new(0, 5), Selection::cursor(3)]);
        assert_eq!(inside.len(), 1);
    }

    #[test]
    fn reversed_selections_keep_their_direction() {
        let selection = Selection::new(9, 2);
        assert!(selection.is_reversed());
        assert_eq!(selection.range(), 2..9);
        let merged = Selections::from_vec(vec![Selection::new(9, 2), Selection::new(5, 12)]);
        assert_eq!(merged.primary().range(), 2..12);
    }
}
