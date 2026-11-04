//! The Welcome page, shown in the editor area when nothing is open.

use commands::CommandId;
use gpui::{
    AnyElement, App, Context, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, div, px,
};
use theme::ActiveTheme as _;
use ui::{Button, ButtonSize, ButtonVariant, Icon, IconName, Kbd, Label};

use crate::{Workbench, command_ids};

/// Recent projects listed on the page; the rest are in quick open later.
const RECENT_ROWS: usize = 5;

/// Rows of "Get started": message id of the label and the command.
const GET_STARTED: [(&str, &str); 4] = [
    ("welcome-start-palette", command_ids::SHOW_COMMAND_PALETTE),
    ("welcome-start-search", command_ids::QUICK_OPEN),
    ("welcome-start-open-folder", command_ids::OPEN_FOLDER),
    ("welcome-start-panel", command_ids::TOGGLE_PANEL),
];

/// A handler that runs `command`.
fn run(
    workbench: &Workbench,
    command: &'static str,
) -> impl Fn(&mut gpui::Window, &mut App) + 'static {
    let dispatch = workbench.dispatch.clone();
    move |window, cx| {
        if let Ok(id) = CommandId::new(command) {
            dispatch(&id, window, cx);
        }
    }
}

fn section_title(text: &SharedString, cx: &App) -> AnyElement {
    let theme = cx.theme();
    Label::new(text.to_uppercase())
        .size(theme.metrics.font_size.sm)
        .semibold()
        .color(theme.colors.text_subtle)
        .into_any_element()
}

fn hero(workbench: &Workbench, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let colors = &theme.colors;
    let metrics = &theme.metrics;
    let logo = div()
        .flex()
        .items_center()
        .justify_center()
        .size(px(metrics.size.welcome_logo))
        .rounded(px(metrics.radius.xxl))
        .bg(colors.bg_tile_accent)
        .child(
            Icon::new(IconName::Terminal)
                .medium(cx)
                .color(colors.accent),
        );
    let later = workbench.text("welcome-later");
    let width = metrics.size.welcome_button;
    let buttons = div()
        .flex()
        .flex_wrap()
        .gap(px(metrics.space.md))
        .child(
            Button::new(
                "welcome-open-folder",
                workbench.text("titlebar-open-folder"),
            )
            .size(ButtonSize::Medium)
            .width(width)
            .on_activate(run(workbench, command_ids::OPEN_FOLDER)),
        )
        .child(
            Button::new("welcome-new-project", workbench.text("welcome-new-project"))
                .variant(ButtonVariant::Secondary)
                .size(ButtonSize::Medium)
                .width(width)
                .disabled(true)
                .tooltip(later.clone()),
        )
        .child(
            Button::new("welcome-clone", workbench.text("welcome-clone"))
                .variant(ButtonVariant::Secondary)
                .size(ButtonSize::Medium)
                .width(width)
                .disabled(true)
                .tooltip(later),
        );
    div()
        .flex()
        .flex_col()
        .child(logo)
        .child(
            div().mt(px(metrics.space.xl)).child(
                Label::new(workbench.text("welcome-title"))
                    .size(metrics.font_size.display)
                    .semibold()
                    .color(colors.text_primary),
            ),
        )
        .child(
            div().mt(px(metrics.space.sm)).child(
                Label::new(workbench.text("welcome-subtitle"))
                    .size(metrics.font_size.xl)
                    .color(colors.text_secondary),
            ),
        )
        .child(
            div()
                .mt(px(metrics.space.xl + metrics.space.lg))
                .child(buttons),
        )
        .into_any_element()
}

fn recent(workbench: &Workbench, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let colors = &theme.colors;
    let metrics = &theme.metrics;
    let card = div()
        .flex()
        .flex_col()
        .w_full()
        .rounded(px(metrics.radius.lg))
        .bg(colors.bg_card);
    let card = if workbench.recent.is_empty() {
        card.min_h(px(metrics.size.welcome_recent_height))
            .flex_row()
            .items_center()
            .gap(px(metrics.space.lg))
            .px(px(metrics.space.xl))
            .child(Icon::new(IconName::Folder).medium(cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(metrics.space.sm))
                    .child(
                        Label::new(workbench.text("welcome-recent-empty-title"))
                            .size(metrics.font_size.lg)
                            .semibold()
                            .color(colors.text_primary),
                    )
                    .child(
                        Label::new(workbench.text("welcome-recent-empty-text"))
                            .size(metrics.font_size.md)
                            .color(colors.text_subtle),
                    ),
            )
    } else {
        let rows = workbench
            .recent
            .iter()
            .take(RECENT_ROWS)
            .enumerate()
            .map(|(index, entry)| {
                let open_path = workbench.open_path.clone();
                let root = entry.root.clone();
                let hover: gpui::Hsla = colors.bg_list_hover.into();
                div()
                    .id(("welcome-recent", index))
                    .flex()
                    .items_center()
                    .gap(px(metrics.space.lg))
                    .px(px(metrics.space.xl))
                    .py(px(metrics.space.md))
                    .rounded(px(metrics.radius.lg))
                    .cursor_pointer()
                    .hover(move |style| style.bg(hover))
                    .on_click(move |_, window, cx| open_path(root.clone(), window, cx))
                    .child(Icon::new(IconName::Folder).medium(cx))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .min_w_0()
                            .gap(px(metrics.space.xs))
                            .child(
                                Label::new(entry.name.clone())
                                    .size(metrics.font_size.lg)
                                    .semibold()
                                    .color(colors.text_primary),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(metrics.font_size.md))
                                    .text_color(colors.text_subtle)
                                    .child(entry.path.clone()),
                            ),
                    )
            });
        card.py(px(metrics.space.xs)).children(rows)
    };
    div()
        .flex()
        .flex_col()
        .gap(px(metrics.space.lg))
        .child(section_title(&workbench.text("welcome-recent"), cx))
        .child(card)
        .into_any_element()
}

