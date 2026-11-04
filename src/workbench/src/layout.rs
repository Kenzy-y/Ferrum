//! Layout state: which sidebar view and bottom panel tab are shown.

use gpui::Context;

use crate::{Workbench, command_ids};

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
    /// Changes the layout state for a command; `false` if it is not ours.
    pub(crate) fn apply_command(&mut self, id: &str) -> bool {
        match id {
            command_ids::TOGGLE_SIDEBAR => self.sidebar_visible = !self.sidebar_visible,
            command_ids::TOGGLE_PANEL => self.panel_visible = !self.panel_visible,
            other => match command_ids::view_for(other) {
                Some(view) => self.show_view(view),
                None => return false,
            },
        }
        true
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
        assert!(workbench.apply_command(command_ids::TOGGLE_SIDEBAR));
        assert!(workbench.apply_command(command_ids::TOGGLE_PANEL));
        assert!(!workbench.sidebar_visible && workbench.panel_visible);
        assert!(!workbench.apply_command("editor.save"));
    }

    #[test]
    fn showing_the_visible_view_hides_the_sidebar() {
        let mut workbench = workbench();
        workbench.apply_command(command_ids::SHOW_SEARCH);
        assert_eq!(workbench.view, SidebarView::Search);
        assert!(workbench.sidebar_visible);

        workbench.apply_command(command_ids::SHOW_SEARCH);
        assert!(!workbench.sidebar_visible);

        workbench.apply_command(command_ids::SHOW_EXPLORER);
        assert_eq!(workbench.view, SidebarView::Explorer);
        assert!(workbench.sidebar_visible);
    }

    #[test]
    fn every_view_has_a_command() {
        for view in SidebarView::ALL {
            let id = command_ids::show_view(view);
            assert_eq!(command_ids::view_for(id), Some(view));
        }
    }
}
