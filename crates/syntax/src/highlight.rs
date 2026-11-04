//! Highlighting a byte range: which highlight colors each piece of text.

use std::ops::Range;

use ropey::Rope;
use streaming_iterator::StreamingIterator as _;
use tree_sitter::{Node, QueryCursor};

use crate::{Grammar, SyntaxTree};

/// A capture of the highlight query; [`Grammar::highlight_names`] names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HighlightId(pub u32);

/// Text colored by one highlight.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightSpan {
    /// Byte range in the text.
    pub range: Range<usize>,
    /// The highlight.
    pub highlight: HighlightId,
}

/// The longest range highlighted in one call; longer requests are cut, so
/// the cost stays bounded (callers ask for what is visible).
pub const MAX_HIGHLIGHT_RANGE: usize = 1024 * 1024;

/// Marks a byte no capture covers.
const NONE: u32 = u32::MAX;

/// The highlighted spans of `range` (bytes, on character boundaries),
/// sorted and disjoint; text without a highlight has no span. Where
/// captures overlap, the earlier pattern of the query wins, so a query can
/// color an attribute as a whole, strings inside it included.
///
/// Costs a query over the nodes in `range` and a pass over its bytes.
pub fn highlights(
    grammar: &Grammar,
    tree: &SyntaxTree,
    rope: &Rope,
    range: Range<usize>,
) -> Vec<HighlightSpan> {
    let end = range
        .end
        .min(rope.len_bytes())
        .min(range.start.saturating_add(MAX_HIGHLIGHT_RANGE));
    let start = range.start.min(end);
    if start == end {
        return Vec::new();
    }
    // Per byte of the range: the winning (pattern, capture), packed so a
    // smaller value means an earlier pattern.
    let mut owners = vec![NONE; end - start];
    let names = grammar.highlight_names();
    let mut cursor = QueryCursor::new();
    cursor.set_byte_range(start..end);
    let mut matches = cursor.matches(&grammar.highlights, tree.tree.root_node(), node_text(rope));
    while let Some(found) = matches.next() {
        let Ok(pattern) = u32::try_from(found.pattern_index) else {
            continue;
        };
        for capture in found.captures() {
            if names
                .get(capture.index as usize)
                .is_none_or(Option::is_none)
            {
                continue;
            }
            let owner = pack(pattern, capture.index);
            let node = capture.node.byte_range();
            let from = node.start.clamp(start, end) - start;
            let to = node.end.clamp(start, end) - start;
            for slot in &mut owners[from..to] {
                *slot = (*slot).min(owner);
            }
        }
    }
    spans(&owners, start)
}

/// Text of a node for query predicates, read from the rope's chunks.
fn node_text<'rope>(rope: &'rope Rope) -> impl FnMut(Node<'_>) -> ropey::iter::Chunks<'rope> {
    move |node| rope.byte_slice(node.byte_range()).chunks()
}

fn pack(pattern: u32, capture: u32) -> u32 {
    // Patterns and captures of a query stay far below 2^16.
    (pattern.min(0xFFFE) << 16) | capture.min(0xFFFF)
}

/// Runs of bytes with the same winning capture.
fn spans(owners: &[u32], start: usize) -> Vec<HighlightSpan> {
    let mut spans: Vec<HighlightSpan> = Vec::new();
    let mut offset = 0;
    while offset < owners.len() {
        let owner = owners[offset];
        let run = owners[offset..]
            .iter()
            .position(|other| *other != owner)
            .unwrap_or(owners.len() - offset);
        if owner != NONE {
            let highlight = HighlightId(owner & 0xFFFF);
            let range = start + offset..start + offset + run;
            // Different patterns with the same capture make one span.
            match spans.last_mut() {
                Some(last) if last.highlight == highlight && last.range.end == range.start => {
                    last.range.end = range.end;
                }
                _ => spans.push(HighlightSpan { range, highlight }),
            }
        }
        offset += run;
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn earlier_patterns_win_and_equal_captures_merge() {
        let none = NONE;
        let owners = [none, pack(1, 2), pack(1, 2), pack(0, 2), pack(0, 3), none];
        assert_eq!(
            spans(&owners, 10),
            vec![
                HighlightSpan {
                    range: 11..14,
                    highlight: HighlightId(2)
                },
                HighlightSpan {
                    range: 14..15,
                    highlight: HighlightId(3)
                },
            ]
        );
        assert!(pack(0, 9) < pack(1, 0));
    }
}
