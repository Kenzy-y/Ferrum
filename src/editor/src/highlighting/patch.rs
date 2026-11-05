//! Patches: highlights of the items around recent edits from a quick parse
//! of just those items. On large files a full reparse takes 100 ms and
//! more; a patch colors the edited text within milliseconds and wins over
//! the full tree inside its range until the full parse catches up. Pure
//! logic over offsets.

use std::ops::Range;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use gpui::Context;
use syntax::{HighlightSpan, SyntaxTree};
use text::TextEdit;

use crate::editor::Editor;

/// Full parses slower than this get patches.
const SLOW_PARSE: Duration = Duration::from_millis(8);

/// Largest text a patch parses; wider edits wait for the full parse.
const MAX_PATCH_BYTES: usize = 64 * 1024;

/// Batches with more edits than this (many cursors) get no patch.
const MAX_PATCH_EDITS: usize = 8;

/// Highlights of `range`, from a parse of just that text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Patch {
    pub(crate) range: Range<usize>,
    pub(crate) spans: Vec<HighlightSpan>,
}

impl Patch {
    /// Moves the patch through `edit`. Text inserted inside a span widens
    /// it; spans whose text is replaced are cut or dropped. The patch
    /// grows over text inserted inside it.
    pub(crate) fn edit(&mut self, edit: &TextEdit) {
        self.range =
            map_offset(self.range.start, edit, true)..map_offset(self.range.end, edit, false);
        self.spans.retain_mut(|span| {
            let start = map_offset(span.range.start, edit, false);
            let end = map_offset(span.range.end, edit, true);
            span.range = start..end;
            start < end
        });
    }

    /// Whether the patch covers part of `range`.
    pub(crate) fn overlaps(&self, range: &Range<usize>) -> bool {
        self.range.start < range.end && range.start < self.range.end
    }

    /// `base` (sorted spans of `range`) with the patch's spans instead
    /// inside the patch's range.
    pub(crate) fn overlay(
        &self,
        base: &[HighlightSpan],
        range: &Range<usize>,
    ) -> Vec<HighlightSpan> {
        let mut spans: Vec<HighlightSpan> = base
            .iter()
            .flat_map(|span| {
                [
                    clip(span, 0, self.range.start),
                    clip(span, self.range.end, usize::MAX),
                ]
            })
            .flatten()
            .collect();
        spans.extend(
            spans_in(&self.spans, range)
                .iter()
                .filter_map(|span| clip(span, self.range.start, self.range.end)),
        );
        spans.sort_by_key(|span| span.range.start);
        spans
    }
}

/// The spans of sorted `spans` that overlap `range`.
pub(crate) fn spans_in(spans: &[HighlightSpan], range: &Range<usize>) -> Vec<HighlightSpan> {
    let first = spans.partition_point(|span| span.range.end <= range.start);
    spans[first..]
        .iter()
        .take_while(|span| span.range.start < range.end)
        .cloned()
        .collect()
}

/// The text the edits of one batch touched, in the coordinates after all of
/// them (edits are listed in the order they were applied).
pub(crate) fn edited_range(edits: &[TextEdit]) -> Option<Range<usize>> {
    let mut range: Option<Range<usize>> = None;
    for edit in edits {
        let moved = range
            .map(|range| map_offset(range.start, edit, true)..map_offset(range.end, edit, false));
        range = Some(match moved {
            Some(moved) => moved.start.min(edit.start)..moved.end.max(edit.new_end),
            None => edit.start..edit.new_end,
        });
    }
    range
}

fn clip(span: &HighlightSpan, from: usize, to: usize) -> Option<HighlightSpan> {
    let range = span.range.start.max(from)..span.range.end.min(to);
    (!range.is_empty()).then_some(HighlightSpan {
        range,
        highlight: span.highlight,
    })
}

/// `offset` moved through `edit`; an offset inside the replaced text goes
/// to the start of the edit (`start_side`) or to the end of the new text.
pub(crate) fn map_offset(offset: usize, edit: &TextEdit, start_side: bool) -> usize {
    if offset <= edit.start {
        offset
    } else if offset >= edit.old_end {
        offset - edit.old_end + edit.new_end
    } else if start_side {
        edit.start
    } else {
        edit.new_end
    }
}

