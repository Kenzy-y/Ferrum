//! What the status bar and the breadcrumbs say about languages (ADR
//! 0011): the state of each language server, the toolchain versions, the
//! language of the active file and a detail of it (`2021 edition`). The
//! application feeds it; this module only shows it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use gpui::{
    AnyElement, App, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, div, px,
};
use i18n::FluentArgs;
use language::ServerStatus;
use theme::ActiveTheme as _;
use ui::{Icon, IconName};

use crate::Workbench;

/// What is known about languages.
#[derive(Debug, Default)]
pub(crate) struct LanguageStatus {
    /// Names of the languages (`Rust`), by id.
    names: HashMap<String, SharedString>,
    /// Toolchain versions (`1.82.0`), by language id, in registration
    /// order.
    versions: Vec<(String, SharedString)>,
    /// The running servers and their state, in start order.
    servers: Vec<(SharedString, ServerStatus)>,
    /// Details of open files (`2021 edition`).
    file_details: HashMap<PathBuf, SharedString>,
}

impl Workbench {
    /// Names of the languages, by id; the status bar and the breadcrumbs
    /// show the active file's.
    pub fn set_language_names(
        &mut self,
        names: impl IntoIterator<Item = (String, SharedString)>,
        cx: &mut Context<Self>,
    ) {
        self.language_status.names = names.into_iter().collect();
        cx.notify();
    }

    /// The toolchain version of `language`, or none.
    pub fn set_toolchain_version(
        &mut self,
        language: &str,
        version: Option<SharedString>,
        cx: &mut Context<Self>,
    ) {
        let versions = &mut self.language_status.versions;
        let index = versions.iter().position(|(id, _)| id == language);
        match (index, version) {
            (Some(index), Some(version)) => versions[index].1 = version,
            (None, Some(version)) => versions.push((language.to_owned(), version)),
            (Some(index), None) => {
                versions.remove(index);
            }
            (None, None) => return,
        }
        cx.notify();
    }

    /// The state of the server `name`; `None` once it is gone.
    pub fn set_server_status(
        &mut self,
        name: SharedString,
        status: Option<ServerStatus>,
        cx: &mut Context<Self>,
    ) {
        let servers = &mut self.language_status.servers;
        let index = servers.iter().position(|(server, _)| *server == name);
        match (index, status) {
            (Some(index), Some(status)) => servers[index].1 = status,
            (None, Some(status)) => servers.push((name, status)),
            (Some(index), None) => {
                servers.remove(index);
            }
            (None, None) => return,
        }
        cx.notify();
    }

    /// Forgets every server (the folder closed).
    pub fn clear_server_statuses(&mut self, cx: &mut Context<Self>) {
        if !self.language_status.servers.is_empty() {
            self.language_status.servers.clear();
            cx.notify();
        }
    }

    /// A detail of the file at `path`, shown after its language in the
    /// breadcrumbs; `None` removes it.
    pub fn set_file_detail(
        &mut self,
        path: PathBuf,
        detail: Option<SharedString>,
        cx: &mut Context<Self>,
    ) {
        let details = &mut self.language_status.file_details;
        let changed = match detail {
            Some(detail) => details.insert(path, detail.clone()).as_ref() != Some(&detail),
            None => details.remove(&path).is_some(),
        };
        if changed {
            cx.notify();
        }
    }

    /// Id of the active editor's language.
    fn active_language_id(&self, cx: &App) -> Option<String> {
        let editor = self.active_editor()?;
        editor.read(cx).language_id().map(str::to_owned)
    }

    fn version_of(&self, language: &str) -> Option<&SharedString> {
        self.language_status
            .versions
            .iter()
            .find(|(id, _)| id == language)
            .map(|(_, version)| version)
    }
}

/// The servers, as in `editor.svg`: a check when ready, a turning arrow
/// while starting or indexing, a warning or error icon with the reason in
/// a tooltip. Without servers, the ready toolchains (`welcome.svg`).
pub(crate) fn left_items(workbench: &Workbench, cx: &App) -> Vec<AnyElement> {
    let status = &workbench.language_status;
    if status.servers.is_empty() {
        return toolchains_ready(workbench, cx);
    }
    status
        .servers
        .iter()
        .enumerate()
        .filter_map(|(index, (name, server))| server_item(workbench, index, name, server, cx))
        .collect()
}

fn toolchains_ready(workbench: &Workbench, cx: &App) -> Vec<AnyElement> {
    let theme = cx.theme();
    workbench
        .language_status
        .versions
        .iter()
        .filter_map(|(id, _)| workbench.language_status.names.get(id))
        .map(|name| {
            let mut args = FluentArgs::new();
            args.set("language", name.to_string());
            div()
                .flex_none()
                .text_color(theme.colors.text_success)
                .child(workbench.i18n.text_with("status-toolchain-ready", &args))
                .into_any_element()
        })
        .collect()
}

