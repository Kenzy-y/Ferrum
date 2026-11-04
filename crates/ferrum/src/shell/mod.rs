//! Root view of the main window: routes keystrokes and button clicks to
//! command owners and hosts the workbench.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use commands::{CommandId, Keymap, KeymapMatcher, MatchResult};
use gpui::{
    AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement,
    KeyDownEvent, ParentElement as _, Render, SharedString, Styled as _, Task, Window, div, px,
};
use i18n::I18n;
use settings::{LayerKind, SettingsStore};
use theme::ActiveTheme as _;
use workbench::{Dispatch, Workbench, command_ids};

use crate::app_commands;

mod files;
mod keys;
mod workspace;
use crate::config::{self, LoadedConfig};
#[cfg(debug_assertions)]
use crate::gallery::Gallery;
use crate::live_config::{self, Reloaded, UserSettings};

pub(crate) struct Shell {
    focus_handle: FocusHandle,
    keymap: Arc<Keymap>,
    matcher: KeymapMatcher,
    /// Runs the pending sequence's own command if no further key arrives.
    chord_timer: Option<Task<()>>,
    /// Contexts of the keystroke that started the pending sequence.
    chord_contexts: Vec<String>,
    /// Keys of the active keyboard layout to the US keys of keybindings.
    layout_keys: commands::LayoutKeys,
    /// Re-reads `layout_keys` when the layout changes.
    _layout_watch: Option<gpui::Subscription>,
    /// Reacts to files activated in the file tree.
    open_file_watch: Option<gpui::Subscription>,
    chord_timeout: Duration,
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
        let shell = cx.entity().downgrade();
        let layout_watch = cx.on_keyboard_layout_change(move |cx| {
            let _ = shell.update(cx, |shell, cx| {
                shell.layout_keys = keys::layout_keys(cx);
                tracing::debug!("keyboard layout changed");
            });
        });
        Self {
            focus_handle,
            keymap,
            matcher: KeymapMatcher::new(),
            chord_timer: None,
            chord_contexts: Vec::new(),
            layout_keys: keys::layout_keys(cx),
            _layout_watch: Some(layout_watch),
            open_file_watch: None,
            chord_timeout,
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

    /// Applies configuration files that changed on disk: language, keyboard
    /// settings and keybindings take effect without a restart.
    pub(crate) fn apply_config(
        &mut self,
        reloaded: Reloaded,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let change = match reloaded.settings {
            UserSettings::Untouched => None,
            UserSettings::Present(layer) => Some(self.settings.set_layer(LayerKind::User, layer)),
            UserSettings::Absent => Some(self.settings.remove_layer(LayerKind::User)),
        };
        if let Some(change) = change
            && !change.is_empty()
        {
            self.settings_changed(window, cx);
        }
        if let Some(keymap) = reloaded.keymap {
            let keymap = Arc::new(keymap);
            self.keymap = keymap.clone();
            self.matcher.reset();
            self.chord_timer = None;
            self.workbench
                .update(cx, |workbench, cx| workbench.set_keymap(keymap, cx));
            tracing::info!("applied keymap changes");
        }
    }

    /// Re-applies everything that depends on settings.
    fn settings_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        config::report_settings(&self.settings);
        self.chord_timeout = config::chord_timeout(&self.settings);
        self.apply_language(window, cx);
        let editors: Vec<_> = self.workbench.read(cx).editors().cloned().collect();
        for editor in editors {
            editor.update(cx, |editor, cx| {
                // `[lang.<id>]` overrides apply to files of that language.
                let language = editor.language_id().map(str::to_owned);
                let language = language.as_deref();
                editor.set_settings(self.settings.get::<editor::EditorSettings>(language), cx);
                editor.set_syntax_settings(self.settings.get::<syntax::SyntaxSettings>(language));
            });
        }
        let hidden = self.settings.get::<project::FilesSettings>(None).hidden;
        if hidden != self.hidden
            && let Some(tree) = &self.file_tree
        {
            tree.update(cx, |tree, cx| tree.set_hidden(&hidden, cx));
            self.hidden = hidden;
        }
    }

    fn apply_language(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let i18n = match config::localization(&self.settings) {
            Ok(i18n) => i18n,
            Err(error) => {
                tracing::error!("{error:#}");
                return;
            }
        };
        if i18n.locale() == self.i18n.locale() {
            return;
        }
        let i18n = Arc::new(i18n);
        window.set_window_title(&window_title(&i18n, self.workspace.as_ref()));
        self.i18n = i18n.clone();
        self.workbench
            .update(cx, |workbench, cx| workbench.set_i18n(i18n, cx));
    }

    /// Runs a command by id, whoever owns it.
    fn run_command(&mut self, id: &CommandId, window: &mut Window, cx: &mut Context<Self>) {
        tracing::debug!(command = %id, "running command");
        match id.as_str() {
            app_commands::QUIT => cx.quit(),
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
        if event.is_held && !keys::repeats(&event.keystroke) {
            return;
        }
        self.chord_timer = None;
        let keystroke = keys::to_keystroke(&event.keystroke, &self.layout_keys);
        let contexts = keys::focus_contexts(window);
        let context_names: Vec<&str> = contexts.iter().map(String::as_str).collect();
        match self.matcher.push(&self.keymap, keystroke, &context_names) {
            MatchResult::Command(id) => {
                cx.stop_propagation();
                self.run_command(&id, window, cx);
            }
            MatchResult::Pending => {
                cx.stop_propagation();
                self.chord_contexts = contexts;
                self.start_chord_timer(window, cx);
            }
            MatchResult::NoMatch => {}
        }
    }
}

impl Shell {
    /// When the pending keys are also a complete shortcut (`Ctrl K` next to
    /// `Ctrl K, Ctrl G`), runs that shortcut unless another key arrives in
    /// time. No timer runs otherwise, so idle stays idle.
    fn start_chord_timer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let contexts: Vec<&str> = self.chord_contexts.iter().map(String::as_str).collect();
        if self
            .matcher
            .pending_command(&self.keymap, &contexts)
            .is_none()
        {
            return;
        }
        let timeout = self.chord_timeout;
        self.chord_timer = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(timeout).await;
            let _ = this.update_in(cx, |shell, window, cx| {
                shell.chord_timer = None;
                let contexts: Vec<&str> = shell.chord_contexts.iter().map(String::as_str).collect();
                let command = shell.matcher.pending_command(&shell.keymap, &contexts);
                shell.matcher.reset();
                if let Some(id) = command {
                    shell.run_command(&id, window, cx);
                }
            });
        }));
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
