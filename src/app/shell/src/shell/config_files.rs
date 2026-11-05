//! Opening the user's `settings.toml` and `keymap.toml` in an editor
//! (`preferences.openSettings`, `preferences.openKeymap`). A missing file
//! is created first, with a comment saying what it is; changes apply as
//! soon as the file is saved (`live_config`).

use std::fmt::Write as _;
use std::path::Path;

use fs::FsError;
use gpui::{Context, Window};

use super::Shell;

/// Which configuration file to open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ConfigFile {
    Settings,
    Keymap,
}

impl Shell {
    /// Opens `file` in an editor, creating it if it does not exist.
    pub(super) fn open_config_file(
        &mut self,
        file: ConfigFile,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(paths) = &self.config_paths else {
            tracing::warn!("no home directory: there is no user configuration to open");
            return;
        };
        let (path, header) = match file {
            ConfigFile::Settings => (
                paths.user_settings(),
                self.i18n.text("config-settings-header"),
            ),
            ConfigFile::Keymap => (paths.user_keymap(), self.i18n.text("config-keymap-header")),
        };
        let fs = self.fs.clone();
        cx.spawn_in(window, async move |shell, cx| {
            let created_path = path.clone();
            let created = cx
                .background_executor()
                .spawn(async move { create_if_missing(fs.as_ref(), &created_path, &header).await })
                .await;
            if let Err(error) = created {
                tracing::error!(%error, path = %path.display(), "cannot create the configuration file");
                return;
            }
            let _ = shell.update_in(cx, |shell, window, cx| {
                shell.open_file(path, None, window, cx);
            });
        })
        .detach();
    }
}

/// Writes `header` (a TOML comment) to `path` unless the file exists.
async fn create_if_missing(fs: &dyn fs::Fs, path: &Path, header: &str) -> Result<(), FsError> {
    if fs.metadata(path).await?.is_some() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs.create_dir_all(parent).await?;
    }
    let mut text = String::new();
    for line in header.lines() {
        let _ = writeln!(text, "# {line}");
    }
    fs.write(path, text.as_bytes()).await
}

#[cfg(test)]
mod tests {
    use fs::{FakeFs, Fs as _};
    use futures::executor::block_on;

    use super::*;

    #[test]
    fn creates_a_commented_file_and_keeps_an_existing_one() {
        let fs = FakeFs::new();
        let path = Path::new("/config/settings.toml");
        block_on(create_if_missing(&fs, path, "Settings.\n[editor]")).unwrap();
        let text = block_on(fs.read_to_string(path)).unwrap();
        assert_eq!(text, "# Settings.\n# [editor]\n");
        // Only comments, so the file is valid and empty TOML.
        assert!(text.lines().all(|line| line.starts_with('#')));

        block_on(fs.write(path, b"[editor]\ntab_size = 2\n")).unwrap();
        block_on(create_if_missing(&fs, path, "Settings.")).unwrap();
        assert_eq!(
            block_on(fs.read_to_string(path)).unwrap(),
            "[editor]\ntab_size = 2\n"
        );
    }
}
