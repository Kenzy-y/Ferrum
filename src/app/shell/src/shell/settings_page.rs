//! The settings page: fed from the settings registry, the store and the
//! keymap; its edits go to `settings.toml`, the workspace settings file or
//! `keymap.toml`.

use std::collections::HashSet;

use commands::{Keymap, Platform};
use gpui::{AppContext as _, Context, Entity, SharedString, Subscription, Window};
use settings::LayerKind;
use workbench::{
    KeybindingEntry, SettingItem, SettingsGroup, SettingsPage, SettingsPageData, SettingsPageEvent,
    SettingsPageTexts, humanize_key,
};

pub(crate) use workbench::SettingsPageTab;

use super::Shell;
use super::config_files::ConfigFile;
use super::setting_writes::{SettingWrite, edit_file};

/// The page and the subscription to its events.
pub(crate) struct OpenSettingsPage {
    page: Entity<SettingsPage>,
    _events: Subscription,
}

impl Shell {
    /// Shows the settings page on `tab`, creating it on first use.
    pub(super) fn open_settings_page(
        &mut self,
        tab: SettingsPageTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let data = self.settings_page_data(cx);
        let page = if let Some(open) = &self.settings_page {
            let page = open.page.clone();
            page.update(cx, |page, cx| {
                page.set_data(data, cx);
                page.set_tab(tab, cx);
            });
            page
        } else {
            let texts = self.settings_page_texts();
            let page = cx.new(|cx| SettingsPage::new(data, texts, tab, window, cx));
            let events = cx.subscribe_in(&page, window, Self::on_settings_page_event);
            self.settings_page = Some(OpenSettingsPage {
                page: page.clone(),
                _events: events,
            });
            page
        };
        let title: SharedString = self.i18n.text("settings-page-title").into();
        self.workbench.update(cx, |workbench, cx| {
            workbench.set_view_tab(title, Some(page.into()), window, cx);
        });
    }

    /// Settings, keymap, folder or language changed: the page follows.
    pub(super) fn refresh_settings_page(&mut self, cx: &mut Context<Self>) {
        let Some(open) = &self.settings_page else {
            return;
        };
        let page = open.page.clone();
        let data = self.settings_page_data(cx);
        let texts = self.settings_page_texts();
        page.update(cx, |page, cx| {
            page.set_texts(texts, cx);
            page.set_data(data, cx);
        });
    }

    fn settings_page_texts(&self) -> SettingsPageTexts {
        SettingsPageTexts::new(|id| self.i18n.text(id))
    }

    fn settings_page_data(&self, _cx: &Context<Self>) -> SettingsPageData {
        let registry = self.settings.registry();
        let mut groups: Vec<SettingsGroup> = registry
            .namespaces()
            .map(|namespace| {
                let title_id = format!("settings-group-{namespace}");
                let title = if self.i18n.has(&title_id) {
                    self.i18n.text(&title_id)
                } else {
                    humanize_key(namespace)
                };
                let items = registry
                    .fields(namespace)
                    .into_iter()
                    .map(|field| SettingItem {
                        namespace: namespace.to_owned(),
                        title: humanize_key(&field.key).into(),
                        description: field.description.into(),
                        kind: field.kind,
                        default: field.default,
                        user: self
                            .settings
                            .layer_value(LayerKind::User, namespace, None, &field.key)
                            .cloned(),
                        workspace: self
                            .settings
                            .layer_value(LayerKind::Workspace, namespace, None, &field.key)
                            .cloned(),
                        key: field.key,
                    })
                    .collect();
                SettingsGroup {
                    namespace: namespace.to_owned(),
                    title: title.into(),
                    items,
                }
            })
            .collect();
        groups.sort_by_key(|group| group.title.to_lowercase());
        SettingsPageData {
            groups,
            has_workspace: self.workspace.is_some(),
            keybindings: self.keybinding_entries(),
        }
    }

