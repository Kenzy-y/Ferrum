//! Commands built into the application shell and their handlers.

use std::collections::HashMap;

use commands::{CommandDescriptor, CommandId, CommandRegistry};
use gpui::{App, Window};

/// Quits the application.
pub(crate) const QUIT: &str = "app.quit";

/// Runs a command in the window where it was invoked.
pub(crate) type Handler = fn(&mut Window, &mut App);

/// Registry of the built-in commands with their localized titles.
pub(crate) fn registry() -> anyhow::Result<CommandRegistry> {
    let mut registry = CommandRegistry::new();
    registry.register(CommandDescriptor::new(
        CommandId::new(QUIT)?,
        "command-app-quit",
    ))?;
    Ok(registry)
}

/// Handlers of the built-in commands, by id.
pub(crate) fn handlers() -> HashMap<CommandId, Handler> {
    let mut handlers: HashMap<CommandId, Handler> = HashMap::new();
    if let Ok(id) = CommandId::new(QUIT) {
        handlers.insert(id, |_window, cx| cx.quit());
    }
    handlers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_command_has_a_handler_and_a_title() {
        let registry = registry().unwrap();
        let handlers = handlers();
        let i18n = i18n::I18n::new(i18n::DEFAULT_LOCALE).unwrap();
        for command in registry.iter() {
            assert!(
                handlers.contains_key(&command.id),
                "{} has no handler",
                command.id
            );
            assert!(
                i18n.has(&command.title_message),
                "{} has no title",
                command.id
            );
        }
        assert_eq!(handlers.len(), registry.iter().count());
    }

    #[test]
    fn default_keymaps_only_use_registered_commands() {
        let registry = registry().unwrap();
        for platform in [
            commands::Platform::Windows,
            commands::Platform::MacOs,
            commands::Platform::Linux,
        ] {
            let keymap = commands::Keymap::defaults(platform);
            assert!(keymap.diagnostics(&registry).is_empty(), "{platform:?}");
        }
    }
}
