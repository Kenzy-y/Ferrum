//! The menus of the title bar: File, Edit, View and Go. Each lists
//! existing commands with their shortcuts; a menu adds no action of its
//! own. A click opens one, `workbench.openMenuBar` (F10) opens the first;
//! Up and Down choose, Left and Right switch menus, Enter runs, Escape
//! closes and gives the focus back.

pub(crate) mod view;

use crate::command_ids;

/// A row of a menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MenuItem {
    /// Runs the command with this id.
    Command(&'static str),
    /// A line between groups.
    Separator,
}

use MenuItem::{Command, Separator};

/// A menu: the message id of its label and its rows.
pub(crate) struct Menu {
    pub(crate) label: &'static str,
    pub(crate) items: &'static [MenuItem],
}

/// The menu behind "…" at the right of the tabs.
pub(crate) const TAB_MENU: &[MenuItem] = &[
    Command(command_ids::CLOSE_TAB),
    Command(command_ids::CLOSE_OTHER_TABS),
    Command(command_ids::CLOSE_SAVED_TABS),
    Command(command_ids::CLOSE_ALL_TABS),
    Separator,
    Command(command_ids::SPLIT_EDITOR),
    Command(command_ids::FOCUS_OTHER_GROUP),
];

/// The menus, left to right. Commands that are not registered (debug-only
/// ones in a release build) are left out when shown.
pub(crate) const MENUS: &[Menu] = &[
    Menu {
        label: "menu-file",
        items: &[
            Command(command_ids::NEW_FILE),
            Separator,
            Command(command_ids::OPEN_FOLDER),
            Command(command_ids::OPEN_FOLDER_BY_PATH),
            Separator,
            Command(editor::SAVE),
            Separator,
            Command(command_ids::OPEN_SETTINGS),
            Command(command_ids::OPEN_KEYMAP),
            Separator,
            Command(command_ids::CLOSE_TAB),
            Command(command_ids::CLOSE_WINDOW),
            Command("app.quit"),
        ],
    },
    Menu {
        label: "menu-edit",
        items: &[
            Command("editor.undo"),
            Command("editor.redo"),
            Separator,
            Command("editor.cut"),
            Command("editor.copy"),
            Command("editor.paste"),
            Separator,
            Command("editor.selectAll"),
            Command("editor.addCursorAbove"),
            Command("editor.addCursorBelow"),
            Separator,
            Command("editor.triggerSuggest"),
            Command("editor.convertLineEndingsToLf"),
            Command("editor.convertLineEndingsToCrlf"),
        ],
    },
    Menu {
        label: "menu-view",
        items: &[
            Command(command_ids::SHOW_COMMAND_PALETTE),
            Command(command_ids::QUICK_OPEN),
            Separator,
            Command(command_ids::SHOW_EXPLORER),
            Command(command_ids::SHOW_SEARCH),
            Command(command_ids::SHOW_SOURCE_CONTROL),
            Command(command_ids::SHOW_RUN_AND_DEBUG),
            Command(command_ids::SHOW_EXTENSIONS),
            Separator,
            Command(command_ids::TOGGLE_SIDEBAR),
            Command(command_ids::TOGGLE_PANEL),
            Command(command_ids::TOGGLE_OUTLINE),
        ],
    },
    Menu {
        label: "menu-go",
        items: &[
            Command("editor.goToDefinition"),
            Command("editor.showHover"),
            Separator,
            Command(command_ids::NEXT_TAB),
            Command(command_ids::PREVIOUS_TAB),
            Command(command_ids::FOCUS_OUTLINE),
            Separator,
            Command("editor.moveToDocumentStart"),
            Command("editor.moveToDocumentEnd"),
        ],
    },
];

#[cfg(test)]
mod tests {
    use commands::{CommandId, CommandRegistry};

    use super::*;

    /// Every command in a menu exists, except the shell's own (registered
    /// by the application, checked there).
    #[test]
    fn menus_only_name_known_commands() {
        let mut registry = CommandRegistry::new();
        crate::register_commands(&mut registry).unwrap();
        let shell_owned = [
            command_ids::OPEN_FOLDER,
            command_ids::OPEN_FOLDER_BY_PATH,
            command_ids::OPEN_SETTINGS,
            command_ids::OPEN_KEYMAP,
            "app.quit",
        ];
        for items in MENUS.iter().map(|menu| menu.items).chain([TAB_MENU]) {
            for item in items {
                if let Command(id) = item
                    && !shell_owned.contains(id)
                {
                    let id = CommandId::new(id).unwrap();
                    assert!(registry.get(&id).is_some(), "{id} is not registered");
                }
            }
        }
    }
}
