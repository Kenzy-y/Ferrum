//! Keymaps: keybindings grouped by context, layered defaults → user.
//!
//! A `keymap.toml` file has one table per context. Keys are key sequences,
//! values are command ids; an empty string removes a binding inherited from
//! an earlier layer:
//!
//! ```toml
//! [global]
//! "mod-shift-p" = "workbench.showCommandPalette"
//!
//! [editor]
//! "mod-s" = "editor.save"
//! "ctrl-k ctrl-s" = ""        # unbind
//! ```

use std::fmt;

use crate::{CommandId, CommandRegistry, KeySequence, KeystrokeError, Platform};

/// Context in which bindings of the `global` table apply: everywhere.
pub const GLOBAL_CONTEXT: &str = "global";

/// Name of a keybinding context (`global`, `editor`, `terminal`,
/// `file_tree`, ...). Plugins may introduce their own contexts.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContextName(String);

impl ContextName {
    fn new(name: &str) -> Option<Self> {
        let valid = name.starts_with(|first: char| first.is_ascii_lowercase())
            && name.chars().all(|character| {
                character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
            });
        valid.then(|| Self(name.to_owned()))
    }

    /// The name as a string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ContextName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// One keybinding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    /// Keys to press.
    pub keys: KeySequence,
    /// Context in which the binding is active.
    pub context: ContextName,
    /// Command to run.
    pub command: CommandId,
}

/// Errors in a keymap file. The whole file is rejected so that a typo never
/// silently leaves the user with half of their bindings.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum KeymapError {
    /// Not valid TOML or not a table of tables of strings.
    #[error("invalid keymap: {0}")]
    Syntax(String),
    /// A context name does not follow the naming rules.
    #[error("invalid keymap context `{0}`: use lowercase ASCII letters, digits and `_`")]
    BadContext(String),
    /// A key sequence cannot be parsed.
    #[error("invalid keys in context `{context}`: {source}")]
    BadKeys {
        /// The context of the binding.
        context: String,
        /// The parse error.
        #[source]
        source: KeystrokeError,
    },
    /// A command id is malformed.
    #[error("invalid command in context `{context}`: {source}")]
    BadCommand {
        /// The context of the binding.
        context: String,
        /// The parse error.
        #[source]
        source: crate::InvalidCommandId,
    },
}

/// A non-fatal problem with a keymap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeymapDiagnostic {
    /// A binding refers to a command nobody registered.
    UnknownCommand(Binding),
}

/// Effective keybindings after applying all layers.
///
/// Each (keys, context) pair has at most one binding: a later layer replaces
/// or removes what an earlier one defined.
#[derive(Debug, Clone, Default)]
pub struct Keymap {
    bindings: Vec<Binding>,
}

impl Keymap {
    /// The built-in default keymap for `platform`.
    pub fn defaults(platform: Platform) -> Self {
        let mut keymap = Self::default();
        if let Err(error) = keymap.apply(default_source(platform), platform) {
            // Covered by tests; keep the IDE usable if it ever happens.
            tracing::error!(%error, "built-in keymap is invalid");
        }
        keymap
    }

    /// Applies a keymap file on top of the current bindings.
    ///
    /// # Errors
    ///
    /// Fails without changing anything if the file has any error.
    pub fn apply(&mut self, source: &str, platform: Platform) -> Result<(), KeymapError> {
        let table: toml::Table = source
            .parse()
            .map_err(|error: toml::de::Error| KeymapError::Syntax(error.to_string()))?;

        let mut updates = Vec::new();
        for (context, bindings) in &table {
            let context_name = ContextName::new(context)
                .ok_or_else(|| KeymapError::BadContext(context.clone()))?;
            let toml::Value::Table(bindings) = bindings else {
                return Err(KeymapError::Syntax(format!(
                    "context `{context}` must be a table"
                )));
            };
            for (keys, command) in bindings {
                let toml::Value::String(command) = command else {
                    return Err(KeymapError::Syntax(format!(
                        "binding `{keys}` in context `{context}` must be a command id string"
                    )));
                };
                let keys =
                    KeySequence::parse(keys, platform).map_err(|source| KeymapError::BadKeys {
                        context: context.clone(),
                        source,
                    })?;
                let command = if command.is_empty() {
                    None
                } else {
                    Some(
                        CommandId::new(command).map_err(|source| KeymapError::BadCommand {
                            context: context.clone(),
                            source,
                        })?,
                    )
                };
                updates.push((keys, context_name.clone(), command));
            }
        }

        for (keys, context, command) in updates {
            self.bindings
                .retain(|binding| binding.keys != keys || binding.context != context);
            if let Some(command) = command {
                self.bindings.push(Binding {
                    keys,
                    context,
                    command,
                });
            }
        }
        Ok(())
    }

