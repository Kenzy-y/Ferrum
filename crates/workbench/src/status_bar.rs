//! The status bar. Items will be contributed by services (git, language
//! servers, editor position); without a workspace it shows its state.

use gpui::{Context, IntoElement, ParentElement as _, Styled as _, div, px};
use theme::ActiveTheme as _;

use crate::Workbench;

pub(crate) fn render(
    workbench: &Workbench,
    cx: &mut Context<Workbench>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let metrics = &theme.metrics;
    div()
        .flex()
        .flex_none()
        .items_center()
        .h(px(metrics.size.statusbar_height))
        .px(px(metrics.space.xl))
        .gap(px(metrics.space.xl))
        .text_size(px(metrics.font_size.sm))
        .text_color(theme.colors.text_secondary)
        .child(match &workbench.workspace {
            Some(workspace) => workspace.path.clone(),
            None => workbench.text("status-no-folder"),
        })
}
