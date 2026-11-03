//! Loading settings and keymaps at startup.

use std::sync::Arc;

use commands::{CommandRegistry, Keymap, KeymapDiagnostic, Platform};
use fs::Fs;
use i18n::{I18n, LocalizationSettings};
use settings::paths::ConfigPaths;
use settings::{LayerKind, SettingsRegistry, SettingsStore, load};

/// Configuration ready for the UI.
pub(crate) struct LoadedConfig {
    pub(crate) settings: SettingsStore,
    pub(crate) keymap: Keymap,
}

/// Registers the settings of every crate linked into the application.
pub(crate) fn settings_registry() -> anyhow::Result<SettingsRegistry> {
    Ok(SettingsRegistry::builder()
        .register::<LocalizationSettings>()?
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
    let platform = Platform::current();
    let mut settings = SettingsStore::new(settings_registry);
    let mut keymap = Keymap::defaults(platform);

    if let Some(paths) = paths {
        let settings_path = paths.user_settings();
        match load::load_layer(fs.as_ref(), &settings_path).await {
            Ok(Some(layer)) => {
                settings.set_layer(LayerKind::User, layer);
                tracing::info!(path = %settings_path.display(), "loaded user settings");
            }
            Ok(None) => tracing::debug!(path = %settings_path.display(), "no user settings file"),
            Err(error) => tracing::warn!("{error}; using default settings"),
        }

        let keymap_path = paths.user_keymap();
        match load::read_config_file(fs.as_ref(), &keymap_path).await {
            Ok(Some(text)) => match keymap.apply(&text, platform) {
                Ok(()) => tracing::info!(path = %keymap_path.display(), "loaded user keymap"),
                Err(error) => {
                    tracing::warn!(path = %keymap_path.display(), "{error}; using default keymap");
                }
            },
            Ok(None) => tracing::debug!(path = %keymap_path.display(), "no user keymap file"),
            Err(error) => tracing::warn!("{error}; using default keymap"),
        }
    }

    for diagnostic in settings
        .unknown_namespaces()
        .into_iter()
        .chain(settings.check::<LocalizationSettings>())
    {
        tracing::warn!(?diagnostic, "settings problem");
    }
    for KeymapDiagnostic::UnknownCommand(binding) in keymap.diagnostics(&command_registry) {
        tracing::warn!(keys = %binding.keys, command = %binding.command, "keymap binds an unknown command");
    }

    LoadedConfig { settings, keymap }
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
