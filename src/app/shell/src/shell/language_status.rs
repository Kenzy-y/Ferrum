//! Feeds the workbench what the status bar and the breadcrumbs say about
//! languages (ADR 0011): the names of the languages, the toolchain
//! versions of the open folder, the state of each running server and the
//! details of open files. Toolchains and details are found in the
//! background.

use std::path::Path;
use std::sync::Arc;

use futures::StreamExt as _;
use gpui::{Context, SharedString};
use i18n::FluentArgs;
use language::{LanguageEntry, LanguageService, ServiceEvent};

use super::Shell;

impl Shell {
    /// Tells the workbench the names of the languages and looks for their
    /// toolchains without a folder.
    pub(super) fn start_language_status(&mut self, cx: &mut Context<Self>) {
        let names: Vec<(String, SharedString)> = self
            .languages
            .iter()
            .map(|entry| (entry.id().to_owned(), entry.name().to_owned().into()))
            .collect();
        self.workbench.update(cx, |workbench, cx| {
            workbench.set_language_names(names, cx);
        });
        self.find_toolchains(cx);
    }

    /// Finds the toolchain version of every language that has one, for the
    /// open folder (a folder may pin its own, as `rust-toolchain.toml`).
    pub(super) fn find_toolchains(&mut self, cx: &mut Context<Self>) {
        let root = self
            .workspace
            .as_ref()
            .map(|workspace| workspace.root().to_path_buf());
        self.toolchain_tasks.clear();
        for entry in self.languages.iter() {
            let Some(toolchain) = entry.toolchain() else {
                continue;
            };
            let id = entry.id().to_owned();
            let version = cx
                .background_executor()
                .spawn(toolchain.version(root.clone(), self.process.clone()));
            let workbench = self.workbench.downgrade();
            self.toolchain_tasks.push(cx.spawn(async move |_, cx| {
                let version = version.await;
                tracing::info!(language = %id, ?version, "toolchain");
                let _ = workbench.update(cx, |workbench, cx| {
                    workbench.set_toolchain_version(&id, version.map(SharedString::from), cx);
                });
            }));
        }
    }

    /// Shows the state of `service` in the status bar until the folder
    /// closes.
    pub(super) fn watch_server_status(
        &mut self,
        service: &Arc<dyn LanguageService>,
        cx: &mut Context<Self>,
    ) {
        let name: SharedString = service.name().to_owned().into();
        let status = service.status();
        self.workbench.update(cx, |workbench, cx| {
            workbench.set_server_status(name.clone(), Some(status), cx);
        });
        let mut events = service.events();
        let workbench = self.workbench.downgrade();
        self.problem_watches.push(cx.spawn(async move |_, cx| {
            while let Some(event) = events.next().await {
                let ServiceEvent::Status(status) = event else {
                    continue;
                };
                let updated = workbench.update(cx, |workbench, cx| {
                    workbench.set_server_status(name.clone(), Some(status), cx);
                });
                if updated.is_err() {
                    break;
                }
            }
        }));
    }

    /// Finds the detail of the file at `path` (its edition) for the
    /// breadcrumbs.
    pub(super) fn find_file_detail(
        &mut self,
        path: &Path,
        entry: &LanguageEntry,
        cx: &mut Context<Self>,
    ) {
        let Some(toolchain) = entry.toolchain() else {
            return;
        };
        let detail = cx
            .background_executor()
            .spawn(toolchain.file_detail(path, self.fs.clone()));
        let path = path.to_path_buf();
        let i18n = self.i18n.clone();
        let workbench = self.workbench.downgrade();
        cx.spawn(async move |_, cx| {
            let detail = detail.await.map(|detail| {
                let mut args = FluentArgs::new();
                args.set("value", detail.value);
                SharedString::from(i18n.text_with(detail.message_id, &args))
            });
            let _ = workbench.update(cx, |workbench, cx| {
                workbench.set_file_detail(path, detail, cx);
            });
        })
        .detach();
    }
}
