//! The Explorer view of the sidebar: an empty state without a folder, the
//! project card and the files section with one. The file tree itself is
//! filled in by the project tree (stage 2, part 3).

pub(crate) mod file_icons;
pub(crate) mod file_tree_view;
pub(crate) mod new_file;

use std::rc::Rc;

use commands::CommandId;
use gpui::{
    AnyElement, App, Context, FontWeight, IntoElement, ParentElement as _, Styled as _, Window,
    div, px,
};
use i18n::FluentArgs;
use theme::ActiveTheme as _;
use ui::{Button, ButtonSize, Icon, IconButton, IconName, Kbd, Label, SectionHeader};

use crate::dialogs::path_prompt::PathPromptTexts;
use crate::popovers::PopoverSlot;
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

fn project_card(
    workbench: &Workbench,
    workspace: &WorkspaceInfo,
    cx: &mut Context<Workbench>,
) -> AnyElement {
    let entity = cx.entity().downgrade();
    let popover = workbench.popover_at(PopoverSlot::ProjectCard, gpui::Anchor::TopLeft, cx);
    let picker = IconButton::new("project-picker", IconName::ChevronDown)
        .tooltip(workbench.text("command-workbench-pick-project"))
        .on_activate(move |window, cx| {
            let _ = entity.update(cx, |workbench, cx| workbench.toggle_projects(window, cx));
        });
    let theme = cx.theme();
    let colors = &theme.colors;
    let metrics = &theme.metrics;
    div()
        .relative()
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
        .child(picker)
        .child(
            div()
                .absolute()
                .left_0()
                .bottom_0()
                .w_full()
                .h(px(0.0))
                .children(popover),
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
    let card = project_card(workbench, workspace, cx);
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
                .action({
                    let dispatch = workbench.dispatch.clone();
                    IconButton::new("explorer-new-file", IconName::Plus)
                        .tooltip(workbench.command_title(command_ids::NEW_FILE))
                        .on_activate(move |window, cx| {
                            if let Ok(id) = CommandId::new(command_ids::NEW_FILE) {
                                dispatch(&id, window, cx);
                            }
                        })
                })
                .action(IconButton::new("explorer-files-more", IconName::Ellipsis)),
        )
        .children(workbench.file_tree.clone())
        .child(workbench.render_outline(cx))
        .into_any_element()
}

impl Workbench {
    /// `explorer.newFile`: asks for a name, then creates the file in the
    /// folder selected in the tree (or the root) and opens it.
    pub(crate) fn new_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_new_file_prompt(None, None, window, cx);
    }

    fn show_new_file_prompt(
        &mut self,
        initial: Option<String>,
        error: Option<gpui::SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.file_tree.is_none() {
            return;
        }
        let texts = PathPromptTexts {
            title: self.text("new-file-title"),
            placeholder: self.text("new-file-placeholder"),
            open: self.text("new-file-create"),
            cancel: self.text("path-prompt-cancel"),
            icon: IconName::File,
        };
        self.open_prompt(
            texts,
            initial,
            error,
            Rc::new(|this, name, window, cx| {
                this.close_path_prompt(window, cx);
                let Some(tree) = this.file_tree.clone() else {
                    return;
                };
                let created = tree.update(cx, |tree, cx| tree.create_file(&name, cx));
                cx.spawn_in(window, async move |this, cx| {
                    if let Err(error) = created.await {
                        tracing::warn!(%error, "cannot create the file");
                        let _ = this.update_in(cx, |this, window, cx| {
                            let mut args = FluentArgs::new();
                            args.set("error", error.to_string());
                            let message = this.i18n.text_with(error.message_id(), &args);
                            this.show_new_file_prompt(Some(name), Some(message.into()), window, cx);
                        });
                    }
                })
                .detach();
            }),
            window,
            cx,
        );
    }
}
