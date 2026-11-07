//! Terminal colors from the theme.

use terminal::{Palette, Rgb};
use theme::{Color, Theme};

fn rgb(color: Color) -> Rgb {
    let [r, g, b, _] = color.rgba().to_be_bytes();
    Rgb::new(r, g, b)
}

/// The palette of `theme`: `[terminal]` colors on `bg.editor`.
pub(crate) fn palette(theme: &Theme) -> Palette {
    let tokens = &theme.terminal;
    Palette {
        foreground: rgb(tokens.foreground),
        background: rgb(theme.colors.bg_editor),
        cursor: rgb(tokens.cursor),
        ansi: tokens.ansi().map(rgb),
    }
}
