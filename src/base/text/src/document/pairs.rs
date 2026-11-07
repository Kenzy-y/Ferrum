//! Auto-closing pairs: typing an opening bracket or quote also inserts its
//! closing one, typing the closing one before it steps over it, Backspace
//! between an empty pair deletes both, and typing an opening character
//! over a selection wraps it. Every cursor is handled, in one undo step.

use std::ops::Range;

use crate::{EditKind, Selection};

use super::{Document, Indent, Placement};

/// An opening and a closing character that go together.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutoPair {
    /// Typed by the user (`(`, `"`).
    pub open: char,
    /// Inserted after the cursor (`)`, `"`).
    pub close: char,
}

impl AutoPair {
    /// A pair of `open` and `close`.
    pub const fn new(open: char, close: char) -> Self {
        Self { open, close }
    }

    /// Quotes close themselves, so they do not open after a word
    /// character (`don't`).
    fn symmetric(self) -> bool {
        self.open == self.close
    }
}

/// Brackets and quotes most languages pair.
pub const DEFAULT_PAIRS: &[AutoPair] = &[
    AutoPair::new('(', ')'),
    AutoPair::new('[', ']'),
    AutoPair::new('{', '}'),
    AutoPair::new('"', '"'),
    AutoPair::new('\'', '\''),
];

impl Document {
    /// Types `typed` at every cursor with the auto-closing `pairs`. Falls
    /// back to [`Self::insert`] when `typed` is not a pair character.
    pub fn insert_paired(&mut self, typed: char, pairs: &[AutoPair]) {
        let opening = pairs.iter().copied().find(|pair| pair.open == typed);
        let closing = pairs.iter().any(|pair| pair.close == typed);
        if opening.is_none() && !closing {
            self.insert(typed.encode_utf8(&mut [0; 4]));
            return;
        }
        // Typing the closing character right before it steps over it.
        if closing && self.all_cursors(|document, head| document.char_after(head) == Some(typed)) {
            let width = typed.len_utf8();
            let moved = self
                .selections
                .map(|selection| Selection::cursor(selection.head + width));
            // Only the cursors move: no edit, no new version.
            self.history.break_group();
            self.selections = moved;
            return;
        }
        let Some(pair) = opening else {
            self.insert(typed.encode_utf8(&mut [0; 4]));
            return;
        };
        if self
            .selections
            .iter()
            .all(|selection| !selection.is_empty())
        {
            self.wrap_selections(pair);
            return;
        }
        if !self.all_cursors(|document, head| document.may_close(pair, head)) {
            self.insert(typed.encode_utf8(&mut [0; 4]));
            return;
        }
        let text = format!("{}{}", pair.open, pair.close);
        let open_width = pair.open.len_utf8();
        let replacements = self
            .selections
            .iter()
            .map(|selection| (selection.head..selection.head, text.clone()))
            .collect();
        self.apply(
            replacements,
            EditKind::Typing,
            Some(typed),
            &Placement::PerEdit(&|edit| Selection::cursor(edit.new_range.start + open_width)),
        );
    }

    /// Deletes the character before every cursor; between an empty pair
    /// both characters go.
    pub fn delete_backward_paired(&mut self, pairs: &[AutoPair]) {
        let empty_pair = |document: &Self, head: usize| {
            let (Some(before), Some(after)) =
                (document.char_before(head), document.char_after(head))
            else {
                return false;
            };
            pairs
                .iter()
                .any(|pair| pair.open == before && pair.close == after)
        };
        if !self.all_cursors(empty_pair) {
            self.delete_backward();
            return;
        }
        let replacements: Vec<(Range<usize>, String)> = self
            .selections
            .iter()
            .map(|selection| {
                let head = selection.head;
                let before = self.char_before(head).map_or(0, char::len_utf8);
                let after = self.char_after(head).map_or(0, char::len_utf8);
                (head - before..head + after, String::new())
            })
            .collect();
        self.apply(
            replacements,
            EditKind::Deletion,
            None,
            &Placement::PerEdit(&|edit| Selection::cursor(edit.new_range.start)),
        );
    }

