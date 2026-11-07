//! List rows and activity bar buttons.

use std::ops::Range;

use gpui::{
    AnyElement, App, ElementId, HighlightStyle, Hsla, IntoElement, ParentElement as _, RenderOnce,
    SharedString, Styled as _, StyledText, Window, div, px, transparent_black,
};
use theme::ActiveTheme as _;

use crate::interaction::{Interactivity, StateColors, interactive_builders};
use crate::{Icon, IconName, InteractionState};

/// A row of a list or tree (files, search results, changes, call stack).
#[derive(IntoElement)]
pub struct ListRow {
    interactivity: Interactivity,
    label: SharedString,
    leading: Option<AnyElement>,
    trailing: Option<AnyElement>,
    depth: usize,
    expanded: Option<bool>,
    selected: bool,
    ignored: bool,
    highlights: Vec<Range<usize>>,
    disclosure_slot: bool,
    font: RowFont,
    height: Option<f32>,
}

/// The font of a row's label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowFont {
    /// The interface font (files, results).
    Interface,
    /// The code font in shorter rows (symbols).
    Code,
    /// The code font at the normal row height (search results).
    Mono,
}

impl ListRow {
    /// Creates a top-level, unselected row.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            interactivity: Interactivity::new(id),
            label: label.into(),
            leading: None,
            trailing: None,
            depth: 0,
            expanded: None,
            selected: false,
            ignored: false,
            highlights: Vec::new(),
            disclosure_slot: true,
            font: RowFont::Interface,
            height: None,
        }
    }

    /// Drops the space reserved for a disclosure chevron: for flat lists
    /// (palette, search results) rather than trees.
    #[must_use]
    pub fn flat(mut self) -> Self {
        self.disclosure_slot = false;
        self
    }

    /// Shows the label in the code font, in shorter rows: code symbols
    /// (Outline).
    #[must_use]
    pub fn code(mut self) -> Self {
        self.font = RowFont::Code;
        self
    }

    /// Shows the label in the code font without making the row shorter
    /// (lines of search results).
    #[must_use]
    pub fn mono(mut self) -> Self {
        self.font = RowFont::Mono;
        self
    }

    /// Overrides the row height (a size token of the list it belongs to).
    #[must_use]
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    /// Highlights byte ranges of the label (matched characters of a search).
    #[must_use]
    pub fn highlights(mut self, highlights: Vec<Range<usize>>) -> Self {
        self.highlights = highlights;
        self
    }

    /// Element before the label (file icon).
    #[must_use]
    pub fn leading(mut self, element: impl IntoElement) -> Self {
        self.leading = Some(element.into_any_element());
        self
    }

    /// Element at the end of the row (status letter, count).
    #[must_use]
    pub fn trailing(mut self, element: impl IntoElement) -> Self {
        self.trailing = Some(element.into_any_element());
        self
    }

    /// Tree depth; each level indents the row.
    #[must_use]
    pub fn depth(mut self, depth: usize) -> Self {
        self.depth = depth;
        self
    }

    /// Shows a disclosure chevron (`true` = expanded).
    #[must_use]
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = Some(expanded);
        self
    }

    /// Marks the row as selected.
    #[must_use]
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Dims the label (ignored files).
    #[must_use]
    pub fn ignored(mut self, ignored: bool) -> Self {
        self.ignored = ignored;
        self
    }
}

interactive_builders!(ListRow);

/// Depth as a multiplier of the indent token.
fn depth_factor(depth: usize) -> f32 {
    u16::try_from(depth).map_or(f32::from(u16::MAX), f32::from)
}

