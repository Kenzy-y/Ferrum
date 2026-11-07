//! A line as it is drawn: tabs expanded to spaces and inlay hints put in,
//! with offset mapping in both directions. Pure logic, tested without a
//! window.

use std::ops::Range;

/// Drawn text that differs from the source: a tab's spaces, or inserted
/// text that is not in the source (an inlay hint).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Piece {
    /// Byte offset in the source: of the tab, or where the text goes.
    source: usize,
    /// Source bytes it stands for: 1 for a tab, 0 for inserted text.
    source_len: usize,
    /// Byte offset of its first drawn byte.
    display: usize,
    /// Drawn bytes.
    display_len: usize,
}

impl Piece {
    fn display_end(self) -> usize {
        self.display + self.display_len
    }

    fn inserted(self) -> bool {
        self.source_len == 0
    }
}

/// The drawn text of (part of) a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DisplayLine {
    /// The text to shape: no tabs, no line breaks.
    pub(crate) text: String,
    /// Sorted by source offset; inserted text before a tab at the same
    /// offset.
    pieces: Vec<Piece>,
}

impl DisplayLine {
    /// Expands tabs in `source` to the next multiple of `tab_size`;
    /// `start_column` is the visual column where `source` starts (non-zero
    /// for a window into a long line).
    #[cfg(test)]
    pub(crate) fn new(source: &str, tab_size: usize, start_column: usize) -> Self {
        Self::with_inserts(source, tab_size, start_column, &[])
    }

    /// Like [`Self::new`], with `inserts` (source offset, text; sorted by
    /// offset) drawn before the character at their offset. Inserted text
    /// does not move tab stops.
    pub(crate) fn with_inserts(
        source: &str,
        tab_size: usize,
        start_column: usize,
        inserts: &[(usize, &str)],
    ) -> Self {
        let tab_size = tab_size.max(1);
        if inserts.is_empty() && !source.contains('\t') {
            return Self {
                text: source.to_owned(),
                pieces: Vec::new(),
            };
        }
        let extra: usize = inserts.iter().map(|(_, text)| text.len()).sum();
        let mut text = String::with_capacity(source.len() + extra + 8);
        let mut pieces = Vec::new();
        let mut column = start_column;
        let mut next_insert = inserts.iter().peekable();
        let source_end = source.len();
        let mut insert_before = |offset: usize, text: &mut String, pieces: &mut Vec<Piece>| {
            while let Some((_, inserted)) = next_insert.next_if(|(at, _)| *at <= offset) {
                pieces.push(Piece {
                    // Inserts after the last character (`usize::MAX` below)
                    // go at the end of the line.
                    source: offset.min(source_end),
                    source_len: 0,
                    display: text.len(),
                    display_len: inserted.len(),
                });
                text.push_str(inserted);
            }
        };
        for (offset, character) in source.char_indices() {
            insert_before(offset, &mut text, &mut pieces);
            if character == '\t' {
                let width = tab_size - column % tab_size;
                pieces.push(Piece {
                    source: offset,
                    source_len: 1,
                    display: text.len(),
                    display_len: width,
                });
                text.extend(std::iter::repeat_n(' ', width));
                column += width;
            } else {
                text.push(character);
                column += 1;
            }
        }
        insert_before(usize::MAX, &mut text, &mut pieces);
        Self { text, pieces }
    }

    /// The drawn offset of source offset `source`. At an insert, the
    /// offset before the inserted text, so a cursor there stays next to
    /// the source character it follows.
    pub(crate) fn to_display(&self, source: usize) -> usize {
        let before = self.pieces.partition_point(|piece| piece.source < source);
        match before.checked_sub(1).map(|index| self.pieces[index]) {
            // Every byte after a piece is shifted by its drawn bytes.
            Some(piece) => piece.display_end() + (source - piece.source - piece.source_len),
            None => source,
        }
    }