    /// Enter at every cursor, keeping the indentation of the line. After
    /// an opening bracket of `pairs` the new line gets one more level of
    /// `indent`; between an opening bracket and its closing one the closing
    /// one moves to a third line at the old indentation and the cursor
    /// stays on the indented middle line.
    pub fn newline_paired(&mut self, pairs: &[AutoPair], indent: Indent) {
        let ending = self.buffer.line_ending().as_str();
        let unit = if indent.spaces {
            " ".repeat(indent.tab_size.max(1))
        } else {
            "\t".to_owned()
        };
        let replacements = self
            .selections
            .iter()
            .map(|selection| {
                let row = self.buffer.offset_to_point(selection.start()).row;
                let indentation: String = self
                    .buffer
                    .line(row)
                    .chars()
                    .take_while(|character| *character == ' ' || *character == '\t')
                    .collect();
                let before = self.char_before(selection.start());
                let after = self.char_after(selection.end());
                let bracket = pairs
                    .iter()
                    .find(|pair| !pair.symmetric() && Some(pair.open) == before);
                let text = match bracket {
                    Some(pair) if Some(pair.close) == after => {
                        format!("{ending}{indentation}{unit}{ending}{indentation}")
                    }
                    Some(_) => format!("{ending}{indentation}{unit}"),
                    None => format!("{ending}{indentation}"),
                };
                (selection.range(), text)
            })
            .collect();
        self.apply(
            replacements,
            EditKind::Other,
            None,
            &Placement::PerEdit(&|edit| {
                // Three lines: the cursor ends the middle one.
                let middle_end = (edit.new_text.matches('\n').count() == 2)
                    .then(|| edit.new_text.rfind(ending))
                    .flatten();
                Selection::cursor(
                    middle_end.map_or(edit.new_range.end, |end| edit.new_range.start + end),
                )
            }),
        );
    }

    /// Puts `pair` around every selection, which stays selected inside.
    fn wrap_selections(&mut self, pair: AutoPair) {
        let replacements = self
            .selections
            .iter()
            .map(|selection| {
                let range = selection.range();
                let inner = self.buffer.slice(range.clone());
                (range, format!("{}{inner}{}", pair.open, pair.close))
            })
            .collect();
        let (open, close) = (pair.open.len_utf8(), pair.close.len_utf8());
        self.apply(
            replacements,
            EditKind::Other,
            None,
            &Placement::PerEdit(&|edit| {
                Selection::new(edit.new_range.start + open, edit.new_range.end - close)
            }),
        );
    }

    /// Whether every selection is a cursor for which `check` holds.
    fn all_cursors(&self, check: impl Fn(&Self, usize) -> bool) -> bool {
        self.selections
            .iter()
            .all(|selection| selection.is_empty() && check(self, selection.head))
    }

    /// The closing character is added only where it does not split a
    /// word: before the end of the line, whitespace or a closing
    /// character; quotes also not right after a word character.
    fn may_close(&self, pair: AutoPair, head: usize) -> bool {
        let next_ok = self
            .char_after(head)
            .is_none_or(|next| next.is_whitespace() || matches!(next, ')' | ']' | '}' | ',' | ';'));
        let previous_ok = !pair.symmetric()
            || self
                .char_before(head)
                .is_none_or(|previous| !(previous.is_alphanumeric() || previous == pair.open));
        next_ok && previous_ok
    }

    fn char_after(&self, offset: usize) -> Option<char> {
        let rope = self.buffer.rope();
        (offset < rope.len_bytes()).then(|| rope.char(rope.byte_to_char(offset)))
    }

    fn char_before(&self, offset: usize) -> Option<char> {
        let rope = self.buffer.rope();
        let index = rope.byte_to_char(offset);
        (index > 0).then(|| rope.char(index - 1))
    }
}

