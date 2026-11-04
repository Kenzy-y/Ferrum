//! The root view of a main window and the state it shares with its parts.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use commands::{CommandId, CommandRegistry, Keymap, Platform};
use gpui::{
    App, Context, Entity, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _,
    Render, SharedString, Styled as _, Window, WindowDecorations, WindowOptions, div, px,
};
use i18n::I18n;
use theme::ActiveTheme as _;

use crate::command_ids;
use crate::contrib::explorer::file_icons::FileIcons;
use crate::contrib::explorer::file_tree_view::FileTreeView;
use crate::contrib::outline;
use crate::contrib::quick_open::{QuickOpenContext, QuickOpenMode};
use crate::layout::{PanelKind, SidebarView};
use crate::overlays::{OpenPathPrompt, OpenQuickOpen};
use crate::parts::editor_area::{closing, tabs};
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
    pub(crate) open_path: OpenPath,
    pub(crate) path_prompt: Option<OpenPathPrompt>,
    pub(crate) workspace: Option<WorkspaceInfo>,
    pub(crate) file_tree: Option<Entity<FileTreeView>>,
    pub(crate) recent: Vec<RecentEntry>,
    pub(crate) view: SidebarView,
    pub(crate) sidebar_visible: bool,
    pub(crate) panel_visible: bool,
    pub(crate) panel: PanelKind,
    pub(crate) tabs: Vec<tabs::Tab>,
    pub(crate) active_tab: Option<usize>,
    pub(crate) file_icons: Arc<FileIcons>,
    pub(crate) outline: outline::OutlineSection,
    pub(crate) closing: closing::Closing,
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
            open_path,
            path_prompt: None,
            workspace: None,
            file_tree: None,
            recent: Vec::new(),
            view: SidebarView::Explorer,
            sidebar_visible: true,
            panel_visible: false,
            panel: PanelKind::Terminal,
            tabs: Vec::new(),
            active_tab: None,
            file_icons: Arc::new(FileIcons::new()),
            outline: outline::OutlineSection::default(),
            closing: closing::Closing::default(),
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
        match id.as_str() {
            command_ids::CLOSE_TAB => {
                self.close_active_tab(window, cx);
                return true;
            }
            command_ids::CLOSE_WINDOW => {
                self.close(closing::AfterClose::CloseWindow, window, cx);
                return true;
            }
            editor::SAVE => {
                if let Some(editor) = self.active_editor() {
                    self.save_editor(&editor, window, cx).detach();
                }
                return true;
            }
            command_ids::FOCUS_OUTLINE => {
                self.focus_outline(window, cx);
                return true;
            }
            command_ids::TOGGLE_OUTLINE => {
                self.toggle_outline(cx);
                return true;
            }
            command_ids::NEXT_TAB | command_ids::PREVIOUS_TAB => {
                self.cycle_tab(id.as_str() == command_ids::NEXT_TAB, window, cx);
                return true;
            }
            other if editor::is_editor_command(other) => {
                if let Some(editor) = self.active_editor() {
                    editor.update(cx, |editor, cx| editor.run_command(other, window, cx));
                }
                return true;
            }
            _ => {}
        }
        let handled = self.apply_command(id.as_str());
        if handled {
            cx.notify();
        }
        handled
    }

    /// Icons of files in tabs and breadcrumbs.
    pub fn set_file_icons(&mut self, icons: Arc<FileIcons>, cx: &mut Context<Self>) {
        self.file_icons = icons;
        cx.notify();
    }

    /// Switches the interface language.
    pub fn set_i18n(&mut self, i18n: Arc<I18n>, cx: &mut Context<Self>) {
        self.quick_open_context.i18n = i18n.clone();
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

    pub(crate) fn text(&self, id: &str) -> SharedString {
        self.i18n.text(id).into()
    }
}

impl Render for Workbench {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_outline(window, cx);
        let title_bar = title_bar::render(self, window, cx);
        let activity_bar = activity_bar::render(self, cx);
        let sidebar = self.sidebar_visible.then(|| sidebar::render(self, cx));
        let editor = editor_area::render(self, cx);
        let panel = self.panel_visible.then(|| bottom_panel::render(self, cx));
        let status_bar = status_bar::render(self, cx);
        let dialog = self.path_prompt.as_ref().map(|open| {
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
        });
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
            .children(backdrop)
            .children(dialog)
    }
}
