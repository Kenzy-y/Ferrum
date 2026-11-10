//! Densities and their metric tokens.

use crate::{FontSizeTokens, RadiusTokens, SizeTokens, SpaceTokens, ThemeError};

/// How tightly the interface is laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Density {
    /// The density of the mockups.
    #[default]
    Comfortable,
    /// Smaller rows, paddings and bars.
    Compact,
}

impl Density {
    /// Every density.
    pub const ALL: [Self; 2] = [Self::Comfortable, Self::Compact];

    fn id(self) -> &'static str {
        match self {
            Self::Comfortable => "comfortable",
            Self::Compact => "compact",
        }
    }

    fn source(self) -> &'static str {
        match self {
            Self::Comfortable => include_str!("../density/comfortable.toml"),
            Self::Compact => include_str!("../density/compact.toml"),
        }
    }
}

/// Sizes, spacing, radii and font sizes, in logical pixels.
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metrics {
    /// Fixed element sizes.
    pub size: SizeTokens,
    /// Spacing scale.
    pub space: SpaceTokens,
    /// Corner radii.
    pub radius: RadiusTokens,
    /// Font sizes.
    pub font_size: FontSizeTokens,
}

impl Metrics {
    /// Loads the metrics of `density`.
    ///
    /// # Errors
    ///
    /// Fails only if a built-in density file is broken (caught by tests).
    pub fn load(density: Density) -> Result<Self, ThemeError> {
        toml::from_str(density.source()).map_err(|error| ThemeError::Invalid {
            kind: "density",
            name: density.id().to_owned(),
            message: error.to_string(),
        })
    }

    /// Every metric multiplied by `factor` (the interface zoom).
    #[must_use]
    pub fn scaled(&self, factor: f32) -> Self {
        // Rebuilt from the token keys, so a new token is scaled without
        // touching this code.
        let mut file = toml::Table::new();
        let groups = [
            ("size", self.size.entries()),
            ("space", self.space.entries()),
            ("radius", self.radius.entries()),
            ("font_size", self.font_size.entries()),
        ];
        for (group, entries) in groups {
            let table: toml::Table = entries
                .into_iter()
                .map(|(key, value)| {
                    (
                        key.to_owned(),
                        toml::Value::Float(f64::from(value * factor)),
                    )
                })
                .collect();
            file.insert(group.to_owned(), toml::Value::Table(table));
        }
        file.try_into().unwrap_or_else(|error| {
            tracing::error!("cannot scale metrics: {error}");
            self.clone()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_is_never_larger() {
        let comfortable = Metrics::load(Density::Comfortable).unwrap();
        let compact = Metrics::load(Density::Compact).unwrap();
        let pairs = comfortable
            .size
            .entries()
            .into_iter()
            .zip(compact.size.entries());
        for ((key, large), (_, small)) in pairs {
            assert!(small <= large, "{key}: {small} > {large}");
        }
        assert_eq!(
            comfortable.font_size, compact.font_size,
            "density does not change font sizes"
        );
    }

    #[test]
    fn scaling_multiplies_every_token() {
        let metrics = Metrics::load(Density::Comfortable).unwrap();
        let double = metrics.scaled(2.0);
        assert!((double.size.titlebar_height - metrics.size.titlebar_height * 2.0).abs() < 1e-4);
        assert!((double.font_size.base - metrics.font_size.base * 2.0).abs() < 1e-4);
        assert_eq!(metrics.scaled(1.0), metrics);
    }
}
