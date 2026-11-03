//! Gallery sections for containers: panels, lists, inputs, icons.

use gpui::{
    AnyElement, App, Context, IntoElement, ParentElement as _, Styled as _, Window, div, px,
};
use theme::ActiveTheme as _;
use ui::{
    ActivityButton, Card, IconButton, IconName, InteractionState, Label, ListRow, Panel,
    PanelSurface, SearchField, SectionHeader, StatusKind, StatusLetter, TextArea, TextField,
};

use super::super::samples::{self};
use super::super::{Gallery, section};
use super::controls::{caption, rust_letter, with_state};

/// Width of the panel holding the sample card.
const SAMPLE_CARD_PANEL_WIDTH: f32 = 380.0;

/// Width of the sample panels.
const SAMPLE_PANEL_WIDTH: f32 = 291.0;

/// Width of the sample text fields.
const SAMPLE_FIELD_WIDTH: f32 = 260.0;

/// Width of the title bar search sample.
const SAMPLE_SEARCH_WIDTH: f32 = 420.0;

pub(in crate::gallery) fn panels(gallery: &Gallery, cx: &mut Context<Gallery>) -> AnyElement {
    let theme = cx.theme();
    let space = &theme.metrics.space;
    let sidebar = Panel::new(PanelSurface::Sidebar)
        .w(px(SAMPLE_PANEL_WIDTH))
        .py(px(space.lg))
        .gap(px(space.md))
        .child(
            SectionHeader::panel(gallery.text("gallery-sample-explorer"))
                .action(IconButton::new("panel-more", IconName::Ellipsis).on_activate(|_, _| {})),
        )
        .child(
            SectionHeader::group(gallery.text("gallery-sample-files"), None)
                .action(IconButton::new("panel-add", IconName::Plus).on_activate(|_, _| {})),
        )
        .child(SectionHeader::group(
            gallery.text("gallery-sample-outline"),
            Some(true),
        ));
    let card = Card::new()
        .child(
            Label::new(gallery.text("gallery-sample-card-title"))
                .size(theme.metrics.font_size.lg)
                .semibold()
                .color(theme.colors.text_primary),
        )
        .child(
            Label::new(gallery.text("gallery-sample-card-text"))
                .size(theme.metrics.font_size.md)
                .color(theme.colors.text_subtle),
        );
    let editor = Panel::new(PanelSurface::Editor)
        .w(px(SAMPLE_CARD_PANEL_WIDTH))
        .p(px(space.lg))
        .child(card);
    let row = div()
        .flex()
        .items_start()
        .gap(px(space.lg))
        .child(sidebar)
        .child(editor)
        .into_any_element();
    section(gallery.text("gallery-section-panels"), vec![row], cx)
}

pub(in crate::gallery) fn lists(gallery: &Gallery, cx: &mut Context<Gallery>) -> AnyElement {
    let entity = cx.entity().downgrade();
    let space = cx.theme().metrics.space.clone();
    let mut rows = vec![gallery.states_row("list-row", cx, |state, id| {
        let row = ListRow::new(id, samples::FILE_LIB)
            .leading(rust_letter(cx))
            .trailing(StatusLetter::new(
                samples::STATUS_MODIFIED,
                StatusKind::Modified,
            ))
            .on_activate(|_, _| {});
        div()
            .w(px(SAMPLE_FIELD_WIDTH / 1.5))
            .child(with_state!(row, state))
            .into_any_element()
    })];

    let tree = [
        (samples::FOLDER_SRC, 0, Some(true), false),
        (samples::FILE_MAIN, 1, None, false),
        (samples::FILE_LIB, 1, None, false),
        (samples::FOLDER_TARGET, 0, Some(false), true),
        (samples::FILE_CARGO, 0, None, false),
    ];
    let tree_rows =
        tree.into_iter()
            .enumerate()
            .map(|(index, (name, depth, expanded, ignored))| {
                let entity = entity.clone();
                let leading = if expanded.is_some() {
                    ui::Icon::new(IconName::Folder).into_any_element()
                } else if depth > 0 {
                    rust_letter(cx)
                } else {
                    ui::Icon::new(IconName::File).into_any_element()
                };
                let mut row = ListRow::new(("live-tree", index), name)
                    .depth(depth)
                    .leading(leading)
                    .ignored(ignored)
                    .selected(gallery.live_row == index)
                    .on_activate(move |_, cx| {
                        let _ = entity.update(cx, |gallery, cx| gallery.select_row(index, cx));
                    });
                if let Some(expanded) = expanded {
                    row = row.expanded(expanded);
                }
                if index == 1 {
                    row = row.trailing(StatusLetter::new(
                        samples::STATUS_MODIFIED,
                        StatusKind::Modified,
                    ));
                }
                row
            });
    rows.push(caption(gallery.text("gallery-state-live"), cx));
    rows.push(
        Panel::new(PanelSurface::Sidebar)
            .w(px(SAMPLE_PANEL_WIDTH))
            .p(px(space.sm))
            .children(tree_rows)
            .into_any_element(),
    );

    rows.push(gallery.states_row("activity", cx, |state, id| {
        with_state!(
            ActivityButton::new(id, IconName::Search).on_activate(|_, _| {}),
            state
        )
        .into_any_element()
    }));
    let activity_icons = [
        IconName::Files,
        IconName::Search,
        IconName::GitBranch,
        IconName::Play,
        IconName::Package,
    ];
    let activity = activity_icons.into_iter().enumerate().map(|(index, icon)| {
        let entity = entity.clone();
        ActivityButton::new(("live-activity", index), icon)
            .active(gallery.live_row == index)
            .on_activate(move |_, cx| {
                let _ = entity.update(cx, |gallery, cx| gallery.select_row(index, cx));
            })
    });
    let bg = cx.theme().colors.bg_app;
    let bar_width = cx.theme().metrics.size.activity_bar_width;
    rows.push(
        div()
            .flex()
            .flex_col()
            .items_center()
            .w(px(bar_width))
            .p(px(space.sm))
            .gap(px(space.lg))
            .bg(bg)
            .children(activity)
            .into_any_element(),
    );
    section(gallery.text("gallery-section-lists"), rows, cx)
}