impl Editor {
    /// When full parses of this text are slow, parses just the items
    /// around `edits` in the background, so the edited text is colored
    /// within milliseconds while the full parse runs.
    pub(crate) fn start_patch(&mut self, edits: &[TextEdit], cx: &mut Context<Self>) {
        let rope = self.document.buffer().rope().clone();
        let Some(syntax) = &mut self.syntax else {
            return;
        };
        let (Some(grammar), Some(tree)) = (syntax.grammar.clone(), syntax.tree.clone()) else {
            return;
        };
        if syntax.job.is_none() || syntax.parse_time < SLOW_PARSE || edits.len() > MAX_PATCH_EDITS {
            return;
        }
        let Some(edited) = edited_range(edits) else {
            return;
        };
        // Keep covering earlier edits the full parse has not caught up with.
        let region = syntax.patch.as_ref().map_or(edited.clone(), |patch| {
            edited.start.min(patch.range.start)..edited.end.max(patch.range.end)
        });
        let generation = syntax.generation;
        let timeout = syntax.settings.parse_timeout();
        let task = cx.spawn(async move |editor, cx| {
            let patch = cx
                .background_executor()
                .spawn(async move {
                    let mut range = tree.top_level_span(region);
                    if range.len() > MAX_PATCH_BYTES {
                        range = tree.top_level_span(edited);
                    }
                    if range.len() > MAX_PATCH_BYTES {
                        return None;
                    }
                    let part = SyntaxTree::parse_range(
                        &grammar,
                        &rope,
                        range.clone(),
                        timeout,
                        &AtomicBool::new(false),
                    )
                    .ok()?;
                    let spans = syntax::highlights(&grammar, &part, &rope, range.clone());
                    Some(Patch { range, spans })
                })
                .await;
            let _ = editor.update(cx, |editor, cx| {
                if let Some(syntax) = &mut editor.syntax
                    && syntax.generation == generation
                    && syntax.job.is_some()
                {
                    syntax.patch = patch;
                    syntax.patch_revision += 1;
                    cx.notify();
                }
            });
        });
        syntax.patch_task = Some(task);
    }
}

#[cfg(test)]
mod tests {
    use syntax::HighlightId;
    use text::Point;

    use super::*;

    fn span(range: Range<usize>, id: u32) -> HighlightSpan {
        HighlightSpan {
            range,
            highlight: HighlightId(id),
        }
    }

    fn insert(at: usize, len: usize) -> TextEdit {
        TextEdit {
            start: at,
            old_end: at,
            new_end: at + len,
            start_point: Point::default(),
            old_end_point: Point::default(),
            new_end_point: Point::default(),
            inserted: "x".repeat(len).into(),
        }
    }

    #[test]
    fn patches_follow_edits() {
        let mut patch = Patch {
            range: 10..30,
            spans: vec![span(10..13, 0), span(20..25, 1)],
        };
        // Typing inside the second span widens it over the new text.
        patch.edit(&insert(22, 2));
        assert_eq!(patch.range, 10..32);
        assert_eq!(patch.spans, vec![span(10..13, 0), span(20..27, 1)]);
        // Typing before the patch shifts it.
        patch.edit(&insert(0, 5));
        assert_eq!(patch.range, 15..37);
        assert_eq!(patch.spans[0], span(15..18, 0));
    }

    #[test]
    fn the_patch_wins_inside_its_range() {
        let patch = Patch {
            range: 10..20,
            spans: vec![span(12..15, 7)],
        };
        let base = [span(0..5, 1), span(8..12, 2), span(14..30, 3)];
        assert_eq!(
            patch.overlay(&base, &(0..40)),
            vec![
                span(0..5, 1),
                span(8..10, 2),
                span(12..15, 7),
                span(20..30, 3)
            ]
        );
        assert!(patch.overlaps(&(19..25)) && !patch.overlaps(&(20..25)));
    }

    #[test]
    fn the_edited_range_covers_a_batch_applied_from_the_end() {
        // Two cursors: the later one is edited first.
        let edits = [insert(50, 1), insert(10, 1)];
        assert_eq!(edited_range(&edits), Some(10..52));
        assert_eq!(edited_range(&[]), None);
    }
}