    fn keybinding_entries(&self) -> Vec<KeybindingEntry> {
        let platform = Platform::current();
        let keymap = self.keyboard.keymap();
        let defaults: HashSet<(String, String, String)> = Keymap::defaults(platform)
            .bindings()
            .iter()
            .map(|binding| {
                (
                    binding.keys.to_string(),
                    binding.context.as_str().to_owned(),
                    binding.command.to_string(),
                )
            })
            .collect();
        let mut entries = Vec::new();
        for command in self.commands.iter() {
            let title: SharedString = self.i18n.text(&command.title_message).into();
            let mut bound = false;
            for binding in keymap.bindings_for(&command.id) {
                bound = true;
                let keys = binding.keys.to_string();
                let context = binding.context.as_str().to_owned();
                let user =
                    !defaults.contains(&(keys.clone(), context.clone(), command.id.to_string()));
                entries.push(KeybindingEntry {
                    command: command.id.to_string().into(),
                    title: title.clone(),
                    keys_label: Some(binding.keys.label(platform).into()),
                    keys: Some(keys.into()),
                    context: context.into(),
                    user,
                });
            }
            if !bound {
                entries.push(KeybindingEntry {
                    command: command.id.to_string().into(),
                    title,
                    keys: None,
                    keys_label: None,
                    context: GLOBAL_CONTEXT.into(),
                    user: false,
                });
            }
        }
        entries.sort_by(|left, right| left.title.cmp(&right.title));
        entries
    }

    fn on_settings_page_event(
        &mut self,
        _page: &Entity<SettingsPage>,
        event: &SettingsPageEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            SettingsPageEvent::Change {
                layer,
                namespace,
                key,
                value,
            } => self.write_setting(
                SettingWrite {
                    layer: *layer,
                    path: vec![namespace.clone(), key.clone()],
                    value: value.clone(),
                },
                window,
                cx,
            ),
            SettingsPageEvent::OpenFile(LayerKind::Workspace) => {
                if let Some(file) = self.layer_file(LayerKind::Workspace) {
                    self.open_file(file, None, window, cx);
                }
            }
            SettingsPageEvent::OpenFile(_) => {
                self.open_config_file(ConfigFile::Settings, window, cx);
            }
            SettingsPageEvent::OpenKeymapFile => {
                self.open_config_file(ConfigFile::Keymap, window, cx);
            }
            SettingsPageEvent::Bind {
                command,
                context,
                previous,
                keystroke,
            } => {
                let keys = self.keyboard.keymap_keystroke(keystroke).to_string();
                let mut edits = Vec::new();
                if let Some(previous) = previous {
                    edits.push(unbind_edit(context, previous, command));
                }
                edits.push((
                    vec![context.to_string(), keys],
                    Some(toml::Value::String(command.to_string())),
                ));
                self.write_keymap(edits, cx);
            }
            SettingsPageEvent::Unbind {
                command,
                context,
                keys,
                ..
            } => {
                let edit = unbind_edit(context, keys, command);
                self.write_keymap(vec![edit], cx);
            }
        }
    }

    /// Writes `edits` to the user `keymap.toml`; the file watcher applies
    /// the new keymap.
    fn write_keymap(&self, edits: Vec<(Vec<String>, Option<toml::Value>)>, cx: &mut Context<Self>) {
        let Some(paths) = &self.config_paths else {
            tracing::warn!("no home directory: there is no keymap file to change");
            return;
        };
        let file = paths.user_keymap();
        let fs = self.fs.clone();
        let lock = self.setting_writes.clone();
        cx.background_executor()
            .spawn(async move {
                let _guard = lock.lock().await;
                if let Err(error) = edit_file(fs, &file, &edits).await {
                    tracing::error!("cannot change the keymap: {error:#}");
                }
            })
            .detach();
    }
}

/// Context of commands that have no binding yet.
const GLOBAL_CONTEXT: &str = "global";

/// A default binding is switched off with an empty command; the user's
/// own is removed from the file.
fn unbind_edit(context: &str, keys: &str, command: &str) -> (Vec<String>, Option<toml::Value>) {
    let is_default = Keymap::defaults(Platform::current())
        .bindings()
        .iter()
        .any(|binding| {
            binding.keys.to_string() == keys
                && binding.context.as_str() == context
                && binding.command.as_str() == command
        });
    let path = vec![context.to_owned(), keys.to_owned()];
    if is_default {
        (path, Some(toml::Value::String(String::new())))
    } else {
        (path, None)
    }
}
