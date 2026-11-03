//! Panels, section headers and cards.

use gpui::{
    AnyElement, App, Div, FontWeight, IntoElement, ParentElement, RenderOnce, SharedString,
    StyleRefinement, Styled, Window, div, px,
};
use theme::ActiveTheme as _;

use crate::{Icon, IconName};

/// Background of a [`Panel`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PanelSurface {
    /// Sidebar (explorer, search, source control).
    #[default]
    Sidebar,
    /// Editor area and bottom panel.
    Editor,
}

/// A rounded, bordered container: the building block of the window layout.
#[derive(IntoElement)]
pub struct Panel {
    surface: PanelSurface,
    base: Div,
}

impl Panel {
    /// Creates an empty panel. Size and padding are set by the caller.
    pub fn new(surface: PanelSurface) -> Self {
        Self {
            surface,
            base: div().flex().flex_col(),
        }
    }
}

impl ParentElement for Panel {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.base.extend(elements);
    }
}

impl Styled for Panel {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl RenderOnce for Panel {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let background = match self.surface {
            PanelSurface::Sidebar => theme.colors.bg_sidebar,
            PanelSurface::Editor => theme.colors.bg_editor,
        };
        self.base
            .overflow_hidden()
            .bg(background)
            .border_1()
            .border_color(theme.colors.border_panel)
            .rounded(px(theme.metrics.radius.xl))
    }
}

/// Level of a [`SectionHeader`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum HeaderLevel {
    /// Title of a whole panel (EXPLORER).
    #[default]
    Panel,
    /// Collapsible group inside a panel (FILES, OUTLINE).
    Group,
}

/// An uppercase heading with optional actions on the right.
#[derive(IntoElement)]
pub struct SectionHeader {
    title: SharedString,
    level: HeaderLevel,
    expanded: Option<bool>,
    actions: Vec<AnyElement>,
}

impl SectionHeader {
    /// A panel title (EXPLORER).
    pub fn panel(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            level: HeaderLevel::Panel,
            expanded: None,
            actions: Vec::new(),
        }
    }

    /// A group title inside a panel (FILES); `expanded` shows a chevron.
    pub fn group(title: impl Into<SharedString>, expanded: Option<bool>) -> Self {
        Self {
            title: title.into(),
            level: HeaderLevel::Group,
            expanded,
            actions: Vec::new(),
        }
    }

    /// Adds an element on the right (usually an [`crate::IconButton`]).
    #[must_use]
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.actions.push(action.into_any_element());
        self
    }
}

impl RenderOnce for SectionHeader {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let metrics = &theme.metrics;
        let (font_size, color) = match self.level {
            HeaderLevel::Panel => (metrics.font_size.sm, colors.text_subtle),
            HeaderLevel::Group => (metrics.font_size.xs, colors.text_muted),
        };
        let chevron = self.expanded.map(|expanded| {
            let icon = if expanded {
                IconName::ChevronDown
            } else {
                IconName::ChevronRight
            };
            Icon::new(icon)
                .size(metrics.size.icon_tiny)
                .color(colors.icon)
        });
        div()
            .flex()
            .items_center()
            .w_full()
            .gap(px(metrics.space.xs))
            .px(px(metrics.size.section_header_inset))
            .text_size(px(font_size))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(color)
            .children(chevron)
            .child(div().flex_1().child(self.title.to_uppercase()))
            .children(self.actions)
    }
}

/// A raised block on a panel (recent projects, toolkit, code samples).
#[derive(IntoElement)]
pub struct Card {
    base: Div,
}

impl Card {
    /// Creates an empty card.
    pub fn new() -> Self {
        Self {
            base: div().flex().flex_col(),
        }
    }
}

impl Styled for Card {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl Default for Card {
    fn default() -> Self {
        Self::new()
    }
}

impl ParentElement for Card {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.base.extend(elements);
    }
}

impl RenderOnce for Card {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        self.base
            .gap(px(theme.metrics.space.sm))
            .p(px(theme.metrics.space.lg))
            .rounded(px(theme.metrics.radius.lg))
            .bg(theme.colors.bg_card)
    }
}
