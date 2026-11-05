//! Diagnostic marks under the text: waves under errors, warnings and
//! information, dots under hints. Only diagnostics on visible rows are
//! looked at.

use gpui::{Bounds, Hsla, PaintQuad, Pixels, Point, UnderlineStyle, Window, fill, point, px, size};
use language::Severity;

use crate::render::scene::Scene;

/// A mark to paint after the text.
pub(crate) enum Mark {
    /// A wavy underline from `origin`, `width` long.
    Wave {
        origin: Point<Pixels>,
        width: Pixels,
        style: UnderlineStyle,
    },
    /// A dot.
    Dot(PaintQuad),
}

impl Mark {
    pub(crate) fn paint(self, window: &mut Window) {
        match self {
            Self::Wave {
                origin,
                width,
                style,
            } => window.paint_underline(origin, width, &style),
            Self::Dot(quad) => window.paint_quad(quad),
        }
    }
}

impl Scene<'_> {
    /// Marks of the diagnostics on the visible rows, least severe first so
    /// errors are drawn on top.
    pub(crate) fn diagnostic_marks(&self) -> Vec<Mark> {
        if self.rows.is_empty() {
            return Vec::new();
        }
        let buffer = self.editor.document.buffer();
        let first = buffer.line_range(self.rows.start).start;
        let last = buffer.line_range(self.rows.end - 1).end;
        let mut found = self.editor.diagnostics_in(first..last.max(first + 1));
        found.sort_by_key(|diagnostic| std::cmp::Reverse(diagnostic.severity));
        let sizes = &self.theme.metrics.size;
        let thickness = sizes.diagnostic_underline;
        let dot = sizes.diagnostic_dot;
        let snapshot = self.snapshot;
        let mut marks = Vec::new();
        for diagnostic in found {
            let color = Hsla::from(self.color(diagnostic.severity));
            for row in self.rows.clone() {
                let line = buffer.line_range(row);
                let start = diagnostic.range.start.max(line.start);
                let end = diagnostic.range.end.min(line.end);
                if start >= end && !(diagnostic.range.is_empty() && start == end) {
                    continue;
                }
                let x0 = snapshot.text_left + snapshot.x_of(buffer, start);
                let x1 = (snapshot.text_left + snapshot.x_of(buffer, end)).max(x0 + dot * 2.0);
                let bottom = snapshot.text_top
                    + crate::render::viewport::to_f32(row + 1) * snapshot.line_height;
                if diagnostic.severity == Severity::Hint {
                    // Two dots under the start, as hints are only suggestions.
                    for index in 0..2 {
                        let x = x0 + crate::render::viewport::to_f32(index) * dot * 2.0;
                        marks.push(Mark::Dot(fill(
                            Bounds::new(
                                point(px(x), px(bottom - dot * 2.0)),
                                size(px(dot), px(dot)),
                            ),
                            color,
                        )));
                    }
                } else {
                    marks.push(Mark::Wave {
                        origin: point(px(x0), px(bottom - thickness * 3.0)),
                        width: px(x1 - x0),
                        style: UnderlineStyle {
                            thickness: px(thickness),
                            color: Some(color),
                            wavy: true,
                        },
                    });
                }
            }
        }
        marks
    }

    fn color(&self, severity: Severity) -> theme::Color {
        let colors = &self.theme.colors;
        match severity {
            Severity::Error => colors.diagnostic_error,
            Severity::Warning => colors.diagnostic_warning,
            Severity::Information => colors.diagnostic_info,
            Severity::Hint => colors.diagnostic_hint,
        }
    }
}
