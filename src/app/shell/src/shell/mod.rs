//! Root view of the main window: routes keystrokes and button clicks to
//! command owners and hosts the workbench.

mod config_files;
mod extensions;
mod files;
mod keyboard;
mod language_status;
mod problems;
mod search;
mod setting_writes;
mod settings_page;
mod settings_sync;
mod terminals;
mod window_sizes;
mod workspace;
mod zoom;

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use commands::{CommandId, RegistryError};
use gpui::{
    AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement,
    KeyDownEvent, ParentElement as _, Render, SharedString, Styled as _, Task, WeakEntity, Window,
    div, px,
};
use i18n::I18n;
use settings::SettingsStore;
use theme::ActiveTheme as _;
use workbench::{CommandHandlers, Dispatch, Workbench, command_ids};

use self::config_files::ConfigFile;
use self::keyboard::{KeyOutcome, KeyboardDispatcher};
use self::settings_page::SettingsPageTab;
use crate::app_commands;
use crate::config::{self, LoadedConfig};
#[cfg(debug_assertions)]
use crate::gallery::Gallery;
use crate::language_servers::LanguageServers;
use crate::live_config;

pub(crate) struct Shell {
    focus_handle: FocusHandle,
    /// Keymap, pending key sequence and keyboard layout.
    keyboard: KeyboardDispatcher,
    /// What runs for each command id in this window.
    handlers: Rc<CommandHandlers>,
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
    languages: Arc<language::LanguageRegistry>,
    /// Running language services of the open folder.
    language_servers: LanguageServers,
    /// Runs programs: language servers, toolchains.
    process: Arc<dyn process::ProcessRunner>,
    /// Finding the toolchain versions of the open folder.
    toolchain_tasks: Vec<Task<()>>,
    /// Where the user configuration lives; `None` without a home directory.
    config_paths: Option<settings::paths::ConfigPaths>,
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
    /// The Problems panel, fed by every running language service.
    problems: Entity<workbench::ProblemsView>,
    /// Opens what is activated in the Problems panel.
    _problems_open: gpui::Subscription,
    /// Pass each running service's diagnostics to `problems` and its
    /// state to the status bar.
    problem_watches: Vec<Task<()>>,
    /// The Search view of the sidebar.
    search: Entity<workbench::SearchView>,
    /// Run the view's searches and open its matches.
    _search_watches: [gpui::Subscription; 3],
    /// The search in progress; dropping it cancels it.
    running_search: Option<search::RunningSearch>,
    /// The terminals of the window.
    terminal: Entity<workbench::TerminalPanel>,
    /// Every command, for the Keyboard Shortcuts page.
    commands: Arc<commands::CommandRegistry>,
    /// Serializes writes of settings files from the UI.
    setting_writes: Arc<futures::lock::Mutex<()>>,
    /// The settings page, while open.
    settings_page: Option<settings_page::OpenSettingsPage>,
    /// Saves panel sizes the user drags to.
    _sizes_watch: gpui::Subscription,
    #[cfg(debug_assertions)]
    gallery: Option<Entity<Gallery>>,
}

/// Pause before each `FERRUM_DEBUG_COMMANDS` command, so the previous
/// one has rendered.
#[cfg(debug_assertions)]
const DEBUG_COMMAND_PAUSE_MS: u64 = 700;

/// Closing the window asks about unsaved files first.
fn ask_before_closing(workbench: &Entity<Workbench>, window: &mut Window, cx: &mut Context<Shell>) {
    let closing = workbench.downgrade();
    window.on_window_should_close(cx, move |window, cx| {
        closing
            .update(cx, |workbench, cx| workbench.should_close(window, cx))
            .unwrap_or(true)
    });
}

