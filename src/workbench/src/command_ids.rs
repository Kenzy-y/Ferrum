//! Ids of the workbench commands.

use crate::SidebarView;

/// Shows or hides the sidebar.
pub const TOGGLE_SIDEBAR: &str = "workbench.toggleSidebar";
/// Shows or hides the bottom panel.
pub const TOGGLE_PANEL: &str = "workbench.togglePanel";
/// Shows the Explorer view.
pub const SHOW_EXPLORER: &str = "workbench.showExplorer";
/// Shows the Search view.
pub const SHOW_SEARCH: &str = "workbench.showSearch";
/// Shows the Source Control view.
pub const SHOW_SOURCE_CONTROL: &str = "workbench.showSourceControl";
/// Shows the Run and Debug view.
pub const SHOW_RUN_AND_DEBUG: &str = "workbench.showRunAndDebug";
/// Shows the Extensions view.
pub const SHOW_EXTENSIONS: &str = "workbench.showExtensions";

/// Opens quick open (title bar search).
pub const QUICK_OPEN: &str = "workbench.quickOpen";
/// Opens the command palette.
pub const SHOW_COMMAND_PALETTE: &str = "workbench.showCommandPalette";

/// Closes the active tab (asking about unsaved changes).
pub const CLOSE_TAB: &str = "workbench.closeTab";
/// Activates the next tab.
pub const NEXT_TAB: &str = "workbench.nextTab";
/// Activates the previous tab.
pub const PREVIOUS_TAB: &str = "workbench.previousTab";

/// Opens the first menu of the title bar for the keyboard.
pub const OPEN_MENU_BAR: &str = "workbench.openMenuBar";

/// Closes the window, asking about unsaved files.
pub const CLOSE_WINDOW: &str = "workbench.closeWindow";

/// Shows the Explorer and moves the focus to its Outline section.
pub const FOCUS_OUTLINE: &str = "workbench.focusOutline";
/// Expands or collapses the Outline section of the Explorer.
pub const TOGGLE_OUTLINE: &str = "workbench.toggleOutline";

/// Opens a folder with the system folder picker. Owned by the
/// application (it knows projects); the workbench shows buttons for it.
pub const OPEN_FOLDER: &str = "workspace.openFolder";
/// Opens a folder by typing its path. Owned by the application.
pub const OPEN_FOLDER_BY_PATH: &str = "workspace.openFolderByPath";

/// Opens the user `settings.toml` in an editor. Owned by the application
/// (it knows where configuration lives); the activity bar shows it.
pub const OPEN_SETTINGS: &str = "preferences.openSettings";
/// Opens the user `keymap.toml` in an editor. Owned by the application.
pub const OPEN_KEYMAP: &str = "preferences.openKeymap";

/// Every workbench command with the id of its localized title.
pub(crate) const ALL: &[(&str, &str)] = &[
    (QUICK_OPEN, "command-workbench-quick-open"),
    (
        SHOW_COMMAND_PALETTE,
        "command-workbench-show-command-palette",
    ),
    (TOGGLE_SIDEBAR, "command-workbench-toggle-sidebar"),
    (TOGGLE_PANEL, "command-workbench-toggle-panel"),
    (SHOW_EXPLORER, "command-workbench-show-explorer"),
    (SHOW_SEARCH, "command-workbench-show-search"),
    (SHOW_SOURCE_CONTROL, "command-workbench-show-source-control"),
    (SHOW_RUN_AND_DEBUG, "command-workbench-show-run-and-debug"),
    (SHOW_EXTENSIONS, "command-workbench-show-extensions"),
    (CLOSE_TAB, "command-workbench-close-tab"),
    (NEXT_TAB, "command-workbench-next-tab"),
    (PREVIOUS_TAB, "command-workbench-previous-tab"),
    (CLOSE_WINDOW, "command-workbench-close-window"),
    (FOCUS_OUTLINE, "command-workbench-focus-outline"),
    (TOGGLE_OUTLINE, "command-workbench-toggle-outline"),
    (OPEN_MENU_BAR, "command-workbench-open-menu-bar"),
];

/// The command that shows `view`.
pub fn show_view(view: SidebarView) -> &'static str {
    match view {
        SidebarView::Explorer => SHOW_EXPLORER,
        SidebarView::Search => SHOW_SEARCH,
        SidebarView::SourceControl => SHOW_SOURCE_CONTROL,
        SidebarView::RunAndDebug => SHOW_RUN_AND_DEBUG,
        SidebarView::Extensions => SHOW_EXTENSIONS,
    }
}
