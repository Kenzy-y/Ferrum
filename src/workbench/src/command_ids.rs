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

/// Creates a file in the folder selected in the Explorer (or the root)
/// and opens it.
pub const NEW_FILE: &str = "explorer.newFile";

/// Shows the terminal and focuses it, starting one if none runs; hides
/// the panel when the terminal already has the focus.
pub const TOGGLE_TERMINAL: &str = "workbench.toggleTerminal";
/// Lets the bottom panel take the whole height of the editor area, or
/// gives the editor its place back.
pub const TOGGLE_MAXIMIZED_PANEL: &str = "workbench.toggleMaximizedPanel";

/// Opens a new terminal tab with the default shell.
pub const TERMINAL_NEW: &str = "terminal.new";
/// Opens a terminal next to the active one.
pub const TERMINAL_SPLIT: &str = "terminal.split";
/// Ends the active terminal's shell and closes it.
pub const TERMINAL_KILL: &str = "terminal.kill";
/// Opens the list of shells for a new terminal.
pub const TERMINAL_SELECT_SHELL: &str = "terminal.selectShell";
/// Shows the next terminal tab.
pub const TERMINAL_FOCUS_NEXT: &str = "terminal.focusNext";
/// Shows the previous terminal tab.
pub const TERMINAL_FOCUS_PREVIOUS: &str = "terminal.focusPrevious";
/// Copies the terminal selection.
pub const TERMINAL_COPY: &str = "terminal.copy";
/// Pastes into the terminal.
pub const TERMINAL_PASTE: &str = "terminal.paste";
/// Selects the whole terminal buffer.
pub const TERMINAL_SELECT_ALL: &str = "terminal.selectAll";
/// Clears the terminal and its scrollback.
pub const TERMINAL_CLEAR: &str = "terminal.clear";
/// Scrolls the terminal a screen up.
pub const TERMINAL_SCROLL_PAGE_UP: &str = "terminal.scrollPageUp";
/// Scrolls the terminal a screen down.
pub const TERMINAL_SCROLL_PAGE_DOWN: &str = "terminal.scrollPageDown";
/// Scrolls the terminal to its oldest line.
pub const TERMINAL_SCROLL_TO_TOP: &str = "terminal.scrollToTop";
/// Scrolls the terminal back to the live screen.
pub const TERMINAL_SCROLL_TO_BOTTOM: &str = "terminal.scrollToBottom";

/// Opens a folder with the system folder picker. Owned by the
/// application (it knows projects); the workbench shows buttons for it.
pub const OPEN_FOLDER: &str = "workspace.openFolder";
/// Opens a folder by typing its path. Owned by the application.
pub const OPEN_FOLDER_BY_PATH: &str = "workspace.openFolderByPath";

/// Opens the settings page. Owned by the application (it knows the
/// settings); the activity bar shows it.
pub const OPEN_SETTINGS: &str = "preferences.openSettings";
/// Opens the user `settings.toml` in an editor. Owned by the application.
pub const OPEN_SETTINGS_FILE: &str = "preferences.openSettingsFile";
/// Opens the keyboard shortcuts page. Owned by the application.
pub const OPEN_KEYBINDINGS: &str = "preferences.openKeybindings";
/// Opens the user `keymap.toml` in an editor. Owned by the application.
pub const OPEN_KEYMAP: &str = "preferences.openKeymap";

/// Makes the whole interface one step larger. Owned by the application
/// (zoom is a setting).
pub const ZOOM_IN: &str = "workbench.zoomIn";
/// Makes the whole interface one step smaller.
pub const ZOOM_OUT: &str = "workbench.zoomOut";
/// Returns the interface to its normal size.
pub const ZOOM_RESET: &str = "workbench.zoomReset";

