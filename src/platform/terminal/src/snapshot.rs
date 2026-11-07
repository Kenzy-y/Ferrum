//! What the UI draws: the visible screen of a terminal as styled spans
//! with resolved colors, the selection and the cursor.

use alacritty_terminal::Term;
use alacritty_terminal::event::EventListener;
use alacritty_terminal::grid::Dimensions as _;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::{self, NamedColor};

use crate::palette::{Palette, Rgb};

/// Text attributes of a span.
// Each flag is an independent text attribute.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CellStyle {
    /// Bold weight.
    pub bold: bool,
    /// Italic.
    pub italic: bool,
    /// Underlined (any underline style).
    pub underline: bool,
    /// Struck through.
    pub strikethrough: bool,
}

/// Cells of one row that look the same.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    /// First column.
    pub column: usize,
    /// Columns covered; a wide character covers two.
    pub cells: usize,
    /// The characters, with combining marks.
    pub text: String,
    /// Text color.
    pub foreground: Rgb,
    /// Background; `None` for the terminal's default background.
    pub background: Option<Rgb>,
    /// Attributes.
    pub style: CellStyle,
}

/// One row of the screen.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SnapshotRow {
    /// Spans worth drawing, left to right. Blank cells on the default
    /// background are left out.
    pub spans: Vec<Span>,
    /// Selected columns of the row (`start..end`).
    pub selection: Option<(usize, usize)>,
}

/// How the cursor is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorShape {
    /// A filled cell.
    Block,
    /// A line below the cell.
    Underline,
    /// A thin bar before the cell.
    Beam,
    /// An outlined cell.
    HollowBlock,
}

/// The cursor, when it is on screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor {
    /// Row on screen.
    pub row: usize,
    /// Column.
    pub column: usize,
    /// Columns it covers (two on a wide character).
    pub cells: usize,
    /// Shape the program asked for.
    pub shape: CursorShape,
    /// The character under it, drawn over a block cursor.
    pub text: String,
    /// The program hid the cursor (DECTCEM). Shells hide it for a moment
    /// while they redraw the line; the UI uses that to skip half-drawn
    /// frames.
    pub hidden: bool,
}

/// The visible screen of a terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// Columns of the grid.
    pub columns: usize,
    /// Rows, top to bottom; always as many as the screen has lines.
    pub rows: Vec<SnapshotRow>,
    /// The cursor, if shown and inside the visible part.
    pub cursor: Option<Cursor>,
    /// Lines scrolled back from the bottom.
    pub display_offset: usize,
    /// Lines of scrollback above the screen.
    pub history_size: usize,
    /// Default background, possibly changed by the program.
    pub background: Rgb,
}

/// Which parts of a cell make a span end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SpanKey {
    foreground: Rgb,
    background: Option<Rgb>,
    style: CellStyle,
}

/// Builds the snapshot of `term`'s visible screen. Runs with the terminal
/// locked; its cost is linear in the visible cells.
pub(crate) fn build<T: EventListener>(term: &Term<T>, palette: &Palette) -> Snapshot {
    let content = term.renderable_content();
    let columns = term.columns();
    let screen_lines = term.screen_lines();
    let display_offset = content.display_offset;
    let colors = content.colors;
    let background = colors[NamedColor::Background as usize].map_or(palette.background, Rgb::from);

    let mut rows: Vec<SnapshotRow> = vec![SnapshotRow::default(); screen_lines];
    let mut current: Option<(usize, SpanKey, Span)> = None;
    let finish = |rows: &mut Vec<SnapshotRow>, span: Option<(usize, SpanKey, Span)>| {
        let Some((row, key, mut span)) = span else {
            return;
        };
        // Trailing blanks on the default background draw nothing.
        if key.background.is_none() && !key.style.underline && !key.style.strikethrough {
            let trimmed = span.text.trim_end_matches(' ').len();
            span.cells -= span.text.len() - trimmed;
            span.text.truncate(trimmed);
        }
        if !span.text.is_empty() {
            rows[row].spans.push(span);
        }
    };

    for indexed in content.display_iter {
        let Some(row) = viewport_row(indexed.point.line.0, display_offset, screen_lines) else {
            continue;
        };
        let cell = indexed.cell;
        if cell
            .flags
            .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
        {
            continue;
        }
        let column = indexed.point.column.0;
        let wide = cell.flags.contains(Flags::WIDE_CHAR);
        let key = span_key(cell, palette, colors);
        let hidden = cell.flags.contains(Flags::HIDDEN);
        let continues = matches!(
            &current,
            Some((span_row, span_key, span))
                if *span_row == row
                    && *span_key == key
                    && span.column + span.cells == column
                    && !wide
        );
        if !continues {
            finish(&mut rows, current.take());
            current = Some((
                row,
                key,
                Span {
                    column,
                    cells: 0,
                    text: String::new(),
                    foreground: key.foreground,
                    background: key.background,
                    style: key.style,
                },
            ));
        }
        if let Some((_, _, span)) = &mut current {
            span.cells += if wide { 2 } else { 1 };
            if hidden {
                span.text.push(' ');
            } else {
                span.text.push(cell.c);
                if let Some(marks) = cell.zerowidth() {
                    span.text.extend(marks);
                }
            }
        }
        // A wide character is a span of its own, so that the next span
        // starts exactly at its column whatever width the font gives it.
        if wide {
            finish(&mut rows, current.take());
        }
    }
    finish(&mut rows, current.take());

    if let Some(selection) = content.selection {
        mark_selection(&mut rows, &selection, display_offset, columns);
    }

    let cursor = cursor(term, &content.cursor, display_offset, screen_lines);
    Snapshot {
        columns,
        rows,
        cursor,
        display_offset,
        history_size: term.history_size(),
        background,
    }
}