impl Shell {
    // Wiring of every part of the window in one place; split further only
    // when a part grows its own setup.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn new(
        config: LoadedConfig,
        i18n: Arc<I18n>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let languages = language_registry();
        let LoadedConfig {
            settings,
            keymap,
            commands: registry,
            fs,
            paths,
            recent,
            recent_file,
            window_state,
            window_state_file,
        } = config;
        let keymap = Arc::new(keymap);
        let chord_timeout = config::chord_timeout(&settings);
        let config_paths = paths.clone();
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
        let file_icons = Arc::new(file_icons(&languages));
        let process: Arc<dyn process::ProcessRunner> = Arc::new(process::RealProcessRunner);
        let workbench_icons = file_icons.clone();
        let page_commands = registry.clone();
        let workbench = cx.new(|cx| {
            let mut workbench = Workbench::new(
                workbench_i18n,
                dispatch,
                registry,
                workbench_keymap,
                open_path,
            );
            workbench.set_recent(entries, cx);
            workbench.set_panel_sizes(window_state, cx);
            workbench.set_file_icons(workbench_icons, cx);
            workbench.set_extensions(vec![extensions::sample(&i18n, true)], cx);
            workbench
        });
        ask_before_closing(&workbench, window, cx);
        let (problems, problems_open) =
            problems::create(&i18n, file_icons.clone(), &workbench, window, cx);
        let (search, search_watches) =
            search::create(&i18n, file_icons.clone(), &settings, &workbench, window, cx);
        let keyboard = KeyboardDispatcher::new(keymap, chord_timeout, cx);
        let sizes_watch = window_sizes::watch(fs.clone(), window_state_file, &workbench, cx);
        let terminal = terminals::create(&i18n, fs.clone(), &settings, &workbench, window, cx);
        let handlers = Rc::new(command_handlers(
            &cx.entity().downgrade(),
            &workbench.downgrade(),
        ));
        let shell = cx.entity().downgrade();
        let layout_watch = cx.on_keyboard_layout_change(move |cx| {
            let _ = shell.update(cx, Self::keyboard_layout_changed);
        });
        let mut shell = Self {
            focus_handle,
            keyboard,
            handlers,
            _layout_watch: Some(layout_watch),
            open_file_watch: None,
            i18n,
            settings,
            fs,
            workspace: None,
            recent,
            file_icons,
            languages,
            config_paths,
            language_servers: LanguageServers::new(process.clone()),
            process,
            toolchain_tasks: Vec::new(),
            workspace_watch: None,
            file_tree: None,
            hidden: Vec::new(),
            recent_file,
            _config_watch: config_watch,
            workbench,
            problems,
            _problems_open: problems_open,
            problem_watches: Vec::new(),
            search,
            _search_watches: search_watches,
            running_search: None,
            terminal,
            _sizes_watch: sizes_watch,
            commands: page_commands,
            setting_writes: Arc::default(),
            settings_page: None,
            #[cfg(debug_assertions)]
            gallery: None,
        };
        shell.start_language_status(cx);
        shell
    }

    /// Debug builds: runs the comma-separated command ids of the
    /// `FERRUM_DEBUG_COMMANDS` environment variable after startup, for
    /// looking at views without a keyboard (screenshots in development).
    #[cfg(debug_assertions)]
    pub(crate) fn run_debug_commands(window: &mut Window, cx: &mut Context<Self>) {
        let Ok(list) = std::env::var("FERRUM_DEBUG_COMMANDS") else {
            return;
        };
        cx.spawn_in(window, async move |shell, cx| {
            for id in list.split(',').map(str::trim).filter(|id| !id.is_empty()) {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(DEBUG_COMMAND_PAUSE_MS))
                    .await;
                // `folder:<path>` and `file:<path>` open them first.
                if let Some(path) = id.strip_prefix("folder:") {
                    let path = PathBuf::from(path);
                    let _ = shell
                        .update_in(cx, |shell, window, cx| shell.open_folder(path, window, cx));
                    continue;
                }
                if let Some(path) = id.strip_prefix("file:") {
                    let path = PathBuf::from(path);
                    let _ = shell.update_in(cx, |shell, window, cx| {
                        shell.open_file(path, None, window, cx);
                    });
                    continue;
                }
                let Ok(id) = CommandId::new(id) else {
                    tracing::warn!(id, "FERRUM_DEBUG_COMMANDS: not a command id");
                    continue;
                };
                let _ = shell.update_in(cx, |shell, window, cx| shell.run_command(&id, window, cx));
            }
        })
        .detach();
    }

    /// The active keyboard layout changed: shortcuts follow.
    fn keyboard_layout_changed(&mut self, cx: &mut Context<Self>) {
        self.keyboard.layout_changed(cx);
        tracing::debug!("keyboard layout changed");
    }

    /// Registers the handlers of the commands the shell owns.
    pub(crate) fn register_handlers(
        handlers: &mut CommandHandlers,
        this: &WeakEntity<Self>,
    ) -> Result<(), RegistryError> {
        handlers.register_entity(app_commands::QUIT, this, |this, window, cx| {
            this.workbench
                .update(cx, |workbench, cx| workbench.quit(window, cx));
        })?;
        handlers.register_entity(command_ids::OPEN_FOLDER, this, Self::prompt_for_folder)?;
        handlers.register_entity(
            command_ids::OPEN_FOLDER_BY_PATH,
            this,
            |this, window, cx| {
                this.show_path_prompt(None, None, window, cx);
            },
        )?;
        handlers.register_entity(command_ids::OPEN_SETTINGS, this, |this, window, cx| {
            this.open_settings_page(SettingsPageTab::Settings, window, cx);
        })?;
        handlers.register_entity(command_ids::OPEN_KEYBINDINGS, this, |this, window, cx| {
            this.open_settings_page(SettingsPageTab::Keybindings, window, cx);
        })?;
        handlers.register_entity(command_ids::OPEN_SETTINGS_FILE, this, |this, window, cx| {
            this.open_config_file(ConfigFile::Settings, window, cx);
        })?;
        handlers.register_entity(command_ids::OPEN_KEYMAP, this, |this, window, cx| {
            this.open_config_file(ConfigFile::Keymap, window, cx);
        })?;
        handlers.register_entity(command_ids::ZOOM_IN, this, |this, window, cx| {
            this.change_zoom(theme::zoom::zoom_in, window, cx);
        })?;
        handlers.register_entity(command_ids::ZOOM_OUT, this, |this, window, cx| {
            this.change_zoom(theme::zoom::zoom_out, window, cx);
        })?;
        handlers.register_entity(command_ids::ZOOM_RESET, this, |this, window, cx| {
            this.change_zoom(|_| theme::zoom::DEFAULT, window, cx);
        })?;
        #[cfg(debug_assertions)]
        handlers.register_entity(
            app_commands::TOGGLE_COMPONENT_GALLERY,
            this,
            Self::toggle_gallery,
        )?;
        Ok(())
    }

    /// Runs a command by id, whoever owns it. Handlers run right after the
    /// current update, because some of them update the shell itself.
    fn run_command(&mut self, id: &CommandId, window: &mut Window, cx: &mut Context<Self>) {
        tracing::debug!(command = %id, "running command");
        let handlers = self.handlers.clone();
        let id = id.clone();
        window.defer(cx, move |window, cx| {
            if !handlers.run(&id, window, cx) {
                tracing::debug!(command = %id, "command has no handler");
            }
        });
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

    /// Keys typed in a terminal: the keymap first, except keys that belong
    /// to the terminal's program; whatever is left goes on to the terminal.
    fn on_key_down_capture(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if keyboard::terminal_focused(window) && !self.keyboard.goes_to_terminal_program(event) {
            self.dispatch_key(event, window, cx);
        }
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        // A terminal's keys went through the keymap in the capture phase.
        if !keyboard::terminal_focused(window) {
            self.dispatch_key(event, window, cx);
        }
    }

    fn dispatch_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
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
            .capture_key_down(cx.listener(Self::on_key_down_capture))
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

/// The built-in language packs; empty (and logged) if one is broken.
fn language_registry() -> Arc<language::LanguageRegistry> {
    crate::languages::language_registry().map_or_else(
        |error| {
            tracing::error!("{error:#}");
            Arc::new(language::LanguageRegistry::new())
        },
        Arc::new,
    )
}

/// File icons the language packs declare, for their extensions and file
/// names.
fn file_icons(languages: &language::LanguageRegistry) -> workbench::FileIcons {
    let mut icons = workbench::FileIcons::new();
    for entry in languages.iter() {
        let Some(spec) = entry.file_icon() else {
            continue;
        };
        let icon = workbench::FileIcon::Letter {
            text: spec.letter.into(),
            color_token: spec.color_token,
        };
        for extension in entry.extensions() {
            icons.register_extension(extension, icon.clone());
        }
        for name in entry.file_names() {
            icons.register_name(name, icon.clone());
        }
    }
    icons
}

/// The handlers of every command in a window: the shell's own and the
/// workbench's (which include the editor commands).
fn command_handlers(
    shell: &WeakEntity<Shell>,
    workbench: &WeakEntity<Workbench>,
) -> CommandHandlers {
    let mut handlers = CommandHandlers::new();
    let registered = Shell::register_handlers(&mut handlers, shell)
        .and_then(|()| Workbench::register_handlers(&mut handlers, workbench));
    if let Err(error) = registered {
        tracing::error!("{error}");
    }
    handlers
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
