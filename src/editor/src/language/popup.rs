//! Look of the editor's popups (hover, completion list, its
//! documentation), as the command palette: `bg.overlay`, `border.overlay`,
//! `radius.md` (ADR 0007). Positions come from the last frame's layout.

use gpui::{
    AnyElement, Div, HighlightStyle, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, Point, SharedString, StatefulInteractiveElement as _, Styled as _,
    StyledText, deferred, div, point, px,
};
use theme::Theme;

use crate::editor::Editor;
use crate::language::markdown::Block;

/// Draw order of editor popups above other deferred elements.
const POPUP_PRIORITY: usize = 1;

/// Below this fraction of the view's height, popups open above the line.
const LOWER_PART: f32 = 0.6;

/// Where a popup opens relative to its line of text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Side {
    Above,
    Below,
}

/// The box of a popup.
pub(crate) fn container(theme: &Theme) -> Div {
    let metrics = &theme.metrics;
    div()
        .flex()
        .flex_col()
        .rounded(px(metrics.radius.md))
        .border_1()
        .border_color(theme.colors.border_overlay)
        .bg(theme.colors.bg_overlay)
        .text_size(px(metrics.font_size.md))
        .text_color(theme.colors.text_secondary)
        .font_family(SharedString::from(theme.fonts.ui.clone()))
        // Clicks and the wheel stay in the popup.
        .occlude()
}

/// `blocks` as elements: text in the UI font, code in the code font with
/// `colors` (the editor's syntax colors by highlight id).
pub(crate) fn blocks(blocks: &[Block], colors: &[Option<Hsla>], theme: &Theme) -> Vec<AnyElement> {
    let metrics = &theme.metrics;
    blocks
        .iter()
        .map(|block| match block {
            Block::Text(text) => div()
                .px(px(metrics.space.md))
                .py(px(metrics.space.xs))
                .child(text.clone())
                .into_any_element(),
            Block::Rule => div()
                .my(px(metrics.space.xs))
                .h(px(1.0))
                .bg(theme.colors.border_divider)
                .into_any_element(),
            Block::Code { text, spans, .. } => {
                let highlights: Vec<_> = spans
                    .iter()
                    .filter_map(|span| {
                        let index = usize::try_from(span.highlight.0).ok()?;
                        let color = (*colors.get(index)?)?;
                        Some((
                            span.range.clone(),
                            HighlightStyle {
                                color: Some(color),
                                ..HighlightStyle::default()
                            },
                        ))
                    })
                    .collect();
                div()
                    .px(px(metrics.space.md))
                    .py(px(metrics.space.xs))
                    .font_family(SharedString::from(theme.fonts.mono.clone()))
                    .text_color(Hsla::from(theme.syntax.plain))
                    .child(StyledText::new(text.clone()).with_highlights(highlights))
                    .into_any_element()
            }
        })
        .collect()
}

impl Editor {
    /// Window position of the corner a popup for `offset` hangs from, and
    /// the side of the line it opens on: below in the upper part of the
    /// view, above in the lower part.
    pub(crate) fn popup_anchor(&self, offset: usize, gap: f32) -> Option<(Point<Pixels>, Side)> {
        let layout = self.layout.as_ref()?;
        let buffer = self.document.buffer();
        let row = buffer.offset_to_point(offset).row;
        if !layout.rows.iter().any(|shaped| shaped.row == row) {
            return None;
        }
        let bounds = layout.bounds_of(buffer, offset);
        let first = layout.rows.first().map_or(row, |shaped| shaped.row);
        let view_top =
            layout.text_top + crate::render::viewport::to_f32(first) * layout.line_height;
        let middle = view_top + layout.text_height * LOWER_PART;
        let top = bounds.top().as_f32();
        if top > middle {
            Some((point(bounds.left(), px(top - gap)), Side::Above))
        } else {
            Some((point(bounds.left(), bounds.bottom() + px(gap)), Side::Below))
        }
    }
}

/// `content` drawn above everything at `anchor`; it switches sides when
/// it does not fit in the window.
pub(crate) fn place(content: impl IntoElement, anchor: Point<Pixels>, side: Side) -> AnyElement {
    let corner = match side {
        Side::Above => gpui::Anchor::BottomLeft,
        Side::Below => gpui::Anchor::TopLeft,
    };
    deferred(
        gpui::anchored()
            .position(anchor)
            .anchor(corner)
            .child(content),
    )
    .with_priority(POPUP_PRIORITY)
    .into_any_element()
}

/// A scrollable column for long popup content.
pub(crate) fn scrolling(id: &'static str, max_height: f32) -> gpui::Stateful<Div> {
    div()
        .id(id)
        .flex()
        .flex_col()
        .max_h(px(max_height))
        .overflow_y_scroll()
}
