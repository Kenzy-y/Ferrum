//! The sidebar panel. Views fill it in later stages (explorer in stage 2,
//! search in stage 5, ...); for now it shows the view title.

use gpui::{Context, IntoElement, ParentElement as _, Styled as _, px};
use theme::ActiveTheme as _;
use ui::{IconButton, IconName, Panel, PanelSurface, SectionHeader};

use crate::contrib::explorer;
use crate::{SidebarView, Workbench};

/// Message id of the title of `view`.
pub(crate) fn title(view: SidebarView) -> &'static str {
    match view {
        SidebarView::Explorer => "view-explorer",
        SidebarView::Search => "view-search",
        SidebarView::SourceControl => "view-source-control",
        SidebarView::RunAndDebug => "view-run-and-debug",
        SidebarView::Extensions => "view-extensions",
    }
}

pub(crate) fn render(
    workbench: &Workbench,
    cx: &mut Context<Workbench>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let size = &theme.metrics.size;
    // The header sits at the height of the first activity button.
    let header_top = size.activity_bar_top - size.layout_gap_top;
    Panel::new(PanelSurface::Sidebar)
        .w(px(size.sidebar_width))
        .flex_none()
        .pb(px(theme.metrics.space.sm))
        .pt(px(header_top))
        .gap(px(theme.metrics.space.lg))
        .child(
            SectionHeader::panel(workbench.text(title(workbench.view)))
                .action(IconButton::new("sidebar-more", IconName::Ellipsis)),
        )
        .children(
            (workbench.view == SidebarView::Explorer).then(|| explorer::render(workbench, cx)),
        )
}
