//! The layers of a frame around the text: current line highlights,
//! selections, indent guides, cursors and line numbers. Only rows in view
//! and selections touching them are looked at.

use std::ops::Range;

use gpui::{
    Bounds, Hsla, PaintQuad, Pixels, Point, ShapedLine, SharedString, TextRun, Window, fill, point,
    px, size,
};
use text::{Buffer, Selection};
use theme::Theme;

use crate::editor::Editor;
use crate::render::layout::LayoutSnapshot;
use crate::render::viewport::{guide_levels, to_f32};

/// Read-only inputs of the layers.
pub(crate) struct Scene<'a> {
    pub(crate) editor: &'a Editor,
    pub(crate) theme: &'a Theme,
    pub(crate) snapshot: &'a LayoutSnapshot,
    pub(crate) rows: Range<usize>,
    /// Selections that touch the visible rows, in document order.
    pub(crate) near: &'a [Selection],
    /// Window x of the right edge of the line numbers.
    pub(crate) numbers_end: f32,
    /// Window x of the right edge of the current line highlight.
    pub(crate) highlight_right: f32,
}

/// The selections of `selections` (sorted, not overlapping) that touch
/// `rows`.
pub(crate) fn near_rows<'a>(
    selections: &'a [Selection],
    buffer: &Buffer,
    rows: &Range<usize>,
) -> &'a [Selection] {
    if rows.is_empty() {
        return &[];
    }
    let first = buffer.line_range(rows.start).start;
    let last = buffer.line_break_range(rows.end - 1).end;
    let from = selections.partition_point(|selection| selection.end() < first);
    let to = selections.partition_point(|selection| selection.start() <= last);
    &selections[from..to.max(from)]
}