    /// The source offset drawn at `display`; inside a tab's spaces, the
    /// nearer edge of the tab; inside inserted text, its offset.
    pub(crate) fn to_source(&self, display: usize) -> usize {
        let before = self.pieces.partition_point(|piece| piece.display < display);
        let Some(piece) = before.checked_sub(1).map(|index| self.pieces[index]) else {
            return display;
        };
        let end = piece.display_end();
        if display < end {
            if piece.inserted() || (display - piece.display) * 2 < piece.display_len {
                piece.source
            } else {
                piece.source + 1
            }
        } else {
            piece.source + piece.source_len + (display - end)
        }
    }

    /// Drawn ranges of the inserted text, in order.
    pub(crate) fn inserted(&self) -> impl Iterator<Item = Range<usize>> + '_ {
        self.pieces
            .iter()
            .filter(|piece| piece.inserted())
            .map(|piece| piece.display..piece.display_end())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_inserted_at_the_end_maps_to_the_end() {
        // Regression: a hint after the last character got offset
        // `usize::MAX`, and hovering it overflowed.
        let line = DisplayLine::with_inserts("let x = 5;", 4, 0, &[(10, ": i32")]);
        assert_eq!(line.text, "let x = 5;: i32");
        for display in 10..line.text.len() {
            assert_eq!(line.to_source(display), 10, "{display}");
        }
        assert_eq!(line.to_display(10), 10);
    }

    #[test]
    fn text_without_tabs_is_unchanged() {
        let line = DisplayLine::new("let x = 1;", 4, 0);
        assert_eq!(line.text, "let x = 1;");
        assert_eq!(line.to_display(4), 4);
        assert_eq!(line.to_source(4), 4);
    }

    #[test]
    fn tabs_expand_to_the_next_stop() {
        let line = DisplayLine::new("a\tb\t\tc", 4, 0);
        assert_eq!(line.text, "a   b       c");
        assert_eq!(line.to_display(1), 1, "before the first tab");
        assert_eq!(line.to_display(2), 4, "b");
        assert_eq!(line.to_display(5), 12, "c");
        assert_eq!(line.to_source(4), 2);
        assert_eq!(line.to_source(12), 5);
        assert_eq!(line.to_source(2), 1, "left half of a tab");
        assert_eq!(line.to_source(3), 2, "right half of a tab");
    }

    #[test]
    fn a_window_starts_at_its_column() {
        let line = DisplayLine::new("\tx", 4, 2);
        assert_eq!(line.text, "  x");
    }

    #[test]
    fn inserted_text_shifts_what_follows() {
        let line = DisplayLine::with_inserts("let x = 1;", 4, 0, &[(5, ": i32")]);
        assert_eq!(line.text, "let x: i32 = 1;");
        assert_eq!(
            line.to_display(5),
            5,
            "a cursor at the hint stays before it"
        );
        assert_eq!(
            line.to_display(6),
            11,
            "after the space that follows the hint"
        );
        assert_eq!(line.to_source(7), 5, "inside the hint");
        assert_eq!(line.to_source(11), 6);
        assert_eq!(line.inserted().collect::<Vec<_>>(), vec![5..10]);
    }

    #[test]
    fn inserts_and_tabs_together() {
        let line = DisplayLine::with_inserts("\tf(a)", 4, 0, &[(3, "x: "), (5, ";")]);
        assert_eq!(line.text, "    f(x: a);");
        assert_eq!(line.to_display(1), 4, "after the tab");
        assert_eq!(line.to_display(4), 10, "the closing parenthesis");
        assert_eq!(line.to_source(9), 3, "the argument after the insert");
        assert_eq!(line.to_source(11), 5, "inside the insert at the end");
        assert_eq!(line.inserted().collect::<Vec<_>>(), vec![6..9, 11..12]);
    }

    #[test]
    fn an_insert_before_a_tab_comes_first() {
        let line = DisplayLine::with_inserts("a\tb", 4, 0, &[(1, ":")]);
        assert_eq!(line.text, "a:   b", "the tab keeps its stop");
        assert_eq!(line.to_display(1), 1);
        assert_eq!(line.to_display(2), 5);
    }
}
