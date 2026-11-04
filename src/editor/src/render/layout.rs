//! Geometry of the last drawn frame: where rows and characters are, for
//! mouse clicks and input method windows.

use gpui::{Bounds, Pixels, Point, ShapedLine, point, px, size};
use text::Buffer;

use crate::decorations::minimap::Minimap;
use crate::decorations::scrollbar::Scrollbar;
use crate::render::display::DisplayLine;
use crate::render::viewport::{to_f32, to_index};

/// A visible row as it was shaped.
pub(crate) struct ShapedRow {
    pub(crate) row: usize,
    /// Buffer offset of the first shaped character.
    pub(crate) start: usize,
    /// Buffer offset after the last shaped character.
    pub(crate) end: usize,
    /// x of the first shaped character from the start of the text,
    /// unscrolled (non-zero for a window into a long line).
    pub(crate) x: f32,
    /// Width of the whole line (estimated from columns for long lines).
    pub(crate) width: f32,
    pub(crate) display: DisplayLine,
    pub(crate) shaped: ShapedLine,
}

impl ShapedRow {
    /// x of buffer offset `offset` (inside the shaped part) from the start
    /// of the text, unscrolled.
    fn x_of(&self, offset: usize) -> f32 {
        let display = self.display.to_display(offset - self.start);
        self.x + self.shaped.x_for_index(display).as_f32()
    }
}

/// Positions of the last frame, in window coordinates.
pub(crate) struct LayoutSnapshot {
    /// Horizontal scroll the frame was drawn with.
    pub(crate) scroll_x: f32,
    /// Largest horizontal scroll for the visible rows.
    pub(crate) max_scroll_x: f32,
    /// Window x of the start of the text, with the horizontal scroll.
    pub(crate) text_left: f32,
    /// Window y of the top of row 0, with the vertical scroll.
    pub(crate) text_top: f32,
    pub(crate) line_height: f32,
    pub(crate) char_width: f32,
    /// Height of the area that shows text.
    pub(crate) text_height: f32,
    /// The visible rows, in order.
    pub(crate) rows: Vec<ShapedRow>,
    /// The scrollbar, if the text is taller than the view.
    pub(crate) scrollbar: Option<Scrollbar>,
    /// The minimap, if it is shown.
    pub(crate) minimap: Option<Minimap>,
}

impl LayoutSnapshot {
    /// Rows moved by Page Up / Page Down.
    pub(crate) fn page_rows(&self) -> usize {
        to_index((self.text_height / self.line_height).floor()).saturating_sub(1)
    }

    fn shaped(&self, row: usize) -> Option<&ShapedRow> {
        let first = self.rows.first()?.row;
        self.rows
            .get(row.checked_sub(first)?)
            .filter(|shaped| shaped.row == row)
    }

    /// x of `offset` from the start of the text, unscrolled. Outside the
    /// shaped part of a row, an estimate from the character column.
    pub(crate) fn x_of(&self, buffer: &Buffer, offset: usize) -> f32 {
        let row = buffer.offset_to_point(offset).row;
        match self.shaped(row) {
            Some(shaped) if (shaped.start..=shaped.end).contains(&offset) => shaped.x_of(offset),
            _ => to_f32(buffer.char_column(offset)) * self.char_width,
        }
    }

    /// The row under window y `y`, clamped to the document.
    pub(crate) fn row_at(&self, buffer: &Buffer, y: f32) -> usize {
        let row = to_index(((y - self.text_top) / self.line_height).floor());
        row.min(buffer.line_count() - 1)
    }

    /// The character boundary nearest to window position `position`.
    pub(crate) fn offset_at(&self, buffer: &Buffer, position: Point<Pixels>) -> usize {
        let row = self.row_at(buffer, position.y.as_f32());
        let x = position.x.as_f32() - self.text_left;
        let line_end = buffer.line_range(row).end;
        match self.shaped(row) {
            Some(shaped) if x >= shaped.x || shaped.start == buffer.line_range(row).start => {
                let local = px(x - shaped.x);
                let display = shaped.shaped.closest_index_for_x(local);
                (shaped.start + shaped.display.to_source(display)).min(line_end)
            }
            _ => {
                let column = to_index((x / self.char_width).round());
                buffer.offset_at_char_column(row, column)
            }
        }
    }

    /// Window bounds of the character at `offset` (for input method
    /// windows, which open below it).
    pub(crate) fn bounds_of(&self, buffer: &Buffer, offset: usize) -> Bounds<Pixels> {
        let row = buffer.offset_to_point(offset).row;
        let x = self.text_left + self.x_of(buffer, offset);
        let y = self.text_top + to_f32(row) * self.line_height;
        Bounds::new(
            point(px(x), px(y)),
            size(px(self.char_width), px(self.line_height)),
        )
    }
}
