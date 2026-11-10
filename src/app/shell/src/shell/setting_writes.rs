//! Changing one setting from the UI (the settings page, zoom commands):
//! the value is written into the layer's file, keeping the user's comments,
//! and applied at once without waiting for the file watcher.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use fs::Fs;
use gpui::{Context, Window};
use settings::{LayerKind, SettingsLayer};

use super::Shell;

/// One change: `path` inside the layer file (`["editor", "tab_size"]`),
/// `None` removes the value so the lower layer applies.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SettingWrite {
    pub(crate) layer: LayerKind,
    pub(crate) path: Vec<String>,
    pub(crate) value: Option<toml::Value>,
}

impl Shell {
    /// Writes `change` to its file in the background, then applies the
    /// new layer. Writes run one after another, so quick repeated changes
    /// (holding Ctrl +) never lose each other.
    pub(crate) fn write_setting(
        &mut self,
        change: SettingWrite,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(file) = self.layer_file(change.layer) else {
            tracing::warn!(layer = ?change.layer, "this settings layer has no file");
            return;
        };
        let fs = self.fs.clone();
        let lock = self.setting_writes.clone();
        cx.spawn_in(window, async move |shell, cx| {
            let layer = change.layer;
            let written = cx
                .background_executor()
                .spawn(async move {
                    let _guard = lock.lock().await;
                    write(fs, &file, &change).await
                })
                .await;
            match written {
                Ok(new_layer) => {
                    let _ = shell.update_in(cx, |shell, window, cx| {
                        if !shell.settings.set_layer(layer, new_layer).is_empty() {
                            shell.settings_changed(window, cx);
                        }
                    });
                }
                Err(error) => tracing::error!("cannot change the setting: {error:#}"),
            }
        })
        .detach();
    }

    pub(super) fn layer_file(&self, layer: LayerKind) -> Option<PathBuf> {
        match layer {
            LayerKind::User => self
                .config_paths
                .as_ref()
                .map(settings::paths::ConfigPaths::user_settings),
            LayerKind::Workspace => self
                .workspace
                .as_ref()
                .map(project::Workspace::settings_file),
            _ => None,
        }
    }
}

/// Reads the file (missing is empty), changes the value, writes it back and
/// returns the parsed result.
async fn write(
    fs: Arc<dyn Fs>,
    file: &Path,
    change: &SettingWrite,
) -> anyhow::Result<SettingsLayer> {
    let text = edit_file(fs, file, &[(change.path.clone(), change.value.clone())]).await?;
    Ok(SettingsLayer::parse(&text, Some(file))?)
}

/// Applies `edits` (path, value or `None` to remove) to a TOML file through
/// `fs`, keeping comments; a missing file starts empty. Returns the new
/// text.
pub(crate) async fn edit_file(
    fs: Arc<dyn Fs>,
    file: &Path,
    edits: &[(Vec<String>, Option<toml::Value>)],
) -> anyhow::Result<String> {
    let text = match fs.read_to_string(file).await {
        Ok(text) => text,
        Err(error) if error.is_not_found() => String::new(),
        Err(error) => return Err(error.into()),
    };
    let mut new_text = text.clone();
    for (path, value) in edits {
        let path: Vec<&str> = path.iter().map(String::as_str).collect();
        new_text = settings::edit::set_value(&new_text, &path, value.as_ref())?;
    }
    if new_text != text {
        if let Some(parent) = file.parent() {
            fs.create_dir_all(parent).await?;
        }
        fs.write(file, new_text.as_bytes()).await?;
    }
    Ok(new_text)
}

#[cfg(test)]
mod tests {
    use fs::FakeFs;
    use futures::executor::block_on;

    use super::*;

    #[test]
    fn writes_into_a_missing_file_and_keeps_comments() {
        let fake = Arc::new(FakeFs::new());
        let file = Path::new("/config/settings.toml");
        let fs: Arc<dyn Fs> = fake.clone();
        let change = SettingWrite {
            layer: LayerKind::User,
            path: vec!["appearance".into(), "zoom".into()],
            value: Some(toml::Value::Float(1.2)),
        };
        block_on(write(fs.clone(), file, &change)).unwrap();
        assert_eq!(
            block_on(fake.read_to_string(file)).unwrap(),
            "[appearance]\nzoom = 1.2\n"
        );

        block_on(fake.write(file, b"# mine\n[appearance]\nzoom = 1.2 # big\n")).unwrap();
        let change = SettingWrite {
            value: Some(toml::Value::Float(1.3)),
            ..change
        };
        block_on(write(fs, file, &change)).unwrap();
        assert_eq!(
            block_on(fake.read_to_string(file)).unwrap(),
            "# mine\n[appearance]\nzoom = 1.3 # big\n"
        );
    }
}
