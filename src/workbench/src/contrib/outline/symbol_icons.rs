//! Glyphs of outline symbol kinds (Outline, breadcrumbs): a monospace
//! character in a theme color, as in the editor mockup (`S` for a struct,
//! `ƒ` for a function).

use gpui::{AnyElement, App, IntoElement as _};
use syntax::SymbolKind;
use theme::{ActiveTheme as _, Color, Theme};
use ui::Label;

/// The glyph of `kind`. These are icons, not words, so they are not
/// localized.
fn glyph(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Module => "M",
        SymbolKind::Struct => "S",
        SymbolKind::Enum => "E",
        SymbolKind::Union => "U",
        SymbolKind::Trait => "T",
        SymbolKind::Impl => "I",
        SymbolKind::Function => "ƒ",
        SymbolKind::Constant => "C",
        SymbolKind::Static => "V",
        SymbolKind::TypeAlias => "A",
        SymbolKind::Macro => "!",
        SymbolKind::Table => "[",
        SymbolKind::Key => "=",
    }
}

fn color(kind: SymbolKind, theme: &Theme) -> Color {
    let syntax = &theme.syntax;
    match kind {
        SymbolKind::Function => theme.colors.icon_symbol_function,
        SymbolKind::Module => syntax.keyword,
        SymbolKind::Struct
        | SymbolKind::Enum
        | SymbolKind::Union
        | SymbolKind::Trait
        | SymbolKind::Impl
        | SymbolKind::TypeAlias
        | SymbolKind::Table => syntax.type_name,
        SymbolKind::Constant | SymbolKind::Static => syntax.constant,
        SymbolKind::Macro => syntax.macro_name,
        SymbolKind::Key => syntax.attribute,
    }
}

/// Draws the glyph of `kind` at font size `size`.
pub(crate) fn render(kind: SymbolKind, size: f32, cx: &App) -> AnyElement {
    let theme = cx.theme();
    Label::new(glyph(kind))
        .mono()
        .size(size)
        .color(color(kind, theme))
        .into_any_element()
}
