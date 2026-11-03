//! Buttons.

use gpui::{
    App, ElementId, FontWeight, Hsla, IntoElement, ParentElement as _, RenderOnce, SharedString,
    Styled as _, Window, px, transparent_black,
};
use theme::ActiveTheme as _;

use crate::interaction::{Interactivity, StateColors, interactive_builders};
use crate::{Icon, IconName};

/// Visual weight of a [`Button`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    /// Accent background: the main action of an area ("Run project").
    #[default]
    Primary,
    /// Neutral background: other actions ("New Rust project").
    Secondary,
    /// No background until hovered: toolbar actions.
    Ghost,
}

/// Height of a [`Button`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonSize {
    /// Title bar and inline actions.
    #[default]
    Small,
    /// Primary actions of a page (Welcome).
    Medium,
}

/// A text button with an optional leading icon.
#[derive(IntoElement)]
pub struct Button {
    interactivity: Interactivity,
    label: SharedString,
    icon: Option<IconName>,
    variant: ButtonVariant,
    size: ButtonSize,
    width: Option<f32>,
}

impl Button {
    /// Creates a primary, small button.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            interactivity: Interactivity::new(id),
            label: label.into(),
            icon: None,
            variant: ButtonVariant::default(),
            size: ButtonSize::default(),
            width: None,
        }
    }

    /// Sets the variant.
    #[must_use]
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Sets the size.
    #[must_use]
    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    /// Adds a leading icon.
    #[must_use]
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Fixed width in logical pixels; the label stays left-aligned as in
    /// the mockups.
    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }
}

interactive_builders!(Button);

impl RenderOnce for Button {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let metrics = &theme.metrics;
        let (background, text): (StateColors, Hsla) = match self.variant {
            ButtonVariant::Primary => (
                StateColors {
                    normal: colors.bg_button_primary.into(),
                    hover: colors.bg_button_primary_hover.into(),
                    pressed: colors.bg_button_primary_pressed.into(),
                },
                colors.text_on_accent.into(),
            ),
            ButtonVariant::Secondary => (
                StateColors {
                    normal: colors.bg_button_secondary.into(),
                    hover: colors.bg_button_secondary_hover.into(),
                    pressed: colors.bg_button_secondary_pressed.into(),
                },
                colors.text_primary.into(),
            ),
            ButtonVariant::Ghost => (
                StateColors {
                    normal: transparent_black(),
                    hover: colors.bg_ghost_hover.into(),
                    pressed: colors.bg_ghost_pressed.into(),
                },
                colors.text_secondary.into(),
            ),
        };
        let height = match self.size {
            ButtonSize::Small => metrics.size.button_small_height,
            ButtonSize::Medium => metrics.size.button_medium_height,
        };
        let radius = metrics.radius.md;
        let padding = metrics.space.lg;
        let gap = metrics.space.sm;
        let font_size = metrics.font_size.md;
        let icon = self.icon.map(|name| Icon::new(name).color(text));
        let mut interactivity = self.interactivity;
        if self.variant == ButtonVariant::Primary {
            interactivity.focus_ring = Some(colors.border_focus_on_accent.into());
        }

        let mut root = interactivity
            .root(background, window, cx)
            .h(px(height))
            .px(px(padding))
            .gap(px(gap))
            .rounded(px(radius))
            .text_color(text)
            .text_size(px(font_size))
            .font_weight(FontWeight::SEMIBOLD)
            .children(icon)
            .child(self.label);
        if let Some(width) = self.width {
            root = root.w(px(width));
        }
        root
    }
}

/// A square ghost button with an icon (toolbars, panel headers).
#[derive(IntoElement)]
pub struct IconButton {
    interactivity: Interactivity,
    icon: IconName,
}

impl IconButton {
    /// Creates an icon button.
    pub fn new(id: impl Into<ElementId>, icon: IconName) -> Self {
        Self {
            interactivity: Interactivity::new(id),
            icon,
        }
    }
}

interactive_builders!(IconButton);

/// Side of an [`IconButton`] relative to the small icon size.
const ICON_BUTTON_PADDING_STEPS: f32 = 2.0;

impl RenderOnce for IconButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let metrics = &theme.metrics;
        let side = metrics.size.icon_small + metrics.space.xs * ICON_BUTTON_PADDING_STEPS * 2.0;
        let radius = metrics.radius.sm;
        let state = self.interactivity.static_state();
        let icon_color: Hsla = if matches!(
            state,
            crate::InteractionState::Hover | crate::InteractionState::Pressed
        ) {
            colors.icon_strong.into()
        } else {
            colors.icon.into()
        };
        let background = StateColors {
            normal: transparent_black(),
            hover: colors.bg_ghost_hover.into(),
            pressed: colors.bg_ghost_pressed.into(),
        };
        self.interactivity
            .root(background, window, cx)
            .size(px(side))
            .justify_center()
            .rounded(px(radius))
            .child(Icon::new(self.icon).color(icon_color))
    }
}
