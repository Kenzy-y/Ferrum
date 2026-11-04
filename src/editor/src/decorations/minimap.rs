//! The minimap: a miniature of the text at the right of the editor, one
//! thin bar per run of characters colored by syntax, with the visible part
//! marked. Only the lines it can show are computed, and the bars are cached
//! until the text, the highlights, the scroll or the theme change.

use std::cell::RefCell;
use std::ops::Range;

use gpui::{Bounds, Hsla, PaintQuad, Pixels, fill, point, px, size};
use syntax::HighlightSpan;
use text::Buffer;
use theme::Theme;

use crate::editor::Editor;
use crate::render::viewport::{to_f32, to_index};

/// Where the minimap is in one frame and which lines it shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Minimap {
    pub(crate) left: f32,
    pub(crate) top: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
    /// First text line shown.
    pub(crate) first_row: usize,
    /// Lines shown.
    pub(crate) rows: usize,
    /// Height of a line on the minimap.
    pub(crate) line_height: f32,
    /// The visible part of the text, in minimap lines from `first_row`.
    viewport_top: f32,
    viewport_rows: f32,
    /// Whether the whole text fits (the minimap does not scroll).
    fits: bool,
    max_scroll_y: f32,
    /// Editor line height, to turn lines into scroll positions.
    editor_line_height: f32,
}

/// The text view the minimap mirrors.
pub(crate) struct MinimapView {
    pub(crate) line_count: usize,
    pub(crate) scroll_y: f32,
    pub(crate) max_scroll_y: f32,
    pub(crate) line_height: f32,
    pub(crate) text_height: f32,
}

impl Minimap {
    /// The minimap with its right edge at window x `right`, from window y
    /// `top`, `height` pixels tall; sizes from `theme`.
    pub(crate) fn new(
        right: f32,
        top: f32,
        height: f32,
        view: &MinimapView,
        theme: &Theme,
    ) -> Option<Self> {
        let tokens = &theme.metrics.size;
        let line_height = tokens.minimap_line_height.max(1.0);
        if height < line_height {
            return None;
        }
        let capacity = to_index((height / line_height).floor()).max(1);
        let fits = view.line_count <= capacity;
        let fraction = if view.max_scroll_y > 0.0 {
            (view.scroll_y / view.max_scroll_y).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let first_row = if fits {
            0
        } else {
            to_index((fraction * to_f32(view.line_count - capacity)).round())
        };
        let first_visible = view.scroll_y / view.line_height;
        Some(Self {
            left: right - tokens.minimap_width,
            top,
            width: tokens.minimap_width,
            height,
            first_row,
            rows: capacity.min(view.line_count - first_row),
            line_height,
            viewport_top: first_visible - to_f32(first_row),
            viewport_rows: view.text_height / view.line_height,
            fits,
            max_scroll_y: view.max_scroll_y,
            editor_line_height: view.line_height,
        })
    }

    /// Whether window position (`x`, `y`) is over the minimap.
    pub(crate) fn contains(&self, x: f32, y: f32) -> bool {
        (self.left..=self.left + self.width).contains(&x)
            && (self.top..=self.top + self.height).contains(&y)
    }

    /// The marked visible part: window y and height.
    pub(crate) fn viewport(&self) -> (f32, f32) {
        let top = self.top + self.viewport_top * self.line_height;
        let height = (self.viewport_rows * self.line_height).min(self.height);
        (top.max(self.top), height)
    }

    /// The scroll position for the pointer at window y `y`, `grab` pixels
    /// below the top of the marked part. While the whole text fits, the
    /// line under the pointer moves to the middle of the view; otherwise
    /// the marked part moves along the minimap like a scrollbar thumb.
    pub(crate) fn scroll_for(&self, y: f32, grab: f32) -> f32 {
        let (_, viewport_height) = self.viewport();
        let scroll = if self.fits {
            let row = (y - self.top) / self.line_height;
            (row - self.viewport_rows / 2.0) * self.editor_line_height
        } else {
            let travel = (self.height - viewport_height).max(1.0);
            ((y - grab - self.top) / travel).clamp(0.0, 1.0) * self.max_scroll_y
        };
        scroll.clamp(0.0, self.max_scroll_y)
    }

    /// The rows shown.
    pub(crate) fn row_range(&self) -> Range<usize> {
        self.first_row..self.first_row + self.rows
    }
}

/// A bar: a run of characters of one color on one line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Bar {
    /// Line from the minimap's first row.
    pub(crate) line: usize,
    /// x from the left of the bars, and width.
    pub(crate) x: f32,
    pub(crate) width: f32,
    pub(crate) color: Hsla,
}