impl Scene<'_> {
    fn buffer(&self) -> &Buffer {
        self.editor.document.buffer()
    }

    fn row_top(&self, row: usize) -> f32 {
        self.snapshot.text_top + to_f32(row) * self.snapshot.line_height
    }

    fn row_of(&self, offset: usize) -> usize {
        self.buffer().offset_to_point(offset).row
    }

    /// Visible rows with a cursor head.
    pub(crate) fn cursor_rows(&self) -> Vec<usize> {
        let mut rows: Vec<usize> = self
            .near
            .iter()
            .map(|selection| self.row_of(selection.head))
            .filter(|row| self.rows.contains(row))
            .collect();
        rows.dedup();
        rows
    }

    /// Highlights of the rows with a cursor and no selection.
    pub(crate) fn current_lines(&self) -> Vec<PaintQuad> {
        let sizes = &self.theme.metrics.size;
        let left = self.numbers_end + sizes.editor_active_line_inset;
        let height = sizes.editor_active_line_height;
        let mut rows: Vec<usize> = self
            .near
            .iter()
            .filter(|selection| selection.is_empty())
            .map(|selection| self.row_of(selection.head))
            .filter(|row| self.rows.contains(row))
            .collect();
        rows.dedup();
        rows.into_iter()
            .map(|row| {
                let y = self.row_top(row) - (height - self.snapshot.line_height) / 2.0;
                fill(
                    Bounds::new(
                        point(px(left), px(y)),
                        size(px(self.highlight_right - left), px(height)),
                    ),
                    Hsla::from(self.theme.colors.bg_line_active),
                )
                .corner_radii(px(self.theme.metrics.radius.sm))
            })
            .collect()
    }

    /// Selection backgrounds, one rectangle per selection and row; a
    /// selection that continues to the next line covers one more column.
    pub(crate) fn selections(&self) -> Vec<PaintQuad> {
        let colors = &self.theme.colors;
        let color = Hsla::from(if self.editor.focused {
            colors.bg_selection
        } else {
            colors.bg_selection_inactive
        });
        let buffer = self.buffer();
        let snapshot = self.snapshot;
        let mut quads = Vec::new();
        for row in self.rows.clone() {
            let line = buffer.line_range(row);
            for selection in self.near.iter().filter(|selection| !selection.is_empty()) {
                if selection.end() < line.start || selection.start() > line.end {
                    continue;
                }
                let x0 = snapshot.x_of(buffer, selection.start().max(line.start));
                let mut x1 = snapshot.x_of(buffer, selection.end().min(line.end));
                if selection.end() > line.end {
                    x1 += snapshot.char_width;
                }
                if x1 > x0 {
                    quads.push(fill(
                        Bounds::new(
                            point(px(snapshot.text_left + x0), px(self.row_top(row))),
                            size(px(x1 - x0), px(snapshot.line_height)),
                        ),
                        color,
                    ));
                }
            }
        }
        quads
    }

    /// Backgrounds of the find matches on the visible rows.
    pub(crate) fn find_matches(&self) -> Vec<PaintQuad> {
        if self.rows.is_empty() || self.editor.find.is_none() {
            return Vec::new();
        }
        let buffer = self.buffer();
        let snapshot = self.snapshot;
        let first = buffer.line_range(self.rows.start).start;
        let last = buffer.line_range(self.rows.end - 1).end;
        let color = Hsla::from(self.theme.colors.bg_find_match);
        let mut quads = Vec::new();
        for found in self.editor.find_matches_in(first..last + 1) {
            for row in self.rows.clone() {
                let line = buffer.line_range(row);
                if found.end <= line.start || found.start > line.end {
                    continue;
                }
                let x0 = snapshot.x_of(buffer, found.start.max(line.start));
                let x1 = snapshot.x_of(buffer, found.end.min(line.end));
                if x1 > x0 {
                    quads.push(fill(
                        Bounds::new(
                            point(px(snapshot.text_left + x0), px(self.row_top(row))),
                            size(px(x1 - x0), px(snapshot.line_height)),
                        ),
                        color,
                    ));
                }
            }
        }
        quads
    }

    /// Indent guides: one line per indentation level, at the column where
    /// the level starts (as in the mockups).
    pub(crate) fn guides(&self) -> Vec<PaintQuad> {
        let buffer = self.buffer();
        let tab_size = self.editor.settings.tab_size.max(1);
        let levels = guide_levels(self.rows.clone(), buffer.line_count(), tab_size, |row| {
            indentation(buffer, row, tab_size)
        });
        let color = Hsla::from(self.theme.colors.guide_indent);
        let mut quads = Vec::new();
        for (row, level) in self.rows.clone().zip(levels) {
            for step in 1..=level {
                let x =
                    self.snapshot.text_left + to_f32(step * tab_size) * self.snapshot.char_width;
                quads.push(fill(
                    Bounds::new(
                        point(px(x), px(self.row_top(row))),
                        size(px(1.0), px(self.snapshot.line_height)),
                    ),
                    color,
                ));
            }
        }
        quads
    }

    /// Cursors, when shown (focused, and in the visible blink phase).
    pub(crate) fn cursors(&self) -> Vec<PaintQuad> {
        if !self.editor.focused || !self.editor.blink.visible {
            return Vec::new();
        }
        let sizes = &self.theme.metrics.size;
        let color = Hsla::from(self.theme.colors.cursor);
        self.near
            .iter()
            .filter(|selection| self.rows.contains(&self.row_of(selection.head)))
            .map(|selection| {
                let x = self.snapshot.text_left + self.snapshot.x_of(self.buffer(), selection.head);
                fill(
                    Bounds::new(
                        point(px(x), px(self.row_top(self.row_of(selection.head)))),
                        size(px(sizes.cursor_width), px(sizes.cursor_height)),
                    ),
                    color,
                )
            })
            .collect()
    }

    /// Line numbers, right-aligned; rows with a cursor brighter.
    pub(crate) fn numbers(
        &self,
        font: &gpui::Font,
        font_size: Pixels,
        window: &Window,
    ) -> Vec<(Point<Pixels>, ShapedLine)> {
        let colors = &self.theme.colors;
        let cursor_rows = self.cursor_rows();
        self.rows
            .clone()
            .map(|row| {
                let color = if cursor_rows.contains(&row) {
                    colors.text_line_number_active
                } else {
                    colors.text_line_number
                };
                let label: SharedString = (row + 1).to_string().into();
                let run = TextRun {
                    len: label.len(),
                    font: font.clone(),
                    color: Hsla::from(color),
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };
                let number = window
                    .text_system()
                    .shape_line(label, font_size, &[run], None);
                let x = self.numbers_end - number.width().as_f32();
                (point(px(x), px(self.row_top(row))), number)
            })
            .collect()
    }
}

/// Indentation of `row` in columns; `None` for a blank row.
fn indentation(buffer: &Buffer, row: usize, tab_size: usize) -> Option<usize> {
    let mut columns = 0;
    for character in buffer
        .line(row)
        .chars()
        .take(crate::render::viewport::LONG_LINE_CHARS)
    {
        match character {
            ' ' => columns += 1,
            '\t' => columns = (columns / tab_size + 1) * tab_size,
            _ => return Some(columns),
        }
    }
    None
}
