//! Settings of the `[search]` namespace.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Settings of project-wide search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct SearchSettings {
    /// How long typing in the search field pauses before the search runs,
    /// in milliseconds.
    pub debounce_ms: u64,
    /// The search stops after this many matches.
    pub max_results: usize,
    /// Files larger than this many bytes are not searched.
    pub max_file_bytes: u64,
    /// Longest preview of a matching line, in characters.
    pub preview_chars: usize,
}

impl Default for SearchSettings {
    fn default() -> Self {
        Self {
            debounce_ms: 150,
            max_results: 20_000,
            max_file_bytes: 5 * 1024 * 1024,
            preview_chars: 160,
        }
    }
}

impl settings::Settings for SearchSettings {
    const NAMESPACE: &'static str = "search";
}
