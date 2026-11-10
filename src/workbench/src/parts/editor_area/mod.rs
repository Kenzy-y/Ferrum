//! The editor area: the tab strip, the breadcrumbs of the active file and
//! the active tab; the Welcome page when no tab is open; an extension page
//! over both while one is open.

pub(crate) mod closing;
pub(crate) mod groups;
pub(crate) mod message_view;
pub(crate) mod tabs;

use std::path::{Component, Path};

use commands::CommandId;
use editor::DiskState;
use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    Styled as _, div, px,
};
use syntax::OutlineItem;
use theme::ActiveTheme as _;
use ui::{
    Button, ButtonVariant, EditorTab, Icon, IconButton, IconName, Label, NoticeBar, Panel,
    PanelSurface,
};

use crate::contrib::explorer::file_icons;
use crate::contrib::extensions;
use crate::contrib::outline::symbol_icons;
use crate::contrib::welcome;
use crate::parts::editor_area::tabs::TabContent;
use crate::popovers::PopoverSlot;
use crate::{Workbench, command_ids};

pub(crate) fn render(workbench: &Workbench, cx: &mut Context<Workbench>) -> AnyElement {
    if let Some(page) = workbench.extensions.page {
        return Panel::new(PanelSurface::Editor)
            .flex_1()
            .min_h_0()
            .child(extensions::render_page(workbench, page, cx))
            .into_any_element();
    }
    let Some(split) = &workbench.split else {
        return Panel::new(PanelSurface::Editor)
            .flex_1()
            .min_h_0()
            .child(group(workbench, Group::Active, cx))
            .into_any_element();
    };
    let other_on_right = split.other_on_right;
    let gap = cx.theme().metrics.size.panel_gap;
    let active = Panel::new(PanelSurface::Editor)
        .flex_1()
        .min_w_0()
        .child(group(workbench, Group::Active, cx));
    let other = Panel::new(PanelSurface::Editor)
        .flex_1()
        .min_w_0()
        .child(group(workbench, Group::Other, cx));
    let (left, right) = if other_on_right {
        (active.into_any_element(), other.into_any_element())
    } else {
        (other.into_any_element(), active.into_any_element())
    };
    div()
        .flex()
        .flex_1()
        .min_h_0()
        .gap(px(gap))
        .child(left)
        .child(right)
        .into_any_element()
}

/// Which editor group to draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Group {
    /// The group with the focus (`Workbench::tabs`).
    Active,
    /// The other group of a split.
    Other,
}

/// One group: its tabs, the breadcrumbs and notices of its active tab, the
/// tab itself (Welcome or a hint when it has none).
fn group(workbench: &Workbench, which: Group, cx: &mut Context<Workbench>) -> AnyElement {
    let (tabs, active_tab, group_id) = match (which, &workbench.split) {
        (Group::Other, Some(split)) => (&split.other.tabs, split.other.active_tab, split.other.id),
        _ => (
            &workbench.tabs,
            workbench.active_tab,
            workbench.active_group,
        ),
    };
    let Some(active) = active_tab.filter(|index| *index < tabs.len()) else {
        if workbench.split.is_none() {
            return welcome::render(workbench, cx).into_any_element();
        }
        let theme = cx.theme();
        return div()
            .flex()
            .flex_1()
            .size_full()
            .items_center()
            .justify_center()
            .p(px(theme.metrics.space.xl))
            .text_size(px(theme.metrics.font_size.md))
            .text_color(theme.colors.text_muted)
            .child(workbench.text("editor-group-empty"))
            .into_any_element();
    };
    let strip = tab_strip(workbench, tabs, active, (which, group_id), cx);
    let tab = &tabs[active];
    let notice = match &tab.content {
        TabContent::Editor(editor) => disk_notice(workbench, editor.read(cx).disk_state()),
        TabContent::View { .. } => None,
    };
    let (breadcrumbs, content): (Option<AnyElement>, AnyElement) = match &tab.content {
        TabContent::Editor(editor) => (
            Some({
                let editor = editor.read(cx);
                let path = editor.path().map(Path::to_path_buf);
                let symbols = editor.symbols_at_cursor();
                let language = editor.language_id().map(str::to_owned);
                breadcrumbs(
                    workbench,
                    path.as_deref(),
                    language.as_deref(),
                    &symbols,
                    cx,
                )
            }),
            editor.clone().into_any_element(),
        ),
        TabContent::View { view, .. } => (None, view.clone().into_any_element()),
    };
    let find = (which == Group::Active)
        .then(|| find_overlay(workbench, tab, cx))
        .flatten();
    let mut root = div()
        .id(match which {
            Group::Active => "editor-group-active",
            Group::Other => "editor-group-other",
        })
        .flex()
        .flex_col()
        .size_full()
        .child(strip)
        .children(breadcrumbs)
        .children(notice)
        .child(
            div()
                .relative()
                .flex_1()
                .min_h_0()
                .child(content)
                .children(find),
        );
    if which == Group::Other {
        // A click anywhere in the other group makes it the active one first.
        root = root.on_mouse_down(
            gpui::MouseButton::Left,
            cx.listener(move |workbench, _, window, cx| {
                workbench.focus_group(group_id, window, cx);
            }),
        );
    }
    root.into_any_element()
}