/// What the bars of a line are drawn with.
pub(crate) struct BarStyle<'a> {
    pub(crate) plain: Hsla,
    /// Color of each highlight id (already dimmed).
    pub(crate) colors: &'a [Option<Hsla>],
    pub(crate) char_width: f32,
    pub(crate) tab_size: usize,
    /// Columns that fit on the minimap; the rest of a line is not shown.
    pub(crate) max_columns: usize,
}

/// The bars of `line` (the text of a line starting at buffer offset
/// `start`), colored by `spans` (sorted, buffer offsets). A bar runs from
/// a character to the last following one of the same color, spaces
/// included; only the first `max_columns` columns are read.
pub(crate) fn line_bars(
    line: &str,
    start: usize,
    spans: &[HighlightSpan],
    style: &BarStyle<'_>,
    line_index: usize,
) -> Vec<Bar> {
    let mut bars: Vec<Bar> = Vec::new();
    let mut column = 0;
    let mut span = 0;
    for (offset, character) in line.char_indices() {
        if column >= style.max_columns {
            break;
        }
        if character == '\t' {
            column += style.tab_size.max(1) - column % style.tab_size.max(1);
            continue;
        }
        if character.is_whitespace() {
            column += 1;
            continue;
        }
        let at = start + offset;
        while span < spans.len() && spans[span].range.end <= at {
            span += 1;
        }
        let color = spans
            .get(span)
            .filter(|found| found.range.start <= at)
            .and_then(|found| {
                style
                    .colors
                    .get(found.highlight.0 as usize)
                    .copied()
                    .flatten()
            })
            .unwrap_or(style.plain);
        let x = to_f32(column) * style.char_width;
        match bars.last_mut() {
            // The same color extends the bar, over spaces too: a line reads
            // as one stroke per color, as in the mockup.
            Some(last) if last.color == color => {
                last.width = x + style.char_width - last.x;
            }
            _ => bars.push(Bar {
                line: line_index,
                x,
                width: style.char_width,
                color,
            }),
        }
        column += 1;
    }
    bars
}

/// What the cached bars were computed for.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CacheKey {
    pub(crate) version: u64,
    pub(crate) highlights: (u64, u64),
    pub(crate) rows: Range<usize>,
    pub(crate) colors: Vec<Option<Hsla>>,
    pub(crate) plain: Hsla,
}

/// The bars of the rows shown last.
#[derive(Default)]
pub(crate) struct MinimapCache {
    entry: RefCell<Option<(CacheKey, Vec<Bar>)>>,
}

impl MinimapCache {
    /// The bars for `key`, computed with `compute` only if the key changed.
    pub(crate) fn bars(&self, key: CacheKey, compute: impl FnOnce() -> Vec<Bar>) -> Vec<Bar> {
        if let Some((cached, bars)) = self.entry.borrow().as_ref()
            && *cached == key
        {
            return bars.clone();
        }
        let bars = compute();
        *self.entry.borrow_mut() = Some((key, bars.clone()));
        bars
    }
}

/// Longest text the minimap reads from one line, in bytes per column (a
/// character is at most four bytes).
const MAX_BYTES_PER_COLUMN: usize = 4;

/// Lines shown longer than this altogether are highlighted line by line.
const MAX_REGION_BYTES: usize = 256 * 1024;

