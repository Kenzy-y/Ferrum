//! Commands owned by the application shell, and the registry of all
//! commands linked into the application.

use commands::{CommandDescriptor, CommandId, CommandRegistry};

/// Quits the application.
pub(crate) const QUIT: &str = "app.quit";

/// Shows or hides the temporary component gallery in the editor area
/// (stage 1; see ROADMAP).
pub(crate) const TOGGLE_COMPONENT_GALLERY: &str = "workbench.toggleComponentGallery";

/// Commands of the shell with the ids of their localized titles.
const SHELL_COMMANDS: &[(&str, &str)] = &[
    (QUIT, "command-app-quit"),
    (
        TOGGLE_COMPONENT_GALLERY,
        "command-workbench-toggle-component-gallery",
    ),
];

/// Registry of every command: the shell's and the workbench's.
pub(crate) fn registry() -> anyhow::Result<CommandRegistry> {
    let mut registry = CommandRegistry::new();
    for (id, title) in SHELL_COMMANDS {
        registry.register(CommandDescriptor::new(CommandId::new(id)?, *title))?;
    }
    workbench::register_commands(&mut registry)?;
    Ok(registry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_command_has_a_title_in_every_locale() {
        let registry = registry().unwrap();
        for locale in i18n::AVAILABLE_LOCALES {
            let i18n = i18n::I18n::new(locale).unwrap();
            for command in registry.iter() {
                assert!(
                    i18n.has(&command.title_message),
                    "{} has no title",
                    command.id
                );
            }
        }
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
