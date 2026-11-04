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

/// Every workbench command with the id of its localized title.
pub(crate) const ALL: &[(&str, &str)] = &[
    (TOGGLE_SIDEBAR, "command-workbench-toggle-sidebar"),
    (TOGGLE_PANEL, "command-workbench-toggle-panel"),
    (SHOW_EXPLORER, "command-workbench-show-explorer"),
    (SHOW_SEARCH, "command-workbench-show-search"),
    (SHOW_SOURCE_CONTROL, "command-workbench-show-source-control"),
    (SHOW_RUN_AND_DEBUG, "command-workbench-show-run-and-debug"),
    (SHOW_EXTENSIONS, "command-workbench-show-extensions"),
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

/// The view a show command opens.
pub(crate) fn view_for(id: &str) -> Option<SidebarView> {
    SidebarView::ALL
        .into_iter()
        .find(|view| show_view(*view) == id)
}
