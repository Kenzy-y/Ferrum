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
}