fn server_item(
    workbench: &Workbench,
    index: usize,
    name: &SharedString,
    status: &ServerStatus,
    cx: &App,
) -> Option<AnyElement> {
    let theme = cx.theme();
    let colors = &theme.colors;
    let i18n = &workbench.i18n;
    let mut args = FluentArgs::new();
    args.set("name", name.to_string());
    let (icon, text, tooltip): (Icon, SharedString, Option<String>) = match status {
        ServerStatus::Stopped => return None,
        ServerStatus::Ready => (
            Icon::new(IconName::Check).color(colors.icon_success),
            name.clone(),
            None,
        ),
        ServerStatus::Starting => (
            Icon::new(IconName::RefreshCw),
            i18n.text_with("status-server-starting", &args).into(),
            None,
        ),
        ServerStatus::Indexing {
            message,
            percentage,
        } => {
            let detail = match (message, percentage) {
                (Some(message), Some(percentage)) => format!("{message} {percentage}%"),
                (Some(message), None) => message.clone(),
                (None, Some(percentage)) => format!("{percentage}%"),
                (None, None) => String::new(),
            };
            args.set("detail", detail);
            (
                Icon::new(IconName::RefreshCw),
                i18n.text_with("status-server-indexing", &args).into(),
                None,
            )
        }
        ServerStatus::Warning { message } => (
            Icon::new(IconName::TriangleAlert).color(colors.diagnostic_warning),
            name.clone(),
            Some(message.clone()),
        ),
        ServerStatus::Failed { message, hint } => {
            let mut lines = Vec::new();
            lines.push(i18n.text_with("status-server-failed", &args));
            if let Some(hint) = hint {
                lines.push(i18n.text(hint));
            }
            lines.push(message.clone());
            (
                Icon::new(IconName::CircleX).color(colors.diagnostic_error),
                name.clone(),
                Some(lines.join("\n")),
            )
        }
        ServerStatus::NotFound { program } => {
            args.set("program", program.clone());
            (
                Icon::new(IconName::CircleX).color(colors.diagnostic_error),
                name.clone(),
                Some(i18n.text_with("status-server-not-found", &args)),
            )
        }
    };
    let mut item = div()
        .id(("status-server", index))
        .flex()
        .flex_none()
        .min_w_0()
        .max_w(px(theme.metrics.size.status_item_max_width))
        .items_center()
        .gap(px(theme.metrics.space.sm))
        .child(icon)
        .child(div().truncate().child(text));
    if let Some(tooltip) = tooltip {
        item = item.tooltip(ui::tooltip(tooltip));
    }
    Some(item.into_any_element())
}

/// The active file's language and its toolchain version (`editor.svg`);
/// without one, every known toolchain (`welcome.svg`).
pub(crate) fn right_items(workbench: &Workbench, cx: &App) -> Vec<AnyElement> {
    let theme = cx.theme();
    let status = &workbench.language_status;
    let item = |text: SharedString, color| {
        div()
            .flex_none()
            .text_color(color)
            .child(text)
            .into_any_element()
    };
    if let Some(id) = workbench.active_language_id(cx) {
        let Some(name) = status.names.get(&id) else {
            return Vec::new();
        };
        let mut items = vec![item(name.clone(), theme.colors.text_accent)];
        if let Some(version) = workbench.version_of(&id) {
            items.push(item(version.clone(), theme.colors.text_secondary));
        }
        return items;
    }
    status
        .versions
        .iter()
        .filter_map(|(id, version)| {
            let name = status.names.get(id)?;
            Some(item(
                format!("{name} {version}").into(),
                theme.colors.text_secondary,
            ))
        })
        .collect()
}

/// `Rust  /  2021 edition` at the right of the breadcrumbs.
pub(crate) fn breadcrumb_detail(
    workbench: &Workbench,
    path: Option<&Path>,
    cx: &App,
) -> Option<AnyElement> {
    let id = workbench.active_language_id(cx)?;
    let name = workbench.language_status.names.get(&id)?;
    let text: SharedString =
        match path.and_then(|path| workbench.language_status.file_details.get(path)) {
            Some(detail) => format!("{name}  /  {detail}").into(),
            None => name.clone(),
        };
    let theme = cx.theme();
    Some(
        div()
            .flex_none()
            .font_family(SharedString::from(theme.fonts.mono.clone()))
            .text_size(px(theme.metrics.font_size.sm))
            .text_color(theme.colors.text_ignored)
            .child(text)
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;
    use std::sync::Arc;

    use commands::{CommandRegistry, Keymap};
    use gpui::TestAppContext;
    use i18n::I18n;

    use super::*;

    #[gpui::test]
    fn servers_replace_the_ready_toolchains_and_go_when_stopped(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let theme =
                theme::Theme::built_in(theme::DEFAULT_THEME, theme::Density::default()).unwrap();
            theme::set_theme(theme, cx);
        });
        let window = cx.add_window(|_, _| {
            Workbench::new(
                Arc::new(I18n::new("en").unwrap()),
                Rc::new(|_, _, _| {}),
                Arc::new(CommandRegistry::new()),
                Arc::new(Keymap::default()),
                Rc::new(|_, _, _| {}),
            )
        });
        window
            .update(cx, |workbench, _, cx| {
                workbench.set_language_names([("rust".to_owned(), "Rust".into())], cx);
                workbench.set_toolchain_version("rust", Some("1.82.0".into()), cx);
                // Without servers or an editor: "Rust toolchain ready" and
                // "Rust 1.82.0".
                assert_eq!(left_items(workbench, cx).len(), 1);
                assert_eq!(right_items(workbench, cx).len(), 1);

                let name: SharedString = "rust-analyzer".into();
                workbench.set_server_status(name.clone(), Some(ServerStatus::Starting), cx);
                workbench.set_server_status(name.clone(), Some(ServerStatus::Ready), cx);
                assert_eq!(workbench.language_status.servers.len(), 1);
                assert_eq!(left_items(workbench, cx).len(), 1);
                workbench.set_server_status(name, Some(ServerStatus::Stopped), cx);
                assert!(left_items(workbench, cx).is_empty());

                workbench.clear_server_statuses(cx);
                workbench.set_toolchain_version("rust", None, cx);
                assert!(left_items(workbench, cx).is_empty());
                assert!(right_items(workbench, cx).is_empty());
            })
            .unwrap();
    }
}
