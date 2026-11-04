//! Text runs of a shaped row: syntax colors, and the input method's
//! composition underlined. Pure logic over display offsets.

use std::ops::Range;

use gpui::{Font, Hsla, TextRun, UnderlineStyle, px};
use syntax::HighlightSpan;

use crate::render::display::DisplayLine;

/// What a row is drawn with.
pub(crate) struct RunStyle<'a> {
    pub(crate) font: &'a Font,
    /// Color of text without a highlight.
    pub(crate) plain: Hsla,
    /// Color of each highlight id; `None` draws it plain.
    pub(crate) colors: &'a [Option<Hsla>],
}

/// One piece of a row: its length in drawn bytes, color and underline.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Piece {
    len: usize,
    color: Hsla,
    underline: bool,
}

/// The runs of the drawn text of source bytes `source` (`display` shows
/// them): `spans` (sorted, disjoint, buffer offsets) color it, `marked`
/// (buffer offsets) is underlined.
pub(crate) fn text_runs(
    display: &DisplayLine,
    source: Range<usize>,
    spans: &[HighlightSpan],
    marked: Option<&Range<usize>>,
    style: &RunStyle<'_>,
) -> Vec<TextRun> {
    pieces(display, source, spans, marked, style)
        .into_iter()
        .map(|piece| TextRun {
            len: piece.len,
            font: style.font.clone(),
            color: piece.color,
            background_color: None,
            underline: piece.underline.then_some(UnderlineStyle {
                color: Some(piece.color),
                thickness: px(1.0),
                wavy: false,
            }),
            strikethrough: None,
        })
        .collect()
}

fn pieces(
    display: &DisplayLine,
    source: Range<usize>,
    spans: &[HighlightSpan],
    marked: Option<&Range<usize>>,
    style: &RunStyle<'_>,
) -> Vec<Piece> {
    let total = display.text.len();
    // A buffer offset as a drawn offset, clamped to the row; drawn offsets
    // that are not character boundaries (never for valid spans) move back.
    let to_display = |offset: usize| {
        let mut drawn = display.to_display(offset.clamp(source.start, source.end) - source.start);
        drawn = drawn.min(total);
        while !display.text.is_char_boundary(drawn) {
            drawn -= 1;
        }
        drawn
    };
    // Colored stretches in drawn offsets, plain in between.
    let mut colored: Vec<(Range<usize>, Hsla)> = Vec::new();
    for span in spans {
        if span.range.end <= source.start || span.range.start >= source.end {
            continue;
        }
        let Some(Some(color)) = style.colors.get(span.highlight.0 as usize) else {
            continue;
        };
        let range = to_display(span.range.start)..to_display(span.range.end);
        if !range.is_empty() {
            colored.push((range, *color));
        }
    }
    let underline = marked
        .filter(|marked| marked.start < source.end && marked.end > source.start)
        .map(|marked| to_display(marked.start)..to_display(marked.end));

    let mut cuts = vec![0, total];
    for (range, _) in &colored {
        cuts.extend([range.start, range.end]);
    }
    if let Some(underline) = &underline {
        cuts.extend([underline.start, underline.end]);
    }
    cuts.sort_unstable();
    cuts.dedup();

    let mut pieces: Vec<Piece> = Vec::new();
    let mut next_colored = 0;
    for window in cuts.windows(2) {
        let (from, to) = (window[0], window[1]);
        while next_colored < colored.len() && colored[next_colored].0.end <= from {
            next_colored += 1;
        }
        let color = colored
            .get(next_colored)
            .filter(|(range, _)| range.start <= from)
            .map_or(style.plain, |(_, color)| *color);
        let piece = Piece {
            len: to - from,
            color,
            underline: underline
                .as_ref()
                .is_some_and(|underline| underline.start <= from && to <= underline.end),
        };
        match pieces.last_mut() {
            Some(last) if last.color == piece.color && last.underline == piece.underline => {
                last.len += piece.len;
            }
            _ => pieces.push(piece),
        }
    }
    pieces
}

#[cfg(test)]
mod tests {
    use gpui::{font, hsla};
    use syntax::HighlightId;

    use super::*;

    fn span(range: Range<usize>, id: u32) -> HighlightSpan {
        HighlightSpan {
            range,
            highlight: HighlightId(id),
        }
    }

    #[test]
    fn spans_color_pieces_and_tabs_widen_them() {
        let red = hsla(0.0, 1.0, 0.5, 1.0);
        let plain = hsla(0.0, 0.0, 0.8, 1.0);
        let font = font("Mono");
        let colors = [Some(red), None];
        let style = RunStyle {
            font: &font,
            plain,
            colors: &colors,
        };
        // Row "\tfn x" at buffer offset 10; `fn` is red, `x` has a
        // highlight without a color.
        let display = DisplayLine::new("\tfn x", 4, 0);
        let spans = [span(11..13, 0), span(14..15, 1)];
        let colored = pieces(&display, 10..15, &spans, None, &style);
        let lengths: Vec<(usize, bool)> = colored
            .iter()
            .map(|piece| (piece.len, piece.color == red))
            .collect();
        assert_eq!(lengths, [(4, false), (2, true), (2, false)]);

        // A composition across the colored end is underlined in two
        // pieces; spans outside the row are ignored.
        let spans = [span(0..5, 0), span(11..13, 0)];
        let marked = 12..14;
        let underlined = pieces(&display, 10..15, &spans, Some(&marked), &style);
        let shape: Vec<(usize, bool, bool)> = underlined
            .iter()
            .map(|piece| (piece.len, piece.color == red, piece.underline))
            .collect();
        assert_eq!(
            shape,
            [
                (4, false, false),
                (1, true, false),
                (1, true, true),
                (1, false, true),
                (1, false, false)
            ]
        );
    }
}
