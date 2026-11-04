//! Re-reading `settings.toml` and `keymap.toml` when they change on disk.

use std::path::Path;
use std::sync::Arc;

use commands::{CommandRegistry, Keymap};
use fs::{Fs, FsEvent, FsEventKind, WatchFilter, WatchMode};
use futures::StreamExt as _;
use gpui::{Context, Task, Window};
use settings::SettingsLayer;
use settings::paths::{ConfigPaths, KEYMAP_FILE_NAME, SETTINGS_FILE_NAME};

use crate::config;
use crate::shell::Shell;

/// Which configuration files a batch of events touched.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Touched {
    pub(crate) settings: bool,
    pub(crate) keymap: bool,
}

impl Touched {
    /// Classifies a batch. A rescan touches everything.
    pub(crate) fn from_events(events: &[FsEvent]) -> Self {
        let mut touched = Self::default();
        for event in events {
            if event.kind == FsEventKind::Rescan {
                return Self {
                    settings: true,
                    keymap: true,
                };
            }
            match file_name(&event.path) {
                Some(SETTINGS_FILE_NAME) => touched.settings = true,
                Some(KEYMAP_FILE_NAME) => touched.keymap = true,
                _ => {}
            }
        }
        touched
    }

    fn any(self) -> bool {
        self.settings || self.keymap
    }
}

fn file_name(path: &Path) -> Option<&str> {
    path.file_name().and_then(|name| name.to_str())
}

/// The user settings layer after a change.
pub(crate) enum UserSettings {
    /// The file did not change.
    Untouched,
    /// The file changed and parsed.
    Present(SettingsLayer),
    /// The file is gone or invalid: defaults apply.
    Absent,
}

/// Freshly read configuration.
pub(crate) struct Reloaded {
    pub(crate) settings: UserSettings,
    /// The new keymap, if `keymap.toml` changed.
    pub(crate) keymap: Option<Keymap>,
}

/// Watches the configuration directory and hands every change to
/// [`Shell::apply_config`]. The directory is created if missing so that a
/// settings file created later is noticed.
pub(crate) fn start(
    fs: Arc<dyn Fs>,
    paths: ConfigPaths,
    commands: Arc<CommandRegistry>,
    window: &mut Window,
    cx: &mut Context<Shell>,
) -> Task<()> {
    cx.spawn_in(window, async move |shell, cx| {
        let directory = paths.config_dir().to_path_buf();
        // Only the two configuration files matter; editors' temporary files
        // and everything else are dropped on the watcher thread.
        let filter: Arc<dyn WatchFilter> = Arc::new(|path: &Path| {
            !matches!(file_name(path), Some(SETTINGS_FILE_NAME | KEYMAP_FILE_NAME))
        });
        let watch_fs = fs.clone();
        let watcher = cx
            .background_executor()
            .spawn(async move {
                watch_fs.create_dir_all(&directory).await?;
                watch_fs.watch(&directory, WatchMode::NonRecursive, filter)
            })
            .await;
        let mut watcher = match watcher {
            Ok(watcher) => watcher,
            Err(error) => {
                tracing::warn!("{error}; settings changes need a restart");
                return;
            }
        };
        while let Some(batch) = watcher.events().next().await {
            let touched = Touched::from_events(&batch);
            if !touched.any() {
                continue;
            }
            let read_fs = fs.clone();
            let read_paths = paths.clone();
            let read_commands = commands.clone();
            let reloaded = cx
                .background_executor()
                .spawn(async move {
                    let settings = if touched.settings {
                        match config::read_user_settings(read_fs.as_ref(), &read_paths).await {
                            Some(layer) => UserSettings::Present(layer),
                            None => UserSettings::Absent,
                        }
                    } else {
                        UserSettings::Untouched
                    };
                    let keymap = if touched.keymap {
                        Some(
                            config::read_keymap(
                                read_fs.as_ref(),
                                Some(&read_paths),
                                &read_commands,
                            )
                            .await,
                        )
                    } else {
                        None
                    };
                    Reloaded { settings, keymap }
                })
                .await;
            if shell
                .update_in(cx, |shell, window, cx| {
                    shell.apply_config(reloaded, window, cx);
                })
                .is_err()
            {
                return;
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn event(path: &str, kind: FsEventKind) -> FsEvent {
        FsEvent {
            path: PathBuf::from(path),
            kind,
        }
    }

    #[test]
    fn classifies_configuration_files() {
        let touched = Touched::from_events(&[
            event("/config/settings.toml", FsEventKind::Changed),
            event("/config/.settings.toml.42.tmp", FsEventKind::Created),
        ]);
        assert_eq!(
            touched,
            Touched {
                settings: true,
                keymap: false
            }
        );
        assert!(Touched::from_events(&[event("/config/keymap.toml", FsEventKind::Removed)]).keymap);
        assert!(!Touched::from_events(&[event("/config/other.toml", FsEventKind::Changed)]).any());
    }

    #[test]
    fn rescan_reloads_everything() {
        let touched = Touched::from_events(&[event("/config", FsEventKind::Rescan)]);
        assert!(touched.settings && touched.keymap);
    }
}
