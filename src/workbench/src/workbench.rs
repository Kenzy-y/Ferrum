//! The root view of a main window and the state it shares with its parts.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use commands::{CommandId, CommandRegistry, Keymap, Platform, RegistryError};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, Context, Entity, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _,
    Render, SharedString, Styled as _, WeakEntity, Window, WindowDecorations, WindowOptions, div,
    px,
};
use i18n::I18n;
use theme::ActiveTheme as _;

use crate::command_handlers::CommandHandlers;
use crate::command_ids;
use crate::contrib::explorer::file_icons::FileIcons;
use crate::contrib::explorer::file_tree_view::FileTreeView;
use crate::contrib::extensions::ExtensionsState;
use crate::contrib::outline;
use crate::contrib::problems::ProblemsView;
use crate::contrib::quick_open::{QuickOpenContext, QuickOpenMode};
use crate::contrib::run_debug::DebugSections;
use crate::contrib::search::SearchView;
use crate::contrib::terminal::TerminalPanel;
use crate::layout::{PanelKind, SidebarView};
use crate::overlays::{OpenMenu, OpenPathPrompt, OpenQuickOpen};
use crate::parts::editor_area::{closing, tabs};
use crate::parts::resize::{ResizeDrag, ResizeEdge};
use crate::parts::{activity_bar, bottom_panel, editor_area, sidebar, status_bar, title_bar};

/// Runs a command by id. Provided by the application, which knows every
/// command owner.
pub type Dispatch = Rc<dyn Fn(&CommandId, &mut Window, &mut App)>;

/// Opens a folder by path. Provided by the application.
pub type OpenPath = Rc<dyn Fn(PathBuf, &mut Window, &mut App)>;

/// The open folder, as shown in the layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceInfo {
    /// Folder name.
    pub name: SharedString,
    /// Folder path for display.
    pub path: SharedString,
    /// The folder.
    pub root: PathBuf,
}

/// A recently opened folder, as listed on the Welcome page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentEntry {
    /// Folder name.
    pub name: SharedString,
    /// Folder path for display.
    pub path: SharedString,
    /// The folder to open.
    pub root: PathBuf,
}

/// Root view of a main window.
pub struct Workbench {
    pub(crate) i18n: Arc<I18n>,
    pub(crate) dispatch: Dispatch,
    pub(crate) keymap: Arc<Keymap>,
    pub(crate) quick_open_context: QuickOpenContext,
    pub(crate) quick_open: Option<OpenQuickOpen>,
    /// The open title bar menu.
    pub(crate) menu: Option<OpenMenu>,
    pub(crate) open_path: OpenPath,
    pub(crate) path_prompt: Option<OpenPathPrompt>,
    pub(crate) workspace: Option<WorkspaceInfo>,
    pub(crate) file_tree: Option<Entity<FileTreeView>>,
    pub(crate) recent: Vec<RecentEntry>,
    pub(crate) view: SidebarView,
    pub(crate) sidebar_visible: bool,
    pub(crate) panel_visible: bool,
    pub(crate) panel: PanelKind,
    /// The bottom panel takes the editor area's place.
    pub(crate) panel_maximized: bool,
    /// Panel sizes the user dragged to.
    pub(crate) sizes: project::WindowState,
    /// A panel border being dragged.
    pub(crate) resize: Option<ResizeDrag>,
    pub(crate) tabs: Vec<tabs::Tab>,
    pub(crate) active_tab: Option<usize>,
    pub(crate) file_icons: Arc<FileIcons>,
    /// Servers, toolchains and file details for the status bar.
    pub(crate) language_status: crate::contrib::language_status::LanguageStatus,
    pub(crate) outline: outline::OutlineSection,
    pub(crate) closing: closing::Closing,
    /// The Problems panel, once the application provides it.
    pub(crate) problems: Option<Entity<ProblemsView>>,
    problems_watch: Option<gpui::Subscription>,
    /// The terminals, once the application provides them.
    pub(crate) terminal: Option<Entity<TerminalPanel>>,
    pub(crate) terminal_watch: Option<gpui::Subscription>,
    pub(crate) terminal_observe: Option<gpui::Subscription>,
    /// The Search view, once the application provides it.
    pub(crate) search: Option<Entity<SearchView>>,
    /// Collapsed sections of the Run and Debug view.
    pub(crate) debug_sections: DebugSections,
    /// The Extensions view and the open extension page.
    pub(crate) extensions: ExtensionsState,
    /// The find widget over the active editor.
    pub(crate) find: Option<crate::contrib::find::OpenFind>,
    /// The second editor group, while the editor area is split.
    pub(crate) split: Option<crate::parts::editor_area::groups::Split>,
    /// Id of the active editor group.
    pub(crate) active_group: u64,
    /// The notification center's messages.
    pub(crate) notifications: crate::popovers::Notifications,
    /// The open popover (notifications, projects, tab menu).
    pub(crate) popover: Option<crate::popovers::OpenPopover>,
    /// Focus of the notification and project popovers, made on first use.
    pub(crate) popover_focus: Option<gpui::FocusHandle>,
}