pub(in crate::gallery) fn inputs(
    gallery: &Gallery,
    _window: &mut Window,
    cx: &mut Context<Gallery>,
) -> AnyElement {
    let placeholder = gallery.text("gallery-sample-search-placeholder");
    let mut rows = vec![gallery.states_row("search", cx, |state, id| {
        let field = SearchField::new(id, placeholder.clone())
            .shortcut(samples::SHORTCUT_SEARCH)
            .on_activate(|_, _| {});
        div()
            .w(px(SAMPLE_SEARCH_WIDTH))
            .child(with_state!(field, state))
            .into_any_element()
    })];
    let cells: Vec<Option<InteractionState>> = InteractionState::ALL
        .into_iter()
        .map(Some)
        .chain(std::iter::once(None))
        .collect();
    let field_cells = cells.iter().zip(&gallery.fields).map(|(state, input)| {
        let field = TextField::new(input).icon(IconName::Search);
        let field = match state {
            Some(InteractionState::Disabled) => field.disabled(true),
            Some(state) => field.force_state(*state),
            None => field,
        };
        cell(
            gallery,
            *state,
            div()
                .w(px(SAMPLE_FIELD_WIDTH))
                .child(field)
                .into_any_element(),
            cx,
        )
    });
    let commit_cells = cells.iter().zip(&gallery.commits).map(|(state, input)| {
        let area = TextArea::new(input);
        let area = match state {
            Some(InteractionState::Disabled) => area.disabled(true),
            Some(state) => area.force_state(*state),
            None => area,
        };
        cell(
            gallery,
            *state,
            div()
                .w(px(SAMPLE_FIELD_WIDTH))
                .child(area)
                .into_any_element(),
            cx,
        )
    });
    let gap = cx.theme().metrics.space.lg;
    rows.push(
        div()
            .flex()
            .flex_wrap()
            .gap(px(gap))
            .children(field_cells)
            .into_any_element(),
    );
    rows.push(
        div()
            .flex()
            .flex_wrap()
            .gap(px(gap))
            .children(commit_cells)
            .into_any_element(),
    );
    section(gallery.text("gallery-section-inputs"), rows, cx)
}

/// A captioned cell for components that cannot go through `states_row`
/// because each cell needs its own editing state.
fn cell(
    gallery: &Gallery,
    state: Option<InteractionState>,
    content: AnyElement,
    cx: &App,
) -> AnyElement {
    let theme = cx.theme();
    div()
        .flex()
        .flex_col()
        .gap(px(theme.metrics.space.xs))
        .child(
            Label::new(gallery.state_label(state))
                .size(theme.metrics.font_size.xs)
                .color(theme.colors.text_muted),
        )
        .child(content)
        .into_any_element()
}

pub(in crate::gallery) fn icons(gallery: &Gallery, cx: &mut Context<Gallery>) -> AnyElement {
    let theme = cx.theme();
    let gap = theme.metrics.space.lg;
    let grid = div()
        .flex()
        .flex_wrap()
        .gap(px(gap))
        .children(
            IconName::ALL
                .iter()
                .map(|icon| ui::Icon::new(*icon).medium(cx)),
        )
        .into_any_element();
    section(gallery.text("gallery-section-icons"), vec![grid], cx)
}
