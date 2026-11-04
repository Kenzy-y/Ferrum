//! Shaping text for a frame: the fonts and measures of the text, the
//! visible part of each row with its highlights, and the cursor's x.

use gpui::{Font, Hsla, Pixels, SharedString, TextRun, Window, px};
use text::Buffer;
use theme::Theme;

use crate::editor::Editor;
use crate::render::display::DisplayLine;
use crate::render::layout::ShapedRow;
use crate::render::runs::{RunStyle, text_runs};
use crate::render::viewport::{LONG_LINE_CHARS, column_window, to_f32};

/// Fonts and measures of the text, from the theme.
pub(crate) struct Metrics {
    pub(crate) font: Font,
    pub(crate) font_size: Pixels,
    pub(crate) number_size: Pixels,
    pub(crate) line_height: f32,
    pub(crate) char_width: f32,
    pub(crate) number_char_width: f32,
    /// Height of a line number glyph box: numbers sit at the top of the
    /// row, as in the mockups.
    pub(crate) number_height: f32,
}

impl Metrics {
    pub(crate) fn new(theme: &Theme, window: &Window) -> Self {
        let font = gpui::font(SharedString::from(theme.fonts.mono.clone()));
        let font_size = px(theme.metrics.font_size.code);
        let number_size = px(theme.metrics.font_size.code_small);
        let text_system = window.text_system();
        let font_id = text_system.resolve_font(&font);
        let width = |size: Pixels| {
            text_system
                .advance(font_id, size, 'm')
                .map_or(size.as_f32() * 0.6, |advance| advance.width.as_f32())
        };
        let number_height = (text_system.ascent(font_id, number_size)
            + text_system.descent(font_id, number_size))
        .as_f32();
        Self {
            char_width: width(font_size),
            number_char_width: width(number_size),
            font,
            font_size,
            number_size,
            line_height: theme.metrics.size.code_line_height,
            number_height,
        }
    }
}

/// Shapes the visible part of `row`.
pub(crate) fn shape_row(
    buffer: &Buffer,
    row: usize,
    editor: &Editor,
    text_width: f32,
    metrics: &Metrics,
    style: &RunStyle<'_>,
    window: &Window,
) -> ShapedRow {
    let line = buffer.line(row);
    let line_start = buffer.line_range(row).start;
    let chars = line.len_chars();
    let columns = column_window(chars, editor.scroll_x, text_width, metrics.char_width);
    let start = line_start + line.char_to_byte(columns.start);
    let end = line_start + line.char_to_byte(columns.end);
    let source = line.slice(columns.clone()).to_string();
    let display = DisplayLine::new(&source, editor.settings.tab_size, columns.start);
    let spans = editor
        .syntax
        .as_ref()
        .map(|syntax| syntax.highlights(buffer, row, start..end))
        .unwrap_or_default();
    let runs = text_runs(&display, start..end, &spans, editor.marked.as_ref(), style);
    let shaped = window.text_system().shape_line(
        SharedString::from(display.text.clone()),
        metrics.font_size,
        &runs,
        None,
    );
    let width = if columns.len() < chars {
        to_f32(chars) * metrics.char_width
    } else {
        shaped.width().as_f32()
    };
    ShapedRow {
        row,
        start,
        end,
        x: to_f32(columns.start) * metrics.char_width,
        width,
        display,
        shaped,
    }
}

/// x of the primary cursor from the text start, for scrolling to it: exact
/// for normal lines, from the column for long lines.
pub(crate) fn head_x(
    buffer: &Buffer,
    head: usize,
    tab_size: usize,
    metrics: &Metrics,
    window: &Window,
) -> f32 {
    let row = buffer.offset_to_point(head).row;
    let line = buffer.line(row);
    if line.len_chars() > LONG_LINE_CHARS {
        return to_f32(buffer.char_column(head)) * metrics.char_width;
    }
    let start = buffer.line_range(row).start;
    let display = DisplayLine::new(&line.to_string(), tab_size, 0);
    let run = TextRun {
        len: display.text.len(),
        font: metrics.font.clone(),
        color: Hsla::default(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let index = display.to_display(head - start);
    let shaped = window.text_system().shape_line(
        SharedString::from(display.text.clone()),
        metrics.font_size,
        &[run],
        None,
    );
    shaped.x_for_index(index).as_f32()
}
