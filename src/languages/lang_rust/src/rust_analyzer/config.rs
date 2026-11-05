//! Settings of the `[rust_analyzer]` namespace.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// How rust-analyzer is started and configured.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct RustAnalyzerSettings {
    /// The rust-analyzer program: a name looked up in `PATH` (rustup
    /// installs it with `rustup component add rust-analyzer`) or a full
    /// path.
    pub path: String,
    /// Extra command-line arguments.
    pub args: Vec<String>,
    /// rust-analyzer's own configuration, as in its manual without the
    /// `rust-analyzer.` prefix, for example
    /// `check = { command = "clippy" }`.
    pub config: Map<String, Value>,
}

impl Default for RustAnalyzerSettings {
    fn default() -> Self {
        Self {
            path: "rust-analyzer".to_owned(),
            args: Vec::new(),
            config: Map::new(),
        }
    }
}

impl ::settings::Settings for RustAnalyzerSettings {
    const NAMESPACE: &'static str = "rust_analyzer";
}
