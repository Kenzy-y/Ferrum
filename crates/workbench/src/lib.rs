//! The window layout: title bar, activity bar, sidebar, editor area, bottom
//! panel and status bar, as in `docs/design/mockups`.
//!
//! The workbench owns layout state only (which view is shown, which parts
//! are visible). Every user action goes through a command id: buttons call
//! the [`Dispatch`] callback, and the application routes the id back to
//! [`Workbench::run_command`] or to whoever owns the command.

mod activity_bar;
mod bottom_panel;
pub mod command_ids;
mod editor_area;
mod quick_open;
mod sidebar;
mod status_bar;
mod title_bar;
mod window_controls;

use std::rc::Rc;
use std::sync::Arc;

use commands::{CommandDescriptor, CommandId, CommandRegistry, Keymap, Platform, RegistryError};
use gpui::{
    AnyView, App, AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _,
    IntoElement, MouseButton, ParentElement as _, Render, SharedString, Styled as _, Subscription,
    Window, WindowDecorations, WindowOptions, div, px,
};
use i18n::I18n;
use theme::ActiveTheme as _;

use crate::quick_open::{Dismiss, QuickOpen, QuickOpenContext, QuickOpenMode};

/// Runs a command by id. Provided by the application, which knows every
/// command owner.
pub type Dispatch = Rc<dyn Fn(&CommandId, &mut Window, &mut App)>;

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

/// A view shown in the editor area with the title of its tab.
#[derive(Clone)]
pub struct EditorContent {
    /// Tab title.
    pub title: SharedString,
    /// The view.
    pub view: AnyView,
}

/// Root view of a main window.
pub struct Workbench {
    i18n: Arc<I18n>,
    dispatch: Dispatch,
    keymap: Arc<Keymap>,
    quick_open_context: QuickOpenContext,
    quick_open: Option<OpenQuickOpen>,
    view: SidebarView,
    sidebar_visible: bool,
    panel_visible: bool,
    panel: PanelKind,
    editor: Option<EditorContent>,
}

impl Workbench {
    /// Creates the layout of a window without an open folder. `registry`
    /// and `keymap` feed the command palette.
    pub fn new(
        i18n: Arc<I18n>,
        dispatch: Dispatch,
        registry: Arc<CommandRegistry>,
        keymap: Arc<Keymap>,
    ) -> Self {
        let quick_open_context = QuickOpenContext {
            i18n: i18n.clone(),
            registry,
            keymap: keymap.clone(),
            dispatch: dispatch.clone(),
        };
        Self {
            i18n,
            dispatch,
            keymap,
            quick_open_context,
            quick_open: None,
            view: SidebarView::Explorer,
            sidebar_visible: true,
            panel_visible: false,
            panel: PanelKind::Terminal,
            editor: None,
        }
    }

    /// Window options the workbench is designed for: the title bar is drawn
    /// by the workbench, the platform keeps native window behaviour.
    pub fn window_options() -> WindowOptions {
        WindowOptions {
            titlebar: Some(title_bar::titlebar_options()),
            app_owns_titlebar_drag: true,
            window_decorations: Some(WindowDecorations::Client),
            ..WindowOptions::default()
        }
    }

    /// Runs a workbench command. Returns `false` if `id` is not one of
    /// [`command_ids`].
    pub fn run_command(
        &mut self,
        id: &CommandId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let mode = match id.as_str() {
            command_ids::QUICK_OPEN => Some(QuickOpenMode::Everything),
            command_ids::SHOW_COMMAND_PALETTE => Some(QuickOpenMode::Commands),
            _ => None,
        };
        if let Some(mode) = mode {
            self.open_quick_open(mode, window, cx);
            return true;
        }
        let handled = self.apply_command(id.as_str());
        if handled {
            cx.notify();
        }
        handled
    }

