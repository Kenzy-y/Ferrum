//! The page of an extension in the editor area, as in `extensions.svg`:
//! the header with the path and close button, the icon, name, publisher,
//! description, actions and state, then the Overview or Feature
//! contributions tab.

use gpui::{
    AnyElement, App, Context, FontWeight, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _, div, px,
};
use theme::ActiveTheme as _;
use ui::{Button, ButtonVariant, Card, Icon, IconButton, IconName, SegmentedTab};

use super::{ExtensionEntry, ExtensionItem, PageTab, on_workbench};
use crate::Workbench;

fn header(workbench: &Workbench, entry: &ExtensionEntry, cx: &Context<Workbench>) -> AnyElement {
    let theme = cx.theme();
    let colors = &theme.colors;
    let metrics = &theme.metrics;
    div()
        .flex()
        .flex_none()
        .items_center()
        .h(px(metrics.size.editor_header_height))
        .mx(px(metrics.space.lg))
        .px(px(metrics.space.sm))
        .border_b_1()
        .border_color(colors.border_divider)
        .text_size(px(metrics.font_size.base))
        .text_color(colors.text_primary)
        .child(div().flex_1().truncate().child(format!(
            "{}  /  {}",
            workbench.text("view-extensions"),
            entry.name
        )))
        .child(
            IconButton::new("extension-page-close", IconName::X)
                .tooltip(workbench.text("extensions-close-page"))
                .on_activate(on_workbench(cx, Workbench::close_extension_page)),
        )
        .into_any_element()
}

fn hero(
    workbench: &Workbench,
    index: usize,
    entry: &ExtensionEntry,
    cx: &Context<Workbench>,
) -> AnyElement {
    let theme = cx.theme();
    let colors = &theme.colors;
    let metrics = &theme.metrics;
    let tile = div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(metrics.size.extension_tile))
        .rounded(px(metrics.radius.xxl))
        .bg(colors.bg_tile_accent)
        .child(
            Icon::new(entry.icon)
                .size(metrics.size.empty_state_icon)
                .color(colors.accent),
        );
    let toggle_label = if entry.enabled {
        "extensions-disable"
    } else {
        "extensions-enable"
    };
    let actions = div()
        .flex()
        .gap(px(metrics.space.md))
        .mt(px(metrics.space.lg))
        .child(
            Button::new("extension-toggle", workbench.text(toggle_label))
                .variant(ButtonVariant::Secondary)
                .on_activate(on_workbench(cx, move |workbench, cx| {
                    workbench.toggle_extension(index, cx);
                })),
        )
        .child(
            Button::new(
                "extension-uninstall",
                workbench.text("extensions-uninstall"),
            )
            .variant(ButtonVariant::Secondary)
            .disabled(true)
            .tooltip(workbench.text("welcome-later")),
        );
    let text = div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .gap(px(metrics.space.sm))
        .child(
            div()
                .text_size(px(metrics.font_size.h2))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(colors.text_primary)
                .child(entry.name.clone()),
        )
        .child(
            div()
                .text_size(px(metrics.font_size.base))
                .text_color(colors.text_secondary)
                .child(entry.publisher.clone()),
        )
        .child(
            div()
                .mt(px(metrics.space.sm))
                .text_size(px(metrics.font_size.lg))
                .text_color(colors.text_secondary)
                .child(entry.description.clone()),
        )
        .child(actions);
    div()
        .flex()
        .items_start()
        .gap(px(metrics.space.xl))
        .child(tile)
        .child(text)
        .child(install_state(workbench, entry, cx))
        .into_any_element()
}

/// "Installed and enabled" and where, on the right of the hero.
fn install_state(workbench: &Workbench, entry: &ExtensionEntry, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let colors = &theme.colors;
    let metrics = &theme.metrics;
    let (state, detail, state_color): (SharedString, SharedString, Hsla) = if entry.enabled {
        (
            workbench.text("extensions-installed-enabled"),
            workbench.text("extensions-enabled-workspace"),
            colors.text_success.into(),
        )
    } else {
        (
            workbench.text("extensions-installed-disabled"),
            workbench.text("extensions-disabled-workspace"),
            colors.text_muted.into(),
        )
    };
    div()
        .flex()
        .flex_col()
        .flex_none()
        .gap(px(metrics.space.sm))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(metrics.space.md))
                .text_size(px(metrics.font_size.base))
                .text_color(state_color)
                .child(Icon::new(IconName::Check).color(state_color))
                .child(state),
        )
        .child(
            div()
                .pl(px(metrics.size.icon_small + metrics.space.md))
                .text_size(px(metrics.font_size.sm))
                .text_color(colors.text_muted)
                .child(detail),
        )
        .into_any_element()
}

