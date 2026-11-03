//! Locations of configuration files.

use std::path::{Path, PathBuf};

/// File name of the settings file in the user and workspace configuration
/// directories.
pub const SETTINGS_FILE_NAME: &str = "settings.toml";

/// File name of the user keymap in the configuration directory.
pub const KEYMAP_FILE_NAME: &str = "keymap.toml";

/// Per-user configuration locations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigPaths {
    config_dir: PathBuf,
}

impl ConfigPaths {
    /// The platform's per-user configuration directory for this product:
    ///
    /// - Windows: `%APPDATA%\<Application>\config`;
    /// - macOS: `~/Library/Application Support/<Application>`;
    /// - Linux: `$XDG_CONFIG_HOME/<lowercase name>` (default `~/.config`).
    ///
    /// Names come from `app_identity`. `None` if the platform does not report
    /// a home directory.
    pub fn from_platform() -> Option<Self> {
        directories::ProjectDirs::from_path(PathBuf::from(product_dir_name()))
            .map(|dirs| Self::new(dirs.config_dir().to_path_buf()))
    }

    /// Uses `config_dir` as the configuration directory.
    pub fn new(config_dir: PathBuf) -> Self {
        Self { config_dir }
    }

    /// The configuration directory.
    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    /// The user `settings.toml`.
    pub fn user_settings(&self) -> PathBuf {
        self.config_dir.join(SETTINGS_FILE_NAME)
    }

    /// The user `keymap.toml`.
    pub fn user_keymap(&self) -> PathBuf {
        self.config_dir.join(KEYMAP_FILE_NAME)
    }
}

/// Name of the per-user product directory: capitalized where the platform
/// shows it to users (Windows, macOS), lowercase on Linux.
fn product_dir_name() -> &'static str {
    if cfg!(any(target_os = "windows", target_os = "macos")) {
        app_identity::APPLICATION
    } else {
        app_identity::LOWERCASE_NAME
    }
}

/// The workspace settings file for the workspace rooted at `root`.
pub fn workspace_settings(root: &Path) -> PathBuf {
    root.join(app_identity::WORKSPACE_DIR_NAME)
        .join(SETTINGS_FILE_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_config_dir_is_one_level_deep() {
        let Some(paths) = ConfigPaths::from_platform() else {
            return; // No home directory in this environment.
        };
        let components: Vec<_> = paths
            .config_dir()
            .components()
            .rev()
            .take(2)
            .map(|component| component.as_os_str().to_string_lossy().into_owned())
            .collect();
        if cfg!(target_os = "windows") {
            assert_eq!(components, ["config", app_identity::APPLICATION]);
        } else if cfg!(target_os = "macos") {
            assert_eq!(
                components,
                [app_identity::APPLICATION, "Application Support"]
            );
        } else {
            assert_eq!(components[0], app_identity::LOWERCASE_NAME);
        }
    }

    #[test]
    fn file_locations() {
        let paths = ConfigPaths::new(PathBuf::from("config"));
        assert_eq!(
            paths.user_settings(),
            Path::new("config").join("settings.toml")
        );
        assert_eq!(paths.user_keymap(), Path::new("config").join("keymap.toml"));
        assert_eq!(
            workspace_settings(Path::new("project")),
            Path::new("project")
                .join(app_identity::WORKSPACE_DIR_NAME)
                .join("settings.toml")
        );
    }
}
