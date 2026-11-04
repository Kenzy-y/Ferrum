//! The Explorer view of the sidebar: an empty state without a folder, the
//! project card and the files section with one. The file tree itself is
//! filled in by the project tree (stage 2, part 3).

use commands::CommandId;
use gpui::{
    AnyElement, App, Context, FontWeight, IntoElement, ParentElement as _, Styled as _, div, px,
};
use theme::ActiveTheme as _;
use ui::{Button, ButtonSize, Icon, IconButton, IconName, Kbd, Label, SectionHeader};

use crate::{Workbench, WorkspaceInfo, command_ids};

fn empty_state(workbench: &Workbench, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let colors = &theme.colors;
    let metrics = &theme.metrics;
    let size = &metrics.size;
    let dispatch = workbench.dispatch.clone();
    let hint = workbench.shortcut_label(command_ids::OPEN_FOLDER);
    div()
        .flex()
        .flex_col()
        .items_center()
        .pt(px(size.empty_state_top))
        .px(px(size.section_header_inset))
        .gap(px(metrics.space.lg))
        .child(
            Icon::new(IconName::Folder)
                .size(size.empty_state_icon)
                .color(colors.icon),
        )
        .child(
            Label::new(workbench.text("explorer-empty-title"))
                .size(metrics.font_size.xl)
                .semibold()
                .color(colors.text_primary),
        )
        .child(
            div()
                .text_center()
                .text_size(px(metrics.font_size.md))
                .text_color(colors.text_secondary)
                .child(workbench.text("explorer-empty-text")),
        )
        .child(
            div().mt(px(metrics.space.sm)).child(
                Button::new(
                    "explorer-open-folder",
                    workbench.text("titlebar-open-folder"),
                )
                .size(ButtonSize::Medium)
                .width(size.empty_state_button)
                .on_activate(move |window, cx| {
                    if let Ok(id) = CommandId::new(command_ids::OPEN_FOLDER) {
                        dispatch(&id, window, cx);
                    }
                }),
            ),
        )
        .children(hint.map(|keys| Kbd::new(keys).plain()))
        .into_any_element()
}

fn project_card(workspace: &WorkspaceInfo, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let colors = &theme.colors;
    let metrics = &theme.metrics;
    div()
        .flex()
        .items_center()
        .gap(px(metrics.space.md))
        .mx(px(metrics.space.lg))
        .px(px(metrics.space.lg))
        .py(px(metrics.space.sm))
        .rounded(px(metrics.radius.lg))
        .bg(colors.bg_card)
        .child(Icon::new(IconName::Package).color(colors.accent))
        .child(
            div()
                .flex()
                .flex_col()
                .min_w_0()
                .flex_1()
                .child(
                    div()
                        .truncate()
                        .text_size(px(metrics.font_size.base))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(colors.text_primary)
                        .child(workspace.name.clone()),
                )
                .child(
                    div()
                        .truncate()
                        .font_family(gpui::SharedString::from(theme.fonts.mono.clone()))
                        .text_size(px(metrics.font_size.xs))
                        .text_color(colors.text_subtle)
                        .child(workspace.path.clone()),
                ),
        )
        .into_any_element()
}

pub(crate) fn render(
    workbench: &Workbench,
    cx: &mut Context<Workbench>,
) -> impl IntoElement + use<> {
    let Some(workspace) = &workbench.workspace else {
        return empty_state(workbench, cx);
    };
    let card = project_card(workspace, cx);
    let gap = cx.theme().metrics.space.lg;
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .gap(px(gap))
        .child(card)
        .child(
            SectionHeader::group(workbench.text("explorer-files"), None)
                .action(
                    IconButton::new("explorer-new-file", IconName::Plus)
                        .disabled(true)
                        .tooltip(workbench.text("welcome-later")),
                )
                .action(IconButton::new("explorer-files-more", IconName::Ellipsis)),
        )
        .children(workbench.file_tree.clone())
        .into_any_element()
}
