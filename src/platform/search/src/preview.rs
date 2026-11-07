//! The part of a matching line shown in the results.

use std::ops::Range;

/// Marks text cut from the line.
const ELLIPSIS: &str = "…";

/// Characters kept before the first match when the text before it is cut:
/// the sidebar shows about 35 characters, so the match must come early.
const LEAD_CHARS: usize = 10;

/// A matching line as shown in the results: leading indentation removed,
/// cut around the first match when it is long.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preview {
    /// The text.
    pub text: String,
    /// Byte ranges of the matches in `text`.
    pub highlights: Vec<Range<usize>>,
}

impl Preview {
    /// The preview of `line` with matches at `ranges` (bytes of `line`),
    /// at most `max_chars` characters long.
    pub(crate) fn new(line: &str, ranges: &[Range<usize>], max_chars: usize) -> Self {
        let indent = line.len() - line.trim_start().len();
        let first = ranges.first().map_or(indent, |range| range.start);
        let mut start = indent.min(first);
        // Keep a little text before the first match when cutting.
        let lead = LEAD_CHARS.min(max_chars / 4);
        if line[start..first].chars().count() > lead {
            start = nth_char_back(line, first, lead);
        }
        let end = nth_char_forward(line, start, max_chars);
        let mut text = String::with_capacity(end - start + 2 * ELLIPSIS.len());
        let shift = if start > indent {
            text.push_str(ELLIPSIS);
            ELLIPSIS.len()
        } else {
            0
        };
        text.push_str(&line[start..end]);
        if end < line.len() {
            text.push_str(ELLIPSIS);
        }
        let highlights = ranges
            .iter()
            .filter(|range| range.end > start && range.start < end)
            .map(|range| {
                (range.start.max(start) - start + shift)..(range.end.min(end) - start + shift)
            })
            .collect();
        Self { text, highlights }
    }
}

/// Byte offset `count` characters after `from`, or the end of `text`.
fn nth_char_forward(text: &str, from: usize, count: usize) -> usize {
    text[from..]
        .char_indices()
        .nth(count)
        .map_or(text.len(), |(offset, _)| from + offset)
}

/// Byte offset `count` characters before `to`.
fn nth_char_back(text: &str, to: usize, count: usize) -> usize {
    if count == 0 {
        return to;
    }
    text[..to]
        .char_indices()
        .rev()
        .nth(count - 1)
        .map_or(0, |(offset, _)| offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The preview of `line` with one match at `range`.
    fn one(line: &str, range: Range<usize>, max_chars: usize) -> Preview {
        Preview::new(line, std::slice::from_ref(&range), max_chars)
    }

    #[test]
    fn indentation_is_dropped() {
        let preview = one("    let config = 1;", 8..14, 100);
        assert_eq!(preview.text, "let config = 1;");
        assert_eq!(preview.highlights, vec![4..10]);
    }

    #[test]
    fn long_lines_are_cut_around_the_first_match() {
        let line = format!("{}config{}", "a".repeat(100), "b".repeat(100));
        let preview = one(&line, 100..106, 40);
        assert!(preview.text.starts_with(ELLIPSIS));
        assert!(preview.text.ends_with(ELLIPSIS));
        let highlight = preview.highlights[0].clone();
        assert_eq!(&preview.text[highlight], "config");
        assert_eq!(preview.text.chars().count(), 40 + 2);
    }

    #[test]
    fn a_match_far_into_a_line_stays_near_the_start() {
        let line = format!("{}config rest", "word ".repeat(30));
        let preview = one(&line, 150..156, 160);
        let highlight = preview.highlights[0].clone();
        assert_eq!(&preview.text[highlight.clone()], "config");
        assert!(preview.text[..highlight.start].chars().count() <= LEAD_CHARS + 1);
    }

    #[test]
    fn cutting_keeps_characters_whole() {
        let line = format!("{}ключ", "ж".repeat(50));
        let start = "ж".len() * 50;
        let preview = one(&line, start..line.len(), 20);
        let highlight = preview.highlights[0].clone();
        assert_eq!(&preview.text[highlight], "ключ");
    }
}
