//! Positions and document changes in the server's terms.
//!
//! The IDE counts columns in bytes. A server counts in UTF-8 bytes too if
//! it agrees to (`positionEncoding`), otherwise in UTF-16 code units, and
//! then each position is converted through its line.

use lsp_types::{Position, Range as LspRange, TextDocumentContentChangeEvent};
use text::ropey::{self, Rope};
use text::{Buffer, Point, TextChanges};

/// How the server counts columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Encoding {
    /// Bytes, like the IDE.
    Utf8,
    /// UTF-16 code units, the protocol's default.
    Utf16,
}

/// How the server wants document changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SyncKind {
    /// Not at all.
    None,
    /// The whole text every time.
    Full,
    /// Only the changed ranges.
    Incremental,
}

fn to_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// `point` as a protocol position, `rope` being the text it is in.
pub(crate) fn to_position(point: Point, rope: &Rope, encoding: Encoding) -> Position {
    match encoding {
        Encoding::Utf8 => Position::new(to_u32(point.row), to_u32(point.column)),
        Encoding::Utf16 => {
            if point.row >= rope.len_lines() {
                return Position::new(to_u32(point.row), 0);
            }
            let line = rope.line(point.row);
            let column = point.column.min(line.len_bytes());
            let column = line.byte_slice(..column).len_utf16_cu();
            Position::new(to_u32(point.row), to_u32(column))
        }
    }
}

/// A protocol position as a point of `rope`. Without the text (a file that
/// is not open), UTF-16 columns are taken as bytes.
pub(crate) fn to_point(position: Position, rope: Option<&Rope>, encoding: Encoding) -> Point {
    let row = position.line as usize;
    let character = position.character as usize;
    let Some(rope) = rope else {
        return Point::new(row, character);
    };
    if row >= rope.len_lines() {
        return Point::new(row, 0);
    }
    let line = rope.line(row);
    let content = line_content_bytes(line);
    let column = match encoding {
        Encoding::Utf8 => character,
        Encoding::Utf16 => {
            let mut units = 0;
            let mut bytes = 0;
            for char in line.chars() {
                if units >= character {
                    break;
                }
                units += char.len_utf16();
                bytes += char.len_utf8();
            }
            bytes
        }
    };
    Point::new(row, floor_char_boundary(line, column.min(content)))
}

/// Bytes of a line without its line break.
fn line_content_bytes(line: ropey::RopeSlice<'_>) -> usize {
    let chars = line.len_chars();
    let last = |back: usize| chars.checked_sub(back).map(|index| line.char(index));
    let ending = match (last(2), last(1)) {
        (Some('\r'), Some('\n')) => 2,
        (_, Some('\n' | '\r')) => 1,
        _ => 0,
    };
    line.len_bytes() - ending
}

fn floor_char_boundary(line: ropey::RopeSlice<'_>, byte: usize) -> usize {
    let char = line.byte_to_char(byte);
    line.char_to_byte(char)
}

/// A range of `rope` as a protocol range.
pub(crate) fn to_range(
    range: &std::ops::Range<Point>,
    rope: &Rope,
    encoding: Encoding,
) -> LspRange {
    LspRange::new(
        to_position(range.start, rope, encoding),
        to_position(range.end, rope, encoding),
    )
}

/// A protocol range as points.
pub(crate) fn from_range(
    range: LspRange,
    rope: Option<&Rope>,
    encoding: Encoding,
) -> std::ops::Range<Point> {
    to_point(range.start, rope, encoding)..to_point(range.end, rope, encoding)
}

/// The changes that lead from `previous` to `current`, as the server wants
/// them. Incremental UTF-8 changes come straight from the edits; UTF-16
/// ones replay the edits on a copy of `previous` (logarithmic per edit) to
/// know the lines each one was made in. Runs on the writer thread.
pub(crate) fn content_changes(
    previous: &Buffer,
    changes: &TextChanges,
    current: &Buffer,
    encoding: Encoding,
    kind: SyncKind,
) -> Vec<TextDocumentContentChangeEvent> {
    let full = || TextDocumentContentChangeEvent {
        range: None,
        range_length: None,
        text: current.rope().to_string(),
    };
    match kind {
        SyncKind::None => return Vec::new(),
        SyncKind::Full => return vec![full()],
        SyncKind::Incremental if changes.replaced => return vec![full()],
        SyncKind::Incremental => {}
    }
    match encoding {
        Encoding::Utf8 => changes
            .edits
            .iter()
            .map(|edit| TextDocumentContentChangeEvent {
                range: Some(LspRange::new(
                    Position::new(
                        to_u32(edit.start_point.row),
                        to_u32(edit.start_point.column),
                    ),
                    Position::new(
                        to_u32(edit.old_end_point.row),
                        to_u32(edit.old_end_point.column),
                    ),
                )),
                range_length: None,
                text: edit.inserted.to_string(),
            })
            .collect(),
        Encoding::Utf16 => {
            let mut rope = previous.rope().clone();
            let mut events = Vec::with_capacity(changes.edits.len());
            for edit in &changes.edits {
                if edit.old_end > rope.len_bytes() {
                    return vec![full()];
                }
                let start = to_position(edit.start_point, &rope, encoding);
                let end = to_position(edit.old_end_point, &rope, encoding);
                let start_char = rope.byte_to_char(edit.start);
                let end_char = rope.byte_to_char(edit.old_end);
                rope.remove(start_char..end_char);
                rope.insert(start_char, &edit.inserted);
                events.push(TextDocumentContentChangeEvent {
                    range: Some(LspRange::new(start, end)),
                    range_length: None,
                    text: edit.inserted.to_string(),
                });
            }
            events
        }
    }
}