/// How `cell` looks: resolved colors (inverse applied) and attributes.
fn span_key(
    cell: &alacritty_terminal::term::cell::Cell,
    palette: &Palette,
    colors: &alacritty_terminal::term::color::Colors,
) -> SpanKey {
    let dim = cell.flags.contains(Flags::DIM);
    let mut foreground = palette.resolve(cell.fg, colors, false, dim);
    let mut background = palette.resolve(cell.bg, colors, false, false);
    let mut default_background = cell.bg == ansi::Color::Named(NamedColor::Background);
    if cell.flags.contains(Flags::INVERSE) {
        std::mem::swap(&mut foreground, &mut background);
        default_background = false;
    }
    SpanKey {
        foreground,
        background: (!default_background).then_some(background),
        style: CellStyle {
            bold: cell.flags.contains(Flags::BOLD),
            italic: cell.flags.contains(Flags::ITALIC),
            underline: cell.flags.intersects(Flags::ALL_UNDERLINES),
            strikethrough: cell.flags.contains(Flags::STRIKEOUT),
        },
    }
}

/// Sets the selected columns of each visible row.
fn mark_selection(
    rows: &mut [SnapshotRow],
    selection: &alacritty_terminal::selection::SelectionRange,
    display_offset: usize,
    columns: usize,
) {
    let (start, end) = (selection.start, selection.end);
    for (row, row_content) in rows.iter_mut().enumerate() {
        let line = line_of(row, display_offset);
        if line < start.line.0 || line > end.line.0 {
            continue;
        }
        let (from, to) = if selection.is_block {
            (start.column.0, end.column.0 + 1)
        } else {
            let from = if line == start.line.0 {
                start.column.0
            } else {
                0
            };
            let to = if line == end.line.0 {
                end.column.0 + 1
            } else {
                columns
            };
            (from, to)
        };
        if from < to {
            row_content.selection = Some((from, to.min(columns)));
        }
    }
}

/// The screen row of grid line `line`, if visible.
fn viewport_row(line: i32, display_offset: usize, screen_lines: usize) -> Option<usize> {
    let offset = i32::try_from(display_offset).ok()?;
    let row = usize::try_from(line + offset).ok()?;
    (row < screen_lines).then_some(row)
}

/// The grid line shown on screen row `row`.
pub(crate) fn line_of(row: usize, display_offset: usize) -> i32 {
    let row = i32::try_from(row).unwrap_or(i32::MAX);
    let offset = i32::try_from(display_offset).unwrap_or(i32::MAX);
    row.saturating_sub(offset)
}

fn cursor<T: EventListener>(
    term: &Term<T>,
    cursor: &alacritty_terminal::term::RenderableCursor,
    display_offset: usize,
    screen_lines: usize,
) -> Option<Cursor> {
    let hidden = cursor.shape == ansi::CursorShape::Hidden;
    let requested = if hidden {
        term.cursor_style().shape
    } else {
        cursor.shape
    };
    let shape = match requested {
        ansi::CursorShape::Block | ansi::CursorShape::Hidden => CursorShape::Block,
        ansi::CursorShape::Underline => CursorShape::Underline,
        ansi::CursorShape::Beam => CursorShape::Beam,
        ansi::CursorShape::HollowBlock => CursorShape::HollowBlock,
    };
    let row = viewport_row(cursor.point.line.0, display_offset, screen_lines)?;
    let cell = &term.grid()[cursor.point];
    let wide = cell.flags.contains(Flags::WIDE_CHAR);
    let mut text = String::new();
    if !cell.flags.contains(Flags::HIDDEN) && cell.c != ' ' {
        text.push(cell.c);
        if let Some(marks) = cell.zerowidth() {
            text.extend(marks);
        }
    }
    Some(Cursor {
        row,
        column: cursor.point.column.0,
        cells: if wide { 2 } else { 1 },
        shape,
        text,
        hidden,
    })
}

