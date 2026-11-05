//! Opening folders: the system picker, the typed-path fallback, recent
//! projects and the workspace settings layer.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use gpui::{AppContext as _, Context, PathPromptOptions, SharedString, Window};
use i18n::FluentArgs;
use project::{FilesSettings, ProjectError, RecentProjects, Workspace};
use settings::LayerKind;
use workbench::{
    FileTreeTexts, FileTreeView, RecentEntry, WorkspaceInfo, WorkspaceSettingsChanged,
};

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
        // Servers of the previous folder stop in the background.
        if let Some(previous) = &self.workspace {
            for stopping in self.language_servers.stop(Some(previous.root())) {
                cx.background_executor().spawn(stopping).detach();
            }
        }
        self.apply_workspace_layer(layer, window, cx);

        window.set_window_title(&super::window_title(&self.i18n, Some(&workspace)));
        let info = WorkspaceInfo {
            name: workspace.name().to_owned().into(),
            path: workspace.display_root().into(),
            root: workspace.root().to_path_buf(),
        };
        let texts = FileTreeTexts {
            cycle: self.i18n.text("file-tree-cycle").into(),
            failed: self.i18n.text("file-tree-failed").into(),
        };
        let fs = self.fs.clone();
        let root = workspace.root().to_path_buf();
        let settings_file = workspace.settings_file();
        let icons = self.file_icons.clone();
        let hidden = self.settings.get::<FilesSettings>(None).hidden;
        let tree =
            cx.new(|cx| FileTreeView::new(fs, root, settings_file, &hidden, icons, texts, cx));
        self.hidden = hidden;
        self.file_tree = Some(tree.clone());
        self.open_file_watch = Some(cx.subscribe_in(
            &tree,
            window,
            |shell, _, event: &workbench::OpenFile, window, cx| {
                shell.open_file(event.0.clone(), None, window, cx);
            },
        ));
        self.workspace_watch = Some(cx.subscribe_in(
            &tree,
            window,
            |shell, _, _: &WorkspaceSettingsChanged, window, cx| {
                shell.reload_workspace_settings(window, cx);
            },
        ));
        self.workbench.update(cx, |workbench, cx| {
            workbench.set_workspace(Some((info, tree)), cx);
        });

        self.recent.record(workspace.root(), SystemTime::now());
        self.recent_changed(cx);
        self.workspace = Some(workspace);
    }

    fn apply_workspace_layer(
        &mut self,
        layer: Option<settings::SettingsLayer>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let change = match layer {
            Some(layer) => self.settings.set_layer(LayerKind::Workspace, layer),
            None => self.settings.remove_layer(LayerKind::Workspace),
        };
        if !change.is_empty() {
            self.settings_changed(window, cx);
        }
    }

    /// `.ferrum/settings.toml` changed on disk: read it again.
    fn reload_workspace_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(workspace) = self.workspace.clone() else {
            return;
        };
        let fs = self.fs.clone();
        cx.spawn_in(window, async move |shell, cx| {
            let layer = cx
                .background_executor()
                .spawn(async move { workspace.load_settings(fs.as_ref()).await })
                .await;
            let _ = shell.update_in(cx, |shell, window, cx| {
                tracing::info!("applied workspace settings changes");
                shell.apply_workspace_layer(layer, window, cx);
            });
        })
        .detach();
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