/// The selections of `document` (for tests).
#[cfg(test)]
fn heads(document: &Document) -> Vec<usize> {
    document
        .selections()
        .iter()
        .map(|selection| selection.head)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Buffer, Selections};

    fn document(text: &str, cursor: usize) -> Document {
        let mut document = Document::new(Buffer::from_text(text));
        document.set_selections(&Selections::single(Selection::cursor(cursor)));
        document
    }

    fn text(document: &Document) -> String {
        document.buffer().slice(0..document.buffer().len())
    }

    #[test]
    fn opening_closes_and_closing_steps_over() {
        let mut document = document("", 0);
        document.insert_paired('(', DEFAULT_PAIRS);
        assert_eq!(
            (text(&document), heads(&document)),
            ("()".to_owned(), vec![1])
        );
        document.insert_paired(')', DEFAULT_PAIRS);
        assert_eq!(
            (text(&document), heads(&document)),
            ("()".to_owned(), vec![2])
        );
    }

    #[test]
    fn backspace_deletes_an_empty_pair_and_undo_restores_it() {
        let mut document = document("", 0);
        document.insert_paired('[', DEFAULT_PAIRS);
        document.delete_backward_paired(DEFAULT_PAIRS);
        assert_eq!(text(&document), "");
        document.undo();
        assert_eq!(text(&document), "[]");
    }

    #[test]
    fn no_pair_before_a_word_or_a_quote_after_a_letter() {
        let mut before_word = document("x", 0);
        before_word.insert_paired('(', DEFAULT_PAIRS);
        assert_eq!(text(&before_word), "(x");

        let mut apostrophe = document("don", 3);
        apostrophe.insert_paired('\'', DEFAULT_PAIRS);
        assert_eq!(text(&apostrophe), "don'");
    }

    #[test]
    fn wraps_a_selection() {
        let mut document = document("value", 0);
        document.set_selections(&Selections::single(Selection::new(0, 5)));
        document.insert_paired('"', DEFAULT_PAIRS);
        assert_eq!(text(&document), "\"value\"");
        let selection = document.selections().primary();
        assert_eq!(selection.range(), 1..6);
    }

    #[test]
    fn every_cursor_gets_its_pair() {
        let mut document = document("a\nb\n", 1);
        document.add_selection(Selection::cursor(3));
        document.insert_paired('{', DEFAULT_PAIRS);
        assert_eq!(text(&document), "a{}\nb{}\n");
        assert_eq!(heads(&document), vec![2, 6]);
    }

    #[test]
    fn enter_between_brackets_opens_an_indented_middle_line() {
        let mut document = document("    fn f() {}", 12);
        document.newline_paired(DEFAULT_PAIRS, Indent::default());
        assert_eq!(text(&document), "    fn f() {\n        \n    }");
        assert_eq!(heads(&document), vec![21]);
        document.undo();
        assert_eq!(text(&document), "    fn f() {}");
    }

    #[test]
    fn enter_between_brackets_keeps_crlf() {
        let mut document = document("{}\r\n", 1);
        document.newline_paired(DEFAULT_PAIRS, Indent::default());
        assert_eq!(text(&document), "{\r\n    \r\n}\r\n");
        assert_eq!(heads(&document), vec![7]);
    }

    #[test]
    fn enter_after_an_opening_bracket_indents() {
        let mut document = document("let v = [", 9);
        document.newline_paired(DEFAULT_PAIRS, Indent::default());
        assert_eq!(text(&document), "let v = [\n    ");
    }

    #[test]
    fn enter_between_quotes_is_a_plain_newline() {
        let mut document = document("  \"\"", 3);
        document.newline_paired(DEFAULT_PAIRS, Indent::default());
        assert_eq!(text(&document), "  \"\n  \"");
    }

    #[test]
    fn enter_at_every_cursor_between_brackets() {
        let mut document = document("{}\n{}", 1);
        document.add_selection(Selection::cursor(4));
        document.newline_paired(
            DEFAULT_PAIRS,
            Indent {
                tab_size: 4,
                spaces: false,
            },
        );
        assert_eq!(text(&document), "{\n\t\n}\n{\n\t\n}");
        assert_eq!(heads(&document), vec![3, 9]);
    }
}
