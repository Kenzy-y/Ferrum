//! The text itself.

use std::ops::Range;

use ropey::Rope;

/// A kind of line break.
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

/// Line endings of a buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEndings {
    /// All lines end the same way (or there is a single line).
    Uniform(LineEnding),
    /// Both kinds occur; new lines get the dominant one.
    Mixed {
        /// The more frequent kind.
        dominant: LineEnding,
    },
}

/// Counts of each kind of line break.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct EndingCounts {
    crlf: usize,
    lf: usize,
}

impl EndingCounts {
    fn of(text: &str) -> Self {
        let crlf = text.matches("\r\n").count();
        Self {
            crlf,
            lf: text.matches('\n').count() - crlf,
        }
    }

    /// Counts in a rope slice without copying it.
    fn of_slice(slice: ropey::RopeSlice<'_>) -> Self {
        let mut counts = Self::default();
        let mut previous_was_cr = false;
        for byte in slice.bytes() {
            if byte == b'\n' {
                if previous_was_cr {
                    counts.crlf += 1;
                } else {
                    counts.lf += 1;
                }
            }
            previous_was_cr = byte == b'\r';
        }
        counts
    }

    fn add(&mut self, other: Self) {
        self.crlf += other.crlf;
        self.lf += other.lf;
    }

    fn remove(&mut self, other: Self) {
        self.crlf -= other.crlf;
        self.lf -= other.lf;
    }

    fn dominant(self) -> LineEnding {
        if self.crlf > self.lf {
            LineEnding::CrLf
        } else {
            LineEnding::Lf
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

/// Text with its byte order mark. The text is kept exactly as in the file,
/// line breaks included, so lines nobody edits are saved unchanged.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Buffer {
    rope: Rope,
    endings: EndingCounts,
    /// Ending for new lines when the file has none yet.
    default_ending: LineEnding,
    bom: bool,
}

/// UTF-8 byte order mark.
const BOM: &str = "\u{feff}";

impl Buffer {
    /// An empty buffer.
    pub fn new() -> Self {
        Self::default()
    }

    /// A buffer from file contents. The BOM is remembered and removed;
    /// line breaks are kept as they are.
    pub fn from_text(text: &str) -> Self {
        let (text, bom) = match text.strip_prefix(BOM) {
            Some(rest) => (rest, true),
            None => (text, false),
        };
        Self {
            rope: Rope::from_str(text),
            endings: EndingCounts::of(text),
            default_ending: LineEnding::default(),
            bom,
        }
    }

    /// The file contents: the text with its BOM.
    pub fn to_text(&self) -> String {
        let mut text = String::with_capacity(self.rope.len_bytes() + BOM.len());
        if self.bom {
            text.push_str(BOM);
        }
        for chunk in self.rope.chunks() {
            text.push_str(chunk);
        }
        text
    }

    /// The rope.
    pub fn rope(&self) -> &Rope {
        &self.rope
    }

    /// The ending new lines get: the dominant one in the file, or the
    /// default for files without line breaks.
    pub fn line_ending(&self) -> LineEnding {
        if self.endings.crlf == 0 && self.endings.lf == 0 {
            self.default_ending
        } else {
            self.endings.dominant()
        }
    }

    /// Whether the line endings are uniform or mixed.
    pub fn line_endings(&self) -> LineEndings {
        if self.endings.crlf > 0 && self.endings.lf > 0 {
            LineEndings::Mixed {
                dominant: self.endings.dominant(),
            }
        } else {
            LineEndings::Uniform(self.line_ending())
        }
    }

    /// The ending for new lines in a file without line breaks yet (new
    /// files); files with breaks keep using their dominant one.
    pub fn set_default_line_ending(&mut self, line_ending: LineEnding) {
        self.default_ending = line_ending;
    }

    /// `text` with every line break turned into the buffer's ending.
    pub fn with_line_ending(&self, text: &str) -> String {
        let normalized = text.replace("\r\n", "\n");
        match self.line_ending() {
            LineEnding::Lf => normalized,
            LineEnding::CrLf => normalized.replace('\n', "\r\n"),
        }
    }

    /// The byte range of the line break that ends line `row` (empty for the
    /// last line).
    pub fn line_break_range(&self, row: usize) -> Range<usize> {
        let end = self.line_range(row).end;
        let next = if row + 1 < self.line_count() {
            self.rope.line_to_byte(row + 1)
        } else {
            end
        };
        end..next
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
    /// Line ending counts are updated from the removed and inserted text
    /// only, so the cost is that of the edit, not of the document. A `\r`
    /// right before the range or a `\n` right after it can pair up with the
    /// edges, so one byte of context on each side is counted too.
    pub(crate) fn replace(&mut self, range: Range<usize>, text: &str) {
        let context_start = self.floor_char_boundary(range.start.saturating_sub(1));
        let context_end = (range.end + 1).min(self.len());
        let context_end = self.clamp_up(context_end);
        self.endings.remove(EndingCounts::of_slice(
            self.rope.byte_slice(context_start..context_end),
        ));
        let start = self.rope.byte_to_char(range.start);
        let end = self.rope.byte_to_char(range.end);
        if start < end {
            self.rope.remove(start..end);
        }
        if !text.is_empty() {
            self.rope.insert(start, text);
        }
        let after_end = context_end - range.len() + text.len();
        self.endings.add(EndingCounts::of_slice(
            self.rope.byte_slice(context_start..after_end),
        ));
    }

    /// `offset` moved forward to a character boundary.
    fn clamp_up(&self, offset: usize) -> usize {
        let floor = self.floor_char_boundary(offset);
        if floor == offset {
            offset
        } else {
            let index = self.rope.byte_to_char(offset);
            self.rope.char_to_byte(index + 1)
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
        assert_eq!(windows.line(0).to_string(), "one");
        assert_eq!(windows.line_count(), 3);
        assert_eq!(windows.to_text(), "\u{feff}one\r\ntwo\r\n");

        let unix = Buffer::from_text("one\ntwo");
        assert_eq!(unix.line_endings(), LineEndings::Uniform(LineEnding::Lf));
        assert_eq!(unix.to_text(), "one\ntwo");
    }

    #[test]
    fn mixed_endings_are_saved_as_they_are() {
        let mixed = Buffer::from_text("a\r\nb\nc\nd\n");
        assert_eq!(
            mixed.line_endings(),
            LineEndings::Mixed {
                dominant: LineEnding::Lf
            }
        );
        assert_eq!(mixed.to_text(), "a\r\nb\nc\nd\n");
        assert_eq!(mixed.line_break_range(0), 1..3);
        assert_eq!(mixed.with_line_ending("x\r\ny"), "x\ny");
    }

    #[test]
    fn ending_counts_follow_edits_including_split_pairs() {
        let mut buffer = Buffer::from_text("a\r\nb\r\n");
        // Deleting the `\n` of a pair leaves a lone `\r` (not a counted ending).
        buffer.replace(2..3, "");
        assert_eq!(buffer.endings, EndingCounts { crlf: 1, lf: 0 });
        // Inserting `\n` right after that `\r` forms a pair again.
        buffer.replace(2..2, "\n");
        assert_eq!(buffer.endings, EndingCounts { crlf: 2, lf: 0 });
        buffer.replace(0..0, "x\n");
        assert_eq!(buffer.endings, EndingCounts { crlf: 2, lf: 1 });
        assert_eq!(buffer.endings, EndingCounts::of(&buffer.rope().to_string()));
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