    /// Changes the layout state for a command; `false` if it is not ours.
    fn apply_command(&mut self, id: &str) -> bool {
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
    fn show_view(&mut self, view: SidebarView) {
        if self.sidebar_visible && self.view == view {
            self.sidebar_visible = false;
        } else {
            self.view = view;
            self.sidebar_visible = true;
        }
    }

    /// Replaces what the editor area shows.
    pub fn set_editor_content(&mut self, content: Option<EditorContent>, cx: &mut Context<Self>) {
        self.editor = content;
        cx.notify();
    }

    /// Whether the editor area shows something.
    pub fn has_editor_content(&self) -> bool {
        self.editor.is_some()
    }

    fn open_quick_open(
        &mut self,
        mode: QuickOpenMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let previous_focus = match self.quick_open.take() {
            Some(open) => open.previous_focus,
            None => window.focused(cx),
        };
        let context = self.quick_open_context.clone();
        let view = cx.new(|cx| QuickOpen::new(mode, &context, window, cx));
        let subscription = cx.subscribe_in(&view, window, |this, _, _: &Dismiss, window, cx| {
            this.close_quick_open(window, cx);
        });
        self.quick_open = Some(OpenQuickOpen {
            view,
            previous_focus,
            _subscription: subscription,
        });
        cx.notify();
    }

    fn close_quick_open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(open) = self.quick_open.take() {
            if let Some(focus) = open.previous_focus {
                focus.focus(window, cx);
            }
            cx.notify();
        }
    }

    /// Label of the first shortcut bound to `command`, for hints.
    fn shortcut_label(&self, command: &str) -> Option<SharedString> {
        let id = CommandId::new(command).ok()?;
        self.keymap
            .bindings_for(&id)
            .next()
            .map(|binding| binding.keys.label(Platform::current()).into())
    }

    fn select_panel(&mut self, panel: PanelKind, cx: &mut Context<Self>) {
        self.panel = panel;
        cx.notify();
    }

    fn text(&self, id: &str) -> SharedString {
        self.i18n.text(id).into()
    }
}

/// An open quick open overlay and the focus to restore when it closes.
struct OpenQuickOpen {
    view: Entity<QuickOpen>,
    previous_focus: Option<FocusHandle>,
    _subscription: Subscription,
}

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
    Ok(())
}

impl Render for Workbench {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title_bar = title_bar::render(self, window, cx);
        let activity_bar = activity_bar::render(self, cx);
        let sidebar = self.sidebar_visible.then(|| sidebar::render(self, cx));
        let editor = editor_area::render(self, cx);
        let panel = self.panel_visible.then(|| bottom_panel::render(self, cx));
        let status_bar = status_bar::render(self, cx);
        let overlay = self.quick_open.as_ref().map(|open| {
            let size = &cx.theme().metrics.size;
            let top = (size.titlebar_height - size.input_inset_height) / 2.0
                - cx.theme().metrics.space.sm;
            div()
                .id("quick-open-overlay")
                .absolute()
                .inset_0()
                .flex()
                .flex_col()
                .items_center()
                .pt(px(top))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| this.close_quick_open(window, cx)),
                )
                .child(open.view.clone())
        });

        let theme = cx.theme();
        let metrics = &theme.metrics;
        let gap = metrics.size.panel_gap;
        let main = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .gap(px(gap))
            .child(editor)
            .children(panel);
        let body = div()
            .flex()
            .flex_1()
            .min_h_0()
            .pt(px(metrics.size.layout_gap_top))
            .pr(px(gap))
            .child(activity_bar)
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(gap))
                    .children(sidebar)
                    .child(main),
            );
        div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.colors.bg_app)
            .child(title_bar)
            .child(body)
            .child(status_bar)
            .children(overlay)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_every_command_with_a_localized_title() {
        let mut registry = CommandRegistry::new();
        register_commands(&mut registry).unwrap();
        assert_eq!(registry.iter().count(), command_ids::ALL.len());
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

    fn workbench() -> Workbench {
        let i18n = Arc::new(I18n::new(i18n::DEFAULT_LOCALE).unwrap());
        Workbench::new(
            i18n,
            Rc::new(|_, _, _| {}),
            Arc::new(CommandRegistry::new()),
            Arc::new(Keymap::default()),
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
