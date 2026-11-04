//! What part of the document is visible and how scrolling follows the
//! cursor. Pure arithmetic in pixels, tested without a window.

use std::ops::Range;

/// Lines with more characters than this are drawn only around the visible
/// columns: shaping a whole line of megabytes would take far longer than a
/// frame. Shorter lines are shaped whole, which keeps wide characters
/// exact.
pub(crate) const LONG_LINE_CHARS: usize = 1024;

/// Extra columns shaped on each side of the visible part of a long line,
/// so small horizontal scrolls do not show empty space.
const LONG_LINE_MARGIN_COLUMNS: usize = 32;

/// Rows kept between the cursor and the top or bottom edge when scrolling
/// follows it.
pub(crate) const AUTOSCROLL_MARGIN_ROWS: usize = 3;

/// How far beyond the first and last visible row indent guides look for
/// the indentation of blank lines.
pub(crate) const GUIDE_SCAN_ROWS: usize = 100;

/// Rows at least partly visible with the view scrolled `scroll_y` pixels.
pub(crate) fn visible_rows(
    scroll_y: f32,
    height: f32,
    line_height: f32,
    line_count: usize,
) -> Range<usize> {
    if line_height <= 0.0 || line_count == 0 {
        return 0..0;
    }
    let first = (scroll_y.max(0.0) / line_height).floor();
    let last = ((scroll_y.max(0.0) + height.max(0.0)) / line_height).ceil();
    to_index(first).min(line_count)..to_index(last).min(line_count)
}

/// The columns of a line with `line_chars` characters to shape: all of
/// them for normal lines, the visible ones with a margin for long lines.
pub(crate) fn column_window(
    line_chars: usize,
    scroll_x: f32,
    width: f32,
    char_width: f32,
) -> Range<usize> {
    if line_chars <= LONG_LINE_CHARS || char_width <= 0.0 {
        return 0..line_chars;
    }
    let first = to_index((scroll_x.max(0.0) / char_width).floor());
    let visible = to_index((width.max(0.0) / char_width).ceil());
    let start = first
        .saturating_sub(LONG_LINE_MARGIN_COLUMNS)
        .min(line_chars);
    let end = (first + visible + LONG_LINE_MARGIN_COLUMNS).min(line_chars);
    start..end
}

/// Vertical scroll that shows `row` with a margin of rows, changing
/// `scroll_y` as little as possible.
pub(crate) fn follow_row(scroll_y: f32, height: f32, line_height: f32, row: usize) -> f32 {
    let margin = (to_f32(AUTOSCROLL_MARGIN_ROWS) * line_height).min((height - line_height) / 2.0);
    let margin = margin.max(0.0);
    let top = to_f32(row) * line_height;
    let bottom = top + line_height;
    if top - margin < scroll_y {
        (top - margin).max(0.0)
    } else if bottom + margin > scroll_y + height {
        (bottom + margin - height).max(0.0)
    } else {
        scroll_y
    }
}

/// Horizontal scroll that shows position `x` (from the text start) with a
/// margin of `margin` pixels.
pub(crate) fn follow_x(scroll_x: f32, width: f32, x: f32, margin: f32) -> f32 {
    if x - margin < scroll_x {
        (x - margin).max(0.0)
    } else if x + margin > scroll_x + width {
        (x + margin - width).max(0.0)
    } else {
        scroll_x
    }
}

/// The largest vertical scroll: the last line can move up to the top, as
/// in other editors, so there is room to type at the end.
pub(crate) fn max_scroll_y(line_count: usize, line_height: f32) -> f32 {
    to_f32(line_count.saturating_sub(1)) * line_height
}

/// Indent guide levels of `rows`. `indentation(row)` is the indentation
/// of a row in columns, `None` for a blank row; a blank row takes the
/// smaller level of the nearest non-blank rows above and below (looking at
/// most [`GUIDE_SCAN_ROWS`] away), so guides run through blank lines.
pub(crate) fn guide_levels(
    rows: Range<usize>,
    line_count: usize,
    tab_size: usize,
    indentation: impl Fn(usize) -> Option<usize>,
) -> Vec<usize> {
    let tab_size = tab_size.max(1);
    let level = |columns: usize| columns / tab_size;
    let start = rows.start.saturating_sub(GUIDE_SCAN_ROWS);
    let end = (rows.end + GUIDE_SCAN_ROWS).min(line_count);
    let columns: Vec<Option<usize>> = (start..end).map(&indentation).collect();
    let at = |row: usize| columns[row - start];
    rows.map(|row| {
        at(row).map_or_else(
            || {
                let above = (start..row).rev().find_map(at);
                let below = (row + 1..end).find_map(at);
                above
                    .zip(below)
                    .map_or(0, |(above, below)| level(above.min(below)))
            },
            level,
        )
    })
    .collect()
}

/// A non-negative pixel count, already floored or ceiled, as a row or
/// column index.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
// Negative values are clamped to 0; screen distances in rows or columns are
// far below `usize::MAX`.
pub(crate) fn to_index(value: f32) -> usize {
    value.max(0.0) as usize
}

/// A row or column count as pixels-ready `f32`.
#[allow(clippy::cast_precision_loss)]
// Exact up to 2^24 (16 million rows or columns); beyond that the error is
// below one row, far from visible at the scroll positions it affects.
pub(crate) fn to_f32(value: usize) -> f32 {
    value as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_rows_cover_partly_shown_lines() {
        assert_eq!(visible_rows(0.0, 100.0, 20.0, 1000), 0..5);
        assert_eq!(visible_rows(30.0, 100.0, 20.0, 1000), 1..7);
        assert_eq!(visible_rows(30.0, 100.0, 20.0, 3), 1..3);
        assert_eq!(visible_rows(0.0, 100.0, 20.0, 0), 0..0);
    }

    #[test]
    fn long_lines_are_shaped_around_the_visible_columns() {
        assert_eq!(column_window(80, 0.0, 800.0, 8.0), 0..80);
        let window = column_window(4_000_000, 8000.0, 800.0, 8.0);
        assert_eq!(window, 1000 - 32..1000 + 100 + 32);
    }

    fn close(left: f32, right: f32) -> bool {
        (left - right).abs() < 1e-3
    }

    #[test]
    fn scrolling_follows_the_cursor_with_a_margin() {
        // Viewport of 10 rows of 10 px.
        assert!(
            close(follow_row(0.0, 100.0, 10.0, 5), 0.0),
            "visible: no change"
        );
        assert!(
            close(follow_row(0.0, 100.0, 10.0, 9), 30.0),
            "three rows below the cursor"
        );
        assert!(close(follow_row(500.0, 100.0, 10.0, 49), 460.0));
        assert!(close(follow_x(0.0, 100.0, 150.0, 10.0), 60.0));
        assert!(close(follow_x(60.0, 100.0, 20.0, 10.0), 10.0));
    }

    #[test]
    fn guides_run_through_blank_lines() {
        // 0: "fn x() {", 1: "    a", 2: "", 3: "    b", 4: "}"
        let indentation = [Some(0), Some(4), None, Some(4), Some(0)];
        let levels = guide_levels(0..5, 5, 4, |row| indentation[row]);
        assert_eq!(levels, vec![0, 1, 1, 1, 0]);
    }
}