/// The find widget at the top right of an editor.
fn find_overlay(
    workbench: &Workbench,
    tab: &tabs::Tab,
    cx: &Context<Workbench>,
) -> Option<AnyElement> {
    let open = workbench.find.as_ref()?;
    if !matches!(tab.content, TabContent::Editor(_)) {
        return None;
    }
    let space = &cx.theme().metrics.space;
    Some(
        div()
            .absolute()
            .top(px(space.xs))
            .right(px(space.xl))
            .child(open.view.clone())
            .into_any_element(),
    )
}

/// The bar above an editor whose file changed or was deleted on disk;
/// every button is a command.
fn disk_notice(workbench: &Workbench, state: DiskState) -> Option<AnyElement> {
    let button = |id: &'static str, label: &str, command: &'static str| {
        let dispatch = workbench.dispatch.clone();
        Button::new(id, workbench.text(label))
            .variant(ButtonVariant::Secondary)
            .on_activate(move |window, cx| {
                if let Ok(id) = CommandId::new(command) {
                    dispatch(&id, window, cx);
                }
            })
    };
    let bar = match state {
        DiskState::InSync => return None,
        DiskState::Changed => NoticeBar::new(
            IconName::TriangleAlert,
            workbench.text("editor-disk-changed"),
        )
        .action(button(
            "disk-reload",
            "editor-disk-reload",
            editor::RELOAD_FROM_DISK,
        ))
        .action(button(
            "disk-keep",
            "editor-disk-keep",
            editor::KEEP_LOCAL_CHANGES,
        )),
        DiskState::Deleted => {
            NoticeBar::new(IconName::FileX, workbench.text("editor-disk-deleted"))
                .action(button(
                    "disk-save-again",
                    "editor-disk-save-again",
                    editor::SAVE,
                ))
                .action(button(
                    "disk-close",
                    "editor-disk-close",
                    command_ids::CLOSE_TAB,
                ))
        }
    };
    Some(bar.into_any_element())
}