fn page_tabs(workbench: &Workbench, cx: &Context<Workbench>) -> AnyElement {
    let theme = cx.theme();
    let current = workbench.extensions.page_tab;
    let tab = |id: &'static str, label: &str, kind: PageTab| {
        SegmentedTab::new(id, workbench.text(label))
            .active(current == kind)
            .on_activate(on_workbench(cx, move |workbench, cx| {
                workbench.extensions.page_tab = kind;
                cx.notify();
            }))
    };
    div()
        .flex()
        .gap(px(theme.metrics.space.xxl + theme.metrics.space.lg))
        .border_b_1()
        .border_color(theme.colors.border_divider)
        .child(tab(
            "extension-overview",
            "extensions-overview",
            PageTab::Overview,
        ))
        .child(tab(
            "extension-contributions",
            "extensions-contributions",
            PageTab::Contributions,
        ))
        .into_any_element()
}

/// A checked row: title, then the detail in a second column (features)
/// or below it (the side card).
fn checked(item: &ExtensionItem, columns: bool, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let colors = &theme.colors;
    let metrics = &theme.metrics;
    let title = div()
        .text_size(px(metrics.font_size.base))
        .font_weight(FontWeight::MEDIUM)
        .text_color(colors.text_primary)
        .child(item.title.clone());
    let detail = div()
        .text_size(px(metrics.font_size.md))
        .text_color(colors.text_secondary)
        .child(item.detail.clone());
    let body = if columns {
        div()
            .flex()
            .flex_1()
            .min_w_0()
            .items_start()
            .child(
                title
                    .w(px(metrics.size.extension_feature_title))
                    .flex_none(),
            )
            .child(detail.flex_1().min_w_0())
    } else {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .gap(px(metrics.space.md))
            .child(title)
            .child(detail)
    };
    div()
        .flex()
        .items_start()
        .w_full()
        .min_w_0()
        .gap(px(metrics.space.md))
        .child(Icon::new(IconName::Check).color(colors.icon))
        .child(body)
        .into_any_element()
}

fn code_sample(lines: &[SharedString], cx: &App) -> AnyElement {
    let theme = cx.theme();
    let metrics = &theme.metrics;
    let rows = lines.iter().map(|line| {
        let comment = line.starts_with("//") || line.starts_with('#');
        let color = if comment {
            theme.syntax.comment
        } else {
            theme.syntax.plain
        };
        div().text_color(color).child(line.clone())
    });
    div()
        .flex()
        .flex_col()
        .gap(px(metrics.space.md))
        .p(px(metrics.space.lg))
        .rounded(px(metrics.radius.lg))
        .bg(theme.colors.bg_card)
        .font_family(SharedString::from(theme.fonts.mono.clone()))
        .text_size(px(metrics.font_size.base))
        .children(rows)
        .into_any_element()
}

