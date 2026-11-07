//! Drawing a terminal: the cell grid sized to the bounds, span
//! backgrounds, the selection, the text and the cursor.

use gpui::{
    App, Bounds, ContentMask, Element, ElementId, ElementInputHandler, Entity, Font, FontStyle,
    FontWeight, GlobalElementId, Hsla, InspectorElementId, IntoElement, LayoutId, PaintQuad,
    Pixels, Point, ShapedLine, SharedString, StrikethroughStyle, Style, TextAlign, TextRun,
    UnderlineStyle, Window, fill, outline, point, px, relative, size,
};
use process::PtySize;
use terminal::{CursorShape, Rgb, Snapshot, Span};
use theme::{ActiveTheme as _, Theme};

use super::view::TerminalView;

/// Thickness of underlines, strikethroughs and the underline cursor.
const LINE_THICKNESS: f32 = 1.0;

/// Cell geometry of a drawn terminal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CellLayout {
    /// Top left of the first cell.
    pub(crate) origin: Point<Pixels>,
    pub(crate) cell_width: f32,
    pub(crate) line_height: f32,
    /// Where the cursor is, for the input method's candidate window.
    pub(crate) cursor_bounds: Bounds<Pixels>,
}

pub(crate) struct TerminalElement {
    view: Entity<TerminalView>,
}

impl TerminalElement {
    pub(crate) fn new(view: Entity<TerminalView>) -> Self {
        Self { view }
    }
}

impl IntoElement for TerminalElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// A shaped span and where it goes.
struct Text {
    origin: Point<Pixels>,
    line: ShapedLine,
}

pub(crate) struct Prepaint {
    quads: Vec<PaintQuad>,
    texts: Vec<Text>,
    /// Painted after the text: the cursor and the character over it.
    cursor: Vec<PaintQuad>,
    cursor_texts: Vec<Text>,
    line_height: Pixels,
}

/// Fonts and cell size from the theme.
struct Metrics {
    font: Font,
    font_size: Pixels,
    cell_width: f32,
    line_height: f32,
}

impl Metrics {
    fn new(theme: &Theme, window: &Window) -> Self {
        let font = gpui::font(SharedString::from(theme.fonts.mono.clone()));
        let font_size = px(theme.metrics.font_size.code_small);
        let text_system = window.text_system();
        let font_id = text_system.resolve_font(&font);
        let cell_width = text_system
            .advance(font_id, font_size, 'm')
            .map_or(font_size.as_f32() * 0.6, |advance| advance.width.as_f32());
        Self {
            font,
            font_size,
            cell_width: cell_width.max(1.0),
            line_height: theme.metrics.size.terminal_line_height.max(1.0),
        }
    }
}

pub(crate) fn hsla(color: Rgb) -> Hsla {
    gpui::Rgba {
        r: f32::from(color.r) / 255.0,
        g: f32::from(color.g) / 255.0,
        b: f32::from(color.b) / 255.0,
        a: 1.0,
    }
    .into()
}

/// Columns or rows that fit in `extent`, at least one.
fn cells_in(extent: f32, cell: f32) -> u16 {
    // The quotient is non-negative and clamped to the u16 range.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let cells = (extent / cell).floor().clamp(1.0, f32::from(u16::MAX)) as u16;
    cells
}

/// A pixel size for the pseudo-terminal.
fn pixels(value: f32) -> u16 {
    // Cell sizes are a few dozen pixels.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let pixels = value.round().clamp(1.0, f32::from(u16::MAX)) as u16;
    pixels
}

fn column_x(layout: &CellLayout, column: usize) -> Pixels {
    // Column counts are far below f32 precision limits.
    #[allow(clippy::cast_precision_loss)]
    let x = column as f32 * layout.cell_width;
    layout.origin.x + px(x)
}

fn row_y(layout: &CellLayout, row: usize) -> Pixels {
    #[allow(clippy::cast_precision_loss)]
    let y = row as f32 * layout.line_height;
    layout.origin.y + px(y)
}

fn cells_width(layout: &CellLayout, cells: usize) -> Pixels {
    #[allow(clippy::cast_precision_loss)]
    let width = cells as f32 * layout.cell_width;
    px(width)
}

