//! Colors of a terminal: the theme's 16 ANSI colors, the default
//! foreground and background, and the standard 256-color cube.

use alacritty_terminal::vte::ansi::{self, NamedColor};

/// A color in 8-bit channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgb {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
}

impl Rgb {
    /// A color from its channels.
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// The color at `factor` of its brightness (dim text).
    #[must_use]
    pub fn scaled(self, factor: f32) -> Self {
        // Channels stay within 0..=255 for factors in 0..=1.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let scale = |channel: u8| (f32::from(channel) * factor.clamp(0.0, 1.0)).round() as u8;
        Self::new(scale(self.r), scale(self.g), scale(self.b))
    }
}

impl From<ansi::Rgb> for Rgb {
    fn from(color: ansi::Rgb) -> Self {
        Self::new(color.r, color.g, color.b)
    }
}

impl From<Rgb> for ansi::Rgb {
    fn from(color: Rgb) -> Self {
        Self {
            r: color.r,
            g: color.g,
            b: color.b,
        }
    }
}

/// Brightness of dim text relative to its color.
const DIM_FACTOR: f32 = 0.66;

/// Levels of the 6×6×6 color cube (indices 16..=231).
const CUBE_LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];

/// Colors given by the theme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette {
    /// Default text color.
    pub foreground: Rgb,
    /// Default background.
    pub background: Rgb,
    /// Cursor.
    pub cursor: Rgb,
    /// The 16 ANSI colors: black, red, green, yellow, blue, magenta, cyan,
    /// white, then their bright variants.
    pub ansi: [Rgb; 16],
}

impl Palette {
    /// The color of 256-color index `index`.
    pub fn indexed(&self, index: u8) -> Rgb {
        match index {
            0..=15 => self.ansi[usize::from(index)],
            16..=231 => {
                let cube = index - 16;
                Rgb::new(
                    CUBE_LEVELS[usize::from(cube / 36)],
                    CUBE_LEVELS[usize::from(cube / 6 % 6)],
                    CUBE_LEVELS[usize::from(cube % 6)],
                )
            }
            232..=255 => {
                let level = 8 + (index - 232) * 10;
                Rgb::new(level, level, level)
            }
        }
    }

    /// The theme's color of slot `index` of `alacritty_terminal`'s color
    /// table (0..=255 indexed colors, then the named special colors).
    pub(crate) fn slot(&self, index: usize) -> Rgb {
        if let Ok(index) = u8::try_from(index) {
            return self.indexed(index);
        }
        let named = |color: NamedColor| index == color as usize;
        if named(NamedColor::Background) {
            self.background
        } else if named(NamedColor::Cursor) {
            self.cursor
        } else if named(NamedColor::DimForeground) {
            self.foreground.scaled(DIM_FACTOR)
        } else if (NamedColor::DimBlack as usize..=NamedColor::DimWhite as usize).contains(&index) {
            self.ansi[index - NamedColor::DimBlack as usize].scaled(DIM_FACTOR)
        } else {
            // Foreground and bright foreground.
            self.foreground
        }
    }

    /// Resolves a cell color: `overrides` are the colors programs set with
    /// OSC 4/10/11, `bold` brightens the eight basic colors, `dim` darkens.
    pub(crate) fn resolve(
        &self,
        color: ansi::Color,
        overrides: &alacritty_terminal::term::color::Colors,
        bold: bool,
        dim: bool,
    ) -> Rgb {
        let slot = match color {
            ansi::Color::Spec(rgb) => {
                let rgb = Rgb::from(rgb);
                return if dim { rgb.scaled(DIM_FACTOR) } else { rgb };
            }
            ansi::Color::Indexed(index) => {
                let index = if bold && index < 8 { index + 8 } else { index };
                usize::from(index)
            }
            ansi::Color::Named(named) => {
                let named = if bold { named.to_bright() } else { named };
                let named = if dim { named.to_dim() } else { named };
                named as usize
            }
        };
        let rgb = overrides[slot].map_or_else(|| self.slot(slot), Rgb::from);
        match color {
            ansi::Color::Indexed(_) if dim => rgb.scaled(DIM_FACTOR),
            _ => rgb,
        }
    }
}

#[cfg(test)]
mod tests {
    use alacritty_terminal::term::color::Colors;

    use super::*;

    fn palette() -> Palette {
        let mut ansi = [Rgb::new(0, 0, 0); 16];
        for (index, color) in ansi.iter_mut().enumerate() {
            let index = u8::try_from(index).unwrap();
            *color = Rgb::new(index, index, index);
        }
        Palette {
            foreground: Rgb::new(200, 200, 200),
            background: Rgb::new(10, 10, 10),
            cursor: Rgb::new(255, 255, 255),
            ansi,
        }
    }

    #[test]
    fn indexed_colors_follow_the_xterm_table() {
        let palette = palette();
        assert_eq!(palette.indexed(3), Rgb::new(3, 3, 3));
        assert_eq!(palette.indexed(16), Rgb::new(0, 0, 0));
        assert_eq!(palette.indexed(196), Rgb::new(255, 0, 0));
        assert_eq!(palette.indexed(231), Rgb::new(255, 255, 255));
        assert_eq!(palette.indexed(232), Rgb::new(8, 8, 8));
        assert_eq!(palette.indexed(255), Rgb::new(238, 238, 238));
    }

    #[test]
    fn bold_brightens_basic_colors_and_overrides_win() {
        let palette = palette();
        let colors = Colors::default();
        let red = ansi::Color::Named(NamedColor::Red);
        assert_eq!(
            palette.resolve(red, &colors, false, false),
            Rgb::new(1, 1, 1)
        );
        assert_eq!(
            palette.resolve(red, &colors, true, false),
            Rgb::new(9, 9, 9)
        );
        let mut overrides = Colors::default();
        overrides[NamedColor::Red as usize] = Some(ansi::Rgb { r: 1, g: 2, b: 3 });
        assert_eq!(
            palette.resolve(red, &overrides, false, false),
            Rgb::new(1, 2, 3)
        );
        let foreground = ansi::Color::Named(NamedColor::Foreground);
        assert_eq!(
            palette.resolve(foreground, &colors, false, false),
            palette.foreground
        );
    }
}
