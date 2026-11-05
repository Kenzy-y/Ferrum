//! Diagnostics in the editor's coordinates: byte ranges that follow
//! edits until the service publishes new ones.

use std::ops::Range;

use language::{Diagnostic, Severity};
use text::{Buffer, TextChanges};

use crate::editor::Editor;
use crate::highlighting::patch::map_offset;

/// A diagnostic placed in the text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlacedDiagnostic {
    /// Underlined bytes; never empty unless the text is.
    pub(crate) range: Range<usize>,
    pub(crate) severity: Severity,
    pub(crate) message: String,
    pub(crate) source: Option<String>,
    pub(crate) code: Option<String>,
}

/// Places `diagnostics` in `buffer`, sorted by start. An empty range
/// underlines the character after it, or before it at the end of a line.
pub(crate) fn place(diagnostics: &[Diagnostic], buffer: &Buffer) -> Vec<PlacedDiagnostic> {
    let mut placed: Vec<PlacedDiagnostic> = diagnostics
        .iter()
        .map(|diagnostic| {
            let start = buffer.point_to_offset(diagnostic.range.start);
            let end = buffer.point_to_offset(diagnostic.range.end).max(start);
            PlacedDiagnostic {
                range: widen(start..end, buffer),
                severity: diagnostic.severity,
                message: diagnostic.message.clone(),
                source: diagnostic.source.clone(),
                code: diagnostic.code.clone(),
            }
        })
        .collect();
    placed.sort_by_key(|diagnostic| (diagnostic.range.start, diagnostic.severity));
    placed
}

/// A visible range for `range`: one character when it is empty.
fn widen(range: Range<usize>, buffer: &Buffer) -> Range<usize> {
    if !range.is_empty() {
        return range;
    }
    let row = buffer.offset_to_point(range.start).row;
    let line = buffer.line_range(row);
    let content_end = line.start
        + buffer
            .slice(line.clone())
            .trim_end_matches(['\r', '\n'])
            .len();
    let rope = buffer.rope();
    let next = |offset: usize| {
        let char = rope.byte_to_char(offset);
        rope.char_to_byte((char + 1).min(rope.len_chars()))
    };
    if range.start < content_end {
        range.start..next(range.start)
    } else if range.start > line.start {
        let previous = rope.char_to_byte(rope.byte_to_char(range.start).saturating_sub(1));
        previous..range.start
    } else {
        range
    }
}

/// Moves diagnostics through edits; a whole-text change drops them (the
/// service sends new ones).
pub(crate) fn follow(
    diagnostics: &mut Vec<PlacedDiagnostic>,
    changes: &TextChanges,
    buffer: &Buffer,
) {
    if changes.replaced {
        diagnostics.clear();
        return;
    }
    for diagnostic in diagnostics.iter_mut() {
        for edit in &changes.edits {
            let start = map_offset(diagnostic.range.start, edit, true);
            let end = map_offset(diagnostic.range.end, edit, false);
            diagnostic.range = start..end.max(start);
        }
        let start = buffer.clamp(diagnostic.range.start);
        let end = buffer.clamp(diagnostic.range.end);
        diagnostic.range = widen(start..end, buffer);
    }
    diagnostics.sort_by_key(|diagnostic| (diagnostic.range.start, diagnostic.severity));
}

impl Editor {
    /// Diagnostics overlapping `range`, most severe first where they
    /// overlap each other.
    pub(crate) fn diagnostics_in(&self, range: Range<usize>) -> Vec<&PlacedDiagnostic> {
        let Some(language) = &self.language else {
            return Vec::new();
        };
        let mut found: Vec<&PlacedDiagnostic> = language
            .diagnostics
            .iter()
            .take_while(|diagnostic| diagnostic.range.start <= range.end)
            .filter(|diagnostic| {
                diagnostic.range.end > range.start
                    || (diagnostic.range.is_empty() && diagnostic.range.start == range.start)
            })
            .collect();
        found.sort_by_key(|diagnostic| diagnostic.severity);
        found
    }

    /// How many errors and warnings the text has.
    pub fn problem_counts(&self) -> (usize, usize) {
        let Some(language) = &self.language else {
            return (0, 0);
        };
        language
            .diagnostics
            .iter()
            .fold((0, 0), |(errors, warnings), diagnostic| {
                match diagnostic.severity {
                    Severity::Error => (errors + 1, warnings),
                    Severity::Warning => (errors, warnings + 1),
                    _ => (errors, warnings),
                }
            })
    }
}

#[cfg(test)]
mod tests {
    use language::Diagnostic;
    use text::{Document, Point, Selection};

    use super::*;

    fn diagnostic(start: Point, end: Point, severity: Severity) -> Diagnostic {
        Diagnostic {
            range: start..end,
            severity,
            message: "oops".to_owned(),
            source: None,
            code: None,
        }
    }

    #[test]
    fn empty_ranges_underline_a_character() {
        let buffer = Buffer::from_text("let x\r\ny");
        let placed = place(
            &[
                diagnostic(Point::new(0, 4), Point::new(0, 4), Severity::Error),
                // At the end of the line: the character before.
                diagnostic(Point::new(0, 5), Point::new(0, 5), Severity::Warning),
            ],
            &buffer,
        );
        assert_eq!(placed[0].range, 4..5);
        assert_eq!(placed[1].range, 4..5);
        assert_eq!(placed[0].severity, Severity::Error, "errors sort first");
    }

    #[test]
    fn diagnostics_follow_typing_before_them() {
        let mut document = Document::new(Buffer::from_text("fn main() { x }\n"));
        document.track_changes();
        let mut placed = place(
            &[diagnostic(
                Point::new(0, 12),
                Point::new(0, 13),
                Severity::Error,
            )],
            document.buffer(),
        );
        document.set_primary_selection(Selection::new(0, 0));
        document.insert("pub ");
        follow(&mut placed, &document.take_changes(), document.buffer());
        assert_eq!(placed[0].range, 16..17);
        assert_eq!(document.buffer().slice(placed[0].range.clone()), "x");

        document.reload(Buffer::from_text("other"));
        follow(&mut placed, &document.take_changes(), document.buffer());
        assert!(placed.is_empty());
    }
}
