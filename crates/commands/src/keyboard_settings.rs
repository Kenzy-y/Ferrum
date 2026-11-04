use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Default of [`KeyboardSettings::chord_timeout_ms`].
const DEFAULT_CHORD_TIMEOUT_MS: u64 = 1000;

/// Settings of the `[keyboard]` namespace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct KeyboardSettings {
    /// How long to wait, in milliseconds, for the next key of a sequence
    /// when the keys pressed so far are also a complete shortcut
    /// (for example `Ctrl K` and `Ctrl K, Ctrl G`).
    pub chord_timeout_ms: u64,
}

impl Default for KeyboardSettings {
    fn default() -> Self {
        Self {
            chord_timeout_ms: DEFAULT_CHORD_TIMEOUT_MS,
        }
    }
}

impl settings::Settings for KeyboardSettings {
    const NAMESPACE: &'static str = "keyboard";
}