impl Editor {
    /// The quads of the minimap: background, bars, the visible part.
    pub(crate) fn minimap_quads(&self, minimap: &Minimap, theme: &Theme) -> Vec<PaintQuad> {
        let tokens = &theme.metrics.size;
        let opacity = theme.opacity.minimap;
        let colors: Vec<Option<Hsla>> = self
            .syntax
            .as_ref()
            .map(|syntax| syntax.colors(&theme.syntax))
            .unwrap_or_default()
            .into_iter()
            .map(|color| {
                color.map(|mut color| {
                    color.a *= opacity;
                    color
                })
            })
            .collect();
        let plain = theme.syntax.plain.with_opacity(opacity);
        let max_columns = to_index(
            ((tokens.minimap_width - tokens.minimap_inset * 2.0) / tokens.minimap_char_width)
                .floor(),
        );
        let key = CacheKey {
            version: self.document.version(),
            highlights: self
                .syntax
                .as_ref()
                .map_or((0, 0), crate::highlighting::SyntaxState::revision),
            rows: minimap.row_range(),
            colors: colors.clone(),
            plain,
        };
        let style = BarStyle {
            plain,
            colors: &colors,
            char_width: tokens.minimap_char_width,
            tab_size: self.settings.tab_size,
            max_columns,
        };
        let bars = self.minimap_cache.bars(key, || {
            let _span = tracing::trace_span!("editor.minimap", rows = minimap.rows).entered();
            self.compute_bars(minimap, &style)
        });

        let mut quads = vec![
            fill(
                bounds(minimap.left, minimap.top, minimap.width, minimap.height),
                Hsla::from(theme.colors.bg_minimap),
            )
            .corner_radii(px(tokens.minimap_radius)),
        ];
        let bar_offset = (minimap.line_height - tokens.minimap_bar_height) / 2.0;
        let left = minimap.left + tokens.minimap_inset;
        quads.extend(bars.iter().map(|bar| {
            fill(
                bounds(
                    left + bar.x,
                    minimap.top + to_f32(bar.line) * minimap.line_height + bar_offset,
                    bar.width,
                    tokens.minimap_bar_height,
                ),
                bar.color,
            )
        }));
        let (top, height) = minimap.viewport();
        quads.push(fill(
            bounds(minimap.left, top, minimap.width, height),
            Hsla::from(theme.colors.bg_minimap_viewport),
        ));
        quads
    }

    fn compute_bars(&self, minimap: &Minimap, style: &BarStyle<'_>) -> Vec<Bar> {
        let buffer: &Buffer = self.document.buffer();
        let max_bytes = style.max_columns * MAX_BYTES_PER_COLUMN;
        let rows = minimap.row_range();
        // One query for all the lines shown, unless some line is long (it
        // gets none) or the lines are huge altogether: then line by line.
        let region =
            buffer.line_range(rows.start).start..buffer.line_range(rows.end.saturating_sub(1)).end;
        let region_spans = self.syntax.as_ref().and_then(|syntax| {
            let short = rows
                .clone()
                .all(|row| buffer.line_range(row).len() <= syntax.max_line_length());
            (short && region.len() <= MAX_REGION_BYTES)
                .then(|| syntax.region_highlights(buffer, region.clone()))
        });
        let mut bars = Vec::new();
        for (index, row) in rows.enumerate() {
            let range = buffer.line_range(row);
            let end = buffer.clamp(range.end.min(range.start + max_bytes));
            let text = buffer.slice(range.start..end);
            let shown = range.start..end;
            let spans = match (&region_spans, &self.syntax) {
                (Some(spans), _) => crate::highlighting::patch::spans_in(spans, &shown),
                (None, Some(syntax)) => syntax.highlights(buffer, row, shown.clone()),
                (None, None) => Vec::new(),
            };
            bars.extend(line_bars(&text, shown.start, &spans, style, index));
        }
        bars
    }
}

fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
}

#[cfg(test)]
mod tests {
    use gpui::hsla;
    use syntax::HighlightId;

    use super::*;

    #[test]
    fn bars_follow_words_colors_and_tabs() {
        let red = hsla(0.0, 1.0, 0.5, 1.0);
        let plain = hsla(0.0, 0.0, 0.5, 1.0);
        let colors = [Some(red)];
        let style = BarStyle {
            plain,
            colors: &colors,
            char_width: 1.25,
            tab_size: 4,
            max_columns: 12,
        };
        // "fn" is red; the plain rest is one bar across spaces and the tab;
        // the line is cut at 12 columns.
        let spans = [HighlightSpan {
            range: 10..12,
            highlight: HighlightId(0),
        }];
        let bars = line_bars("fn ab\tx = 123456789", 10, &spans, &style, 3);
        let shape: Vec<(f32, f32, bool)> = bars
            .iter()
            .map(|bar| (bar.x / 1.25, bar.width / 1.25, bar.color == red))
            .collect();
        assert_eq!(shape, [(0.0, 2.0, true), (3.0, 8.0, false)]);
        assert!(bars.iter().all(|bar| bar.line == 3));
    }
}
