//! Settings of the `[lsp]` namespace.

use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// How the IDE talks to language servers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct LspSettings {
    /// Longest wait for an answer, in milliseconds, before a request is
    /// cancelled.
    pub request_timeout_ms: u64,
    /// Longest wait for a server to shut down, in milliseconds, before it
    /// is killed.
    pub shutdown_timeout_ms: u64,
    /// Diagnostics are shown once the server has been quiet this long, in
    /// milliseconds, so a burst of updates redraws once.
    pub diagnostics_debounce_ms: u64,
    /// How many times a server that exits on its own is started again.
    pub max_restarts: u32,
}

impl Default for LspSettings {
    fn default() -> Self {
        Self {
            request_timeout_ms: 10_000,
            shutdown_timeout_ms: 2_000,
            diagnostics_debounce_ms: 150,
            max_restarts: 3,
        }
    }
}

impl settings::Settings for LspSettings {
    const NAMESPACE: &'static str = "lsp";
}

impl LspSettings {
    /// The request timeout.
    pub fn request_timeout(&self) -> Duration {
        Duration::from_millis(self.request_timeout_ms.max(1))
    }

    /// The shutdown timeout.
    pub fn shutdown_timeout(&self) -> Duration {
        Duration::from_millis(self.shutdown_timeout_ms.max(1))
    }

    /// The diagnostics debounce.
    pub fn diagnostics_debounce(&self) -> Duration {
        Duration::from_millis(self.diagnostics_debounce_ms)
    }
}
