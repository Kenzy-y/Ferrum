//! Grapheme and word boundaries over a rope, without copying the text.
//!
//! The cursor moves and deletes by grapheme clusters: a family emoji made of
//! several code points, a letter with combining accents or `\r\n` behave as
//! one character.

use ropey::Rope;
use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};

/// The grapheme boundary after `offset` (the end for the last one).
pub fn next_grapheme(rope: &Rope, offset: usize) -> usize {
    let length = rope.len_bytes();
    if offset >= length {
        return length;
    }
    let (mut chunk, mut chunk_start, _, _) = rope.chunk_at_byte(offset);
    let mut cursor = GraphemeCursor::new(offset, length, true);
    loop {
        match cursor.next_boundary(chunk, chunk_start) {
            Ok(Some(boundary)) => return boundary,
            Ok(None) => return length,
            Err(GraphemeIncomplete::NextChunk) => {
                chunk_start += chunk.len();
                chunk = rope.chunk_at_byte(chunk_start).0;
            }
            Err(GraphemeIncomplete::PreContext(needed)) => {
                let context = rope.chunk_at_byte(needed - 1).0;
                cursor.provide_context(context, needed - context.len());
            }
            // Not produced by `next_boundary`; stop at the next character.
            Err(_) => return next_char(rope, offset),
        }
    }
}

/// The grapheme boundary before `offset` (0 for the first one).
pub fn previous_grapheme(rope: &Rope, offset: usize) -> usize {
    let length = rope.len_bytes();
    let offset = offset.min(length);
    if offset == 0 {
        return 0;
    }
    let (mut chunk, mut chunk_start, _, _) = rope.chunk_at_byte(offset);
    let mut cursor = GraphemeCursor::new(offset, length, true);
    loop {
        match cursor.prev_boundary(chunk, chunk_start) {
            Ok(Some(boundary)) => return boundary,
            Ok(None) => return 0,
            Err(GraphemeIncomplete::PrevChunk) => {
                let (previous, previous_start, _, _) = rope.chunk_at_byte(chunk_start - 1);
                chunk = previous;
                chunk_start = previous_start;
            }
            Err(GraphemeIncomplete::PreContext(needed)) => {
                let context = rope.chunk_at_byte(needed - 1).0;
                cursor.provide_context(context, needed - context.len());
            }
            Err(_) => return previous_char(rope, offset),
        }
    }
}

fn next_char(rope: &Rope, offset: usize) -> usize {
    let index = rope.byte_to_char(offset);
    rope.char_to_byte((index + 1).min(rope.len_chars()))
}

fn previous_char(rope: &Rope, offset: usize) -> usize {
    let index = rope.byte_to_char(offset);
    rope.char_to_byte(index.saturating_sub(1))
}

/// Kinds of characters that form words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CharClass {
    Whitespace,
    Word,
    Punctuation,
}

fn class(character: char) -> CharClass {
    if character.is_whitespace() {
        CharClass::Whitespace
    } else if character.is_alphanumeric() || character == '_' {
        CharClass::Word
    } else {
        CharClass::Punctuation
    }
}

/// The end of the next word after `offset`: whitespace is skipped, then a
/// run of word characters or of punctuation.
pub fn next_word_end(rope: &Rope, offset: usize) -> usize {
    let mut index = rope.byte_to_char(offset.min(rope.len_bytes()));
    let mut characters = rope.chars_at(index).peekable();
    while characters
        .next_if(|character| class(*character) == CharClass::Whitespace)
        .is_some()
    {
        index += 1;
    }
    if let Some(first) = characters.next() {
        index += 1;
        let run = class(first);
        while characters
            .next_if(|character| class(*character) == run)
            .is_some()
        {
            index += 1;
        }
    }
    rope.char_to_byte(index)
}

/// The start of the word before `offset`, mirroring [`next_word_end`].
pub fn previous_word_start(rope: &Rope, offset: usize) -> usize {
    let mut index = rope.byte_to_char(offset.min(rope.len_bytes()));
    let mut characters = rope.chars_at(index).reversed().peekable();
    while characters
        .next_if(|character| class(*character) == CharClass::Whitespace)
        .is_some()
    {
        index -= 1;
    }
    if let Some(first) = characters.next() {
        index -= 1;
        let run = class(first);
        while characters
            .next_if(|character| class(*character) == run)
            .is_some()
        {
            index -= 1;
        }
    }
    rope.char_to_byte(index)
}

