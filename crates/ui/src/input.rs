//! Text fields and the title bar search field.

use gpui::{
    App, ElementId, Entity, Focusable as _, Hsla, IntoElement, ParentElement as _, RenderOnce,
    SharedString, Styled as _, Window, div, px, transparent_black,
};
use gpui_component::input::{Input, InputState, Textarea, TextareaState};
use theme::ActiveTheme as _;

use crate::interaction::{Interactivity, StateColors, interactive_builders};
use crate::{Icon, IconName, InteractionState};

/// The search field of the title bar. It is a button that opens the
/// palette, so it shows a placeholder and a shortcut hint, not typed text.
#[derive(IntoElement)]
pub struct SearchField {
    interactivity: Interactivity,
    placeholder: SharedString,
    shortcut: Option<SharedString>,
}

impl SearchField {
    /// Creates a search field with a placeholder text.
    pub fn new(id: impl Into<ElementId>, placeholder: impl Into<SharedString>) -> Self {
        Self {
            interactivity: Interactivity::new(id),
            placeholder: placeholder.into(),
            shortcut: None,
        }
    }

    /// Shows a shortcut hint on the right (`Ctrl K`).
    #[must_use]
    pub fn shortcut(mut self, shortcut: impl Into<SharedString>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }
}

interactive_builders!(SearchField);

impl RenderOnce for SearchField {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let metrics = &theme.metrics;
        let background = StateColors {
            normal: colors.bg_input.into(),
            hover: colors.bg_button_secondary.into(),
            pressed: colors.bg_button_secondary_pressed.into(),
        };
        let border: Hsla = colors.border_input.into();
        let shortcut = self.shortcut.map(|keys| {
            div()
                .text_size(px(metrics.font_size.sm))
                .text_color(colors.text_muted)
                .child(keys)
        });
        let placeholder = div()
            .flex_1()
            .truncate()
            .text_size(px(metrics.font_size.md))
            .text_color(colors.text_subtle)
            .child(self.placeholder);
        let (height, radius, padding, gap) = (
            metrics.size.input_height,
            metrics.radius.md,
            metrics.space.lg,
            metrics.space.md,
        );
        self.interactivity
            .root_with_border(background, border, window, cx)
            .w_full()
            .min_w_0()
            .flex_shrink(1.0)
            .overflow_hidden()
            .h(px(height))
            .px(px(padding))
            .gap(px(gap))
            .rounded(px(radius))
            .child(Icon::new(IconName::Search))
            .child(placeholder)
            .children(shortcut)
    }
}

/// Look shared by [`TextField`] and [`TextArea`]: inset background, a border
/// that becomes the focus ring, reduced opacity when disabled.
fn field_container(state: InteractionState, bordered: bool, cx: &App) -> gpui::Div {
    let theme = cx.theme();
    let colors = &theme.colors;
    let border: Hsla = match state {
        InteractionState::Focused => colors.border_focus.into(),
        InteractionState::Hover | InteractionState::Pressed => colors.border_input.into(),
        _ if bordered => colors.border_input.into(),
        _ => transparent_black(),
    };
    let background: Hsla = if state == InteractionState::Disabled {
        colors.bg_input_disabled.into()
    } else {
        colors.bg_input_inset.into()
    };
    let mut container = div()
        .flex()
        .items_center()
        .w_full()
        .px(px(theme.metrics.space.md))
        .gap(px(theme.metrics.space.sm))
        .rounded(px(theme.metrics.radius.md))
        .border_1()
        .border_color(border)
        .bg(background)
        .text_size(px(theme.metrics.font_size.base))
        .text_color(colors.text_primary);
    if state == InteractionState::Disabled {
        container = container.opacity(theme.opacity.disabled);
    }
    container
}

/// A single-line text field (sidebar search, "files to include").
///
/// Editing comes from gpui-component; the caller owns the
/// [`InputState`] (create it with `InputState::new(window, cx)`).
#[derive(IntoElement)]
pub struct TextField {
    state: Entity<InputState>,
    icon: Option<IconName>,
    disabled: bool,
    forced: Option<InteractionState>,
    embedded: bool,
}

impl TextField {
    /// Creates a field editing `state`.
    pub fn new(state: &Entity<InputState>) -> Self {
        Self {
            state: state.clone(),
            icon: None,
            disabled: false,
            forced: None,
            embedded: false,
        }
    }

    /// Draws only the text and icon, without background or border, at the
    /// height of the title bar search field: for fields inside another
    /// frame (quick open).
    #[must_use]
    pub fn embedded(mut self) -> Self {
        self.embedded = true;
        self
    }

    /// Adds a leading icon.
    #[must_use]
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Disables editing.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Renders the frame in `state` regardless of focus (gallery, tests).
    #[must_use]
    pub fn force_state(mut self, state: InteractionState) -> Self {
        self.forced = Some(state);
        self
    }
}

impl RenderOnce for TextField {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focused = self.state.focus_handle(cx).is_focused(window);
        let state = self.forced.unwrap_or(if self.disabled {
            InteractionState::Disabled
        } else if focused {
            InteractionState::Focused
        } else {
            InteractionState::Normal
        });
        let icon = self.icon.map(Icon::new);
        let size = &cx.theme().metrics.size;
        let font_size = if self.embedded {
            cx.theme().metrics.font_size.md
        } else {
            cx.theme().metrics.font_size.base
        };
        let container = if self.embedded {
            // Same height as `SearchField` without its 1px border on each side.
            let height = size.input_height - 2.0;
            let padding = cx.theme().metrics.space.lg;
            let gap = cx.theme().metrics.space.md;
            div()
                .flex()
                .items_center()
                .w_full()
                .h(px(height))
                .px(px(padding - 1.0))
                .gap(px(gap))
                .text_size(px(cx.theme().metrics.font_size.md))
                .text_color(cx.theme().colors.text_primary)
        } else {
            let height = size.input_inset_height;
            field_container(state, false, cx).h(px(height))
        };
        container.children(icon).child(
            Input::new(&self.state)
                .appearance(false)
                // The frame owns the padding; the text sits right after the
                // icon, as in `SearchField`.
                .px_0()
                .text_size(px(font_size))
                .disabled(self.disabled || state == InteractionState::Disabled),
        )
    }
}

/// A multi-line text field (commit message).
#[derive(IntoElement)]
pub struct TextArea {
    state: Entity<TextareaState>,
    disabled: bool,
    forced: Option<InteractionState>,
}

impl TextArea {
    /// Creates a text area editing `state` (create it with
    /// `TextareaState::new(window, cx).rows(n)`).
    pub fn new(state: &Entity<TextareaState>) -> Self {
        Self {
            state: state.clone(),
            disabled: false,
            forced: None,
        }
    }

    /// Disables editing.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Renders the frame in `state` regardless of focus (gallery, tests).
    #[must_use]
    pub fn force_state(mut self, state: InteractionState) -> Self {
        self.forced = Some(state);
        self
    }
}

impl RenderOnce for TextArea {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focused = self.state.focus_handle(cx).is_focused(window);
        let state = self.forced.unwrap_or(if self.disabled {
            InteractionState::Disabled
        } else if focused {
            InteractionState::Focused
        } else {
            InteractionState::Normal
        });
        let padding = cx.theme().metrics.space.md;
        field_container(state, true, cx)
            .items_start()
            .py(px(padding))
            .child(
                Textarea::new(&self.state)
                    .appearance(false)
                    .disabled(self.disabled || state == InteractionState::Disabled),
            )
    }
}