fn shape(span: &Span, color: Hsla, metrics: &Metrics, window: &Window) -> ShapedLine {
    let mut font = metrics.font.clone();
    if span.style.bold {
        font.weight = FontWeight::BOLD;
    }
    if span.style.italic {
        font.style = FontStyle::Italic;
    }
    let run = TextRun {
        len: span.text.len(),
        font,
        color,
        background_color: None,
        underline: span.style.underline.then_some(UnderlineStyle {
            thickness: px(LINE_THICKNESS),
            color: Some(color),
            wavy: false,
        }),
        strikethrough: span.style.strikethrough.then_some(StrikethroughStyle {
            thickness: px(LINE_THICKNESS),
            color: Some(color),
        }),
    };
    window.text_system().shape_line(
        SharedString::from(span.text.clone()),
        metrics.font_size,
        &[run],
        None,
    )
}

impl Element for TerminalElement {
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
        let _span = tracing::trace_span!("terminal.prepaint").entered();
        let theme = cx.theme().clone();
        let metrics = Metrics::new(&theme, window);
        let size = PtySize {
            columns: cells_in(bounds.size.width.as_f32(), metrics.cell_width),
            rows: cells_in(bounds.size.height.as_f32(), metrics.line_height),
            cell_width: pixels(metrics.cell_width),
            cell_height: pixels(metrics.line_height),
        };
        let snapshot = self.view.update(cx, |view, cx| {
            view.session.resize(size);
            view.frame(cx)
        });
        let mut layout = CellLayout {
            origin: bounds.origin,
            cell_width: metrics.cell_width,
            line_height: metrics.line_height,
            cursor_bounds: Bounds::new(
                bounds.origin,
                gpui::size(px(metrics.cell_width), px(metrics.line_height)),
            ),
        };
        let view = self.view.read(cx);
        let mut prepaint = Prepaint {
            quads: Vec::new(),
            texts: Vec::new(),
            cursor: Vec::new(),
            cursor_texts: Vec::new(),
            line_height: px(metrics.line_height),
        };
        let background = hsla(snapshot.background);
        if snapshot.background != super::palette::palette(&theme).background {
            // The program changed the default background (OSC 11).
            prepaint.quads.push(fill(bounds, background));
        }
        paint_rows(
            &snapshot,
            &layout,
            &metrics,
            &theme,
            view.focused,
            &mut prepaint,
            window,
        );
        if let Some(cursor) = &snapshot.cursor {
            let origin = point(column_x(&layout, cursor.column), row_y(&layout, cursor.row));
            let cell = gpui::size(cells_width(&layout, cursor.cells), px(metrics.line_height));
            layout.cursor_bounds = Bounds::new(origin, cell);
            if view.cursor_visible() && !view.exited && !cursor.hidden {
                cursor_quads(
                    cursor,
                    origin,
                    cell,
                    &theme,
                    &metrics,
                    view.focused,
                    background,
                    &snapshot,
                    &mut prepaint,
                    window,
                );
            }
        }
        if let Some(marked) = &view.marked {
            marked_text(
                marked,
                layout.cursor_bounds.origin,
                &theme,
                &metrics,
                &mut prepaint,
                window,
            );
        }
        self.view.update(cx, |view, _| view.layout = Some(layout));
        prepaint
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
        let focus_handle = self.view.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.view.clone()),
            cx,
        );
        let line_height = prepaint.line_height;
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            for quad in prepaint.quads.drain(..) {
                window.paint_quad(quad);
            }
            for text in &prepaint.texts {
                let _ =
                    text.line
                        .paint(text.origin, line_height, TextAlign::Left, None, window, cx);
            }
            for quad in prepaint.cursor.drain(..) {
                window.paint_quad(quad);
            }
            for text in &prepaint.cursor_texts {
                let _ =
                    text.line
                        .paint(text.origin, line_height, TextAlign::Left, None, window, cx);
            }
        });
    }
}

