//! Design tokens: colors, syntax colors, sizes, spacing, radii, fonts.
//!
//! A [`Theme`] combines a color theme (`themes/*.toml`) with a [`Density`]
//! (`density/*.toml`). Components never hardcode a color or a size: they
//! read tokens from the active theme via [`ActiveTheme`]. Themes are data,
//! so new ones (light, high contrast, imported) are added as files.

mod color;
mod colors;
mod fonts;
mod macros;
mod metrics;
mod metrics_tokens;

use std::sync::Arc;

use serde::Deserialize;

pub use color::Color;
pub use colors::{ColorTokens, SyntaxTokens};
pub use fonts::{EMBEDDED_FONTS, register_fonts};
pub use metrics::{Density, Metrics};
pub use metrics_tokens::{FontSizeTokens, RadiusTokens, SizeTokens, SpaceTokens};

/// Built-in color themes: `(id, TOML source)`.
const BUILT_IN_THEMES: &[(&str, &str)] = &[("dark", include_str!("../themes/dark.toml"))];

/// Id of the theme used when nothing else is configured.
pub const DEFAULT_THEME: &str = "dark";

/// Errors while loading themes or fonts.
#[derive(Debug, thiserror::Error)]
pub enum ThemeError {
    /// No built-in theme has this id.
    #[error("unknown theme `{0}`")]
    UnknownTheme(String),
    /// A theme or density file is invalid.
    #[error("invalid {kind} `{name}`: {message}")]
    Invalid {
        /// `theme` or `density`.
        kind: &'static str,
        /// Id of the file.
        name: String,
        /// What is wrong.
        message: String,
    },
    /// The platform text system rejected an embedded font.
    #[error("cannot register embedded fonts: {0}")]
    Fonts(String),
}

/// Whether a theme is light or dark; lets the platform match window chrome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    /// Light background, dark text.
    Light,
    /// Dark background, light text.
    Dark,
}

/// Font families.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fonts {
    /// Interface text.
    pub ui: String,
    /// Code, terminal, keyboard shortcuts, paths.
    pub mono: String,
}

/// Opacity tokens.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpacityTokens {
    /// Applied to disabled controls.
    pub disabled: f32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeFile {
    name: String,
    appearance: Appearance,
    fonts: Fonts,
    colors: ColorTokens,
    syntax: SyntaxTokens,
    opacity: OpacityTokens,
}

/// All tokens of the active look.
#[derive(Debug, Clone)]
pub struct Theme {
    /// Display name of the color theme.
    pub name: String,
    /// Light or dark.
    pub appearance: Appearance,
    /// Font families.
    pub fonts: Fonts,
    /// Interface colors.
    pub colors: ColorTokens,
    /// Syntax colors.
    pub syntax: SyntaxTokens,
    /// Opacities.
    pub opacity: OpacityTokens,
    /// Density.
    pub density: Density,
    /// Sizes, spacing, radii and font sizes of the density.
    pub metrics: Metrics,
}

impl Theme {
    /// Parses a color theme from TOML and combines it with `density`.
    ///
    /// # Errors
    ///
    /// Fails if the text is not a complete, valid theme.
    pub fn parse(name: &str, source: &str, density: Density) -> Result<Self, ThemeError> {
        let file: ThemeFile = toml::from_str(source).map_err(|error| ThemeError::Invalid {
            kind: "theme",
            name: name.to_owned(),
            message: error.to_string(),
        })?;
        Ok(Self {
            name: file.name,
            appearance: file.appearance,
            fonts: file.fonts,
            colors: file.colors,
            syntax: file.syntax,
            opacity: file.opacity,
            density,
            metrics: Metrics::load(density)?,
        })
    }

    /// A built-in theme by id (see [`DEFAULT_THEME`]).
    ///
    /// # Errors
    ///
    /// Fails if the id is unknown or the built-in file is broken (caught by
    /// tests).
    pub fn built_in(id: &str, density: Density) -> Result<Self, ThemeError> {
        let (_, source) = BUILT_IN_THEMES
            .iter()
            .find(|(theme_id, _)| *theme_id == id)
            .ok_or_else(|| ThemeError::UnknownTheme(id.to_owned()))?;
        Self::parse(id, source, density)
    }

    /// Ids of the built-in themes.
    pub fn built_in_ids() -> impl Iterator<Item = &'static str> {
        BUILT_IN_THEMES.iter().map(|(id, _)| *id)
    }
}

/// The theme stored in the application, read through [`ActiveTheme`].
struct GlobalTheme(Arc<Theme>);

impl gpui::Global for GlobalTheme {}

/// Makes `theme` the active theme and redraws all windows.
pub fn set_theme(theme: Theme, cx: &mut gpui::App) {
    cx.set_global(GlobalTheme(Arc::new(theme)));
    cx.refresh_windows();
}

/// Access to the active theme.
pub trait ActiveTheme {
    /// The active theme.
    ///
    /// # Panics
    ///
    /// If no theme was set with [`set_theme`] (a startup wiring bug).
    fn theme(&self) -> &Theme;
}

impl ActiveTheme for gpui::App {
    fn theme(&self) -> &Theme {
        &self.global::<GlobalTheme>().0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_themes_load_with_every_density() {
        for id in Theme::built_in_ids() {
            for density in Density::ALL {
                Theme::built_in(id, density).unwrap();
            }
        }
    }

    #[test]
    fn dark_theme_matches_the_mockups() {
        let theme = Theme::built_in(DEFAULT_THEME, Density::Comfortable).unwrap();
        assert_eq!(theme.appearance, Appearance::Dark);
        assert_eq!(theme.colors.bg_app.to_string(), "#17191C");
        assert_eq!(theme.colors.accent.to_string(), "#D69272");
        assert_eq!(theme.fonts.ui, "Inter");
        assert_eq!(theme.fonts.mono, "JetBrains Mono");
    }

    #[test]
    fn number_color_differs_from_keywords() {
        let theme = Theme::built_in(DEFAULT_THEME, Density::Comfortable).unwrap();
        assert_ne!(theme.syntax.number, theme.syntax.keyword);
    }

    #[test]
    fn incomplete_theme_is_rejected() {
        let source = BUILT_IN_THEMES[0].1.replace("\"accent\" = \"#D69272\"", "");
        let error = Theme::parse("broken", &source, Density::Comfortable).unwrap_err();
        assert!(error.to_string().contains("accent"), "{error}");
    }

    #[test]
    fn unknown_token_is_rejected() {
        let source = BUILT_IN_THEMES[0]
            .1
            .replace("[syntax]", "[syntax]\nkeywrd = \"#000000\"");
        assert!(Theme::parse("typo", &source, Density::Comfortable).is_err());
    }
}
