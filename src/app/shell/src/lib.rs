//! The application shell: starts GPUI, loads the configuration, opens the
//! main window and hosts its root view, which routes keystrokes and
//! commands to their owners. The `ferrum` binary only calls [`run`].
//!
//! - `shell/` — the root view of a window: keyboard routing, settings
//!   sync, opening folders and files.
//! - `config`, `live_config` — loading and live reloading of
//!   `settings.toml` and `keymap.toml`.
//! - `app_commands` — the application's own commands and the registry of
//!   every command; `languages` — the built-in language packs.

mod app_commands;
mod config;
mod crash_report;
// Development aid: the component gallery exists only in debug builds.
#[cfg(debug_assertions)]
mod gallery;
mod language_servers;
mod languages;
mod live_config;
mod shell;

use std::sync::Arc;

use anyhow::Context as _;
use gpui::{App, AppContext as _, Bounds, WindowBounds, px, size};

use crate::config::LoadedConfig;
use crate::shell::Shell;
use workbench::Workbench;

/// Initial size of the main window, in logical pixels.
const INITIAL_WINDOW_SIZE: (f32, f32) = (1280.0, 800.0);

/// Runs the application until its last window closes.
///
/// # Errors
///
/// Fails if the built-in settings or commands cannot be registered.
pub fn run() -> anyhow::Result<()> {
    crash_report::install(
        settings::paths::ConfigPaths::from_platform().map(|paths| paths.data_dir().to_path_buf()),
    );
    let settings_registry = Arc::new(config::settings_registry()?);
    let command_registry = Arc::new(app_commands::registry()?);
    let fs: Arc<dyn fs::Fs> = Arc::new(fs::RealFs);
    let config_paths = settings::paths::ConfigPaths::from_platform();
    if config_paths.is_none() {
        tracing::warn!("no home directory: user settings and keymap are not loaded");
    }

    gpui_platform::application()
        .with_assets(ui::Assets)
        .run(move |cx: &mut App| {
            gpui_component::init(cx);
            editor::init(cx);
            if let Err(error) = theme::register_fonts(cx) {
                tracing::error!("{error}");
            }
            match theme::Theme::built_in(theme::DEFAULT_THEME, theme::Density::default()) {
                Ok(active) => theme::set_theme(active, cx),
                Err(error) => {
                    tracing::error!("{error}");
                    cx.quit();
                    return;
                }
            }
            ui::sync_component_theme(cx);
            cx.on_window_closed(|cx, _window| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            // Configuration files are read in the background; the window opens
            // as soon as they are loaded, so it starts with the right language
            // and keymap without blocking the UI thread.
            let loading = cx.background_executor().spawn(config::load(
                fs,
                config_paths,
                settings_registry,
                command_registry,
            ));
            cx.spawn(async move |cx| {
                let loaded = loading.await;
                cx.update(|cx| {
                    if let Err(error) = open_main_window(loaded, cx) {
                        tracing::error!("{error:#}");
                        cx.quit();
                    }
                });
            })
            .detach();
        });
    Ok(())
}

fn open_main_window(loaded: LoadedConfig, cx: &mut App) -> anyhow::Result<()> {
    let _span = tracing::info_span!("open_main_window").entered();
    let i18n = config::localization(&loaded.settings)?;
    let zoom = loaded.settings.get::<theme::AppearanceSettings>(None).zoom;
    match (*theme::shared_theme(cx)).clone().with_zoom(zoom) {
        Ok(zoomed) => {
            theme::set_theme(zoomed, cx);
            ui::sync_component_theme(cx);
        }
        Err(error) => tracing::error!("{error}"),
    }
    let (width, height) = INITIAL_WINDOW_SIZE;
    let mut options = Workbench::window_options();
    // The workbench draws its own title bar; the platform still shows the
    // title in the taskbar and window switcher.
    if let Some(titlebar) = options.titlebar.as_mut() {
        titlebar.title = Some(i18n.text("app-title").into());
    }
    options.window_bounds = Some(WindowBounds::Windowed(Bounds::centered(
        None,
        size(px(width), px(height)),
        cx,
    )));
    options.app_id = Some(app_identity::APP_ID.to_owned());

    let i18n = Arc::new(i18n);
    cx.open_window(options, |window, cx| {
        let shell = cx.new(|cx| Shell::new(loaded, i18n, window, cx));
        #[cfg(debug_assertions)]
        shell.update(cx, |_, cx| Shell::run_debug_commands(window, cx));
        cx.new(|cx| gpui_component::Root::new(shell, window, cx))
    })
    .context("cannot open the main window")?;
    cx.activate(true);
    Ok(())
}
