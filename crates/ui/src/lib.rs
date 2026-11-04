//! Design system: the components panels are built from.
//!
//! Components are stateless `RenderOnce` elements that read every color,
//! size and font from the active [`theme::Theme`]. Editing behaviour (text
//! inputs) comes from gpui-component, styled with the same tokens. Labels
//! are passed in already localized; this crate has no user-facing strings.
//!
//! Interactive components render the states hover, pressed, focused and
//! disabled. [`InteractionState`] lets the component gallery show a state
//! without the pointer or keyboard.

mod assets;
mod button;
mod icon;
mod input;
mod interaction;
mod kbd;
mod list;
mod notice;
mod panel;
mod tabs;
mod text;
mod tooltip;

pub use assets::Assets;
pub use button::{Button, ButtonSize, ButtonVariant, IconButton};
pub use icon::{Icon, IconName};
pub use input::{SearchField, TextArea, TextField};
pub use interaction::InteractionState;
pub use kbd::{CountBadge, Kbd, StatusKind, StatusLetter};
pub use list::{ActivityButton, ListRow};
pub use notice::NoticeBar;
pub use panel::{Card, Panel, PanelSurface, SectionHeader};
pub use tabs::{EditorTab, PanelTab, SegmentedTab};
pub use text::{FontRole, Label};

use theme::ActiveTheme as _;

/// Installs the theme into gpui-component so its built-in elements (text
/// inputs, scrollbars, popovers) use the same tokens. Call after
/// `gpui_component::init` and after every theme change.
pub fn sync_component_theme(cx: &mut gpui::App) {
    let theme = cx.theme().clone();
    let colors = &theme.colors;
    let component_theme = gpui_component::Theme::global_mut(cx);
    component_theme.font_family = theme.fonts.ui.clone().into();
    component_theme.mono_font_family = theme.fonts.mono.clone().into();
    component_theme.font_size = gpui::px(theme.metrics.font_size.base);
    component_theme.mono_font_size = gpui::px(theme.metrics.font_size.code_small);
    component_theme.radius = gpui::px(theme.metrics.radius.md);
    component_theme.radius_lg = gpui::px(theme.metrics.radius.xl);
    component_theme.shadow = false;

    let palette = &mut component_theme.colors;
    palette.background = colors.bg_editor.into();
    palette.foreground = colors.text_primary.into();
    palette.border = colors.border_input.into();
    palette.input = colors.border_input.into();
    palette.ring = colors.border_focus.into();
    palette.primary = colors.bg_button_primary.into();
    palette.primary_foreground = colors.text_on_accent.into();
    palette.muted = colors.bg_card.into();
    palette.muted_foreground = colors.text_muted.into();
    palette.accent = colors.bg_list_hover.into();
    palette.accent_foreground = colors.text_primary.into();
    palette.popover = colors.bg_overlay.into();
    palette.popover_foreground = colors.text_primary.into();
    palette.caret = colors.accent.into();
    palette.selection = colors.accent.with_opacity(SELECTION_OPACITY);
    palette.scrollbar_thumb = colors.bg_scrollbar_thumb.into();
}

/// Opacity of the accent color used for selected text in inputs.
const SELECTION_OPACITY: f32 = 0.3;
