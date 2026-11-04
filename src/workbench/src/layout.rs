//! Layout state: which sidebar view and bottom panel tab are shown.

use gpui::Context;

use crate::Workbench;

/// What the sidebar shows; one per activity bar button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SidebarView {
    /// Files of the workspace.
    Explorer,
    /// Project-wide search.
    Search,
    /// Git.
    SourceControl,
    /// Debugger.
    RunAndDebug,
    /// Plugins.
    Extensions,
}

impl SidebarView {
    /// Every view in activity bar order.
    pub const ALL: [Self; 5] = [
        Self::Explorer,
        Self::Search,
        Self::SourceControl,
        Self::RunAndDebug,
        Self::Extensions,
    ];
}

/// A tab of the bottom panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PanelKind {
    /// Integrated terminal.
    Terminal,
    /// Diagnostics.
    Problems,
    /// Output channels.
    Output,
    /// Debugger console.
    DebugConsole,
}

impl PanelKind {
    /// Every panel in tab order.
    pub const ALL: [Self; 4] = [
        Self::Terminal,
        Self::Problems,
        Self::Output,
        Self::DebugConsole,
    ];
}

impl Workbench {
    /// Shows or hides the sidebar.
    pub(crate) fn toggle_sidebar(&mut self) {
        self.sidebar_visible = !self.sidebar_visible;
    }

    /// Shows or hides the bottom panel.
    pub(crate) fn toggle_panel(&mut self) {
        self.panel_visible = !self.panel_visible;
    }

    /// Shows `view` in the sidebar; activating the visible view hides the
    /// sidebar, like clicking its activity button again.
    pub(crate) fn show_view(&mut self, view: SidebarView) {
        if self.sidebar_visible && self.view == view {
            self.sidebar_visible = false;
        } else {
            self.view = view;
            self.sidebar_visible = true;
        }
    }

    pub(crate) fn select_panel(&mut self, panel: PanelKind, cx: &mut Context<Self>) {
        self.panel = panel;
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;
    use std::sync::Arc;

    use commands::{CommandRegistry, Keymap};
    use i18n::I18n;

    use super::*;
    use crate::command_ids;

    fn workbench() -> Workbench {
        let i18n = Arc::new(I18n::new(i18n::DEFAULT_LOCALE).unwrap());
        Workbench::new(
            i18n,
            Rc::new(|_, _, _| {}),
            Arc::new(CommandRegistry::new()),
            Arc::new(Keymap::default()),
            Rc::new(|_, _, _| {}),
        )
    }

    #[test]
    fn toggles_sidebar_and_panel() {
        let mut workbench = workbench();
        assert!(workbench.sidebar_visible && !workbench.panel_visible);
        workbench.toggle_sidebar();
        workbench.toggle_panel();
        assert!(!workbench.sidebar_visible && workbench.panel_visible);
    }

    #[test]
    fn showing_the_visible_view_hides_the_sidebar() {
        let mut workbench = workbench();
        workbench.show_view(SidebarView::Search);
        assert_eq!(workbench.view, SidebarView::Search);
        assert!(workbench.sidebar_visible);

        workbench.show_view(SidebarView::Search);
        assert!(!workbench.sidebar_visible);

        workbench.show_view(SidebarView::Explorer);
        assert_eq!(workbench.view, SidebarView::Explorer);
        assert!(workbench.sidebar_visible);
    }

    #[test]
    fn every_view_has_its_own_command() {
        let ids: std::collections::HashSet<_> = SidebarView::ALL
            .into_iter()
            .map(command_ids::show_view)
            .collect();
        assert_eq!(ids.len(), SidebarView::ALL.len());
    }
}
