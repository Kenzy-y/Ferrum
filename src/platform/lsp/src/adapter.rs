//! What differs between servers: the program, its options, its own
//! notifications and commands. Each language pack implements
//! [`LspAdapter`] for its server; the client stays generic.

use std::path::Path;

use language::{LensAction, Location, ServerStatus};
use lsp_types::ClientCapabilities;
use process::CommandSpec;
use serde_json::Value;
use settings::SettingsStore;

/// Turns protocol locations of a server into the IDE's.
pub trait LocationConverter {
    /// `location` in the IDE's terms; `None` if it is not a file.
    fn location(&self, location: &lsp_types::Location) -> Option<Location>;
}

/// The server-specific part of a language service.
pub trait LspAdapter: Send + Sync + 'static {
    /// Name shown to the user and used in logs (`rust-analyzer`).
    fn name(&self) -> &str;

    /// How to start the server for the workspace at `root`.
    fn command(&self, root: &Path) -> CommandSpec;

    /// `initializationOptions` of the `initialize` request.
    fn initialization_options(&self) -> Option<Value> {
        None
    }

    /// The answer to `workspace/configuration` for `section` (`None`: all
    /// of it).
    fn configuration(&self, section: Option<&str>) -> Value {
        let _ = section;
        Value::Null
    }

    /// Adds what the server needs beyond the standard capabilities
    /// (`experimental` ones).
    fn extend_capabilities(&self, capabilities: &mut ClientCapabilities) {
        let _ = capabilities;
    }

    /// A notification the generic client does not know. Returns the
    /// server's health if the notification tells it.
    fn notification(&self, method: &str, params: &Value) -> Option<ServerStatus> {
        let _ = (method, params);
        None
    }

    /// What clicking a code lens with `command` does.
    fn lens_action(
        &self,
        command: &lsp_types::Command,
        locations: &dyn LocationConverter,
    ) -> LensAction {
        let _ = (command, locations);
        LensAction::Unsupported
    }

    /// Settings changed. Returns `true` if the server's configuration
    /// changed and it should be told (`workspace/didChangeConfiguration`).
    fn configure(&self, settings: &SettingsStore) -> bool {
        let _ = settings;
        false
    }
}
