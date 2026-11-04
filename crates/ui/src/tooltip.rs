//! Tooltips shown when hovering a component.

use gpui::{
    AnyView, App, AppContext as _, Context, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Window, div, px,
};
use theme::ActiveTheme as _;

/// The view of a tooltip: a short text on the overlay surface.
pub(crate) struct Tooltip {
    text: SharedString,
}

impl Tooltip {
    /// A builder for GPUI's `tooltip` that shows `text`.
    pub(crate) fn builder(
        text: SharedString,
    ) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        move |_window, cx| cx.new(|_| Self { text: text.clone() }).into()
    }
}

impl Render for Tooltip {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let metrics = &theme.metrics;
        div()
            .px(px(metrics.space.sm))
            .py(px(metrics.space.xs))
            .rounded(px(metrics.radius.sm))
            .border_1()
            .border_color(theme.colors.border_overlay)
            .bg(theme.colors.bg_overlay)
            .font_family(SharedString::from(theme.fonts.ui.clone()))
            .text_size(px(metrics.font_size.sm))
            .text_color(theme.colors.text_secondary)
            .child(self.text.clone())
    }
}
