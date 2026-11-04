//! Root view of the main window: routes keystrokes and button clicks to
//! command owners and hosts the workbench.

mod files;
mod keyboard;
mod settings_sync;
mod workspace;

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use commands::CommandId;
use gpui::{
    AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement,
    KeyDownEvent, ParentElement as _, Render, SharedString, Styled as _, Task, Window, div, px,
};
use i18n::I18n;
use settings::SettingsStore;
use theme::ActiveTheme as _;
use workbench::{Dispatch, Workbench, command_ids};

use self::keyboard::{KeyOutcome, KeyboardDispatcher};
use crate::app_commands;
use crate::config::{self, LoadedConfig};
#[cfg(debug_assertions)]
use crate::gallery::Gallery;
use crate::live_config;

pub(crate) struct Shell {
    focus_handle: FocusHandle,
    /// Keymap, pending key sequence and keyboard layout.
    keyboard: KeyboardDispatcher,
    /// Tells `keyboard` when the keyboard layout changes.
    _layout_watch: Option<gpui::Subscription>,
    /// Reacts to files activated in the file tree.
    open_file_watch: Option<gpui::Subscription>,
    i18n: Arc<I18n>,
    settings: SettingsStore,
    fs: Arc<dyn fs::Fs>,
    /// Folder open in this window.
    workspace: Option<project::Workspace>,
    recent: project::RecentProjects,
    /// Icons of the file tree, registered by contributors at startup.
    file_icons: Arc<workbench::FileIcons>,
    /// Grammars of the language packs, by file.
    grammars: Arc<syntax::GrammarRegistry>,
    /// The file tree of the open folder.
    file_tree: Option<Entity<workbench::FileTreeView>>,
    /// `[files] hidden` the tree was last given.
    hidden: Vec<String>,
    /// Reacts to changes of the workspace settings file.
    workspace_watch: Option<gpui::Subscription>,
    /// Where `recent` is saved; `None` without a home directory.
    recent_file: Option<PathBuf>,
    /// Re-reads the configuration files when they change.
    _config_watch: Option<Task<()>>,
    workbench: Entity<Workbench>,
    #[cfg(debug_assertions)]
    gallery: Option<Entity<Gallery>>,
}