    /// All bindings.
    pub fn bindings(&self) -> &[Binding] {
        &self.bindings
    }

    /// Bindings whose command is not in `registry`.
    pub fn diagnostics(&self, registry: &CommandRegistry) -> Vec<KeymapDiagnostic> {
        self.bindings
            .iter()
            .filter(|binding| !registry.contains(&binding.command))
            .cloned()
            .map(KeymapDiagnostic::UnknownCommand)
            .collect()
    }

    /// Bindings that run `command`, e.g. to show the shortcut in the palette.
    pub fn bindings_for<'a>(&'a self, command: &'a CommandId) -> impl Iterator<Item = &'a Binding> {
        self.bindings
            .iter()
            .filter(move |binding| &binding.command == command)
    }
}

/// Text of the embedded default keymap for `platform`.
fn default_source(platform: Platform) -> &'static str {
    match platform {
        Platform::Windows => include_str!("../keymaps/default-windows.toml"),
        Platform::MacOs => include_str!("../keymaps/default-macos.toml"),
        Platform::Linux => include_str!("../keymaps/default-linux.toml"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CommandDescriptor;

    fn keymap(source: &str) -> Keymap {
        let mut keymap = Keymap::default();
        keymap.apply(source, Platform::Linux).unwrap();
        keymap
    }

    fn command(id: &str) -> CommandId {
        CommandId::new(id).unwrap()
    }

    #[test]
    fn defaults_are_valid_on_every_platform() {
        for platform in [Platform::Windows, Platform::MacOs, Platform::Linux] {
            Keymap::default()
                .apply(default_source(platform), platform)
                .unwrap();
            assert!(!Keymap::defaults(platform).bindings().is_empty());
        }
    }

    #[test]
    fn parses_contexts_and_bindings() {
        let keymap = keymap(
            "[global]\n\"ctrl-q\" = \"app.quit\"\n[editor]\n\"ctrl-k ctrl-s\" = \"editor.saveAll\"",
        );

        assert_eq!(keymap.bindings().len(), 2);
        let save_all_command = command("editor.saveAll");
        let save_all = keymap.bindings_for(&save_all_command).next().unwrap();
        assert_eq!(save_all.context.as_str(), "editor");
        assert_eq!(save_all.keys.to_string(), "ctrl-k ctrl-s");
    }

    #[test]
    fn bindings_keep_file_order() {
        // The first binding is the one shown in hints, so it must be the
        // first one written, not the alphabetically first.
        let keymap = keymap("[global]\n\"mod-shift-p\" = \"a.palette\"\n\"f1\" = \"a.palette\"");
        let palette = command("a.palette");
        let first = keymap.bindings_for(&palette).next().unwrap();
        assert_eq!(first.keys.to_string(), "ctrl-shift-p");
    }

    #[test]
    fn later_layer_replaces_and_unbinds() {
        let mut keymap =
            keymap("[editor]\n\"ctrl-s\" = \"editor.save\"\n\"ctrl-w\" = \"editor.close\"");
        keymap
            .apply(
                "[editor]\n\"ctrl-s\" = \"editor.saveAll\"\n\"ctrl-w\" = \"\"",
                Platform::Linux,
            )
            .unwrap();

        assert_eq!(keymap.bindings().len(), 1);
        assert_eq!(keymap.bindings()[0].command, command("editor.saveAll"));
    }

    #[test]
    fn invalid_file_changes_nothing() {
        let mut keymap = keymap("[global]\n\"ctrl-q\" = \"app.quit\"");
        for source in [
            "[global",
            "[Global]\n\"ctrl-q\" = \"app.quit\"",
            "[global]\n\"hyper-q\" = \"app.quit\"",
            "[global]\n\"ctrl-q\" = \"quit\"",
            "[global]\n\"ctrl-q\" = 1",
            "global = 1",
            "[global]\n\"ctrl-w\" = \"\"\n\"ctrl-q\" = \"bad\"",
        ] {
            assert!(keymap.apply(source, Platform::Linux).is_err(), "{source}");
        }
        assert_eq!(keymap.bindings().len(), 1);
    }

    #[test]
    fn reports_unknown_commands() {
        let keymap = keymap("[global]\n\"ctrl-q\" = \"app.quit\"\n\"ctrl-n\" = \"app.newWindow\"");
        let mut registry = CommandRegistry::new();
        registry
            .register(CommandDescriptor::new(
                command("app.quit"),
                "command-app-quit",
            ))
            .unwrap();

        let diagnostics = keymap.diagnostics(&registry);
        assert!(matches!(
            diagnostics.as_slice(),
            [KeymapDiagnostic::UnknownCommand(binding)] if binding.command == command("app.newWindow")
        ));
    }
}
