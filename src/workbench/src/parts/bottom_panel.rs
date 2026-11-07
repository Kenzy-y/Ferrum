//! The bottom panel with its tabs. Contents arrive with their stages
//! (problems in 4, terminal in 6, debug console in 8).

use commands::CommandId;
use gpui::{
    AnyElement, Context, IntoElement, ParentElement as _, SharedString, Styled as _, div, px,
};
use theme::ActiveTheme as _;
use ui::{IconButton, IconName, Panel, PanelSurface, PanelTab};

use crate::contrib::terminal;
use crate::{PanelKind, Workbench, command_ids};

fn title(panel: PanelKind) -> &'static str {
    match panel {
        PanelKind::Terminal => "panel-terminal",
        PanelKind::Problems => "panel-problems",
        PanelKind::Output => "panel-output",
        PanelKind::DebugConsole => "panel-debug-console",
    }
}

/// An icon button that runs `command`.
fn command_button(
    workbench: &Workbench,
    id: &'static str,
    icon: IconName,
    command: &'static str,
) -> IconButton {
    let dispatch = workbench.dispatch.clone();
    IconButton::new(id, icon)
        .tooltip(workbench.command_title(command))
        .on_activate(move |window, cx| {
            if let Ok(id) = CommandId::new(command) {
                dispatch(&id, window, cx);
            }
        })
}

pub(crate) fn render(
    workbench: &Workbench,
    height: f32,
    cx: &mut Context<Workbench>,
) -> impl IntoElement + use<> {
    let entity = cx.entity().downgrade();
    let problems = workbench.problems.clone();
    let problem_count = problems
        .as_ref()
        .map_or(0, |problems| problems.read(cx).total());
    let tabs = PanelKind::ALL.into_iter().map(|panel| {
        let entity = entity.clone();
        let tab = PanelTab::new(title(panel), workbench.text(title(panel)))
            .active(workbench.panel == panel)
            .on_activate(move |window, cx| {
                let _ = entity.update(cx, |workbench, cx| {
                    if panel == PanelKind::Terminal {
                        workbench.show_terminal(window, cx);
                    } else {
                        workbench.select_panel(panel, cx);
                    }
                });
            });
        if panel == PanelKind::Problems {
            tab.count(problem_count.to_string())
        } else {
            tab
        }
    });
    let terminal_panel = workbench
        .terminal
        .clone()
        .filter(|_| workbench.panel == PanelKind::Terminal);
    let hint = terminal_panel
        .as_ref()
        .map(|_| terminal_hint(workbench, cx));

    let theme = cx.theme();
    let space = &theme.metrics.space;
    let size = &theme.metrics.size;
    let actions = actions(workbench, terminal_panel.as_ref(), cx);
    let header = div()
        .flex()
        .flex_none()
        .items_center()
        .px(px(size.section_header_inset + space.xs))
        .pt(px(space.md))
        .gap(px(space.xxl))
        .children(tabs)
        .child(div().flex_1())
        .child(actions);
    let content: Option<AnyElement> = match workbench.panel {
        PanelKind::Problems => problems.map(|problems| {
            div()
                .flex()
                .flex_1()
                .min_h_0()
                .pt(px(space.sm))
                .pb(px(space.sm))
                .child(problems)
                .into_any_element()
        }),
        PanelKind::Terminal => terminal_panel.map(|panel| {
            div()
                .relative()
                .flex()
                .flex_1()
                .min_h_0()
                .pb(px(space.sm))
                .child(panel)
                .children(hint)
                .into_any_element()
        }),
        PanelKind::Output | PanelKind::DebugConsole => None,
    };
    let panel = Panel::new(PanelSurface::Editor);
    let panel = if workbench.panel_maximized {
        panel.flex_1().min_h_0()
    } else {
        panel.flex_none().h(px(height))
    };
    panel.child(header).children(content)
}

/// Buttons at the right of the header: the Terminal tab's own, then
/// maximize and close.
fn actions(
    workbench: &Workbench,
    terminal_panel: Option<&gpui::Entity<terminal::TerminalPanel>>,
    cx: &Context<Workbench>,
) -> impl IntoElement + use<> {
    let terminal_actions =
        terminal_panel.map(|panel| terminal::header_actions(workbench, panel, cx));
    let (maximize_icon, maximize_id) = if workbench.panel_maximized {
        (IconName::ChevronDown, "panel-restore")
    } else {
        (IconName::ChevronUp, "panel-maximize")
    };
    let maximize = command_button(
        workbench,
        maximize_id,
        maximize_icon,
        command_ids::TOGGLE_MAXIMIZED_PANEL,
    );
    let close = command_button(
        workbench,
        "panel-close",
        IconName::X,
        command_ids::TOGGLE_PANEL,
    );
    div()
        .flex()
        .items_center()
        .gap(px(cx.theme().metrics.size.panel_actions_gap))
        .children(terminal_actions)
        .child(maximize)
        .child(close)
}

/// The shortcut hints in the corner of the terminal, as in `editor.svg`.
fn terminal_hint(workbench: &Workbench, cx: &Context<Workbench>) -> AnyElement {
    let theme = cx.theme();
    let metrics = &theme.metrics;
    let hints = [
        (command_ids::TOGGLE_TERMINAL, "terminal-hint-toggle"),
        (command_ids::SHOW_COMMAND_PALETTE, "terminal-hint-palette"),
    ];
    let items = hints.into_iter().filter_map(|(command, label)| {
        let keys = workbench.shortcut_label(command)?;
        Some(
            div()
                .flex()
                .items_center()
                .gap(px(metrics.space.xxl))
                .child(
                    div()
                        .text_size(px(metrics.font_size.xs))
                        .text_color(theme.colors.text_muted)
                        .child(workbench.text(label)),
                )
                .child(
                    div()
                        .font_family(SharedString::from(theme.fonts.mono.clone()))
                        .text_size(px(metrics.font_size.sm))
                        .text_color(theme.colors.text_kbd)
                        .child(keys),
                ),
        )
    });
    div()
        .absolute()
        .right(px(metrics.size.section_header_inset + metrics.space.xs))
        .bottom(px(metrics.space.xs))
        .flex()
        .items_center()
        .gap(px(metrics.space.xxl + metrics.space.lg))
        .h(px(metrics.size.terminal_hint_height))
        .px(px(metrics.space.md))
        .rounded(px(metrics.radius.md))
        .bg(theme.colors.bg_card)
        .children(items)
        .into_any_element()
}
