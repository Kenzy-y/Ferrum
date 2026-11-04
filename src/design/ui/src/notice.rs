//! A notice bar: a message about the content below it with actions
//! (a file changed or was deleted on disk).

use gpui::{
    AnyElement, App, IntoElement, ParentElement as _, RenderOnce, SharedString, Styled as _,
    Window, div, px,
};
use theme::ActiveTheme as _;

use crate::{Icon, IconName, Label};

/// A full-width bar with an icon, a message and buttons on the right.
#[derive(IntoElement)]
pub struct NoticeBar {
    icon: IconName,
    message: SharedString,
    actions: Vec<AnyElement>,
}

impl NoticeBar {
    /// A bar saying `message` with `icon` in the accent color.
    pub fn new(icon: IconName, message: impl Into<SharedString>) -> Self {
        Self {
            icon,
            message: message.into(),
            actions: Vec::new(),
        }
    }

    /// Adds an element on the right (usually a [`crate::Button`]).
    #[must_use]
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.actions.push(action.into_any_element());
        self
    }
}

impl RenderOnce for NoticeBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let metrics = &theme.metrics;
        div()
            .flex()
            .flex_none()
            .items_center()
            .w_full()
            .gap(px(metrics.space.md))
            .px(px(metrics.size.editor_inset))
            .py(px(metrics.space.sm))
            .bg(colors.bg_notice)
            .border_b_1()
            .border_color(colors.border_panel)
            .child(Icon::new(self.icon).color(colors.accent))
            .child(
                div().flex_1().min_w_0().truncate().child(
                    Label::new(self.message)
                        .size(metrics.font_size.md)
                        .color(colors.text_secondary),
                ),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(metrics.space.sm))
                    .children(self.actions),
            )
    }
}
