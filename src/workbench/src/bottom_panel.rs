//! The bottom panel with its tabs. Contents arrive with their stages
//! (terminal in 6, problems in 4, debug console in 8).

use commands::CommandId;
use gpui::{Context, IntoElement, ParentElement as _, Styled as _, div, px};
use theme::ActiveTheme as _;
use ui::{IconButton, IconName, Panel, PanelSurface, PanelTab};

use crate::{PanelKind, Workbench, command_ids};

fn title(panel: PanelKind) -> &'static str {
    match panel {
        PanelKind::Terminal => "panel-terminal",
        PanelKind::Problems => "panel-problems",
        PanelKind::Output => "panel-output",
        PanelKind::DebugConsole => "panel-debug-console",
    }
}

/// Problem count shown on the Problems tab until diagnostics exist.
const NO_PROBLEMS: &str = "0";

pub(crate) fn render(
    workbench: &Workbench,
    cx: &mut Context<Workbench>,
) -> impl IntoElement + use<> {
    let entity = cx.entity().downgrade();
    let tabs = PanelKind::ALL.into_iter().map(|panel| {
        let entity = entity.clone();
        let tab = PanelTab::new(title(panel), workbench.text(title(panel)))
            .active(workbench.panel == panel)
            .on_activate(move |_, cx| {
                let _ = entity.update(cx, |workbench, cx| workbench.select_panel(panel, cx));
            });
        if panel == PanelKind::Problems {
            tab.count(NO_PROBLEMS)
        } else {
            tab
        }
    });
    let dispatch = workbench.dispatch.clone();
    let close = IconButton::new("panel-close", IconName::X).on_activate(move |window, cx| {
        if let Ok(id) = CommandId::new(command_ids::TOGGLE_PANEL) {
            dispatch(&id, window, cx);
        }
    });

    let theme = cx.theme();
    let space = &theme.metrics.space;
    let size = &theme.metrics.size;
    let header = div()
        .flex()
        .flex_none()
        .items_center()
        .px(px(size.section_header_inset + space.xs))
        .pt(px(space.md))
        .gap(px(space.xxl))
        .children(tabs)
        .child(div().flex_1())
        .child(close);
    Panel::new(PanelSurface::Editor)
        .flex_none()
        .h(px(size.bottom_panel_height))
        .child(header)
}
