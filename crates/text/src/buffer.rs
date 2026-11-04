//! The text itself.

use std::ops::Range;

use ropey::Rope;

/// How lines end in the file. The buffer always holds `\n`; the original
/// ending is restored when saving.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineEnding {
    /// `\n` (Unix, macOS).
    #[default]
    Lf,
    /// `\r\n` (Windows).
    CrLf,
}

impl LineEnding {
    /// The characters of the ending.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
        }
    }
}

/// A position as a row and a byte column within the row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Point {
    /// Zero-based line number.
    pub row: usize,
    /// Byte offset from the start of the line.
    pub column: usize,
}

impl Point {
    /// Creates a point.
    pub fn new(row: usize, column: usize) -> Self {
        Self { row, column }
    }
}

/// Text with its line ending and byte order mark.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Buffer {
    rope: Rope,
    line_ending: LineEnding,
    bom: bool,
}

/// UTF-8 byte order mark.
const BOM: &str = "\u{feff}";

impl Buffer {
    /// An empty buffer.
    pub fn new() -> Self {
        Self::default()
    }

    /// A buffer from file contents: the BOM is remembered and removed, line
    /// endings become `\n` and the dominant ending is remembered.
    pub fn from_text(text: &str) -> Self {
        let (text, bom) = match text.strip_prefix(BOM) {
            Some(rest) => (rest, true),
            None => (text, false),
        };
        let crlf = text.matches("\r\n").count();
        let lf = text.matches('\n').count() - crlf;
        let (rope, line_ending) = if crlf > lf {
            (
                Rope::from_str(&text.replace("\r\n", "\n")),
                LineEnding::CrLf,
            )
        } else if crlf > 0 {
            // Mixed endings: the dominant one wins when saving.
            (Rope::from_str(&text.replace("\r\n", "\n")), LineEnding::Lf)
        } else {
            (Rope::from_str(text), LineEnding::Lf)
        };
        Self {
            rope,
            line_ending,
            bom,
        }
    }

    /// The file contents: original line ending and BOM restored.
    pub fn to_text(&self) -> String {
        let mut text = String::with_capacity(self.rope.len_bytes() + BOM.len());
        if self.bom {
            text.push_str(BOM);
        }
        for chunk in self.rope.chunks() {
            match self.line_ending {
                LineEnding::Lf => text.push_str(chunk),
                LineEnding::CrLf => text.push_str(&chunk.replace('\n', "\r\n")),
            }
        }
        text
    }

    /// The rope.
    pub fn rope(&self) -> &Rope {
        &self.rope
    }

    /// The line ending used when saving.
    pub fn line_ending(&self) -> LineEnding {
        self.line_ending
    }

    /// Changes the line ending used when saving.
    pub fn set_line_ending(&mut self, line_ending: LineEnding) {
        self.line_ending = line_ending;
    }

    /// Whether the file had a UTF-8 BOM.
    pub fn has_bom(&self) -> bool {
        self.bom
    }

    /// Length in bytes.
    pub fn len(&self) -> usize {
        self.rope.len_bytes()
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.rope.len_bytes() == 0
    }

    /// Number of lines (an empty buffer has one).
    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    /// Byte range of line `row` without its line break; clamped to the last
    /// line.
    pub fn line_range(&self, row: usize) -> Range<usize> {
        let row = row.min(self.line_count() - 1);
        let start = self.rope.line_to_byte(row);
        let end = if row + 1 < self.line_count() {
            // Ends before the break; the buffer only holds `\n` (and lone
            // `\r` or Unicode separators, which ropey also counts).
            let next = self.rope.line_to_byte(row + 1);
            let line = self.rope.byte_slice(start..next);
            let trailing = line
                .chars_at(line.len_chars())
                .reversed()
                .take_while(|character| is_line_break(*character))
                .map(char::len_utf8)
                .sum::<usize>();
            next - trailing
        } else {
            self.rope.len_bytes()
        };
        start..end
    }

