//! Drawing an editor: only the visible rows, and of a long line only the
//! visible columns.

use std::rc::Rc;

use gpui::{
    App, Bounds, ContentMask, Element, ElementId, ElementInputHandler, Entity, Font,
    GlobalElementId, Hsla, InspectorElementId, IntoElement, LayoutId, PaintQuad, Pixels,
    ShapedLine, SharedString, Style, TextAlign, TextRun, Window, fill, point, px, relative, size,
};
use text::Buffer;
use theme::{ActiveTheme as _, Theme};

use crate::display::DisplayLine;
use crate::editor::Editor;
use crate::layout::{LayoutSnapshot, ShapedRow};
use crate::runs::{RunStyle, text_runs};
use crate::scene::{Scene, near_rows};
use crate::viewport::{
    LONG_LINE_CHARS, column_window, follow_row, follow_x, max_scroll_y, to_f32, visible_rows,
};

/// The fewest digits the line number column is wide for, so the text does
/// not move while a short file grows (four in the mockups).
const MIN_LINE_NUMBER_DIGITS: usize = 4;

/// Smallest scrollbar thumb, in pixels, so it stays easy to see.
const MIN_THUMB_HEIGHT: f32 = 24.0;

/// Columns kept visible right of the cursor when scrolling follows it.
const AUTOSCROLL_MARGIN_COLUMNS: f32 = 4.0;

pub(crate) struct EditorElement {
    editor: Entity<Editor>,
}

impl EditorElement {
    pub(crate) fn new(editor: Entity<Editor>) -> Self {
        Self { editor }
    }
}

impl IntoElement for EditorElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Fonts and measures of the text, from the theme.
struct Metrics {
    font: Font,
    font_size: Pixels,
    number_size: Pixels,
    line_height: f32,
    char_width: f32,
    number_char_width: f32,
    /// Height of a line number glyph box: numbers sit at the top of the
    /// row, as in the mockups.
    number_height: f32,
}

impl Metrics {
    fn new(theme: &Theme, window: &Window) -> Self {
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

/// Everything prepared for painting.
pub(crate) struct Prepaint {
    /// Clip of the text area (text must not cover the line numbers).
    text_area: Bounds<Pixels>,
    /// Current line highlights, selections and indent guides.
    backgrounds: Vec<PaintQuad>,
    numbers: Vec<(gpui::Point<Pixels>, ShapedLine)>,
    /// Line height the numbers are painted with.
    number_height: f32,
    cursors: Vec<PaintQuad>,
    scrollbar: Option<PaintQuad>,
    /// Text origins of the visible rows (same order as the snapshot rows).
    origins: Vec<gpui::Point<Pixels>>,
    snapshot: Option<LayoutSnapshot>,
}

impl Element for EditorElement {
    type RequestLayoutState = ();
    type PrepaintState = Prepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let _span = tracing::trace_span!("editor.prepaint").entered();
        let theme = cx.theme().clone();
        let metrics = Metrics::new(&theme, window);
        let frame = Frame::new(
            &theme,
            &metrics,
            bounds,
            self.editor.read(cx).document.buffer(),
        );
        self.editor.update(cx, |editor, _| {
            frame.follow_cursor(editor, &metrics, window);
        });
        let editor = self.editor.read(cx);
        frame.prepaint(editor, &theme, &metrics, window)
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.editor.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.editor.clone()),
            cx,
        );
        let line_height = px(cx.theme().metrics.size.code_line_height);
        let snapshot = prepaint.snapshot.take();
        window.with_content_mask(
            Some(ContentMask {
                bounds: prepaint.text_area,
            }),
            |window| {
                for quad in prepaint.backgrounds.drain(..) {
                    window.paint_quad(quad);
                }
                if let Some(snapshot) = &snapshot {
                    for (row, origin) in snapshot.rows.iter().zip(&prepaint.origins) {
                        let _ = row.shaped.paint(
                            *origin,
                            line_height,
                            TextAlign::Left,
                            None,
                            window,
                            cx,
                        );
                    }
                }
                for quad in prepaint.cursors.drain(..) {
                    window.paint_quad(quad);
                }
            },
        );
        let number_line_height = px(prepaint.number_height);
        for (origin, number) in prepaint.numbers.drain(..) {
            let _ = number.paint(
                origin,
                number_line_height,
                TextAlign::Left,
                None,
                window,
                cx,
            );
        }
        if let Some(thumb) = prepaint.scrollbar.take() {
            window.paint_quad(thumb);
        }
        if let Some(snapshot) = snapshot {
            self.editor.update(cx, |editor, _| {
                editor.scroll_x = snapshot.scroll_x;
                editor.layout = Some(Rc::new(snapshot));
            });
        }
    }
}

/// Geometry of one frame that does not depend on the scroll position.
struct Frame {
    bounds: Bounds<Pixels>,
    /// x of the right edge of the line numbers, from `bounds.left`.
    numbers_end: f32,
    /// x of the start of the text, from `bounds.left`.
    text_x: f32,
    /// y of the first row, from `bounds.top`.
    text_y: f32,
    text_width: f32,
    text_height: f32,
}

impl Frame {
    fn new(theme: &Theme, metrics: &Metrics, bounds: Bounds<Pixels>, buffer: &Buffer) -> Self {
        let size = &theme.metrics.size;
        let digits = buffer
            .line_count()
            .to_string()
            .len()
            .max(MIN_LINE_NUMBER_DIGITS);
        let numbers_end = size.editor_inset + to_f32(digits) * metrics.number_char_width;
        let text_x = numbers_end + size.editor_gutter_gap;
        Self {
            bounds,
            numbers_end,
            text_x,
            text_y: size.editor_top,
            text_width: (bounds.size.width.as_f32() - text_x - size.editor_right).max(0.0),
            text_height: (bounds.size.height.as_f32() - size.editor_top).max(0.0),
        }
    }

