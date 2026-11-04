//! Opening folders: the system picker, the typed-path fallback, recent
//! projects and the workspace settings layer.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use gpui::{Context, PathPromptOptions, SharedString, Window};
use i18n::FluentArgs;
use project::{ProjectError, RecentProjects, Workspace};
use settings::LayerKind;
use workbench::{RecentEntry, WorkspaceInfo};

use super::Shell;

impl Shell {
    /// `workspace.openFolder`: asks the platform for a folder. If the
    /// platform has no picker (Linux without xdg-desktop-portal), explains
    /// that and offers to type the path.
    pub(super) fn prompt_for_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(self.i18n.text("path-prompt-open").into()),
        });
        cx.spawn_in(window, async move |shell, cx| {
            let result = receiver.await;
            let _ = shell.update_in(cx, |shell, window, cx| match result {
                Ok(Ok(Some(paths))) => {
                    if let Some(path) = paths.into_iter().next() {
                        shell.open_folder(path, window, cx);
                    }
                }
                Ok(Ok(None)) => {}
                Ok(Err(error)) => shell.picker_unavailable(&error.to_string(), window, cx),
                Err(error) => shell.picker_unavailable(&error.to_string(), window, cx),
            });
        })
        .detach();
    }

    fn picker_unavailable(&mut self, reason: &str, window: &mut Window, cx: &mut Context<Self>) {
        tracing::warn!(reason, "system folder picker is unavailable");
        let mut args = FluentArgs::new();
        args.set("error", reason.to_owned());
        let message = self.i18n.text_with("open-folder-picker-unavailable", &args);
        self.show_path_prompt(None, Some(message.into()), window, cx);
    }

    /// `workspace.openFolderByPath` and the fallback of the picker.
    pub(super) fn show_path_prompt(
        &mut self,
        initial: Option<String>,
        error: Option<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.workbench.update(cx, |workbench, cx| {
            workbench.show_path_prompt(initial, error, window, cx);
        });
    }

    /// Opens `path` as the workspace of this window. Checks and reads happen
    /// in the background; on failure the path dialog explains why.
    pub(super) fn open_folder(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let fs = self.fs.clone();
        cx.spawn_in(window, async move |shell, cx| {
            let opened = cx
                .background_executor()
                .spawn(async move {
                    let workspace = Workspace::open(fs.as_ref(), &path).await;
                    match workspace {
                        Ok(workspace) => {
                            let layer = workspace.load_settings(fs.as_ref()).await;
                            Ok((workspace, layer))
                        }
                        Err(error) => Err((path, error)),
                    }
                })
                .await;
            let _ = shell.update_in(cx, |shell, window, cx| match opened {
                Ok((workspace, layer)) => shell.set_workspace(workspace, layer, window, cx),
                Err((path, error)) => shell.open_failed(&path, &error, window, cx),
            });
        })
        .detach();
    }

    fn open_failed(
        &mut self,
        path: &Path,
        error: &ProjectError,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        tracing::warn!(%error, "cannot open folder");
        let display = path.display().to_string();
        let mut args = FluentArgs::new();
        args.set("path", display.clone());
        let message = match error {
            ProjectError::NotFound { .. } => {
                // A recent project that is gone is forgotten.
                self.recent.remove(path);
                self.recent_changed(cx);
                self.i18n.text_with("open-folder-not-found", &args)
            }
            ProjectError::NotAFolder { .. } => {
                self.i18n.text_with("open-folder-not-a-folder", &args)
            }
            other => {
                args.set("error", other.to_string());
                self.i18n.text_with("open-folder-failed", &args)
            }
        };
        self.show_path_prompt(Some(display), Some(message.into()), window, cx);
    }

    fn set_workspace(
        &mut self,
        workspace: Workspace,
        layer: Option<settings::SettingsLayer>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        tracing::info!(root = %workspace.display_root(), "opened folder");
        let change = match layer {
            Some(layer) => self.settings.set_layer(LayerKind::Workspace, layer),
            None => self.settings.remove_layer(LayerKind::Workspace),
        };
        if !change.is_empty() {
            self.settings_changed(window, cx);
        }

        window.set_window_title(&super::window_title(&self.i18n, Some(&workspace)));
        let info = WorkspaceInfo {
            name: workspace.name().to_owned().into(),
            path: workspace.display_root().into(),
        };
        self.workbench
            .update(cx, |workbench, cx| workbench.set_workspace(Some(info), cx));

        self.recent.record(workspace.root(), SystemTime::now());
        self.recent_changed(cx);
        self.workspace = Some(workspace);
    }

    /// Shows the recent list and saves it in the background.
    fn recent_changed(&mut self, cx: &mut Context<Self>) {
        let entries = recent_entries(&self.recent);
        self.workbench
            .update(cx, |workbench, cx| workbench.set_recent(entries, cx));
        let Some(file) = self.recent_file.clone() else {
            return;
        };
        let fs = self.fs.clone();
        let recent = self.recent.clone();
        cx.background_executor()
            .spawn(async move {
                if let Err(error) = recent.save(fs.as_ref(), &file).await {
                    tracing::warn!(%error, "cannot save recent projects");
                }
            })
            .detach();
    }
}

/// The recent list as the Welcome page shows it.
pub(super) fn recent_entries(recent: &RecentProjects) -> Vec<RecentEntry> {
    recent
        .projects()
        .iter()
        .map(|project| RecentEntry {
            name: project.name().into(),
            path: project.display_path().into(),
            root: project.path.clone(),
        })
        .collect()
}
