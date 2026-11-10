//! Settings of the `[appearance]` namespace.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// How the interface looks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct AppearanceSettings {
    /// Size of the whole interface: 1.0 is normal, from 0.5 to 2.0.
    /// Ctrl + and Ctrl − change it in steps of 0.1, Ctrl 0 resets it.
    pub zoom: f32,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            zoom: crate::zoom::DEFAULT,
        }
    }
}

impl settings::Settings for AppearanceSettings {
    const NAMESPACE: &'static str = "appearance";
}