impl RenderOnce for ListRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let metrics = &theme.metrics;
        let background = if self.selected {
            StateColors::fixed(colors.bg_list_selected.into())
        } else {
            StateColors {
                normal: transparent_black(),
                hover: colors.bg_list_hover.into(),
                pressed: colors.bg_list_pressed.into(),
            }
        };
        let label_color: Hsla = if self.selected {
            colors.text_selected.into()
        } else if self.ignored {
            colors.text_ignored.into()
        } else if self.font != RowFont::Interface {
            colors.text_subtle.into()
        } else {
            colors.text_secondary.into()
        };
        let marker = self.selected.then(|| {
            div()
                .absolute()
                .left_0()
                .w(px(metrics.size.tab_marker_width))
                .h(px(metrics.size.tab_marker_height))
                .bg(colors.accent)
        });
        let icon_size = metrics.size.icon_small;
        let chevron = self.expanded.map(|expanded| {
            let name = if expanded {
                IconName::ChevronDown
            } else {
                IconName::ChevronRight
            };
            Icon::new(name).color(colors.icon)
        });
        // Rows without a chevron keep its space so labels stay aligned.
        let chevron_slot = (self.disclosure_slot || chevron.is_some())
            .then(|| div().flex().flex_none().w(px(icon_size)).children(chevron));
        let highlight = HighlightStyle {
            color: Some(colors.text_accent.into()),
            ..HighlightStyle::default()
        };
        let label = StyledText::new(self.label)
            .with_highlights(self.highlights.into_iter().map(|range| (range, highlight)));
        let indent = metrics.space.md + metrics.size.tree_indent * depth_factor(self.depth);
        let code = self.font != RowFont::Interface;
        let height = self.height.unwrap_or(if self.font == RowFont::Code {
            metrics.size.outline_row_height
        } else {
            metrics.size.list_row_height
        });
        let mono = gpui::SharedString::from(theme.fonts.mono.clone());
        let code_size = metrics.font_size.md;
        let (radius, gap, right) = (metrics.radius.md, metrics.space.sm, metrics.space.xl);

        self.interactivity
            .root(background, window, cx)
            .relative()
            .w_full()
            .h(px(height))
            .pl(px(indent))
            .pr(px(right))
            .gap(px(gap))
            .rounded(px(radius))
            .children(marker)
            .children(chevron_slot)
            .children(self.leading)
            .child({
                let text = div().flex_1().truncate().text_color(label_color);
                let text = if code {
                    text.font_family(mono).text_size(px(code_size))
                } else {
                    text
                };
                text.child(label)
            })
            .children(self.trailing)
    }
}

/// A button of the activity bar (Explorer, Search, Source Control, ...).
#[derive(IntoElement)]
pub struct ActivityButton {
    interactivity: Interactivity,
    icon: IconName,
    active: bool,
}

impl ActivityButton {
    /// Creates an inactive activity button.
    pub fn new(id: impl Into<ElementId>, icon: IconName) -> Self {
        Self {
            interactivity: Interactivity::new(id),
            icon,
            active: false,
        }
    }

    /// Marks the button as the current view.
    #[must_use]
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }
}

interactive_builders!(ActivityButton);

impl RenderOnce for ActivityButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let metrics = &theme.metrics;
        let background = if self.active {
            StateColors::fixed(colors.bg_activity_active.into())
        } else {
            StateColors {
                normal: transparent_black(),
                hover: colors.bg_activity_hover.into(),
                pressed: colors.bg_activity_active.into(),
            }
        };
        let state = self.interactivity.static_state();
        let icon_color: Hsla = if self.active {
            colors.icon_active.into()
        } else if matches!(state, InteractionState::Hover | InteractionState::Pressed) {
            colors.icon_strong.into()
        } else {
            colors.icon.into()
        };
        let icon = Icon::new(self.icon)
            .size(metrics.size.icon_medium)
            .color(icon_color);
        let (width, height, radius) = (
            metrics.size.activity_button_width,
            metrics.size.activity_button_height,
            metrics.radius.xl,
        );
        self.interactivity
            .root(background, window, cx)
            .w(px(width))
            .h(px(height))
            .justify_center()
            .rounded(px(radius))
            .child(icon)
    }
}
