//! Find and replace inside one editor: the matches of a query, found in the
//! background on a snapshot of the text and found again after edits; going
//! to the next or previous match; replacing one or all.
//!
//! The find widget (fields and buttons) belongs to the workbench; it calls
//! the methods here and reads [`Editor::find_summary`].

use std::ops::Range;
use std::time::Duration;

use gpui::{Context, Task};
use search::SearchQuery;
use text::{Selection, Selections};

use crate::{Editor, EditorEvent};

/// Matches beyond this are not found (and not highlighted).
pub const MAX_MATCHES: usize = 20_000;
/// Pause after an edit before the matches are found again.
const REFIND_DELAY: Duration = Duration::from_millis(100);

/// The matches of the query, for the version of the text they were found
/// in.
#[derive(Default)]
pub(crate) struct FindState {
    pub(crate) query: SearchQuery,
    /// Sorted, not overlapping.
    pub(crate) matches: Vec<Range<usize>>,
    /// The document version `matches` belong to.
    version: u64,
    /// The query is not a valid regular expression.
    pub(crate) invalid: bool,
    task: Option<Task<()>>,
}

impl FindState {
    /// Takes the outcome of a search of text `version`.
    fn set_found(&mut self, found: Result<Vec<Range<usize>>, search::SearchError>, version: u64) {
        self.invalid = found.is_err();
        self.matches = found.unwrap_or_default();
        self.version = version;
    }
}

/// What the find widget shows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FindSummary {
    /// Number of matches (at most [`MAX_MATCHES`]).
    pub total: usize,
    /// The match the primary selection is on, from 0.
    pub current: Option<usize>,
    /// The query is not a valid regular expression.
    pub invalid: bool,
}

impl Editor {
    /// Starts finding `query` (`None` stops and removes the highlights).
    pub fn set_find_query(&mut self, query: Option<SearchQuery>, cx: &mut Context<Self>) {
        match query {
            Some(query) if !query.is_empty() => {
                let state = self.find.get_or_insert_with(FindState::default);
                state.query = query;
                self.refind(Duration::ZERO, cx);
            }
            _ => {
                self.find = None;
                cx.emit(EditorEvent::FindChanged);
                cx.notify();
            }
        }
    }

    /// Counts and position for the find widget.
    pub fn find_summary(&self) -> FindSummary {
        let Some(state) = &self.find else {
            return FindSummary::default();
        };
        let primary = self.document.selections().primary();
        let range = primary.start()..primary.end();
        FindSummary {
            total: state.matches.len(),
            current: state
                .matches
                .binary_search_by(|found| {
                    found
                        .start
                        .cmp(&range.start)
                        .then(found.end.cmp(&range.end))
                })
                .ok(),
            invalid: state.invalid,
        }
    }

    /// Selects the next match after the cursor (`forward`) or the one
    /// before it, wrapping around, and scrolls to it.
    pub fn find_next(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some(state) = &self.find else {
            return;
        };
        if state.matches.is_empty() {
            return;
        }
        let primary = self.document.selections().primary();
        let count = state.matches.len();
        let index = if forward {
            // Past the start of a selected match, so it is not found again.
            let after = if primary.is_empty() {
                primary.head
            } else {
                primary.start() + 1
            };
            state.matches.partition_point(|found| found.start < after) % count
        } else {
            let before = primary.start();
            (state.matches.partition_point(|found| found.start < before) + count - 1) % count
        };
        let found = state.matches[index].clone();
        self.select(found, cx);
    }

    /// Replaces the selected match with `replacement` and selects the next
    /// one; if no match is selected, only selects the next.
    pub fn replace_next(&mut self, replacement: &str, cx: &mut Context<Self>) {
        let summary = self.find_summary();
        if let (Some(current), Some(state)) = (summary.current, &self.find) {
            let range = state.matches[current].clone();
            // A regular expression fills `$1` from the selected match.
            let matched = self
                .document
                .buffer()
                .rope()
                .byte_slice(range.clone())
                .to_string();
            let text = state
                .query
                .replacements(&matched, replacement, 1)
                .ok()
                .and_then(|found| found.into_iter().next())
                .map_or_else(|| replacement.to_owned(), |(_, text)| text);
            self.change(cx, |document| {
                let replaced = document.replace(range, &text);
                document.set_selections(&Selections::single(Selection::cursor(replaced.end)));
            });
            // The matches move after the edit; find them now, then go on.
            self.refind_now();
        }
        self.find_next(true, cx);
    }

    /// Replaces every match with `replacement` as one undoable edit.
    /// Returns how many were replaced.
    pub fn replace_all(&mut self, replacement: &str, cx: &mut Context<Self>) -> usize {
        let Some(state) = &self.find else {
            return 0;
        };
        let text = self.document.buffer().rope().to_string();
        let Ok(edits) = state.query.replacements(&text, replacement, MAX_MATCHES) else {
            return 0;
        };
        let mut count = 0;
        self.change(cx, |document| {
            count = document.replace_ranges(&edits);
        });
        self.refind_now();
        cx.emit(EditorEvent::FindChanged);
        count
    }