fn get_started(workbench: &Workbench, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let colors = &theme.colors;
    let metrics = &theme.metrics;
    let rows = GET_STARTED.iter().map(|(label, command)| {
        let keys = workbench.shortcut_label(command);
        let hover: gpui::Hsla = colors.text_primary.into();
        div()
            .id(*label)
            .flex()
            .items_center()
            .justify_between()
            .gap(px(metrics.space.lg))
            .h(px(metrics.size.welcome_row))
            .cursor_pointer()
            .text_size(px(metrics.font_size.base))
            .text_color(colors.text_secondary)
            .hover(move |style| style.text_color(hover))
            .on_click({
                let handler = run(workbench, command);
                move |_, window, cx| handler(window, cx)
            })
            .child(workbench.text(label))
            .children(keys.map(|keys| {
                div()
                    .w(px(metrics.size.kbd_width))
                    .flex()
                    .child(Kbd::new(keys).fill())
            }))
    });
    div()
        .flex()
        .flex_col()
        .child(
            div()
                .mb(px(metrics.space.md))
                .child(section_title(&workbench.text("welcome-get-started"), cx)),
        )
        .children(rows)
        .into_any_element()
}

fn footer(workbench: &Workbench, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let colors = &theme.colors;
    let metrics = &theme.metrics;
    let link_hover: gpui::Hsla = colors.text_primary.into();
    let link = div()
        .id("welcome-docs")
        .flex()
        .items_center()
        .gap(px(metrics.space.sm))
        .cursor_pointer()
        .text_size(px(metrics.font_size.md))
        .text_color(colors.text_accent)
        .hover(move |style| style.text_color(link_hover))
        .on_click(|_, _, cx| cx.open_url(app_identity::REPOSITORY_URL))
        .child(workbench.text("welcome-docs-link"))
        .child(Icon::new(IconName::ArrowRight).color(colors.text_accent));
    div()
        .flex()
        .items_start()
        .gap(px(metrics.space.lg))
        .pt(px(metrics.space.xxl + metrics.space.lg))
        .border_t_1()
        .border_color(colors.border_divider)
        .child(Icon::new(IconName::BookOpen).medium(cx))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .gap(px(metrics.space.sm))
                .child(
                    Label::new(workbench.text("welcome-workflow-title"))
                        .size(metrics.font_size.lg)
                        .semibold()
                        .color(colors.text_primary),
                )
                .child(
                    Label::new(workbench.text("welcome-workflow-text"))
                        .size(metrics.font_size.base)
                        .color(colors.text_secondary),
                ),
        )
        .child(link)
        .into_any_element()
}

pub(crate) fn render(
    workbench: &Workbench,
    cx: &mut Context<Workbench>,
) -> impl IntoElement + use<> {
    let hero = hero(workbench, cx);
    let recent = recent(workbench, cx);
    let get_started = get_started(workbench, cx);
    let footer = footer(workbench, cx);
    let theme = cx.theme();
    let colors = &theme.colors;
    let metrics = &theme.metrics;
    let size = &metrics.size;

    let header = div()
        .flex()
        .flex_none()
        .items_center()
        .h(px(size.editor_header_height))
        .mx(px(metrics.space.lg))
        .px(px(metrics.space.sm))
        .border_b_1()
        .border_color(colors.border_divider)
        .text_size(px(metrics.font_size.base))
        .text_color(colors.text_primary)
        .child(workbench.text("welcome-tab"));
    // The left column keeps its mockup width when there is room and gives
    // way first on narrow windows; Get started keeps its natural width.
    let left = div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .max_w(px(size.welcome_column))
        .gap(px(metrics.space.xxl + metrics.space.xl))
        .child(hero)
        .child(recent);
    let right = div()
        .flex_none()
        .pl(px(metrics.space.xxl * 2.0))
        .border_l_1()
        .border_color(colors.border_divider)
        .child(get_started);
    let columns = div()
        .flex()
        .gap(px(metrics.space.xxl * 2.0))
        .child(left)
        .child(right);
    let content = div()
        .flex()
        .flex_col()
        .w_full()
        .max_w(px(size.welcome_width))
        .gap(px(metrics.space.xxl * 2.0))
        .child(columns)
        .child(footer);
    div()
        .flex()
        .flex_col()
        .size_full()
        .child(header)
        .child(
            div()
                .id("welcome-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .px(px(metrics.space.xxl))
                .pt(px(size.welcome_top))
                .pb(px(metrics.space.xxl))
                .flex()
                .flex_col()
                .items_center()
                .child(content),
        )
        .text_color(colors.text_primary)
        .font_weight(FontWeight::NORMAL)
}
