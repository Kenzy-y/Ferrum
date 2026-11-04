//! The vertical scrollbar: where the thumb is, what the pointer hits, and
//! which scroll position a dragged thumb means. Pure geometry in window
//! pixels.

use gpui::{Bounds, Hsla, PaintQuad, fill, point, px, size};

/// Smallest thumb, in pixels, so it stays easy to see and grab.
const MIN_THUMB_HEIGHT: f32 = 24.0;

/// The scrollbar of one frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Scrollbar {
    /// Left edge of the area that reacts to the pointer (the thumb plus
    /// its inset on both sides, up to the editor's right edge).
    hit_left: f32,
    hit_right: f32,
    thumb_x: f32,
    thumb_width: f32,
    track_top: f32,
    track_height: f32,
    pub(crate) thumb_top: f32,
    pub(crate) thumb_height: f32,
    max_scroll_y: f32,
}

/// What the scrollbar is placed in.
pub(crate) struct ScrollbarFrame {
    /// Window x of the editor's right edge.
    pub(crate) right: f32,
    /// Window y and height of the editor.
    pub(crate) top: f32,
    pub(crate) height: f32,
    /// Height of the area that shows text.
    pub(crate) text_height: f32,
    /// Tokens `scrollbar.width` and `scrollbar.inset`.
    pub(crate) width: f32,
    pub(crate) inset: f32,
}

impl Scrollbar {
    /// The scrollbar for `content_height` pixels of text scrolled to
    /// `scroll_y` (at most `max_scroll_y`); none if everything fits.
    pub(crate) fn new(
        frame: &ScrollbarFrame,
        content_height: f32,
        scroll_y: f32,
        max_scroll_y: f32,
    ) -> Option<Self> {
        if content_height <= frame.text_height || max_scroll_y <= 0.0 {
            return None;
        }
        let track_height = frame.height;
        let thumb_height = (track_height * frame.text_height / (max_scroll_y + frame.text_height))
            .max(MIN_THUMB_HEIGHT)
            .min(track_height);
        let thumb_x = frame.right - frame.inset - frame.width;
        let fraction = (scroll_y / max_scroll_y).clamp(0.0, 1.0);
        Some(Self {
            hit_left: thumb_x - frame.inset,
            hit_right: frame.right,
            thumb_x,
            thumb_width: frame.width,
            track_top: frame.top,
            track_height,
            thumb_top: frame.top + (track_height - thumb_height) * fraction,
            thumb_height,
            max_scroll_y,
        })
    }

    /// Whether window position (`x`, `y`) is over the scrollbar.
    pub(crate) fn contains(&self, x: f32, y: f32) -> bool {
        (self.hit_left..=self.hit_right).contains(&x)
            && (self.track_top..=self.track_top + self.track_height).contains(&y)
    }

    /// Whether window y `y` is on the thumb.
    pub(crate) fn on_thumb(&self, y: f32) -> bool {
        (self.thumb_top..=self.thumb_top + self.thumb_height).contains(&y)
    }

    /// The scroll position that puts the thumb's top at window y `top`.
    pub(crate) fn scroll_for(&self, top: f32) -> f32 {
        let travel = self.track_height - self.thumb_height;
        if travel <= 0.0 {
            return 0.0;
        }
        ((top - self.track_top) / travel).clamp(0.0, 1.0) * self.max_scroll_y
    }

    /// The thumb, painted in `color`.
    pub(crate) fn quad(&self, color: Hsla) -> PaintQuad {
        fill(
            Bounds::new(
                point(px(self.thumb_x), px(self.thumb_top)),
                size(px(self.thumb_width), px(self.thumb_height)),
            ),
            color,
        )
        .corner_radii(px(self.thumb_width / 2.0))
    }

    /// The area that reacts to the pointer.
    pub(crate) fn hit_bounds(&self) -> Bounds<gpui::Pixels> {
        Bounds::new(
            point(px(self.hit_left), px(self.track_top)),
            size(px(self.hit_right - self.hit_left), px(self.track_height)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> ScrollbarFrame {
        ScrollbarFrame {
            right: 1000.0,
            top: 100.0,
            height: 500.0,
            text_height: 480.0,
            width: 4.0,
            inset: 5.0,
        }
    }

    #[test]
    fn the_thumb_follows_the_scroll_and_back() {
        let scrollbar = Scrollbar::new(&frame(), 4800.0, 0.0, 4320.0).unwrap();
        assert!((scrollbar.thumb_top - 100.0).abs() < 0.01);
        assert!((scrollbar.thumb_height - 500.0 * 480.0 / 4800.0).abs() < 0.01);
        // Dragging the thumb to the bottom scrolls to the end.
        let bottom = 100.0 + 500.0 - scrollbar.thumb_height;
        assert!((scrollbar.scroll_for(bottom) - 4320.0).abs() < 0.01);
        assert!(
            scrollbar.scroll_for(0.0).abs() < 0.01,
            "clamped above the track"
        );
        let halfway = Scrollbar::new(&frame(), 4800.0, 2160.0, 4320.0).unwrap();
        assert!((halfway.scroll_for(halfway.thumb_top) - 2160.0).abs() < 0.01);
    }

    #[test]
    fn the_pointer_hits_the_strip_around_the_thumb() {
        let scrollbar = Scrollbar::new(&frame(), 4800.0, 0.0, 4320.0).unwrap();
        assert!(
            scrollbar.contains(988.0, 300.0),
            "the inset left of the thumb"
        );
        assert!(scrollbar.contains(999.0, 300.0));
        assert!(!scrollbar.contains(980.0, 300.0));
        assert!(scrollbar.on_thumb(120.0) && !scrollbar.on_thumb(400.0));
        assert!(
            Scrollbar::new(&frame(), 400.0, 0.0, 0.0).is_none(),
            "everything fits"
        );
    }
}