fn overview(workbench: &Workbench, entry: &ExtensionEntry, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let colors = &theme.colors;
    let metrics = &theme.metrics;
    let overview = &entry.overview;
    let left = div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .gap(px(metrics.space.lg))
        .child(
            div()
                .text_size(px(metrics.font_size.h3 + metrics.space.xs))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(colors.text_primary)
                .child(overview.title.clone()),
        )
        .child(
            div()
                .text_size(px(metrics.font_size.lg))
                .text_color(colors.text_secondary)
                .child(overview.text.clone()),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(metrics.space.xl))
                .mt(px(metrics.space.md))
                .children(overview.features.iter().map(|item| checked(item, true, cx))),
        )
        .child(
            div()
                .mt(px(metrics.space.xl))
                .child(code_sample(&overview.sample, cx)),
        );
    let side = div()
        .flex()
        .flex_col()
        .flex_none()
        // A third of the page, as in the mockup, at most the token.
        .w(gpui::relative(1.0 / 3.0))
        .max_w(px(metrics.size.extension_side_width))
        .gap(px(metrics.space.xl))
        .p(px(metrics.space.xl))
        .rounded(px(metrics.radius.lg))
        .bg(colors.bg_card)
        .child(
            div()
                .text_size(px(metrics.font_size.sm))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(colors.text_subtle)
                .child(overview.side_title.to_uppercase()),
        )
        .children(overview.side.iter().map(|item| checked(item, false, cx)));
    let columns = div()
        .flex()
        .items_start()
        .gap(px(metrics.space.xxl * 2.0))
        .child(left)
        .child(side);
    div()
        .flex()
        .flex_col()
        .gap(px(metrics.space.xxl))
        .child(columns)
        .child(more(workbench, cx))
        .into_any_element()
}

/// "Make your workspace yours": the marketplace comes with stage 9.
fn more(workbench: &Workbench, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let colors = &theme.colors;
    let metrics = &theme.metrics;
    let card = div()
        .flex()
        .items_center()
        .gap(px(metrics.space.lg))
        .h(px(metrics.size.extension_card_height))
        .px(px(metrics.space.lg))
        .rounded(px(metrics.radius.lg))
        .bg(colors.bg_card)
        .child(Icon::new(IconName::Package).size(metrics.size.icon_medium))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .gap(px(metrics.space.sm))
                .child(
                    div()
                        .text_size(px(metrics.font_size.lg))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(colors.text_primary)
                        .child(workbench.text("extensions-marketplace")),
                )
                .child(
                    div()
                        .text_size(px(metrics.font_size.md))
                        .text_color(colors.text_secondary)
                        .child(workbench.text("extensions-marketplace-later")),
                ),
        )
        .child(
            Button::new("extension-browse", workbench.text("extensions-install"))
                .variant(ButtonVariant::Secondary)
                .disabled(true)
                .tooltip(workbench.text("welcome-later")),
        );
    div()
        .flex()
        .flex_col()
        .gap(px(metrics.space.xl))
        .child(
            div()
                .text_size(px(metrics.font_size.h3))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(colors.text_primary)
                .child(workbench.text("extensions-more-title")),
        )
        .child(
            div()
                .flex()
                .gap(px(metrics.space.xl))
                .child(card.flex_1().max_w(px(metrics.size.extension_side_width))),
        )
        .into_any_element()
}

fn contributions(workbench: &Workbench, entry: &ExtensionEntry, cx: &App) -> AnyElement {
    let theme = cx.theme();
    if entry.contributions.is_empty() {
        return div()
            .text_size(px(theme.metrics.font_size.md))
            .text_color(theme.colors.text_muted)
            .child(workbench.text("extensions-no-contributions"))
            .into_any_element();
    }
    Card::new()
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(theme.metrics.space.xl))
                .children(
                    entry
                        .contributions
                        .iter()
                        .map(|item| checked(item, true, cx)),
                ),
        )
        .into_any_element()
}

/// The page of entry `index`.
pub(crate) fn render(
    workbench: &Workbench,
    index: usize,
    cx: &mut Context<Workbench>,
) -> AnyElement {
    let Some(entry) = workbench.extensions.entries.get(index) else {
        return div().into_any_element();
    };
    let header = header(workbench, entry, cx);
    let hero = hero(workbench, index, entry, cx);
    let tabs = page_tabs(workbench, cx);
    let body = match workbench.extensions.page_tab {
        PageTab::Overview => overview(workbench, entry, cx),
        PageTab::Contributions => contributions(workbench, entry, cx),
    };
    let metrics = &cx.theme().metrics;
    let inset = metrics.space.xxl + metrics.space.lg;
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .child(header)
        .child(
            div()
                .id("extension-page")
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .px(px(inset))
                .pt(px(metrics.space.xxl + metrics.space.sm))
                .pb(px(metrics.space.xxl))
                .gap(px(metrics.space.xxl))
                .child(hero)
                .child(tabs)
                .child(body),
        )
        .into_any_element()
}