#[cfg(test)]
mod tests {
    use alacritty_terminal::event::VoidListener;
    use alacritty_terminal::index::{Column, Line, Point, Side};
    use alacritty_terminal::selection::{Selection, SelectionType};
    use alacritty_terminal::term::Config;
    use alacritty_terminal::vte::ansi::Processor;

    use super::*;
    use crate::session::GridSize;

    fn palette() -> Palette {
        Palette {
            foreground: Rgb::new(200, 200, 200),
            background: Rgb::new(10, 10, 10),
            cursor: Rgb::new(255, 255, 255),
            ansi: std::array::from_fn(|index| {
                let index = u8::try_from(index).unwrap();
                Rgb::new(index, 0, 0)
            }),
        }
    }

    fn term(columns: usize, lines: usize, input: &str) -> Term<VoidListener> {
        let mut term = Term::new(
            Config::default(),
            &GridSize { columns, lines },
            VoidListener,
        );
        let mut parser: Processor = Processor::new();
        parser.advance(&mut term, input.as_bytes());
        term
    }

    fn texts(row: &SnapshotRow) -> Vec<&str> {
        row.spans.iter().map(|span| span.text.as_str()).collect()
    }

    #[test]
    fn spans_split_on_style_and_skip_blank_cells() {
        let term = term(20, 3, "ab\x1b[31mcd\x1b[0m   e\r\nx");
        let snapshot = build(&term, &palette());
        assert_eq!(snapshot.rows.len(), 3);
        let first = &snapshot.rows[0];
        assert_eq!(texts(first), ["ab", "cd", "   e"]);
        assert_eq!(first.spans[1].column, 2);
        assert_eq!(first.spans[1].foreground, Rgb::new(1, 0, 0));
        assert_eq!(first.spans[0].foreground, palette().foreground);
        assert_eq!(texts(&snapshot.rows[1]), ["x"]);
        assert!(snapshot.rows[2].spans.is_empty());
        let cursor = snapshot.cursor.unwrap();
        assert_eq!((cursor.row, cursor.column), (1, 1));
    }

    #[test]
    fn inverse_and_backgrounds() {
        let term = term(10, 1, "\x1b[7mi\x1b[0m\x1b[42m \x1b[0m");
        let snapshot = build(&term, &palette());
        let spans = &snapshot.rows[0].spans;
        assert_eq!(spans[0].background, Some(palette().foreground));
        assert_eq!(spans[0].foreground, palette().background);
        assert_eq!(spans[1].background, Some(Rgb::new(2, 0, 0)));
    }

    #[test]
    fn wide_characters_are_spans_of_two_cells() {
        let term = term(10, 1, "a界b");
        let snapshot = build(&term, &palette());
        let spans = &snapshot.rows[0].spans;
        assert_eq!(texts(&snapshot.rows[0]), ["a", "界", "b"]);
        assert_eq!(spans[1].cells, 2);
        assert_eq!(spans[2].column, 3);
    }

    #[test]
    fn selection_covers_rows_between_its_ends() {
        let mut term = term(5, 3, "aaaaa\r\nbbbbb\r\nccccc");
        let mut selection = Selection::new(
            SelectionType::Simple,
            Point::new(Line(0), Column(3)),
            Side::Left,
        );
        selection.update(Point::new(Line(2), Column(1)), Side::Right);
        term.selection = Some(selection);
        let snapshot = build(&term, &palette());
        assert_eq!(snapshot.rows[0].selection, Some((3, 5)));
        assert_eq!(snapshot.rows[1].selection, Some((0, 5)));
        assert_eq!(snapshot.rows[2].selection, Some((0, 2)));
    }

    #[test]
    fn a_cursor_hidden_by_the_program_is_marked() {
        let hidden = build(&term(5, 1, "ab\x1b[?25l"), &palette())
            .cursor
            .unwrap();
        assert!(hidden.hidden);
        assert_eq!(hidden.column, 2);
        let shown = build(&term(5, 1, "ab"), &palette()).cursor.unwrap();
        assert!(!shown.hidden);
    }
}