    /// Scrolls so the primary cursor is visible, if the editor asked for it.
    fn follow_cursor(&self, editor: &mut Editor, metrics: &Metrics, window: &Window) {
        let buffer = editor.document.buffer();
        let max_y = max_scroll_y(buffer.line_count(), metrics.line_height);
        if editor.autoscroll {
            editor.autoscroll = false;
            let head = editor.document.selections().primary().head;
            let row = buffer.offset_to_point(head).row;
            editor.scroll_y =
                follow_row(editor.scroll_y, self.text_height, metrics.line_height, row);
            let x = head_x(buffer, head, editor.settings.tab_size, metrics, window);
            let margin = metrics.char_width * AUTOSCROLL_MARGIN_COLUMNS;
            editor.scroll_x = follow_x(editor.scroll_x, self.text_width, x, margin);
        }
        editor.scroll_y = editor.scroll_y.clamp(0.0, max_y);
    }

    fn prepaint(
        &self,
        editor: &Editor,
        theme: &Theme,
        metrics: &Metrics,
        window: &mut Window,
    ) -> Prepaint {
        let buffer = editor.document.buffer();
        let left = self.bounds.left().as_f32();
        let rows = visible_rows(
            editor.scroll_y,
            self.text_height,
            metrics.line_height,
            buffer.line_count(),
        );
        let colors = editor
            .syntax
            .as_ref()
            .map(|syntax| {
                if !rows.is_empty() {
                    let visible =
                        buffer.line_range(rows.start).start..buffer.line_range(rows.end - 1).end;
                    syntax.prepare(buffer, visible);
                }
                syntax.colors(&theme.syntax)
            })
            .unwrap_or_default();
        let style = RunStyle {
            font: &metrics.font,
            plain: Hsla::from(theme.syntax.plain),
            colors: &colors,
        };
        let shaped: Vec<ShapedRow> = rows
            .clone()
            .map(|row| {
                shape_row(
                    buffer,
                    row,
                    editor,
                    self.text_width,
                    metrics,
                    &style,
                    window,
                )
            })
            .collect();
        let widest = shaped.iter().map(|row| row.width).fold(0.0_f32, f32::max);
        let max_scroll_x = (widest + metrics.char_width * 2.0 - self.text_width).max(0.0);
        let scroll_x = editor.scroll_x.clamp(0.0, max_scroll_x);
        let snapshot = LayoutSnapshot {
            scroll_x,
            max_scroll_x,
            text_left: left + self.text_x - scroll_x,
            text_top: self.bounds.top().as_f32() + self.text_y - editor.scroll_y,
            line_height: metrics.line_height,
            char_width: metrics.char_width,
            text_height: self.text_height,
            rows: shaped,
        };
        let selections: Vec<text::Selection> =
            editor.document.selections().iter().copied().collect();
        let scene = Scene {
            editor,
            theme,
            snapshot: &snapshot,
            near: near_rows(&selections, buffer, &rows),
            rows,
            numbers_end: left + self.numbers_end,
            highlight_right: self.bounds.right().as_f32() - theme.metrics.size.editor_right,
        };
        let mut backgrounds = scene.current_lines();
        backgrounds.extend(scene.selections());
        backgrounds.extend(scene.guides());
        let cursors = scene.cursors();
        let numbers = scene.numbers(&metrics.font, metrics.number_size, window);
        let origins = snapshot
            .rows
            .iter()
            .map(|row| {
                let top = snapshot.text_top + to_f32(row.row) * snapshot.line_height;
                point(px(snapshot.text_left + row.x), px(top))
            })
            .collect();
        Prepaint {
            text_area: Bounds::new(
                point(px(left + self.text_x), self.bounds.top()),
                size(
                    px(self.text_width + theme.metrics.size.editor_right),
                    self.bounds.size.height,
                ),
            ),
            backgrounds,
            numbers,
            number_height: metrics.number_height,
            cursors,
            scrollbar: self.thumb(editor, buffer, theme, metrics),
            origins,
            snapshot: Some(snapshot),
        }
    }

    /// The vertical scrollbar thumb, if the text is taller than the view.
    fn thumb(
        &self,
        editor: &Editor,
        buffer: &Buffer,
        theme: &Theme,
        metrics: &Metrics,
    ) -> Option<PaintQuad> {
        let size = &theme.metrics.size;
        let max_y = max_scroll_y(buffer.line_count(), metrics.line_height);
        let content = to_f32(buffer.line_count()) * metrics.line_height;
        if content <= self.text_height || max_y <= 0.0 {
            return None;
        }
        let track = self.bounds.size.height.as_f32();
        let thumb = (track * self.text_height / (max_y + self.text_height)).max(MIN_THUMB_HEIGHT);
        let y = self.bounds.top().as_f32() + (track - thumb) * (editor.scroll_y / max_y);
        let x = self.bounds.right().as_f32() - size.scrollbar_inset - size.scrollbar_width;
        Some(
            fill(
                Bounds::new(
                    point(px(x), px(y)),
                    gpui::size(px(size.scrollbar_width), px(thumb)),
                ),
                Hsla::from(theme.colors.bg_scrollbar_thumb),
            )
            .corner_radii(px(size.scrollbar_width / 2.0)),
        )
    }
}

/// Shapes the visible part of `row`.
fn shape_row(
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
fn head_x(
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
