//! Badges: keyboard shortcuts, counters, status letters.

use gpui::{
    App, IntoElement, ParentElement as _, RenderOnce, SharedString, Styled as _, Window, div, px,
};
use theme::ActiveTheme as _;

/// A keyboard shortcut, e.g. `Ctrl Shift P`.
///
/// The boxed form is used in lists of shortcuts (Welcome); the plain form is
/// a hint inside other controls (title bar search, terminal hints).
#[derive(IntoElement)]
pub struct Kbd {
    keys: SharedString,
    plain: bool,
    fill: bool,
}

impl Kbd {
    /// Creates a boxed shortcut badge. `keys` is the display text.
    pub fn new(keys: impl Into<SharedString>) -> Self {
        Self {
            keys: keys.into(),
            plain: false,
            fill: false,
        }
    }

    /// Takes the full width of its parent (fixed-width columns of
    /// shortcuts).
    #[must_use]
    pub fn fill(mut self) -> Self {
        self.fill = true;
        self
    }

    /// Without the background box.
    #[must_use]
    pub fn plain(mut self) -> Self {
        self.plain = true;
        self
    }
}

impl RenderOnce for Kbd {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let metrics = &theme.metrics;
        let text = div()
            .font_family(SharedString::from(theme.fonts.mono.clone()))
            .text_size(px(metrics.font_size.sm))
            .text_color(colors.text_kbd)
            .child(self.keys);
        if self.plain {
            return text;
        }
        let text = if self.fill { text.w_full() } else { text };
        text.flex()
            .items_center()
            .flex_none()
            .h(px(metrics.size.kbd_height))
            .px(px(metrics.space.md))
            .rounded(px(metrics.radius.sm))
            .bg(colors.bg_kbd)
    }
}

/// A small number in a box (problem count, number of installed items).
#[derive(IntoElement)]
pub struct CountBadge {
    count: SharedString,
}

impl CountBadge {
    /// Creates a badge showing `count`.
    pub fn new(count: impl Into<SharedString>) -> Self {
        Self {
            count: count.into(),
        }
    }
}

impl RenderOnce for CountBadge {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let metrics = &theme.metrics;
        div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .min_w(px(metrics.size.badge_size))
            .h(px(metrics.size.badge_size))
            .px(px(metrics.space.xs))
            .rounded(px(metrics.radius.xs))
            .bg(theme.colors.bg_badge)
            .text_size(px(metrics.font_size.xs))
            .text_color(theme.colors.text_subtle)
            .child(self.count)
    }
}

/// Kind of a [`StatusLetter`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    /// Modified.
    Modified,
    /// Added or untracked.
    Added,
    /// Deleted or conflicting.
    Deleted,
}

/// A one-letter file status at the end of a row (`M`, `A`, `D`).
#[derive(IntoElement)]
pub struct StatusLetter {
    letter: SharedString,
    kind: StatusKind,
}

impl StatusLetter {
    /// Creates a status letter.
    pub fn new(letter: impl Into<SharedString>, kind: StatusKind) -> Self {
        Self {
            letter: letter.into(),
            kind,
        }
    }
}

impl RenderOnce for StatusLetter {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let color = match self.kind {
            StatusKind::Modified => theme.colors.text_warning,
            StatusKind::Added => theme.colors.text_success,
            StatusKind::Deleted => theme.colors.text_error,
        };
        div()
            .flex_none()
            .font_family(SharedString::from(theme.fonts.mono.clone()))
            .text_size(px(theme.metrics.font_size.xs))
            .text_color(color)
            .child(self.letter)
    }
}