/// The run of same-class characters around `offset` (a word, punctuation
/// or whitespace), for double-click selection. At the end of a word the
/// word before the offset wins over whitespace after it.
pub fn word_range(rope: &Rope, offset: usize) -> std::ops::Range<usize> {
    let index = rope.byte_to_char(offset.min(rope.len_bytes()));
    let after = rope
        .get_char(index)
        .filter(|character| !is_break(*character));
    let before = index
        .checked_sub(1)
        .and_then(|previous| rope.get_char(previous))
        .filter(|character| !is_break(*character));
    let run = match (before.map(class), after.map(class)) {
        (Some(before), Some(CharClass::Whitespace)) if before != CharClass::Whitespace => before,
        (_, Some(after)) => after,
        (Some(before), None) => before,
        (None, None) => return offset..offset,
    };
    let mut start = index;
    for character in rope.chars_at(index).reversed() {
        if is_break(character) || class(character) != run {
            break;
        }
        start -= 1;
    }
    let mut end = index;
    for character in rope.chars_at(index) {
        if is_break(character) || class(character) != run {
            break;
        }
        end += 1;
    }
    rope.char_to_byte(start)..rope.char_to_byte(end)
}

fn is_break(character: char) -> bool {
    character == '\n' || character == '\r'
}

#[cfg(test)]
mod tests {
    use super::*;

    const FAMILY: &str = "👨\u{200d}👩\u{200d}👧";

    #[test]
    fn emoji_sequences_are_one_grapheme() {
        let text = format!("a{FAMILY}b");
        let rope = Rope::from_str(&text);
        let after_a = 1;
        let after_family = 1 + FAMILY.len();
        assert_eq!(next_grapheme(&rope, after_a), after_family);
        assert_eq!(previous_grapheme(&rope, after_family), after_a);
        assert_eq!(next_grapheme(&rope, after_family), text.len());
        assert_eq!(previous_grapheme(&rope, 0), 0);
        assert_eq!(next_grapheme(&rope, text.len()), text.len());
    }

    #[test]
    fn combining_marks_and_flags_are_one_grapheme() {
        let text = "e\u{301}\u{1f1f7}\u{1f1fa}";
        let rope = Rope::from_str(text);
        assert_eq!(next_grapheme(&rope, 0), 3, "e + combining acute");
        assert_eq!(
            next_grapheme(&rope, 3),
            text.len(),
            "regional indicators pair"
        );
    }

    #[test]
    fn graphemes_across_rope_chunks() {
        // Large enough for ropey to split it into several chunks.
        let text = format!("{}{FAMILY}{}", "x".repeat(5000), "y".repeat(5000));
        let rope = Rope::from_str(&text);
        assert!(rope.chunks().count() > 1);
        assert_eq!(next_grapheme(&rope, 5000), 5000 + FAMILY.len());
        assert_eq!(previous_grapheme(&rope, 5000 + FAMILY.len()), 5000);
    }

    #[test]
    fn word_around_an_offset() {
        let rope = Rope::from_str("let value = 1;\nnext");
        assert_eq!(word_range(&rope, 6), 4..9, "inside");
        assert_eq!(
            word_range(&rope, 9),
            4..9,
            "at the end, the word wins over the space"
        );
        assert_eq!(word_range(&rope, 10), 10..11, "punctuation");
        assert_eq!(word_range(&rope, 14), 13..14, "end of the line stays on it");
        assert_eq!(word_range(&rope, 15), 15..19, "next line");
    }

    #[test]
    fn words() {
        let rope = Rope::from_str("let config = load(&cli);");
        assert_eq!(next_word_end(&rope, 0), 3);
        assert_eq!(next_word_end(&rope, 3), 10);
        assert_eq!(
            next_word_end(&rope, 10),
            12,
            "punctuation is a word of its own"
        );
        assert_eq!(previous_word_start(&rope, 10), 4);
        assert_eq!(previous_word_start(&rope, 4), 0);
        assert_eq!(previous_word_start(&rope, 24), 22, ");");
    }
}
