//! The IDE application: `main()` and wiring of the crates. Logic belongs in
//! the library crates; this crate only connects them.

// Release builds on Windows are GUI applications without a console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_commands;
mod config;
mod gallery;
mod logging;
mod shell;

use std::sync::Arc;

use anyhow::Context as _;
use gpui::{App, AppContext as _, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size};

use crate::config::LoadedConfig;
use crate::shell::Shell;

/// Initial size of the main window, in logical pixels.
const INITIAL_WINDOW_SIZE: (f32, f32) = (1280.0, 800.0);

fn main() -> anyhow::Result<()> {
    logging::init();
    tracing::info!(
        product = app_identity::PRODUCT_NAME,
        version = env!("CARGO_PKG_VERSION"),
        "starting"
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
    let (width, height) = INITIAL_WINDOW_SIZE;
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(width), px(height)),
            cx,
        ))),
        titlebar: Some(TitlebarOptions {
            title: Some(i18n.text("app-title").into()),
            ..TitlebarOptions::default()
        }),
        app_id: Some(app_identity::APP_ID.to_owned()),
        ..WindowOptions::default()
    };

    let keymap = Arc::new(loaded.keymap);
    let handlers = Arc::new(app_commands::handlers());
    let i18n = Arc::new(i18n);
    cx.open_window(options, |window, cx| {
        let shell = cx.new(|cx| Shell::new(keymap, handlers, i18n, window, cx));
        cx.new(|cx| gpui_component::Root::new(shell, window, cx))
    })
    .context("cannot open the main window")?;
    cx.activate(true);
    Ok(())
}
