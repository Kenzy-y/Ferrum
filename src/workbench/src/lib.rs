//! The window layout: title bar, activity bar, sidebar, editor area, bottom
//! panel and status bar, as in `docs/design/mockups`.
//!
//! - `parts/` — the regions of the window, each drawn from [`Workbench`]
//!   state.
//! - `contrib/` — features shown in those regions (Explorer, Outline,
//!   quick open, Welcome), one folder each.
//! - `dialogs/` — modal dialogs.
//!
//! The workbench owns layout state only (which view is shown, which parts
//! are visible). Every user action goes through a command id: buttons call
//! the [`Dispatch`] callback, and the application routes the id back to
//! [`Workbench::run_command`] or to whoever owns the command.

pub mod command_ids;
mod contrib;
mod dialogs;
mod layout;
mod overlays;
mod parts;
mod workbench;

use commands::{CommandDescriptor, CommandId, CommandRegistry, RegistryError};

pub use crate::contrib::explorer::file_icons::{FileIcon, FileIcons};
pub use crate::contrib::explorer::file_tree_view::{
    FileTreeTexts, FileTreeView, OpenFile, WorkspaceSettingsChanged,
};
pub use crate::layout::{PanelKind, SidebarView};
pub use crate::workbench::{Dispatch, OpenPath, RecentEntry, Workbench, WorkspaceInfo};

/// Registers the workbench commands.
///
/// # Errors
///
/// Fails if a command id is already taken.
pub fn register_commands(registry: &mut CommandRegistry) -> Result<(), RegistryError> {
    for (id, title) in command_ids::ALL {
        if let Ok(id) = CommandId::new(id) {
            registry.register(CommandDescriptor::new(id, *title))?;
        }
    }
    editor::register_commands(registry)
}

#[cfg(test)]
mod tests {
    use i18n::I18n;

    use super::*;

    #[test]
    fn registers_every_command_with_a_localized_title() {
        let mut registry = CommandRegistry::new();
        register_commands(&mut registry).unwrap();
        assert!(registry.iter().count() > command_ids::ALL.len());
        for locale in i18n::AVAILABLE_LOCALES {
            let i18n = I18n::new(locale).unwrap();
            for command in registry.iter() {
                assert!(
                    i18n.has(&command.title_message),
                    "{}",
                    command.title_message
                );
            }
        }
    }
}