impl Workbench {
    /// Creates the layout of a window without an open folder. `registry`
    /// and `keymap` feed the command palette.
    pub fn new(
        i18n: Arc<I18n>,
        dispatch: Dispatch,
        registry: Arc<CommandRegistry>,
        keymap: Arc<Keymap>,
        open_path: OpenPath,
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
            menu: None,
            open_path,
            path_prompt: None,
            workspace: None,
            file_tree: None,
            recent: Vec::new(),
            view: SidebarView::Explorer,
            sidebar_visible: true,
            panel_visible: false,
            panel: PanelKind::Terminal,
            panel_maximized: false,
            sizes: project::WindowState::default(),
            resize: None,
            tabs: Vec::new(),
            active_tab: None,
            file_icons: Arc::new(FileIcons::new()),
            language_status: crate::contrib::language_status::LanguageStatus::default(),
            outline: outline::OutlineSection::default(),
            closing: closing::Closing::default(),
            problems: None,
            problems_watch: None,
            terminal: None,
            terminal_watch: None,
            terminal_observe: None,
            search: None,
            debug_sections: DebugSections::default(),
            extensions: ExtensionsState::default(),
            find: None,
            split: None,
            active_group: 0,
            notifications: crate::popovers::Notifications::default(),
            popover: None,
            popover_focus: None,
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

    /// Registers the handlers of the workbench commands, and of every
    /// editor command, which runs on the active editor.
    ///
    /// # Errors
    ///
    /// Fails if one of these commands already has a handler.
    pub fn register_handlers(
        handlers: &mut CommandHandlers,
        this: &WeakEntity<Self>,
    ) -> Result<(), RegistryError> {
        handlers.register_entity(command_ids::QUICK_OPEN, this, |this, window, cx| {
            this.open_quick_open(QuickOpenMode::Everything, window, cx);
        })?;
        handlers.register_entity(
            command_ids::SHOW_COMMAND_PALETTE,
            this,
            |this, window, cx| {
                this.open_quick_open(QuickOpenMode::Commands, window, cx);
            },
        )?;
        handlers.register_entity(command_ids::TOGGLE_SIDEBAR, this, |this, _, cx| {
            this.toggle_sidebar();
            cx.notify();
        })?;
        handlers.register_entity(command_ids::TOGGLE_PANEL, this, |this, window, cx| {
            this.toggle_panel();
            let empty_terminal = this
                .terminal
                .as_ref()
                .is_some_and(|panel| panel.read(cx).is_empty());
            if this.panel_visible && this.panel == PanelKind::Terminal && empty_terminal {
                this.show_terminal(window, cx);
            }
            cx.notify();
        })?;
        for view in SidebarView::ALL {
            handlers.register_entity(
                command_ids::show_view(view),
                this,
                move |this, window, cx| {
                    this.show_view(view);
                    if this.sidebar_visible && this.view == SidebarView::Search {
                        this.focus_search(window, cx);
                    }
                    cx.notify();
                },
            )?;
        }
        handlers.register_entity(command_ids::CLOSE_TAB, this, Self::close_active_tab)?;
        handlers.register_entity(command_ids::NEXT_TAB, this, |this, window, cx| {
            this.cycle_tab(true, window, cx);
        })?;
        handlers.register_entity(command_ids::PREVIOUS_TAB, this, |this, window, cx| {
            this.cycle_tab(false, window, cx);
        })?;
        handlers.register_entity(command_ids::CLOSE_WINDOW, this, |this, window, cx| {
            this.close(closing::AfterClose::CloseWindow, window, cx);
        })?;
        handlers.register_entity(command_ids::FOCUS_OUTLINE, this, Self::focus_outline)?;
        handlers.register_entity(command_ids::OPEN_MENU_BAR, this, |this, window, cx| {
            this.toggle_menu(0, true, window, cx);
        })?;
        handlers.register_entity(command_ids::TOGGLE_OUTLINE, this, |this, _, cx| {
            this.toggle_outline(cx);
        })?;
        Self::register_terminal_handlers(handlers, this)?;
        Self::register_group_handlers(handlers, this)?;
        handlers.register_entity(command_ids::FIND, this, |this, window, cx| {
            this.open_find(false, window, cx);
        })?;
        handlers.register_entity(command_ids::FIND_REPLACE, this, |this, window, cx| {
            this.open_find(true, window, cx);
        })?;
        handlers.register_entity(command_ids::FIND_NEXT, this, |this, window, cx| {
            this.find_next(true, window, cx);
        })?;
        handlers.register_entity(command_ids::FIND_PREVIOUS, this, |this, window, cx| {
            this.find_next(false, window, cx);
        })?;
        handlers.register_entity(command_ids::NEW_FILE, this, Self::new_file)?;
        handlers.register_entity(editor::SAVE, this, |this, window, cx| {
            if let Some(editor) = this.active_editor() {
                this.save_editor(&editor, window, cx).detach();
            }
        })?;
        for id in editor::command_ids().filter(|id| *id != editor::SAVE) {
            handlers.register_entity(id, this, move |this, window, cx| {
                if let Some(editor) = this.active_editor() {
                    editor.update(cx, |editor, cx| editor.run_command(id, window, cx));
                }
            })?;
        }
        Ok(())
    }

    /// Editor groups, the tab menu and the popovers.
    fn register_group_handlers(
        handlers: &mut CommandHandlers,
        this: &WeakEntity<Self>,
    ) -> Result<(), RegistryError> {
        handlers.register_entity(command_ids::SPLIT_EDITOR, this, Self::split_editor)?;
        handlers.register_entity(
            command_ids::FOCUS_OTHER_GROUP,
            this,
            Self::focus_other_group,
        )?;
        handlers.register_entity(command_ids::SHOW_TAB_MENU, this, Self::show_tab_menu)?;
        handlers.register_entity(
            command_ids::TOGGLE_NOTIFICATIONS,
            this,
            Self::toggle_notifications,
        )?;
        handlers.register_entity(command_ids::CLEAR_NOTIFICATIONS, this, |this, _, cx| {
            this.clear_notifications(cx);
        })?;
        handlers.register_entity(command_ids::PICK_PROJECT, this, Self::toggle_projects)?;
        handlers.register_entity(command_ids::CLOSE_OTHER_TABS, this, |this, window, cx| {
            let active = this.active_tab;
            this.close_tabs_where(move |index, _, _| Some(index) != active, window, cx);
        })?;
        handlers.register_entity(command_ids::CLOSE_SAVED_TABS, this, |this, window, cx| {
            this.close_tabs_where(|_, tab, cx| !tab.is_dirty(cx), window, cx);
        })?;
        handlers.register_entity(command_ids::CLOSE_ALL_TABS, this, |this, window, cx| {
            this.close_tabs_where(|_, _, _| true, window, cx);
        })?;
        Ok(())
    }

    /// Icons of files in tabs and breadcrumbs.
    pub fn set_file_icons(&mut self, icons: Arc<FileIcons>, cx: &mut Context<Self>) {
        self.file_icons = icons;
        cx.notify();
    }

    /// Shows `problems` in the Problems panel; its counts go to the
    /// status bar and the panel tab.
    pub fn set_problems(&mut self, problems: Entity<ProblemsView>, cx: &mut Context<Self>) {
        self.problems_watch = Some(cx.observe(&problems, |_, _, cx| cx.notify()));
        self.problems = Some(problems);
        cx.notify();
    }

    /// Shows `search` as the Search view of the sidebar.
    pub fn set_search(&mut self, search: Entity<SearchView>, cx: &mut Context<Self>) {
        self.search = Some(search);
        cx.notify();
    }

    /// Moves the focus to the query of the Search view.
    pub(crate) fn focus_search(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(search) = &self.search {
            search.update(cx, |search, cx| search.focus_query(window, cx));
        }
    }

    /// Errors and warnings of the project, if problems are shown.
    pub(crate) fn problem_counts(&self, cx: &App) -> Option<(usize, usize)> {
        self.problems
            .as_ref()
            .map(|problems| problems.read(cx).counts())
    }

    /// Switches the interface language.
    pub fn set_i18n(&mut self, i18n: Arc<I18n>, cx: &mut Context<Self>) {
        self.quick_open_context.i18n = i18n.clone();
        if let Some(panel) = &self.terminal {
            let i18n = i18n.clone();
            panel.update(cx, |panel, cx| panel.set_i18n(i18n, cx));
        }
        if let Some(search) = &self.search {
            let i18n = i18n.clone();
            search.update(cx, |search, cx| search.set_i18n(i18n, cx));
        }
        self.i18n = i18n;
        self.sync_outline(cx);
        cx.notify();
    }

    /// Replaces the keybindings shown in hints and the palette.
    pub fn set_keymap(&mut self, keymap: Arc<Keymap>, cx: &mut Context<Self>) {
        self.quick_open_context.keymap = keymap.clone();
        self.keymap = keymap;
        cx.notify();
    }

    /// Shows `workspace` as the open folder with its file tree (`None`:
    /// no folder).
    pub fn set_workspace(
        &mut self,
        workspace: Option<(WorkspaceInfo, Entity<FileTreeView>)>,
        cx: &mut Context<Self>,
    ) {
        let (workspace, file_tree) = workspace.unzip();
        self.workspace = workspace;
        self.file_tree = file_tree;
        self.sync_outline(cx);
        cx.notify();
    }

    /// Replaces the recent projects listed on the Welcome page.
    pub fn set_recent(&mut self, recent: Vec<RecentEntry>, cx: &mut Context<Self>) {
        self.recent = recent;
        cx.notify();
    }
    /// Label of the first shortcut bound to `command`, for hints.
    pub(crate) fn shortcut_label(&self, command: &str) -> Option<SharedString> {
        let id = CommandId::new(command).ok()?;
        self.keymap
            .bindings_for(&id)
            .next()
            .map(|binding| binding.keys.label(Platform::current()).into())
    }

    /// Localized title of `command`, for tooltips.
    pub(crate) fn command_title(&self, command: &str) -> SharedString {
        CommandId::new(command)
            .ok()
            .and_then(|id| self.quick_open_context.registry.get(&id))
            .map_or_else(
                || SharedString::from(command.to_owned()),
                |descriptor| self.i18n.text(&descriptor.title_message).into(),
            )
    }

    /// The open path dialog over a backdrop that closes it.
    fn path_prompt_dialog(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        self.path_prompt.as_ref().map(|open| {
            let size = &cx.theme().metrics.size;
            div()
                .id("path-prompt-backdrop")
                .absolute()
                .inset_0()
                .flex()
                .flex_col()
                .items_center()
                .pt(px(size.titlebar_height * 2.0))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| this.close_path_prompt(window, cx)),
                )
                .child(open.view.clone())
        })
    }

    pub(crate) fn text(&self, id: &str) -> SharedString {
        self.i18n.text(id).into()
    }
}

