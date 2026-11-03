//! Gallery sections for controls: buttons, badges, tabs.

use gpui::{
    AnyElement, App, Context, IntoElement, ParentElement as _, SharedString, Styled as _, div, px,
};
use theme::ActiveTheme as _;
use ui::{
    Button, ButtonSize, ButtonVariant, CountBadge, EditorTab, IconButton, IconName, Kbd, Label,
    PanelTab, SegmentedTab, StatusKind, StatusLetter,
};

use super::super::samples::{self};
use super::super::{Gallery, section};

/// Width of the Welcome-style buttons.
const WELCOME_BUTTON_WIDTH: f32 = 180.0;

/// Applies a forced state when the gallery cell has one.
macro_rules! with_state {
    ($component:expr, $state:expr) => {
        match $state {
            Some(state) => $component.force_state(state),
            None => $component,
        }
    };
}

pub(in crate::gallery) fn buttons(gallery: &Gallery, cx: &mut Context<Gallery>) -> AnyElement {
    let variants = [
        (
            "gallery-variant-primary",
            ButtonVariant::Primary,
            ButtonSize::Small,
        ),
        (
            "gallery-variant-secondary",
            ButtonVariant::Secondary,
            ButtonSize::Small,
        ),
        (
            "gallery-variant-ghost",
            ButtonVariant::Ghost,
            ButtonSize::Small,
        ),
        (
            "gallery-variant-medium",
            ButtonVariant::Primary,
            ButtonSize::Medium,
        ),
    ];
    let mut rows = Vec::new();
    for (index, (name, variant, size)) in variants.into_iter().enumerate() {
        let label = gallery.text(match variant {
            ButtonVariant::Primary => "gallery-sample-run-project",
            ButtonVariant::Secondary => "gallery-sample-new-project",
            ButtonVariant::Ghost => "gallery-sample-open-folder",
        });
        let icon = (variant == ButtonVariant::Primary && size == ButtonSize::Small)
            .then_some(IconName::Play);
        rows.push(caption(gallery.text(name), cx));
        rows.push(
            gallery.states_row(&format!("button-{index}"), cx, |state, id| {
                let mut button = Button::new(id, label.clone())
                    .variant(variant)
                    .size(size)
                    .on_activate(|_, _| {});
                if let Some(icon) = icon {
                    button = button.icon(icon);
                }
                if size == ButtonSize::Medium {
                    button = button.width(WELCOME_BUTTON_WIDTH);
                }
                with_state!(button, state).into_any_element()
            }),
        );
    }
    rows.push(caption(gallery.text("gallery-variant-icon"), cx));
    rows.push(gallery.states_row("icon-button", cx, |state, id| {
        with_state!(
            IconButton::new(id, IconName::Ellipsis).on_activate(|_, _| {}),
            state
        )
        .into_any_element()
    }));
    section(gallery.text("gallery-section-buttons"), rows, cx)
}

pub(super) fn caption(text: SharedString, cx: &App) -> AnyElement {
    let theme = cx.theme();
    Label::new(text)
        .size(theme.metrics.font_size.md)
        .medium()
        .color(theme.colors.text_secondary)
        .into_any_element()
}

pub(in crate::gallery) fn badges(gallery: &Gallery, cx: &mut Context<Gallery>) -> AnyElement {
    let gap = cx.theme().metrics.space.lg;
    let row = div()
        .flex()
        .items_center()
        .gap(px(gap))
        .child(Kbd::new(samples::SHORTCUT_PALETTE))
        .child(Kbd::new(samples::SHORTCUT_PALETTE).plain())
        .child(CountBadge::new(samples::SAMPLE_COUNT))
        .child(StatusLetter::new(
            samples::STATUS_MODIFIED,
            StatusKind::Modified,
        ))
        .child(StatusLetter::new(samples::STATUS_ADDED, StatusKind::Added))
        .child(StatusLetter::new(
            samples::STATUS_DELETED,
            StatusKind::Deleted,
        ))
        .into_any_element();
    section(gallery.text("gallery-section-badges"), vec![row], cx)
}

pub(super) fn rust_letter(cx: &App) -> AnyElement {
    let theme = cx.theme();
    Label::new(samples::RUST_FILE_LETTER)
        .mono()
        .medium()
        .size(theme.metrics.font_size.base)
        .color(theme.colors.icon_file_rust)
        .into_any_element()
}

pub(in crate::gallery) fn tabs(gallery: &Gallery, cx: &mut Context<Gallery>) -> AnyElement {
    let entity = cx.entity().downgrade();
    let gap = cx.theme().metrics.space.sm;
    let mut rows = vec![gallery.states_row("editor-tab", cx, |state, id| {
        with_state!(
            EditorTab::new(id, samples::FILE_LIB)
                .leading(rust_letter(cx))
                .on_activate(|_, _| {}),
            state
        )
        .into_any_element()
    })];

    let names = [samples::FILE_MAIN, samples::FILE_LIB, samples::FILE_CARGO];
    let live = names.into_iter().enumerate().map(|(index, name)| {
        let entity = entity.clone();
        EditorTab::new(("live-editor-tab", index), name)
            .leading(rust_letter(cx))
            .active(gallery.live_tab == index)
            .dirty(index == 0)
            .on_activate(move |_, cx| {
                let _ = entity.update(cx, |gallery, cx| gallery.select_tab(index, cx));
            })
    });
    rows.push(caption(gallery.text("gallery-state-live"), cx));
    rows.push(div().flex().gap(px(gap)).children(live).into_any_element());

    let panel_labels = [
        "gallery-sample-terminal",
        "gallery-sample-problems",
        "gallery-sample-output",
    ];
    rows.push(gallery.states_row("panel-tab", cx, |state, id| {
        with_state!(
            PanelTab::new(id, gallery.text(panel_labels[2])).on_activate(|_, _| {}),
            state
        )
        .into_any_element()
    }));
    let panel_tabs = panel_labels.into_iter().enumerate().map(|(index, label)| {
        let tab = PanelTab::new(("live-panel-tab", index), gallery.text(label))
            .active(gallery.live_tab == index);
        let tab = if index == 1 { tab.count("0") } else { tab };
        let entity = entity.clone();
        tab.on_activate(move |_, cx| {
            let _ = entity.update(cx, |gallery, cx| gallery.select_tab(index, cx));
        })
    });
    let wide_gap = cx.theme().metrics.space.xxl;
    rows.push(
        div()
            .flex()
            .gap(px(wide_gap))
            .children(panel_tabs)
            .into_any_element(),
    );

    let segments = ["gallery-sample-installed", "gallery-sample-marketplace"];
    let segment_tabs = segments.into_iter().enumerate().map(|(index, label)| {
        let tab = SegmentedTab::new(("live-segment", index), gallery.text(label))
            .active(gallery.live_tab % 2 == index);
        let tab = if index == 0 {
            tab.count(samples::SAMPLE_COUNT)
        } else {
            tab
        };
        let entity = entity.clone();
        tab.on_activate(move |_, cx| {
            let _ = entity.update(cx, |gallery, cx| gallery.select_tab(index, cx));
        })
    });
    rows.push(
        div()
            .flex()
            .gap(px(wide_gap))
            .children(segment_tabs)
            .into_any_element(),
    );
    section(gallery.text("gallery-section-tabs"), rows, cx)
}

pub(super) use with_state;
