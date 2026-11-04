//! Loading settings and keymaps, at startup and when their files change.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use commands::{CommandRegistry, KeyboardSettings, Keymap, KeymapDiagnostic, Platform};
use fs::Fs;
use i18n::{I18n, LocalizationSettings};
use project::{FilesSettings, ProjectSettings, RECENT_PROJECTS_FILE_NAME, RecentProjects};
use settings::paths::ConfigPaths;
use settings::{LayerKind, SettingsLayer, SettingsRegistry, SettingsStore, load};

/// Configuration ready for the UI.
pub(crate) struct LoadedConfig {
    pub(crate) settings: SettingsStore,
    pub(crate) keymap: Keymap,
    pub(crate) commands: Arc<CommandRegistry>,
    pub(crate) fs: Arc<dyn Fs>,
    pub(crate) paths: Option<ConfigPaths>,
    pub(crate) recent: RecentProjects,
    pub(crate) recent_file: Option<PathBuf>,
}

/// Registers the settings of every crate linked into the application.
pub(crate) fn settings_registry() -> anyhow::Result<SettingsRegistry> {
    Ok(SettingsRegistry::builder()
        .register::<LocalizationSettings>()?
        .register::<KeyboardSettings>()?
        .register::<ProjectSettings>()?
        .register::<FilesSettings>()?
        .build())
}

/// Reads the user settings and keymap. Problems are logged and never stop
/// the IDE: whatever cannot be loaded falls back to the defaults.
///
/// Does file I/O through `fs`: poll on a background executor.
pub(crate) async fn load(
    fs: Arc<dyn Fs>,
    paths: Option<ConfigPaths>,
    settings_registry: Arc<SettingsRegistry>,
    command_registry: Arc<CommandRegistry>,
) -> LoadedConfig {
    let mut settings = SettingsStore::new(settings_registry);
    if let Some(paths) = &paths
        && let Some(layer) = read_user_settings(fs.as_ref(), paths).await
    {
        settings.set_layer(LayerKind::User, layer);
    }
    report_settings(&settings);
    let keymap = read_keymap(fs.as_ref(), paths.as_ref(), &command_registry).await;
    let limit = settings.get::<ProjectSettings>(None).recent_projects_limit;
    let recent_file = paths
        .as_ref()
        .map(|paths| paths.data_dir().join(RECENT_PROJECTS_FILE_NAME));
    let recent = match &recent_file {
        Some(file) => RecentProjects::load(fs.as_ref(), file, limit).await,
        None => RecentProjects::new(limit),
    };
    LoadedConfig {
        settings,
        keymap,
        commands: command_registry,
        fs,
        paths,
        recent,
        recent_file,
    }
}

/// The user settings layer, or `None` if the file is missing or invalid
/// (invalid files are logged; the defaults apply).
pub(crate) async fn read_user_settings(fs: &dyn Fs, paths: &ConfigPaths) -> Option<SettingsLayer> {
    let path = paths.user_settings();
    match load::load_layer(fs, &path).await {
        Ok(Some(layer)) => {
            tracing::info!(path = %path.display(), "loaded user settings");
            Some(layer)
        }
        Ok(None) => {
            tracing::debug!(path = %path.display(), "no user settings file");
            None
        }
        Err(error) => {
            tracing::warn!("{error}; using default settings");
            None
        }
    }
}

/// Default keymap of the platform with the user keymap on top. An invalid
/// user keymap is logged and ignored as a whole.
pub(crate) async fn read_keymap(
    fs: &dyn Fs,
    paths: Option<&ConfigPaths>,
    command_registry: &CommandRegistry,
) -> Keymap {
    let platform = Platform::current();
    let mut keymap = Keymap::defaults(platform);
    #[cfg(debug_assertions)]
    if let Err(error) = keymap.apply(crate::app_commands::DEBUG_KEYMAP, platform) {
        tracing::error!(%error, "debug keymap is invalid");
    }
    if let Some(paths) = paths {
        let path = paths.user_keymap();
        match load::read_config_file(fs, &path).await {
            Ok(Some(text)) => match keymap.apply(&text, platform) {
                Ok(()) => tracing::info!(path = %path.display(), "loaded user keymap"),
                Err(error) => {
                    tracing::warn!(path = %path.display(), "{error}; using default keymap");
                }
            },
            Ok(None) => tracing::debug!(path = %path.display(), "no user keymap file"),
            Err(error) => tracing::warn!("{error}; using default keymap"),
        }
    }
    for KeymapDiagnostic::UnknownCommand(binding) in keymap.diagnostics(command_registry) {
        tracing::warn!(keys = %binding.keys, command = %binding.command, "keymap binds an unknown command");
    }
    keymap
}

/// Logs problems in the settings.
pub(crate) fn report_settings(settings: &SettingsStore) {
    for diagnostic in settings
        .unknown_namespaces()
        .into_iter()
        .chain(settings.check::<LocalizationSettings>())
        .chain(settings.check::<KeyboardSettings>())
        .chain(settings.check::<ProjectSettings>())
        .chain(settings.check::<FilesSettings>())
    {
        tracing::warn!(?diagnostic, "settings problem");
    }
}

/// Localized strings in the language chosen by the settings, the system or
/// the default, in that order.
pub(crate) fn localization(settings: &SettingsStore) -> anyhow::Result<I18n> {
    let configured = settings.get::<LocalizationSettings>(None).language;
    let system = i18n::system_locale();
    let locale = i18n::resolve_locale(configured.as_deref(), system.as_deref());
    tracing::info!(locale, ?configured, ?system, "selected UI language");
    Ok(I18n::new(locale)?)
}

/// How long to wait for the next key of an ambiguous sequence.
pub(crate) fn chord_timeout(settings: &SettingsStore) -> Duration {
    Duration::from_millis(settings.get::<KeyboardSettings>(None).chord_timeout_ms)
}