impl Render for Workbench {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_outline(window, cx);
        self.ensure_debug_sections(cx);
        self.ensure_extensions(window, cx);
        let title_bar = title_bar::render(self, window, cx);
        let activity_bar = activity_bar::render(self, cx);
        let sidebar_width = self.sidebar_width(window, cx);
        let sidebar = self
            .sidebar_visible
            .then(|| sidebar::render(self, sidebar_width, cx));
        let sidebar_handle = self
            .sidebar_visible
            .then(|| Self::resize_handle(ResizeEdge::Sidebar, cx));
        let maximized = self.panel_visible && self.panel_maximized;
        let editor = (!maximized).then(|| editor_area::render(self, cx));
        let panel_height = self.panel_height(window, cx);
        let panel = self
            .panel_visible
            .then(|| bottom_panel::render(self, panel_height, cx));
        let panel_handle = (self.panel_visible && !maximized)
            .then(|| Self::resize_handle(ResizeEdge::BottomPanel, cx));
        let picker_backdrop = self.shell_picker_backdrop(cx);
        let popover_backdrop = self.popover_backdrop(cx);
        let status_bar = status_bar::render(self, cx);
        let dialog = self.path_prompt_dialog(cx);
        // Catches clicks outside quick open; the box itself is drawn by the
        // title bar over its search field.
        let backdrop = self.quick_open.is_some().then(|| {
            div()
                .id("quick-open-backdrop")
                .absolute()
                .inset_0()
                // While quick open is open nothing below reacts to the
                // pointer: the wheel does not scroll the editor.
                .occlude()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| this.close_quick_open(window, cx)),
                )
        });
        // Below the title bar, so its menu buttons still switch menus.
        let menu_backdrop = self.menu.is_some().then(|| {
            div()
                .id("menu-backdrop")
                .absolute()
                .top(px(cx.theme().metrics.size.titlebar_height))
                .left_0()
                .right_0()
                .bottom_0()
                .occlude()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| this.close_menu(window, cx)),
                )
        });

        let theme = cx.theme();
        let metrics = &theme.metrics;
        let gap = metrics.size.panel_gap;
        let main = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .children(editor)
            .children(panel_handle)
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
                    .children(sidebar)
                    .children(sidebar_handle)
                    .child(main),
            );
        div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.colors.bg_app)
            .when_some(self.resize_cursor(), gpui::Styled::cursor)
            .on_mouse_move(cx.listener(Self::drag_resize))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.end_resize(cx)),
            )
            .child(title_bar)
            .child(body)
            .child(status_bar)
            .children(backdrop)
            .children(menu_backdrop)
            .children(picker_backdrop)
            .children(popover_backdrop)
            .children(dialog)
    }
}