    /// Line `row` without its line break.
    pub fn line(&self, row: usize) -> ropey::RopeSlice<'_> {
        self.rope.byte_slice(self.line_range(row))
    }

    /// The row and column of `offset` (clamped to the end).
    pub fn offset_to_point(&self, offset: usize) -> Point {
        let offset = self.clamp(offset);
        let row = self.rope.byte_to_line(offset);
        Point::new(row, offset - self.rope.line_to_byte(row))
    }

    /// The offset of `point`, clamped to the line and to the buffer.
    pub fn point_to_offset(&self, point: Point) -> usize {
        let range = self.line_range(point.row);
        let offset = (range.start + point.column).min(range.end);
        self.floor_char_boundary(offset)
    }

    /// `offset` clamped to the buffer and moved back to a character
    /// boundary.
    pub fn clamp(&self, offset: usize) -> usize {
        self.floor_char_boundary(offset.min(self.len()))
    }

    fn floor_char_boundary(&self, offset: usize) -> usize {
        let char_index = self.rope.byte_to_char(offset);
        self.rope.char_to_byte(char_index)
    }

    /// The text in `range` (bytes).
    pub fn slice(&self, range: Range<usize>) -> String {
        self.rope.byte_slice(range).to_string()
    }

    /// The column of `offset` in characters (for vertical movement).
    pub fn char_column(&self, offset: usize) -> usize {
        let offset = self.clamp(offset);
        let row = self.rope.byte_to_line(offset);
        self.rope.byte_to_char(offset) - self.rope.line_to_char(row)
    }

    /// The offset at `char_column` characters into line `row`, clamped to
    /// the end of the line.
    pub fn offset_at_char_column(&self, row: usize, char_column: usize) -> usize {
        let range = self.line_range(row);
        let line_start_char = self.rope.byte_to_char(range.start);
        let line_end_char = self.rope.byte_to_char(range.end);
        self.rope
            .char_to_byte((line_start_char + char_column).min(line_end_char))
    }

    /// Replaces `range` (bytes, on character boundaries) with `text`.
    pub(crate) fn replace(&mut self, range: Range<usize>, text: &str) {
        let start = self.rope.byte_to_char(range.start);
        let end = self.rope.byte_to_char(range.end);
        if start < end {
            self.rope.remove(start..end);
        }
        if !text.is_empty() {
            self.rope.insert(start, text);
        }
    }
}

/// Characters ropey treats as line breaks.
fn is_line_break(character: char) -> bool {
    matches!(
        character,
        '\n' | '\r' | '\u{000B}' | '\u{000C}' | '\u{0085}' | '\u{2028}' | '\u{2029}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_line_endings_and_bom_for_saving() {
        let windows = Buffer::from_text("\u{feff}one\r\ntwo\r\n");
        assert_eq!(windows.line_ending(), LineEnding::CrLf);
        assert!(windows.has_bom());
        assert_eq!(windows.rope().to_string(), "one\ntwo\n");
        assert_eq!(windows.to_text(), "\u{feff}one\r\ntwo\r\n");

        let unix = Buffer::from_text("one\ntwo");
        assert_eq!(unix.line_ending(), LineEnding::Lf);
        assert_eq!(unix.to_text(), "one\ntwo");

        let mixed = Buffer::from_text("a\r\nb\nc\nd\n");
        assert_eq!(mixed.line_ending(), LineEnding::Lf, "dominant ending wins");
        assert_eq!(mixed.to_text(), "a\nb\nc\nd\n");
    }

    #[test]
    fn lines_and_points() {
        let buffer = Buffer::from_text("héllo\nworld\n");
        assert_eq!(buffer.line_count(), 3);
        assert_eq!(buffer.line(0).to_string(), "héllo");
        assert_eq!(buffer.line_range(1), 7..12);
        assert_eq!(buffer.line(2).to_string(), "");
        assert_eq!(buffer.offset_to_point(8), Point::new(1, 1));
        assert_eq!(
            buffer.point_to_offset(Point::new(0, 99)),
            6,
            "clamped to the line"
        );
        assert_eq!(
            buffer.point_to_offset(Point::new(0, 2)),
            1,
            "inside é: back to its start"
        );
        assert_eq!(buffer.char_column(6), 5);
        assert_eq!(buffer.offset_at_char_column(1, 3), 10);
        assert_eq!(buffer.offset_at_char_column(1, 99), 12);
    }

    #[test]
    fn replaces_ranges() {
        let mut buffer = Buffer::from_text("hello world");
        buffer.replace(0..5, "goodbye");
        buffer.replace(8..13, "");
        assert_eq!(buffer.rope().to_string(), "goodbye ");
    }
}