    /// Matches within `range` (visible rows), for highlighting.
    pub(crate) fn find_matches_in(&self, range: Range<usize>) -> &[Range<usize>] {
        let Some(state) = &self.find else {
            return &[];
        };
        if state.version != self.document.version() {
            // Stale offsets would highlight the wrong text; the new matches
            // come shortly.
            return &[];
        }
        let from = state
            .matches
            .partition_point(|found| found.end <= range.start);
        let to = state
            .matches
            .partition_point(|found| found.start < range.end);
        &state.matches[from..to.max(from)]
    }

    /// The text changed: find again after a pause.
    pub(crate) fn find_after_edit(&mut self, cx: &mut Context<Self>) {
        if self.find.is_some() {
            self.refind(REFIND_DELAY, cx);
        }
    }

    /// Finds on the UI thread: only after a replacement, which the user
    /// asked for and waits on.
    fn refind_now(&mut self) {
        let version = self.document.version();
        let text = self.document.buffer().rope().to_string();
        if let Some(state) = &mut self.find {
            let found = state.query.find_all(&text, MAX_MATCHES);
            state.set_found(found, version);
            state.task = None;
        }
    }

    fn refind(&mut self, delay: Duration, cx: &mut Context<Self>) {
        let Some(state) = &mut self.find else {
            return;
        };
        let query = state.query.clone();
        let version = self.document.version();
        // Cloning a rope is cheap; the text is put together in the
        // background.
        let rope = self.document.buffer().rope().clone();
        state.task = Some(cx.spawn(async move |editor, cx| {
            if !delay.is_zero() {
                cx.background_executor().timer(delay).await;
            }
            let found = cx
                .background_executor()
                .spawn(async move { query.find_all(&rope.to_string(), MAX_MATCHES) })
                .await;
            let _ = editor.update(cx, |editor, cx| {
                if let Some(state) = &mut editor.find {
                    state.set_found(found, version);
                    cx.emit(EditorEvent::FindChanged);
                    cx.notify();
                }
            });
        }));
    }
}

#[cfg(test)]
mod tests {
    use gpui::TestAppContext;
    use search::SearchQuery;
    use text::Document;

    use crate::{Editor, EditorSettings};

    fn query(pattern: &str) -> SearchQuery {
        SearchQuery {
            pattern: pattern.to_owned(),
            ..SearchQuery::default()
        }
    }

    #[gpui::test]
    fn finds_steps_and_replaces(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let theme =
                theme::Theme::built_in(theme::DEFAULT_THEME, theme::Density::default()).unwrap();
            theme::set_theme(theme, cx);
        });
        let window = cx.add_window(|window, cx| {
            Editor::new(
                Document::from_text("foo bar foo\nfoo"),
                None,
                EditorSettings::default(),
                window,
                cx,
            )
        });
        window
            .update(cx, |editor, _, cx| {
                editor.set_find_query(Some(query("foo")), cx);
            })
            .unwrap();
        cx.run_until_parked();
        let summary = window
            .update(cx, |editor, _, _| editor.find_summary())
            .unwrap();
        assert_eq!(summary.total, 3);
        assert_eq!(summary.current, None);

        let step = |cx: &mut TestAppContext, forward| {
            window
                .update(cx, |editor, _, cx| {
                    editor.find_next(forward, cx);
                    editor.find_summary().current
                })
                .unwrap()
        };
        assert_eq!(step(cx, true), Some(0));
        assert_eq!(step(cx, true), Some(1));
        assert_eq!(step(cx, true), Some(2));
        assert_eq!(step(cx, true), Some(0), "wraps around");
        assert_eq!(step(cx, false), Some(2), "and back");

        window
            .update(cx, |editor, _, cx| editor.replace_next("x", cx))
            .unwrap();
        let text = window
            .update(cx, |editor, _, _| editor.document().buffer().to_text())
            .unwrap();
        assert_eq!(text, "foo bar foo\nx");
        let replaced = window
            .update(cx, |editor, _, cx| editor.replace_all("qux", cx))
            .unwrap();
        assert_eq!(replaced, 2);
        let text = window
            .update(cx, |editor, _, _| editor.document().buffer().to_text())
            .unwrap();
        assert_eq!(text, "qux bar qux\nx");

        window
            .update(cx, |editor, _, cx| editor.set_find_query(None, cx))
            .unwrap();
        let summary = window
            .update(cx, |editor, _, _| editor.find_summary())
            .unwrap();
        assert_eq!(summary.total, 0);
    }
}