impl Shell {
    pub(crate) fn new(
        config: LoadedConfig,
        i18n: Arc<I18n>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let grammars = match crate::languages::grammar_registry() {
            Ok(registry) => Arc::new(registry),
            Err(error) => {
                tracing::error!("{error:#}");
                Arc::new(syntax::GrammarRegistry::new())
            }
        };
        let LoadedConfig {
            settings,
            keymap,
            commands: registry,
            fs,
            paths,
            recent,
            recent_file,
        } = config;
        let keymap = Arc::new(keymap);
        let chord_timeout = config::chord_timeout(&settings);
        let config_watch =
            paths.map(|paths| live_config::start(fs.clone(), paths, registry.clone(), window, cx));
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        let shell = cx.entity().downgrade();
        let dispatch: Dispatch = Rc::new(move |id, window, cx| {
            let id = id.clone();
            let _ = shell.update(cx, |shell, cx| shell.run_command(&id, window, cx));
        });
        let shell = cx.entity().downgrade();
        let open_path: workbench::OpenPath = Rc::new(move |path, window, cx| {
            let _ = shell.update(cx, |shell, cx| shell.open_folder(path, window, cx));
        });
        let workbench_i18n = i18n.clone();
        let workbench_keymap = keymap.clone();
        let entries = workspace::recent_entries(&recent);
        let file_icons = Arc::new(workbench::FileIcons::new());
        let workbench_icons = file_icons.clone();
        let workbench = cx.new(|cx| {
            let mut workbench = Workbench::new(
                workbench_i18n,
                dispatch,
                registry,
                workbench_keymap,
                open_path,
            );
            workbench.set_recent(entries, cx);
            workbench.set_file_icons(workbench_icons, cx);
            workbench
        });
        // Closing the window asks about unsaved files first.
        let closing = workbench.downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            closing
                .update(cx, |workbench, cx| workbench.should_close(window, cx))
                .unwrap_or(true)
        });
        let shell = cx.entity().downgrade();
        let layout_watch = cx.on_keyboard_layout_change(move |cx| {
            let _ = shell.update(cx, |shell, cx| {
                shell.keyboard.layout_changed(cx);
                tracing::debug!("keyboard layout changed");
            });
        });
        Self {
            focus_handle,
            keyboard: KeyboardDispatcher::new(keymap, chord_timeout, cx),
            _layout_watch: Some(layout_watch),
            open_file_watch: None,
            i18n,
            settings,
            fs,
            workspace: None,
            recent,
            file_icons,
            grammars,
            workspace_watch: None,
            file_tree: None,
            hidden: Vec::new(),
            recent_file,
            _config_watch: config_watch,
            workbench,
            #[cfg(debug_assertions)]
            gallery: None,
        }
    }

    /// Runs a command by id, whoever owns it.
    fn run_command(&mut self, id: &CommandId, window: &mut Window, cx: &mut Context<Self>) {
        tracing::debug!(command = %id, "running command");
        match id.as_str() {
            app_commands::QUIT => self
                .workbench
                .update(cx, |workbench, cx| workbench.quit(window, cx)),
            command_ids::OPEN_FOLDER => self.prompt_for_folder(window, cx),
            command_ids::OPEN_FOLDER_BY_PATH => self.show_path_prompt(None, None, window, cx),
            #[cfg(debug_assertions)]
            app_commands::TOGGLE_COMPONENT_GALLERY => self.toggle_gallery(window, cx),
            _ => {
                let handled = self
                    .workbench
                    .update(cx, |workbench, cx| workbench.run_command(id, window, cx));
                if !handled {
                    tracing::debug!(command = %id, "command is not implemented yet");
                }
            }
        }
    }

    #[cfg(debug_assertions)]
    fn toggle_gallery(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let title: SharedString = self.i18n.text("gallery-title").into();
        let showing = self.workbench.read(cx).has_view_tab(&title);
        let view = if showing {
            None
        } else {
            let i18n = self.i18n.clone();
            let gallery = self
                .gallery
                .get_or_insert_with(|| cx.new(|cx| Gallery::new(i18n, window, cx)))
                .clone();
            Some(gallery.into())
        };
        self.workbench.update(cx, |workbench, cx| {
            workbench.set_view_tab(title, view, window, cx);
        });
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match self.keyboard.key_down(event, window) {
            Some(KeyOutcome::Run(id)) => {
                cx.stop_propagation();
                self.run_command(&id, window, cx);
            }
            Some(KeyOutcome::Pending { chord_timeout }) => {
                cx.stop_propagation();
                if let Some(timeout) = chord_timeout {
                    self.start_chord_timer(timeout, window, cx);
                }
            }
            Some(KeyOutcome::NoMatch) | None => {}
        }
    }

    /// When the pending keys are also a complete shortcut (`Ctrl K` next to
    /// `Ctrl K, Ctrl G`), runs that shortcut unless another key arrives in
    /// time. No timer runs otherwise, so idle stays idle.
    fn start_chord_timer(
        &mut self,
        timeout: std::time::Duration,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let timer = cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(timeout).await;
            let _ = this.update_in(cx, |shell, window, cx| {
                if let Some(id) = shell.keyboard.chord_timed_out() {
                    shell.run_command(&id, window, cx);
                }
            });
        });
        self.keyboard.set_chord_timer(timer);
    }
}

impl Render for Shell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let content = div()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
            .size_full()
            .font_family(SharedString::from(theme.fonts.ui.clone()))
            .text_size(px(theme.metrics.font_size.base))
            .text_color(theme.colors.text_primary)
            .bg(theme.colors.bg_app)
            .child(self.workbench.clone());
        // Draws the resize frame and shadow when the window decorates itself
        // (Linux); a plain container elsewhere.
        gpui_component::window_border().child(content)
    }
}

/// The window title: the folder name with the product name, or just the
/// product name without a folder.
pub(crate) fn window_title(i18n: &I18n, workspace: Option<&project::Workspace>) -> String {
    match workspace {
        Some(workspace) => {
            let mut args = i18n::FluentArgs::new();
            args.set("name", workspace.name().to_owned());
            i18n.text_with("window-title-workspace", &args)
        }
        None => i18n.text("app-title"),
    }
}
