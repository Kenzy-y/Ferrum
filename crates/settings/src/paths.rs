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
    /// The platform's per-user configuration directory for this product
    /// (for example `%APPDATA%\<org>\<app>\config` on Windows,
    /// `~/.config/<app>` on Linux, `~/Library/Application Support/<id>` on
    /// macOS). `None` if the platform does not report a home directory.
    pub fn from_platform() -> Option<Self> {
        directories::ProjectDirs::from(
            app_identity::QUALIFIER,
            app_identity::ORGANIZATION,
            app_identity::APPLICATION,
        )
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

/// The workspace settings file for the workspace rooted at `root`.
pub fn workspace_settings(root: &Path) -> PathBuf {
    root.join(app_identity::WORKSPACE_DIR_NAME)
        .join(SETTINGS_FILE_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;

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