#[cfg(test)]
mod tests {
    use text::Document;

    use super::*;

    fn rope(text: &str) -> Rope {
        Rope::from_str(text)
    }

    #[test]
    fn converts_columns_through_utf16() {
        let text = rope("a😀b\r\nпривет\n");
        // "😀" is 4 bytes and 2 UTF-16 units; "п" 2 bytes and one unit.
        assert_eq!(
            to_position(Point::new(0, 5), &text, Encoding::Utf16),
            Position::new(0, 3)
        );
        assert_eq!(
            to_point(Position::new(0, 3), Some(&text), Encoding::Utf16),
            Point::new(0, 5)
        );
        assert_eq!(
            to_position(Point::new(1, 4), &text, Encoding::Utf16),
            Position::new(1, 2)
        );
        // Past the end of the line: clamped before the line break.
        assert_eq!(
            to_point(Position::new(0, 99), Some(&text), Encoding::Utf16),
            Point::new(0, 6)
        );
        assert_eq!(
            to_point(Position::new(0, 99), Some(&text), Encoding::Utf8),
            Point::new(0, 6)
        );
        // Inside a character: back to its start.
        assert_eq!(
            to_point(Position::new(0, 2), Some(&text), Encoding::Utf8),
            Point::new(0, 1)
        );
        assert_eq!(
            to_position(Point::new(0, 5), &text, Encoding::Utf8),
            Position::new(0, 5)
        );
    }

    fn edited(text: &str, edit: impl FnOnce(&mut Document)) -> (Buffer, TextChanges, Buffer) {
        let mut document = Document::new(Buffer::from_text(text));
        document.track_changes();
        let previous = document.buffer().clone();
        edit(&mut document);
        (previous, document.take_changes(), document.buffer().clone())
    }

    /// Applies protocol changes to `text` the way a server would.
    fn apply(text: &str, events: &[TextDocumentContentChangeEvent], encoding: Encoding) -> String {
        let mut rope = rope(text);
        for event in events {
            let Some(range) = event.range else {
                rope = Rope::from_str(&event.text);
                continue;
            };
            let start = to_point(range.start, Some(&rope), encoding);
            let end = to_point(range.end, Some(&rope), encoding);
            let start = rope.line_to_byte(start.row) + start.column;
            let end = rope.line_to_byte(end.row) + end.column;
            let (start, end) = (rope.byte_to_char(start), rope.byte_to_char(end));
            rope.remove(start..end);
            rope.insert(start, &event.text);
        }
        rope.to_string()
    }

    #[test]
    fn incremental_changes_rebuild_the_text_in_both_encodings() {
        let original = "😀 one\nпри two\nthree";
        let (previous, changes, current) = edited(original, |document| {
            // A cursor at the end of each line, then type and delete.
            document.set_primary_selection(text::Selection::new(8, 8));
            document.add_selection(text::Selection::new(19, 19));
            document.insert("é\n");
            document.delete_backward();
        });
        for encoding in [Encoding::Utf8, Encoding::Utf16] {
            let events = content_changes(
                &previous,
                &changes,
                &current,
                encoding,
                SyncKind::Incremental,
            );
            assert!(events.iter().all(|event| event.range.is_some()));
            assert_eq!(
                apply(original, &events, encoding),
                current.rope().to_string(),
                "{encoding:?}"
            );
        }
    }

    #[test]
    fn full_sync_and_replaced_texts_send_everything() {
        let (previous, changes, current) = edited("a\r\nb", |document| document.insert("x"));
        let events = content_changes(
            &previous,
            &changes,
            &current,
            Encoding::Utf8,
            SyncKind::Full,
        );
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].text, "xa\r\nb");
        assert!(
            content_changes(
                &previous,
                &changes,
                &current,
                Encoding::Utf8,
                SyncKind::None
            )
            .is_empty()
        );
        let replaced = TextChanges {
            edits: Vec::new(),
            replaced: true,
        };
        let events = content_changes(
            &previous,
            &replaced,
            &current,
            Encoding::Utf16,
            SyncKind::Incremental,
        );
        assert!(events[0].range.is_none());
    }
}
