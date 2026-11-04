//! The editor area: the tab strip, the breadcrumbs of the active file and
//! the active tab; the Welcome page when no tab is open.

use std::path::{Component, Path};

use gpui::{
    AnyElement, Context, IntoElement, ParentElement as _, SharedString, Styled as _, div, px,
};
use theme::ActiveTheme as _;
use ui::{EditorTab, Icon, IconName, Label, Panel, PanelSurface};

use crate::tabs::TabContent;
use crate::{Workbench, file_icons, welcome};

pub(crate) fn render(
    workbench: &Workbench,
    cx: &mut Context<Workbench>,
) -> impl IntoElement + use<> {
    let panel = Panel::new(PanelSurface::Editor).flex_1().min_h_0();
    let Some(active) = workbench
        .active_tab
        .filter(|index| *index < workbench.tabs.len())
    else {
        return panel.child(welcome::render(workbench, cx));
    };
    let strip = tab_strip(workbench, active, cx);
    let tab = &workbench.tabs[active];
    let (breadcrumbs, content): (Option<AnyElement>, AnyElement) = match &tab.content {
        TabContent::Editor(editor) => (
            Some({
                let path = editor.read(cx).path().map(Path::to_path_buf);
                breadcrumbs(workbench, path.as_deref(), cx)
            }),
            editor.clone().into_any_element(),
        ),
        TabContent::View { view, .. } => (None, view.clone().into_any_element()),
    };
    panel
        .child(strip)
        .children(breadcrumbs)
        .child(div().flex_1().min_h_0().child(content))
}

fn tab_strip(workbench: &Workbench, active: usize, cx: &mut Context<Workbench>) -> AnyElement {
    let theme = cx.theme();
    let size = &theme.metrics.size;
    let inset = (size.tab_strip_height - size.tab_height) / 2.0;
    let mut strip = div()
        .flex()
        .flex_none()
        .items_center()
        .h(px(size.tab_strip_height))
        .px(px(inset))
        .gap(px(theme.metrics.space.sm))
        .overflow_hidden()
        .bg(theme.colors.bg_editor_tabs);
    for (index, tab) in workbench.tabs.iter().enumerate() {
        let leading = match &tab.content {
            TabContent::Editor(_) => Some(file_icons::render(
                &workbench.file_icons.for_file(&tab.title),
                cx,
            )),
            TabContent::View { .. } => None,
        };
        let entity = cx.entity().downgrade();
        let close_entity = entity.clone();
        let mut item = EditorTab::new(("editor-tab", index), tab.title.clone())
            .active(index == active)
            .dirty(tab.is_dirty(cx))
            .on_activate(move |window, cx| {
                let _ = entity.update(cx, |workbench, cx| {
                    workbench.activate_tab(index, window, cx);
                });
            })
            .on_close(("editor-tab-close", index), move |window, cx| {
                let _ =
                    close_entity.update(cx, |workbench, cx| workbench.close_tab(index, window, cx));
            });
        if let Some(leading) = leading {
            item = item.leading(leading);
        }
        strip = strip.child(item);
    }
    strip.into_any_element()
}

/// Folders of the file from the workspace root, then the file.
fn breadcrumbs(
    workbench: &Workbench,
    path: Option<&Path>,
    cx: &mut Context<Workbench>,
) -> AnyElement {
    let theme = cx.theme();
    let size = &theme.metrics.size;
    let font_size = theme.metrics.font_size.md;
    let mut segments: Vec<SharedString> = Vec::new();
    if let Some(path) = path {
        match workbench.workspace.as_ref().and_then(|workspace| {
            path.strip_prefix(&workspace.root)
                .ok()
                .map(|relative| (workspace, relative))
        }) {
            Some((workspace, relative)) => {
                segments.push(workspace.name.clone());
                segments.extend(
                    relative
                        .components()
                        .filter_map(|component| match component {
                            Component::Normal(name) => {
                                Some(name.to_string_lossy().into_owned().into())
                            }
                            _ => None,
                        }),
                );
            }
            None => segments.push(crate::tabs::file_title(path)),
        }
    }
    let last = segments.len().saturating_sub(1);
    let mut row = div()
        .flex()
        .flex_none()
        .items_center()
        .h(px(size.breadcrumbs_height))
        .px(px(size.editor_inset))
        .gap(px(theme.metrics.space.lg))
        .overflow_hidden();
    for (index, segment) in segments.into_iter().enumerate() {
        if index > 0 {
            row = row.child(
                Icon::new(IconName::ChevronRight)
                    .size(size.icon_tiny)
                    .color(theme.colors.text_breadcrumb),
            );
        }
        if index == last {
            let icon = file_icons::render(&workbench.file_icons.for_file(&segment), cx);
            let theme = cx.theme();
            row = row.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(theme.metrics.space.sm))
                    .child(icon)
                    .child(
                        Label::new(segment)
                            .size(font_size)
                            .color(theme.colors.text_breadcrumb_active),
                    ),
            );
        } else {
            row = row.child(
                Label::new(segment)
                    .size(font_size)
                    .color(cx.theme().colors.text_breadcrumb),
            );
        }
    }
    row.into_any_element()
}