// The tabs and the two group actions share the group's click handling;
// splitting them apart would pass the same closures around.
#[allow(clippy::too_many_lines)]
fn tab_strip(
    workbench: &Workbench,
    tabs: &[tabs::Tab],
    active: usize,
    (which, group_id): (Group, u64),
    cx: &mut Context<Workbench>,
) -> AnyElement {
    let entity = cx.entity().downgrade();
    // In the other group a tab action first makes that group active; the
    // tab keeps its index across the swap.
    let in_group =
        move |workbench: &mut Workbench, window: &mut gpui::Window, cx: &mut Context<Workbench>| {
            workbench.focus_group(group_id, window, cx);
        };
    let menu = (which == Group::Active)
        .then(|| workbench.popover_at(PopoverSlot::TabMenu, gpui::Anchor::TopRight, cx))
        .flatten();
    let theme = cx.theme();
    let size = &theme.metrics.size;
    let inset = (size.tab_strip_height - size.tab_height) / 2.0;
    let mut strip = div()
        .flex()
        .flex_1()
        .min_w_0()
        .items_center()
        .gap(px(theme.metrics.space.sm))
        .overflow_hidden();
    for (index, tab) in tabs.iter().enumerate() {
        let leading = match &tab.content {
            TabContent::Editor(_) => Some(file_icons::render(
                &workbench.file_icons.for_file(&tab.title),
                cx,
            )),
            TabContent::View { .. } => None,
        };
        let entity = entity.clone();
        let close_entity = entity.clone();
        let group_key = match which {
            Group::Active => 0,
            Group::Other => 1,
        };
        let mut item = EditorTab::new(
            ("editor-tab", index + group_key * 100_000),
            tab.title.clone(),
        )
        .active(index == active)
        .dirty(tab.is_dirty(cx))
        .on_activate(move |window, cx| {
            let _ = entity.update(cx, |workbench, cx| {
                in_group(workbench, window, cx);
                workbench.activate_tab(index, window, cx);
            });
        })
        .on_close(
            ("editor-tab-close", index + group_key * 100_000),
            move |window, cx| {
                let _ = close_entity.update(cx, |workbench, cx| {
                    in_group(workbench, window, cx);
                    workbench.close_tab(index, window, cx);
                });
            },
        );
        if let Some(leading) = leading {
            item = item.leading(leading);
        }
        strip = strip.child(item);
    }
    let dispatch = workbench.dispatch.clone();
    let action =
        |id: &'static str, icon: IconName, tooltip: SharedString, command: &'static str| {
            let dispatch = dispatch.clone();
            let entity = entity.clone();
            IconButton::new(gpui::SharedString::from(format!("{id}-{which:?}")), icon)
                .tooltip(tooltip)
                .on_activate(move |window, cx| {
                    let _ = entity.update(cx, |workbench, cx| in_group(workbench, window, cx));
                    if let Ok(id) = CommandId::new(command) {
                        dispatch(&id, window, cx);
                    }
                })
        };
    let theme = cx.theme();
    let actions = div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(theme.metrics.space.xs))
        .child(action(
            "editor-split",
            IconName::Columns,
            workbench.text("editor-split"),
            command_ids::SPLIT_EDITOR,
        ))
        .child(
            div()
                .flex()
                .flex_col()
                .items_end()
                .child(action(
                    "editor-more",
                    IconName::Ellipsis,
                    workbench.text("editor-more"),
                    command_ids::SHOW_TAB_MENU,
                ))
                .child(div().h(px(0.0)).children(menu)),
        );
    div()
        .flex()
        .flex_none()
        .items_center()
        .h(px(size.tab_strip_height))
        .px(px(inset))
        .gap(px(theme.metrics.space.sm))
        .bg(theme.colors.bg_editor_tabs)
        .child(strip)
        .child(actions)
        .into_any_element()
}

/// Folders of the file from the workspace root, the file, then the outline
/// items around the cursor.
fn breadcrumbs(
    workbench: &Workbench,
    path: Option<&Path>,
    language: Option<&str>,
    symbols: &[OutlineItem],
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
            None => segments.push(crate::parts::editor_area::tabs::file_title(path)),
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
    for symbol in symbols {
        let glyph = symbol_icons::render(symbol.kind, cx.theme().metrics.font_size.base, cx);
        let theme = cx.theme();
        row = row
            .child(
                Icon::new(IconName::ChevronRight)
                    .size(size.icon_tiny)
                    .color(theme.colors.text_breadcrumb),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(theme.metrics.space.sm))
                    .child(glyph)
                    .child(
                        Label::new(symbol.name.clone())
                            .size(font_size)
                            .color(theme.colors.text_breadcrumb_active),
                    ),
            );
    }
    let detail = crate::contrib::language_status::breadcrumb_detail(workbench, path, language, cx);
    if let Some(detail) = detail {
        row = row
            .child(div().flex_1().min_w(px(cx.theme().metrics.space.lg)))
            .child(detail);
    }
    row.into_any_element()
}
