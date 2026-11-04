//! Drawing an editor: only the visible rows, and of a long line only the
//! visible columns.

use std::rc::Rc;

use gpui::{
    App, Bounds, ContentMask, CursorStyle, Element, ElementId, ElementInputHandler, Entity,
    GlobalElementId, Hitbox, HitboxBehavior, Hsla, InspectorElementId, IntoElement, LayoutId,
    PaintQuad, Pixels, ShapedLine, Style, TextAlign, Window, point, px, relative, size,
};
use text::Buffer;
use theme::{ActiveTheme as _, Theme};

use crate::decorations::minimap::{Minimap, MinimapView};
use crate::decorations::scrollbar::{Scrollbar, ScrollbarFrame};
use crate::editor::Editor;
use crate::render::layout::{LayoutSnapshot, ShapedRow};
use crate::render::runs::RunStyle;
use crate::render::scene::{Scene, near_rows};
use crate::render::shaping::{Metrics, head_x, shape_row};
use crate::render::viewport::{follow_row, follow_x, max_scroll_y, to_f32, visible_rows};

/// The fewest digits the line number column is wide for, so the text does
/// not move while a short file grows (four in the mockups).
const MIN_LINE_NUMBER_DIGITS: usize = 4;

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
    /// Background, bars and visible part of the minimap.
    minimap: Vec<PaintQuad>,
    /// Areas with the arrow cursor (scrollbar, minimap).
    arrow_areas: Vec<Hitbox>,
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
        let editor = self.editor.read(cx);
        let frame = Frame::new(
            &theme,
            &metrics,
            bounds,
            editor.document.buffer(),
            editor.settings.minimap,
        );
        self.editor.update(cx, |editor, _| {
            frame.follow_cursor(editor, &metrics, window);
        });
        let editor = self.editor.read(cx);
        let mut prepaint = frame.prepaint(editor, &theme, &metrics, window);
        if let Some(snapshot) = &prepaint.snapshot {
            let areas = [
                snapshot.scrollbar.map(|scrollbar| scrollbar.hit_bounds()),
                snapshot.minimap.map(|minimap| {
                    Bounds::new(
                        point(px(minimap.left), px(minimap.top)),
                        size(px(minimap.width), px(minimap.height)),
                    )
                }),
            ];
            prepaint.arrow_areas = areas
                .into_iter()
                .flatten()
                .map(|area| window.insert_hitbox(area, HitboxBehavior::Normal))
                .collect();
        }
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
        for quad in prepaint.minimap.drain(..) {
            window.paint_quad(quad);
        }
        if let Some(thumb) = prepaint.scrollbar.take() {
            window.paint_quad(thumb);
        }
        for area in &prepaint.arrow_areas {
            window.set_cursor_style(CursorStyle::Arrow, area);
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
    /// Window x of the right edge of the text area: the minimap's left
    /// edge less the margin, or the editor's right edge less it.
    text_right: f32,
    /// Window x of the minimap's right edge, if it is shown.
    minimap_right: Option<f32>,
}

impl Frame {
    fn new(
        theme: &Theme,
        metrics: &Metrics,
        bounds: Bounds<Pixels>,
        buffer: &Buffer,
        minimap: bool,
    ) -> Self {
        let size = &theme.metrics.size;
        let digits = buffer
            .line_count()
            .to_string()
            .len()
            .max(MIN_LINE_NUMBER_DIGITS);
        let numbers_end = size.editor_inset + to_f32(digits) * metrics.number_char_width;
        let text_x = numbers_end + size.editor_gutter_gap;
        let right = bounds.right().as_f32();
        // The minimap only while the text keeps a usable width beside it.
        let room = bounds.size.width.as_f32() - text_x;
        let needed = size.minimap_width + size.minimap_right + size.editor_right * 2.0;
        let minimap_right = (minimap && room > needed).then_some(right - size.minimap_right);
        let text_right =
            minimap_right.map_or(right, |edge| edge - size.minimap_width) - size.editor_right;
        Self {
            bounds,
            numbers_end,
            text_x,
            text_y: size.editor_top,
            text_width: (text_right - bounds.left().as_f32() - text_x).max(0.0),
            text_height: (bounds.size.height.as_f32() - size.editor_top).max(0.0),
            text_right,
            minimap_right,
        }
    }

    /// Shapes the visible rows with their highlights.
    fn shape_rows(
        &self,
        editor: &Editor,
        theme: &Theme,
        metrics: &Metrics,
        rows: std::ops::Range<usize>,
        window: &Window,
    ) -> Vec<ShapedRow> {
        let buffer = editor.document.buffer();
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
        rows.map(|row| {
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
        .collect()
    }

    /// The scrollbar and the minimap for the editor's scroll position.
    fn scroll_parts(
        &self,
        editor: &Editor,
        theme: &Theme,
        metrics: &Metrics,
    ) -> (Option<Scrollbar>, Option<Minimap>) {
        let buffer = editor.document.buffer();
        let tokens = &theme.metrics.size;
        let top = self.bounds.top().as_f32();
        let max_y = max_scroll_y(buffer.line_count(), metrics.line_height);
        let scrollbar = Scrollbar::new(
            &ScrollbarFrame {
                right: self.bounds.right().as_f32(),
                top,
                height: self.bounds.size.height.as_f32(),
                text_height: self.text_height,
                width: tokens.scrollbar_width,
                inset: tokens.scrollbar_inset,
            },
            to_f32(buffer.line_count()) * metrics.line_height,
            editor.scroll_y,
            max_y,
        );
        let minimap = self.minimap_right.and_then(|right| {
            Minimap::new(
                right,
                top + self.text_y,
                self.bounds.size.height.as_f32() - self.text_y - tokens.minimap_bottom,
                &MinimapView {
                    line_count: buffer.line_count(),
                    scroll_y: editor.scroll_y,
                    max_scroll_y: max_y,
                    line_height: metrics.line_height,
                    text_height: self.text_height,
                },
                theme,
            )
        });
        (scrollbar, minimap)
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
        let shaped = self.shape_rows(editor, theme, metrics, rows.clone(), window);
        let widest = shaped.iter().map(|row| row.width).fold(0.0_f32, f32::max);
        let max_scroll_x = (widest + metrics.char_width * 2.0 - self.text_width).max(0.0);
        let scroll_x = editor.scroll_x.clamp(0.0, max_scroll_x);
        let (scrollbar, minimap) = self.scroll_parts(editor, theme, metrics);
        let snapshot = LayoutSnapshot {
            scroll_x,
            max_scroll_x,
            text_left: left + self.text_x - scroll_x,
            text_top: self.bounds.top().as_f32() + self.text_y - editor.scroll_y,
            line_height: metrics.line_height,
            char_width: metrics.char_width,
            text_height: self.text_height,
            rows: shaped,
            scrollbar,
            minimap,
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
            highlight_right: self.text_right,
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
                    px(self.text_right - left - self.text_x + theme.metrics.size.editor_right),
                    self.bounds.size.height,
                ),
            ),
            backgrounds,
            numbers,
            number_height: metrics.number_height,
            cursors,
            scrollbar: snapshot.scrollbar.map(|scrollbar| {
                let color = if editor.scrollbar_active() {
                    theme.colors.bg_scrollbar_thumb_hover
                } else {
                    theme.colors.bg_scrollbar_thumb
                };
                scrollbar.quad(Hsla::from(color))
            }),
            minimap: snapshot
                .minimap
                .map(|minimap| editor.minimap_quads(&minimap, theme))
                .unwrap_or_default(),
            arrow_areas: Vec::new(),
            origins,
            snapshot: Some(snapshot),
        }
    }
}