/// Backgrounds, selection and text of every row.
fn paint_rows(
    snapshot: &Snapshot,
    layout: &CellLayout,
    metrics: &Metrics,
    theme: &Theme,
    focused: bool,
    prepaint: &mut Prepaint,
    window: &Window,
) {
    let height = px(metrics.line_height);
    let selection = if focused {
        theme.colors.bg_selection
    } else {
        theme.colors.bg_selection_inactive
    };
    for (row, content) in snapshot.rows.iter().enumerate() {
        let y = row_y(layout, row);
        for span in &content.spans {
            if let Some(background) = span.background {
                prepaint.quads.push(fill(
                    Bounds::new(
                        point(column_x(layout, span.column), y),
                        size(cells_width(layout, span.cells), height),
                    ),
                    hsla(background),
                ));
            }
        }
        if let Some((start, end)) = content.selection {
            prepaint.quads.push(fill(
                Bounds::new(
                    point(column_x(layout, start), y),
                    size(cells_width(layout, end - start), height),
                ),
                Hsla::from(selection),
            ));
        }
        for span in &content.spans {
            prepaint.texts.push(Text {
                origin: point(column_x(layout, span.column), y),
                line: shape(span, hsla(span.foreground), metrics, window),
            });
        }
    }
}

/// The cursor in its shape; a block cursor redraws its character in the
/// background color, an unfocused terminal shows an outline.
#[allow(clippy::too_many_arguments)] // One call site; a struct would only rename them.
fn cursor_quads(
    cursor: &terminal::Cursor,
    origin: Point<Pixels>,
    cell: gpui::Size<Pixels>,
    theme: &Theme,
    metrics: &Metrics,
    focused: bool,
    background: Hsla,
    snapshot: &Snapshot,
    prepaint: &mut Prepaint,
    window: &Window,
) {
    let color: Hsla = theme.terminal.cursor.into();
    let thin = px(theme.metrics.size.cursor_width.max(LINE_THICKNESS));
    let drawn = if focused {
        cursor.shape
    } else {
        CursorShape::HollowBlock
    };
    match drawn {
        CursorShape::Block => {
            prepaint.cursor.push(fill(Bounds::new(origin, cell), color));
            if !cursor.text.is_empty() {
                let span = snapshot.rows[cursor.row].spans.iter().find(|span| {
                    span.column <= cursor.column && cursor.column < span.column + span.cells
                });
                let style = span.map(|span| span.style).unwrap_or_default();
                let under = Span {
                    column: cursor.column,
                    cells: cursor.cells,
                    text: cursor.text.clone(),
                    foreground: Rgb::new(0, 0, 0),
                    background: None,
                    style,
                };
                prepaint.cursor_texts.push(Text {
                    origin,
                    line: shape(&under, background, metrics, window),
                });
            }
        }
        CursorShape::HollowBlock => {
            prepaint.cursor.push(outline(
                Bounds::new(origin, cell),
                color,
                gpui::BorderStyle::Solid,
            ));
        }
        CursorShape::Beam => {
            prepaint
                .cursor
                .push(fill(Bounds::new(origin, size(thin, cell.height)), color));
        }
        CursorShape::Underline => {
            let top = origin.y + cell.height - thin;
            prepaint.cursor.push(fill(
                Bounds::new(point(origin.x, top), size(cell.width, thin)),
                color,
            ));
        }
    }
}

/// Text an input method is composing, underlined over the cells at the
/// cursor.
fn marked_text(
    marked: &str,
    origin: Point<Pixels>,
    theme: &Theme,
    metrics: &Metrics,
    prepaint: &mut Prepaint,
    window: &Window,
) {
    let span = Span {
        column: 0,
        cells: marked.chars().count(),
        text: marked.to_owned(),
        foreground: Rgb::new(0, 0, 0),
        background: None,
        style: terminal::CellStyle {
            underline: true,
            ..terminal::CellStyle::default()
        },
    };
    let line = shape(&span, theme.colors.text_primary.into(), metrics, window);
    prepaint.cursor.push(fill(
        Bounds::new(origin, size(line.width(), px(metrics.line_height))),
        Hsla::from(theme.colors.bg_editor),
    ));
    prepaint.cursor_texts.push(Text { origin, line });
}