/// Closes every tab of the group but the active one.
pub const CLOSE_OTHER_TABS: &str = "workbench.closeOtherTabs";
/// Closes the tabs without unsaved changes.
pub const CLOSE_SAVED_TABS: &str = "workbench.closeSavedTabs";
/// Closes every tab of the group.
pub const CLOSE_ALL_TABS: &str = "workbench.closeAllTabs";
/// Splits the editor area into two groups side by side; the active tab
/// moves to the other group.
pub const SPLIT_EDITOR: &str = "workbench.splitEditor";
/// Moves the focus to the other editor group.
pub const FOCUS_OTHER_GROUP: &str = "workbench.focusOtherGroup";
/// Opens the menu of the tabs ("…").
pub const SHOW_TAB_MENU: &str = "workbench.showTabMenu";
/// Shows or hides the notification center.
pub const TOGGLE_NOTIFICATIONS: &str = "workbench.toggleNotifications";
/// Removes every notification.
pub const CLEAR_NOTIFICATIONS: &str = "workbench.clearNotifications";
/// Opens the list of recent projects from the Explorer card.
pub const PICK_PROJECT: &str = "workbench.pickProject";

/// Opens the find widget over the active editor.
pub const FIND: &str = "find.open";
/// Opens the find widget with the replace row.
pub const FIND_REPLACE: &str = "find.replace";
/// Goes to the next find match.
pub const FIND_NEXT: &str = "find.next";
/// Goes to the previous find match.
pub const FIND_PREVIOUS: &str = "find.previous";

/// Every workbench command with the id of its localized title.
pub(crate) const ALL: &[(&str, &str)] = &[
    (QUICK_OPEN, "command-workbench-quick-open"),
    (FIND, "command-find-open"),
    (CLOSE_OTHER_TABS, "command-workbench-close-other-tabs"),
    (CLOSE_SAVED_TABS, "command-workbench-close-saved-tabs"),
    (CLOSE_ALL_TABS, "command-workbench-close-all-tabs"),
    (SPLIT_EDITOR, "command-workbench-split-editor"),
    (FOCUS_OTHER_GROUP, "command-workbench-focus-other-group"),
    (SHOW_TAB_MENU, "command-workbench-show-tab-menu"),
    (
        TOGGLE_NOTIFICATIONS,
        "command-workbench-toggle-notifications",
    ),
    (CLEAR_NOTIFICATIONS, "command-workbench-clear-notifications"),
    (PICK_PROJECT, "command-workbench-pick-project"),
    (FIND_REPLACE, "command-find-replace"),
    (FIND_NEXT, "command-find-next"),
    (FIND_PREVIOUS, "command-find-previous"),
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
    (NEW_FILE, "command-explorer-new-file"),
    (TOGGLE_TERMINAL, "command-workbench-toggle-terminal"),
    (
        TOGGLE_MAXIMIZED_PANEL,
        "command-workbench-toggle-maximized-panel",
    ),
    (TERMINAL_NEW, "command-terminal-new"),
    (TERMINAL_SPLIT, "command-terminal-split"),
    (TERMINAL_KILL, "command-terminal-kill"),
    (TERMINAL_SELECT_SHELL, "command-terminal-select-shell"),
    (TERMINAL_FOCUS_NEXT, "command-terminal-focus-next"),
    (TERMINAL_FOCUS_PREVIOUS, "command-terminal-focus-previous"),
    (TERMINAL_COPY, "command-terminal-copy"),
    (TERMINAL_PASTE, "command-terminal-paste"),
    (TERMINAL_SELECT_ALL, "command-terminal-select-all"),
    (TERMINAL_CLEAR, "command-terminal-clear"),
    (TERMINAL_SCROLL_PAGE_UP, "command-terminal-scroll-page-up"),
    (
        TERMINAL_SCROLL_PAGE_DOWN,
        "command-terminal-scroll-page-down",
    ),
    (TERMINAL_SCROLL_TO_TOP, "command-terminal-scroll-to-top"),
    (
        TERMINAL_SCROLL_TO_BOTTOM,
        "command-terminal-scroll-to-bottom",
    ),
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
