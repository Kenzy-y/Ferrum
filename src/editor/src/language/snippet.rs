//! Snippet sessions: after a completion with tab stops, the first stop is
//! selected; Tab goes to the next, Shift+Tab to the previous, and the
//! final stop (`$0`, or the end of the snippet) ends the session. Stops
//! with several ranges get a cursor each. Escape, or editing away from the
//! current stop, ends it too.

use std::ops::Range;

use gpui::Context;
use language::TabStop;
use text::{Selection, Selections, TextChanges};

use crate::editor::Editor;
use crate::highlighting::patch::map_offset;

/// A snippet being filled in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SnippetSession {
    /// Ranges of every stop in visiting order, in current offsets; the
    /// last one is the final cursor.
    stops: Vec<Vec<Range<usize>>>,
    current: usize,
}

impl SnippetSession {
    /// A session for `stops` of a snippet inserted at `base` with text
    /// length `length`; `None` when there is nothing to visit but the
    /// end.
    pub(crate) fn new(base: usize, length: usize, stops: &[TabStop]) -> Option<Self> {
        let mut ranges: Vec<Vec<Range<usize>>> = stops
            .iter()
            .map(|stop| {
                stop.ranges
                    .iter()
                    .map(|range| base + range.start..base + range.end)
                    .collect()
            })
            .collect();
        if stops.last().is_none_or(|stop| stop.index != 0) {
            // Without `$0` the session ends after the snippet.
            let end = base + length;
            ranges.push(std::iter::once(end..end).collect());
        }
        (ranges.len() > 1).then_some(Self {
            stops: ranges,
            current: 0,
        })
    }

    /// The final cursor, for a snippet with nothing else to visit.
    pub(crate) fn final_cursor(base: usize, length: usize, stops: &[TabStop]) -> usize {
        stops
            .iter()
            .find(|stop| stop.index == 0)
            .and_then(|stop| stop.ranges.first())
            .map_or(base + length, |range| base + range.start)
    }

    fn selections(&self) -> Selections {
        let ranges = &self.stops[self.current];
        Selections::from_vec(
            ranges
                .iter()
                .map(|range| Selection::new(range.start, range.end))
                .collect(),
        )
    }

    fn is_last(&self) -> bool {
        self.current + 1 == self.stops.len()
    }

    /// Moves every stop through the edits.
    fn follow(&mut self, changes: &TextChanges) {
        for ranges in &mut self.stops {
            for range in ranges.iter_mut() {
                for edit in &changes.edits {
                    let start = map_offset(range.start, edit, true);
                    let end = map_offset(range.end, edit, false);
                    *range = start..end.max(start);
                }
            }
        }
    }

    /// Whether `offset` is in (or at an edge of) the current stop.
    fn holds(&self, offset: usize) -> bool {
        self.stops[self.current]
            .iter()
            .any(|range| range.start <= offset && offset <= range.end)
    }
}

impl Editor {
    /// Starts a session for a snippet just inserted at `base`, or puts the
    /// cursor at its final stop.
    pub(crate) fn start_snippet(
        &mut self,
        base: usize,
        length: usize,
        stops: &[TabStop],
        cx: &mut Context<Self>,
    ) {
        let session = SnippetSession::new(base, length, stops);
        let selections = match &session {
            Some(session) => session.selections(),
            None => Selections::single(Selection::cursor(SnippetSession::final_cursor(
                base, length, stops,
            ))),
        };
        if let Some(language) = &mut self.language {
            language.snippet = session;
        }
        self.change(cx, |document| document.set_selections(&selections));
    }

    /// Tab (`forward`) or Shift+Tab in a session; `false` without one.
    pub(crate) fn snippet_step(&mut self, forward: bool, cx: &mut Context<Self>) -> bool {
        let Some(session) = self
            .language
            .as_mut()
            .and_then(|language| language.snippet.as_mut())
        else {
            return false;
        };
        if forward {
            session.current += 1;
        } else {
            session.current = session.current.saturating_sub(1);
        }
        let selections = session.selections();
        if session.is_last()
            && let Some(language) = &mut self.language
        {
            language.snippet = None;
        }
        self.change(cx, |document| document.set_selections(&selections));
        true
    }

    /// Ends the session; `true` if there was one.
    pub(crate) fn end_snippet(&mut self) -> bool {
        self.language
            .as_mut()
            .and_then(|language| language.snippet.take())
            .is_some()
    }

    /// Moves the session through an edit; editing away from the current
    /// stop ends it.
    pub(crate) fn follow_snippet(&mut self, changes: &TextChanges) {
        let head = self.document.selections().primary().head;
        let Some(language) = &mut self.language else {
            return;
        };
        let Some(session) = &mut language.snippet else {
            return;
        };
        if changes.replaced {
            language.snippet = None;
            return;
        }
        session.follow(changes);
        if !session.holds(head) {
            language.snippet = None;
        }
    }
}

#[cfg(test)]
// A stop's ranges are a list that happens to hold one range here.
#[allow(clippy::single_range_in_vec_init)]
mod tests {
    use super::*;

    fn stop(index: u32, range: Range<usize>) -> TabStop {
        TabStop {
            index,
            ranges: vec![range],
        }
    }

    #[test]
    fn a_lone_final_cursor_needs_no_session() {
        let stops = [stop(0, 9..9)];
        assert_eq!(SnippetSession::new(10, 10, &stops), None);
        assert_eq!(SnippetSession::final_cursor(10, 10, &stops), 19);
    }

    #[test]
    fn stops_get_the_base_and_an_implicit_end() {
        let session = SnippetSession::new(4, 8, &[stop(1, 3..7)]).unwrap();
        assert_eq!(session.stops, vec![vec![7..11], vec![12..12]]);
    }
}
