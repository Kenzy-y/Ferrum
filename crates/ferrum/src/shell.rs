//! Root view of the main window: routes keystrokes and button clicks to
//! command owners and hosts the workbench.

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
use workbench::{Dispatch, Workbench};

use crate::app_commands;
use crate::config::{self, LoadedConfig};
#[cfg(debug_assertions)]
use crate::gallery::Gallery;
use crate::live_config::{self, Reloaded, UserSettings};

/// Keybinding context active everywhere in the main window.
const WORKBENCH_CONTEXT: &str = "workbench";

pub(crate) struct Shell {
    focus_handle: FocusHandle,
    keymap: Arc<Keymap>,
    matcher: KeymapMatcher,
    /// Runs the pending sequence's own command if no further key arrives.
    chord_timer: Option<Task<()>>,
    chord_timeout: Duration,
    i18n: Arc<I18n>,
    settings: SettingsStore,
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
        let LoadedConfig {
            settings,
            keymap,
            commands: registry,
            fs,
            paths,
        } = config;
        let keymap = Arc::new(keymap);
        let chord_timeout = config::chord_timeout(&settings);
        let config_watch =
            paths.map(|paths| live_config::start(fs, paths, registry.clone(), window, cx));
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        let shell = cx.entity().downgrade();
        let dispatch: Dispatch = Rc::new(move |id, window, cx| {
            let id = id.clone();
            let _ = shell.update(cx, |shell, cx| shell.run_command(&id, window, cx));
        });
        let workbench_i18n = i18n.clone();
        let workbench_keymap = keymap.clone();
        let workbench =
            cx.new(|_| Workbench::new(workbench_i18n, dispatch, registry, workbench_keymap));
        Self {
            focus_handle,
            keymap,
            matcher: KeymapMatcher::new(),
            chord_timer: None,
            chord_timeout,
            i18n,
            settings,
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
            config::report_settings(&self.settings);
            self.chord_timeout = config::chord_timeout(&self.settings);
            self.apply_language(window, cx);
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
        window.set_window_title(&i18n.text("app-title"));
        self.i18n = i18n.clone();
        self.workbench
            .update(cx, |workbench, cx| workbench.set_i18n(i18n, cx));
    }

    /// Runs a command by id, whoever owns it.
    fn run_command(&mut self, id: &CommandId, window: &mut Window, cx: &mut Context<Self>) {
        tracing::debug!(command = %id, "running command");
        match id.as_str() {
            app_commands::QUIT => cx.quit(),
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
        let showing = self.workbench.read(cx).has_editor_content();
        let content = if showing {
            None
        } else {
            let i18n = self.i18n.clone();
            let gallery = self
                .gallery
                .get_or_insert_with(|| cx.new(|cx| Gallery::new(i18n, window, cx)))
                .clone();
            Some(workbench::EditorContent {
                title: self.i18n.text("gallery-title").into(),
                view: gallery.into(),
            })
        };
        self.workbench.update(cx, |workbench, cx| {
            workbench.set_editor_content(content, cx);
        });
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.is_held {
            return;
        }
        self.chord_timer = None;
        let keystroke = to_keystroke(&event.keystroke);
        match self
            .matcher
            .push(&self.keymap, keystroke, &[WORKBENCH_CONTEXT])
        {
            MatchResult::Command(id) => {
                cx.stop_propagation();
                self.run_command(&id, window, cx);
            }
            MatchResult::Pending => {
                cx.stop_propagation();
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
        if self
            .matcher
            .pending_command(&self.keymap, &[WORKBENCH_CONTEXT])
            .is_none()
        {
            return;
        }
        let timeout = self.chord_timeout;
        self.chord_timer = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(timeout).await;
            let _ = this.update_in(cx, |shell, window, cx| {
                shell.chord_timer = None;
                let command = shell
                    .matcher
                    .pending_command(&shell.keymap, &[WORKBENCH_CONTEXT]);
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

/// Converts a GPUI keystroke into the keymap representation.
fn to_keystroke(keystroke: &gpui::Keystroke) -> commands::Keystroke {
    let gpui::Modifiers {
        control,
        alt,
        shift,
        platform,
        ..
    } = keystroke.modifiers;
    commands::Keystroke {
        modifiers: commands::Modifiers {
            control,
            alt,
            shift,
            platform,
        },
        key: keystroke.key.to_ascii_lowercase(),
    }
}
