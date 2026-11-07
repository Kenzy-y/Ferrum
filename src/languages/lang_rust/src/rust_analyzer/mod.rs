//! rust-analyzer: how it is started, configured and understood. Everything
//! specific to it lives here; the LSP client is generic.

mod config;

use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use ::settings::SettingsStore;
use language::{LanguageService, LensAction, ServerContext, ServerFactory, ServerStatus};
use lsp::{LocationConverter, LspAdapter};
use lsp_types::ClientCapabilities;
use process::CommandSpec;
use serde_json::{Value, json};

pub use self::config::RustAnalyzerSettings;
use crate::ID;

/// Name of the server, shown in the status bar.
const NAME: &str = "rust-analyzer";

/// The section rust-analyzer asks `workspace/configuration` for.
const SECTION: &str = "rust-analyzer";

/// The code lens command the IDE carries out (see `lens_action`).
/// rust-analyzer gives lenses only the commands a client lists in
/// `experimental.commands`; run and debug wait for the terminal and the
/// debugger.
const SHOW_REFERENCES: &str = "rust-analyzer.showReferences";

/// Starts rust-analyzer for a workspace.
pub(crate) struct RustAnalyzerFactory;

impl ServerFactory for RustAnalyzerFactory {
    fn id(&self) -> &'static str {
        NAME
    }

    fn start(&self, context: ServerContext<'_>) -> Arc<dyn LanguageService> {
        let settings = context.settings.get::<RustAnalyzerSettings>(Some(ID));
        lsp::start(Arc::new(RustAnalyzerAdapter::new(settings)), context)
    }
}

/// The rust-analyzer specifics of the LSP client.
pub(crate) struct RustAnalyzerAdapter {
    settings: Mutex<RustAnalyzerSettings>,
}

impl RustAnalyzerAdapter {
    pub(crate) fn new(settings: RustAnalyzerSettings) -> Self {
        Self {
            settings: Mutex::new(settings),
        }
    }

    fn settings(&self) -> RustAnalyzerSettings {
        self.settings
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn config(&self) -> Value {
        Value::Object(self.settings().config)
    }
}

impl LspAdapter for RustAnalyzerAdapter {
    fn name(&self) -> &'static str {
        NAME
    }

    fn command(&self, root: &Path) -> CommandSpec {
        let settings = self.settings();
        CommandSpec::new(&settings.path)
            .args(&settings.args)
            .current_dir(root)
    }

    fn initialization_options(&self) -> Option<Value> {
        Some(self.config())
    }

    fn configuration(&self, section: Option<&str>) -> Value {
        match section {
            Some(SECTION) => self.config(),
            None => json!({ SECTION: self.config() }),
            Some(_) => Value::Null,
        }
    }

    fn extend_capabilities(&self, capabilities: &mut ClientCapabilities) {
        let experimental = capabilities.experimental.get_or_insert_with(|| json!({}));
        if let Value::Object(experimental) = experimental {
            // `experimental/serverStatus`: whether the project loaded.
            experimental.insert("serverStatusNotification".to_owned(), Value::Bool(true));
            experimental.insert(
                "commands".to_owned(),
                json!({ "commands": [SHOW_REFERENCES] }),
            );
        }
    }

    fn notification(&self, method: &str, params: &Value) -> Option<ServerStatus> {
        (method == "experimental/serverStatus").then(|| server_status(params))
    }

    fn failure_hint(&self, message: &str) -> Option<String> {
        failure_hint(message).map(str::to_owned)
    }

    fn lens_action(
        &self,
        command: &lsp_types::Command,
        locations: &dyn LocationConverter,
    ) -> LensAction {
        // `rust-analyzer.showReferences`: [uri, position, locations] for
        // the implementations and references lenses. Run and debug lenses
        // wait for the terminal and the debugger (ROADMAP).
        if command.command != SHOW_REFERENCES {
            return LensAction::Unsupported;
        }
        let Some(found) = command
            .arguments
            .as_ref()
            .and_then(|arguments| arguments.get(2))
            .and_then(|found| {
                serde_json::from_value::<Vec<lsp_types::Location>>(found.clone()).ok()
            })
        else {
            return LensAction::Unsupported;
        };
        LensAction::ShowLocations(
            found
                .iter()
                .filter_map(|location| locations.location(location))
                .collect(),
        )
    }

    fn configure(&self, settings: &SettingsStore) -> bool {
        let new = settings.get::<RustAnalyzerSettings>(Some(ID));
        let mut current = self.settings.lock().unwrap_or_else(PoisonError::into_inner);
        let changed = current.config != new.config;
        // The path and arguments apply from the next start.
        *current = new;
        changed
    }
}

/// `experimental/serverStatus`: `health` is `ok`, `warning` or `error`,
/// `quiescent` is false while the project loads, `message` explains.
/// The rustup proxy runs, but the toolchain of the folder has no
/// rust-analyzer: `rustup component add rust-analyzer` fixes it.
pub(crate) const HINT_NOT_INSTALLED: &str = "lang-rust-analyzer-not-installed";

/// Recognizes a failure from the server's last error output.
fn failure_hint(message: &str) -> Option<&'static str> {
    // rustup: "error: Unknown binary 'rust-analyzer.exe' in official
    // toolchain 'stable-…'" or "'rust-analyzer' is not installed for the
    // toolchain '…'".
    let rustup = message.contains("rust-analyzer")
        && (message.contains("Unknown binary") || message.contains("is not installed"));
    rustup.then_some(HINT_NOT_INSTALLED)
}

fn server_status(params: &Value) -> ServerStatus {
    let message = params
        .get("message")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let quiescent = params
        .get("quiescent")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    match params.get("health").and_then(Value::as_str) {
        Some("warning" | "error") => ServerStatus::Warning {
            message: message.unwrap_or_default(),
        },
        _ if !quiescent => ServerStatus::Indexing {
            message,
            percentage: None,
        },
        _ => ServerStatus::Ready,
    }
}

#[cfg(test)]
mod tests;
