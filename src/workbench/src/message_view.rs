//! A tab that only explains something: a file that cannot be opened.

use gpui::{
    Context, IntoElement, ParentElement as _, Render, SharedString, Styled as _, Window, div, px,
};
use theme::ActiveTheme as _;

/// A centered message.
pub(crate) struct MessageView {
    message: SharedString,
}

impl MessageView {
    pub(crate) fn new(message: SharedString) -> Self {
        Self { message }
    }
}

impl Render for MessageView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .px(px(theme.metrics.space.xxl))
            .text_size(px(theme.metrics.font_size.base))
            .text_color(theme.colors.text_muted)
            .child(self.message.clone())
    }
}
