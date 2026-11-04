//! The activity bar: one button per sidebar view, settings at the bottom.

use commands::CommandId;
use gpui::{Context, IntoElement, ParentElement as _, Styled as _, div, px};
use theme::ActiveTheme as _;
use ui::{ActivityButton, IconName};

use crate::{SidebarView, Workbench, command_ids};

fn icon(view: SidebarView) -> IconName {
    match view {
        SidebarView::Explorer => IconName::Files,
        SidebarView::Search => IconName::Search,
        SidebarView::SourceControl => IconName::GitBranch,
        SidebarView::RunAndDebug => IconName::Play,
        SidebarView::Extensions => IconName::Package,
    }
}

pub(crate) fn render(
    workbench: &Workbench,
    cx: &mut Context<Workbench>,
) -> impl IntoElement + use<> {
    let buttons = SidebarView::ALL.into_iter().map(|view| {
        let dispatch = workbench.dispatch.clone();
        let active = workbench.sidebar_visible && workbench.view == view;
        ActivityButton::new(command_ids::show_view(view), icon(view))
            .active(active)
            .on_activate(move |window, cx| {
                if let Ok(id) = CommandId::new(command_ids::show_view(view)) {
                    dispatch(&id, window, cx);
                }
            })
    });
    // Settings open in stage 10; the button is part of the layout already.
    let settings = ActivityButton::new("activity-settings", IconName::Settings);

    let theme = cx.theme();
    let size = &theme.metrics.size;
    div()
        .flex()
        .flex_col()
        .flex_none()
        .items_center()
        .w(px(size.activity_bar_width))
        .pt(px(size.activity_bar_top - size.layout_gap_top))
        .pb(px(theme.metrics.space.md))
        .gap(px(size.activity_bar_gap))
        .children(buttons)
        .child(div().flex_1())
        .child(settings)
}
