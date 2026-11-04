//! Settings of the `[syntax]` namespace.

use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Limits of syntax highlighting and the outline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct SyntaxSettings {
    /// Files larger than this, in megabytes, open without highlighting and
    /// outline.
    pub max_file_size_mb: u64,
    /// Longest a parse may take, in milliseconds, before the file is shown
    /// without highlighting.
    pub parse_timeout_ms: u64,
    /// Lines longer than this, in bytes, are drawn without highlighting
    /// (minified files): finding the visible part of such a line in the
    /// tree is too slow for a frame.
    pub max_line_length: usize,
}

impl Default for SyntaxSettings {
    fn default() -> Self {
        Self {
            max_file_size_mb: 16,
            parse_timeout_ms: 5000,
            max_line_length: 20_000,
        }
    }
}

impl settings::Settings for SyntaxSettings {
    const NAMESPACE: &'static str = "syntax";
}

impl SyntaxSettings {
    /// The size limit in bytes.
    pub fn max_file_bytes(&self) -> usize {
        usize::try_from(self.max_file_size_mb.saturating_mul(1024 * 1024)).unwrap_or(usize::MAX)
    }

    /// The parse timeout.
    pub fn parse_timeout(&self) -> Duration {
        Duration::from_millis(self.parse_timeout_ms.max(1))
    }
}
