//! Colors as written in theme files.

use std::fmt;

use serde::de::{self, Deserializer};

/// An sRGB color with alpha, written in themes as `#RRGGBB` or `#RRGGBBAA`.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color(u32);

impl Color {
    /// Creates a color from `0xRRGGBBAA`.
    pub const fn from_rgba(rgba: u32) -> Self {
        Self(rgba)
    }

    /// The color as `0xRRGGBBAA`.
    pub const fn rgba(self) -> u32 {
        self.0
    }

    /// Parses `#RRGGBB` or `#RRGGBBAA` (case-insensitive).
    pub fn parse(text: &str) -> Option<Self> {
        let hex = text.strip_prefix('#')?;
        if !hex.chars().all(|character| character.is_ascii_hexdigit()) {
            return None;
        }
        let value = u32::from_str_radix(hex, 16).ok()?;
        match hex.len() {
            6 => Some(Self((value << 8) | 0xFF)),
            8 => Some(Self(value)),
            _ => None,
        }
    }

    /// The same color with its alpha multiplied by `opacity` (0.0–1.0).
    pub fn with_opacity(self, opacity: f32) -> gpui::Hsla {
        let mut color = gpui::Hsla::from(self);
        color.a *= opacity.clamp(0.0, 1.0);
        color
    }
}

impl From<Color> for gpui::Hsla {
    fn from(color: Color) -> Self {
        gpui::rgba(color.0).into()
    }
}

impl fmt::Debug for Color {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl fmt::Display for Color {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let alpha = self.0 & 0xFF;
        if alpha == 0xFF {
            write!(formatter, "#{:06X}", self.0 >> 8)
        } else {
            write!(formatter, "#{:08X}", self.0)
        }
    }
}

impl<'de> serde::Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).ok_or_else(|| {
            de::Error::custom(format!(
                "`{text}` is not a color: expected #RRGGBB or #RRGGBBAA"
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_colors() {
        assert_eq!(Color::parse("#D69272"), Some(Color::from_rgba(0xD692_72FF)));
        assert_eq!(
            Color::parse("#d6927280"),
            Some(Color::from_rgba(0xD692_7280))
        );
        for invalid in ["D69272", "#D6927", "#GG9272", "#+69272", ""] {
            assert_eq!(Color::parse(invalid), None, "{invalid}");
        }
    }

    #[test]
    fn displays_as_hex() {
        assert_eq!(Color::from_rgba(0xD692_72FF).to_string(), "#D69272");
        assert_eq!(Color::from_rgba(0xD692_7280).to_string(), "#D6927280");
    }

    #[test]
    fn opacity_scales_alpha() {
        let color = Color::from_rgba(0xFFFF_FFFF).with_opacity(0.5);
        assert!((color.a - 0.5).abs() < f32::EPSILON);
    }
}
