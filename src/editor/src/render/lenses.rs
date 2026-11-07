//! Code lenses at the end of their lines, as "1 implementation" in
//! `editor.svg`: the code font at `font_size.xs` in `syntax.inlay_hint`,
//! `code_lens.gap` after the code, `space.lg` between lenses of a line.

use gpui::{Bounds, Hsla, Pixels, Point, ShapedLine, SharedString, TextRun, Window, point, px};
use theme::Theme;

use crate::editor::Editor;
use crate::render::layout::LayoutSnapshot;
use crate::render::shaping::Metrics;
use crate::render::viewport::to_f32;

/// A lens shaped for the frame.
pub(crate) struct LensLabel {
    /// Window position of its row's top left.
    pub(crate) origin: Point<Pixels>,
    pub(crate) shaped: ShapedLine,
    /// Where a click acts on it, in window coordinates.
    pub(crate) bounds: Bounds<Pixels>,
    /// Index into the editor's lenses.
    pub(crate) index: usize,
}

/// The lenses of the rows in `snapshot`.
pub(crate) fn lens_labels(
    editor: &Editor,
    snapshot: &LayoutSnapshot,
    theme: &Theme,
    metrics: &Metrics,
    window: &Window,
) -> Vec<LensLabel> {
    let size = px(theme.metrics.font_size.xs);
    let color = Hsla::from(theme.syntax.inlay_hint);
    let mut labels = Vec::new();
    for row in &snapshot.rows {
        let lenses = editor.code_lenses_on(row.row);
        if lenses.is_empty() {
            continue;
        }
        let top = snapshot.text_top + to_f32(row.row) * snapshot.line_height;
        let mut x = snapshot.text_left + row.width + theme.metrics.size.code_lens_gap;
        for (index, title) in lenses {
            let run = TextRun {
                len: title.len(),
                font: metrics.font.clone(),
                color,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let shaped = window.text_system().shape_line(
                SharedString::from(title.to_owned()),
                size,
                &[run],
                None,
            );
            let width = shaped.width().as_f32();
            let origin = point(px(x), px(top));
            labels.push(LensLabel {
                origin,
                bounds: Bounds::new(origin, gpui::size(px(width), px(snapshot.line_height))),
                shaped,
                index,
            });
            x += width + theme.metrics.space.lg;
        }
    }
    labels
}
