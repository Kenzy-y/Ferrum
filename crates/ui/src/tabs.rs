//! Tabs: editor tabs and underlined tabs (panel tabs, segmented tabs).

use gpui::{
    AnyElement, App, ElementId, Hsla, IntoElement, ParentElement as _, RenderOnce, SharedString,
    Styled as _, Window, div, px, transparent_black,
};
use theme::ActiveTheme as _;

use crate::interaction::{Interactivity, StateColors, interactive_builders};
use crate::{CountBadge, Icon, IconName, InteractionState};

/// A tab of an open file in the editor tab strip.
#[derive(IntoElement)]
pub struct EditorTab {
    interactivity: Interactivity,
    title: SharedString,
    leading: Option<AnyElement>,
    active: bool,
    dirty: bool,
}

impl EditorTab {
    /// Creates an inactive, saved tab.
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            interactivity: Interactivity::new(id),
            title: title.into(),
            leading: None,
            active: false,
            dirty: false,
        }
    }

    /// Element before the title (file type icon).
    #[must_use]
    pub fn leading(mut self, element: impl IntoElement) -> Self {
        self.leading = Some(element.into_any_element());
        self
    }

    /// Marks the tab as the active one.
    #[must_use]
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// Shows the unsaved-changes dot.
    #[must_use]
    pub fn dirty(mut self, dirty: bool) -> Self {
        self.dirty = dirty;
        self
    }
}

interactive_builders!(EditorTab);

impl RenderOnce for EditorTab {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let size = &theme.metrics.size;
        let space = &theme.metrics.space;
        let background = if self.active {
            StateColors::fixed(colors.bg_tab_active.into())
        } else {
            StateColors {
                normal: transparent_black(),
                hover: colors.bg_tab_hover.into(),
                pressed: colors.bg_tab_hover.into(),
            }
        };
        let title_color: Hsla = if self.active {
            colors.text_primary.into()
        } else {
            colors.text_subtle.into()
        };
        let marker = self.active.then(|| {
            div()
                .absolute()
                .left_0()
                .w(px(size.tab_marker_width))
                .h(px(size.tab_marker_height))
                .rounded(px(size.tab_marker_width / 2.0))
                .bg(colors.accent)
        });
        let dirty = self.dirty.then(|| {
            div()
                .size(px(size.dirty_dot_size))
                .rounded_full()
                .bg(colors.marker_dirty)
        });
        let close = Icon::new(IconName::X)
            .size(size.icon_tiny)
            .color(colors.icon);
        let (width, height, radius) = (size.tab_width, size.tab_height, theme.metrics.radius.md);
        let (padding, gap) = (space.lg, space.md);

        self.interactivity
            .root(background, window, cx)
            .relative()
            .w(px(width))
            .h(px(height))
            .pl(px(padding))
            .pr(px(gap))
            .gap(px(gap))
            .rounded(px(radius))
            .children(marker)
            .children(self.leading)
            .child(
                div()
                    .flex_1()
                    .truncate()
                    .text_color(title_color)
                    .child(self.title),
            )
            .children(dirty)
            .child(close)
    }
}

/// Look of an underlined tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnderlineStyle {
    /// Uppercase, small: bottom panel tabs (TERMINAL, PROBLEMS).
    Panel,
    /// Regular case, accent text when active: tabs inside a page or sidebar.
    Segmented,
}

/// Shared implementation of [`PanelTab`] and [`SegmentedTab`].
struct UnderlineTab {
    interactivity: Interactivity,
    label: SharedString,
    count: Option<SharedString>,
    active: bool,
    style: UnderlineStyle,
}

impl UnderlineTab {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let colors = &theme.colors;
        let metrics = &theme.metrics;
        let state = self.interactivity.static_state();
        let hovered = matches!(state, InteractionState::Hover | InteractionState::Pressed);
        let (active_color, idle_color, font_size): (Hsla, Hsla, f32) = match self.style {
            UnderlineStyle::Panel => (
                colors.text_primary.into(),
                colors.text_muted.into(),
                metrics.font_size.sm,
            ),
            UnderlineStyle::Segmented => (
                colors.text_accent.into(),
                colors.text_subtle.into(),
                metrics.font_size.md,
            ),
        };
        let text_color = if self.active {
            active_color
        } else if hovered {
            colors.text_secondary.into()
        } else {
            idle_color
        };
        let hover_color: Hsla = colors.text_secondary.into();
        let underline_color: Hsla = if self.active {
            colors.accent.into()
        } else {
            transparent_black()
        };
        let label = match self.style {
            UnderlineStyle::Panel => self.label.to_uppercase().into(),
            UnderlineStyle::Segmented => self.label,
        };
        let (height, underline, gap, radius) = (
            metrics.size.tab_height,
            metrics.size.panel_tab_underline,
            metrics.space.sm,
            metrics.radius.xs,
        );
        let count = self.count.map(CountBadge::new);
        let mut interactivity = self.interactivity;
        if !self.active {
            interactivity.hover_text = Some(hover_color);
        }

        let content = div()
            .flex()
            .items_center()
            .flex_1()
            .gap(px(gap))
            .child(label)
            .children(count);
        interactivity
            .root(StateColors::fixed(transparent_black()), window, cx)
            .flex_col()
            .items_start()
            .h(px(height))
            .rounded(px(radius))
            .text_size(px(font_size))
            .text_color(text_color)
            .child(content)
            .child(
                div()
                    .w_full()
                    .h(px(underline))
                    .rounded(px(underline / 2.0))
                    .bg(underline_color),
            )
    }
}

macro_rules! underline_tab {
    ($(#[$meta:meta])* $name:ident, $style:expr) => {
        $(#[$meta])*
        #[derive(IntoElement)]
        pub struct $name {
            interactivity: Interactivity,
            label: SharedString,
            count: Option<SharedString>,
            active: bool,
        }

        impl $name {
            /// Creates an inactive tab.
            pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
                Self {
                    interactivity: Interactivity::new(id),
                    label: label.into(),
                    count: None,
                    active: false,
                }
            }

            /// Marks the tab as the active one.
            #[must_use]
            pub fn active(mut self, active: bool) -> Self {
                self.active = active;
                self
            }

            /// Shows a count badge after the label.
            #[must_use]
            pub fn count(mut self, count: impl Into<SharedString>) -> Self {
                self.count = Some(count.into());
                self
            }
        }

        interactive_builders!($name);

        impl RenderOnce for $name {
            fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
                UnderlineTab {
                    interactivity: self.interactivity,
                    label: self.label,
                    count: self.count,
                    active: self.active,
                    style: $style,
                }
                .render(window, cx)
            }
        }
    };
}

underline_tab!(
    /// A tab of the bottom panel (TERMINAL, PROBLEMS, OUTPUT).
    PanelTab,
    UnderlineStyle::Panel
);

underline_tab!(
    /// A tab inside a page or sidebar (Installed / Marketplace, Overview).
    SegmentedTab,
    UnderlineStyle::Segmented
);
