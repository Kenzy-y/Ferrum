//! The sidebar panel: the title of the view and its contents (Explorer,
//! Search, Run and Debug, Extensions; Source Control arrives in its stage
//! and shows only the title).

use gpui::{Context, IntoElement, ParentElement as _, Styled as _, px};
use theme::ActiveTheme as _;
use ui::{IconButton, IconName, Panel, PanelSurface, SectionHeader};

use crate::contrib::{explorer, extensions, run_debug};
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
    width: f32,
    cx: &mut Context<Workbench>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let size = &theme.metrics.size;
    // The header sits at the height of the first activity button.
    let header_top = size.activity_bar_top - size.layout_gap_top;
    Panel::new(PanelSurface::Sidebar)
        .w(px(width))
        .flex_none()
        .pb(px(theme.metrics.space.sm))
        .pt(px(header_top))
        .gap(px(theme.metrics.space.lg))
        .child(
            SectionHeader::panel(workbench.text(title(workbench.view)))
                .action(IconButton::new("sidebar-more", IconName::Ellipsis)),
        )
        .children(match workbench.view {
            SidebarView::Explorer => Some(explorer::render(workbench, cx).into_any_element()),
            SidebarView::Search => workbench
                .search
                .clone()
                .map(gpui::IntoElement::into_any_element),
            SidebarView::RunAndDebug => Some(run_debug::render(workbench, cx).into_any_element()),
            SidebarView::Extensions => Some(extensions::render(workbench, cx).into_any_element()),
            SidebarView::SourceControl => None,
        })
}
