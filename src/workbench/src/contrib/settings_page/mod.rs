//! The settings page in the editor area, generated from the settings
//! schema: a search field, the namespaces on the left, the fields of the
//! chosen namespace on the right, a switch between the user and the
//! workspace layer, and the Keyboard Shortcuts tab.
//!
//! The application gives the page a [`SettingsPageData`] snapshot and
//! applies the [`SettingsPageEvent`]s it emits; the page itself never
//! touches files.

mod controls;
mod keybindings;
mod view;

use std::collections::HashMap;

use gpui::{
    AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable, SharedString,
    Subscription, Window,
};
use gpui_component::input::{InputEvent, InputState};
use settings::{FieldKind, LayerKind};

/// One setting, with its value in each layer.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingItem {
    /// Namespace (`editor`).
    pub namespace: String,
    /// Key in the namespace (`tab_size`).
    pub key: String,
    /// Title shown above the description ("Tab size").
    pub title: SharedString,
    /// The field's doc comment.
    pub description: SharedString,
    /// The control that edits it.
    pub kind: FieldKind,
    /// The built-in default.
    pub default: Option<toml::Value>,
    /// What the user `settings.toml` sets.
    pub user: Option<toml::Value>,
    /// What the workspace settings file sets.
    pub workspace: Option<toml::Value>,
}

impl SettingItem {
    /// The value set in `layer`.
    pub fn in_layer(&self, layer: LayerKind) -> Option<&toml::Value> {
        match layer {
            LayerKind::Workspace => self.workspace.as_ref(),
            _ => self.user.as_ref(),
        }
    }

    /// The value that applies when editing `layer`: the layer's own, or
    /// what lower layers give.
    pub fn shown_value(&self, layer: LayerKind) -> Option<&toml::Value> {
        match layer {
            LayerKind::Workspace => self
                .workspace
                .as_ref()
                .or(self.user.as_ref())
                .or(self.default.as_ref()),
            _ => self.user.as_ref().or(self.default.as_ref()),
        }
    }
}

/// The settings of one namespace.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingsGroup {
    /// Namespace (`editor`).
    pub namespace: String,
    /// Localized title ("Editor").
    pub title: SharedString,
    /// Its fields in declaration order.
    pub items: Vec<SettingItem>,
}

/// One binding on the Keyboard Shortcuts tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeybindingEntry {
    /// The command id.
    pub command: SharedString,
    /// Its localized title.
    pub title: SharedString,
    /// The keys as written in `keymap.toml` (`ctrl-shift-p`), if bound.
    pub keys: Option<SharedString>,
    /// The keys for people (`Ctrl Shift P`).
    pub keys_label: Option<SharedString>,
    /// Keymap context (`global`, `editor`).
    pub context: SharedString,
    /// Whether the user `keymap.toml` defines this binding.
    pub user: bool,
}

/// What the page shows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SettingsPageData {
    /// Namespaces in display order.
    pub groups: Vec<SettingsGroup>,
    /// Whether a folder is open (the workspace layer can be edited).
    pub has_workspace: bool,
    /// Every command with its bindings.
    pub keybindings: Vec<KeybindingEntry>,
}

/// What the user did on the page.
#[derive(Debug, Clone, PartialEq)]
pub enum SettingsPageEvent {
    /// Set (`Some`) or reset (`None`) a setting in a layer.
    Change {
        /// The layer edited.
        layer: LayerKind,
        /// Namespace.
        namespace: String,
        /// Key.
        key: String,
        /// The new value; `None` removes it from the layer.
        value: Option<toml::Value>,
    },
    /// Open the file of a layer as text.
    OpenFile(LayerKind),
    /// Open the user `keymap.toml` as text.
    OpenKeymapFile,
    /// Bind `keystroke` to `command` in `context` (user `keymap.toml`).
    Bind {
        /// Command id.
        command: SharedString,
        /// Keymap context.
        context: SharedString,
        /// The binding it replaces, as written in `keymap.toml`.
        previous: Option<SharedString>,
        /// The recorded key press.
        keystroke: gpui::Keystroke,
    },
    /// Remove the binding `keys` of `command` (`user` says whether it is
    /// the user's own or a default to switch off).
    Unbind {
        /// Command id.
        command: SharedString,
        /// Keymap context.
        context: SharedString,
        /// The keys as written in `keymap.toml`.
        keys: SharedString,
        /// The binding comes from the user file.
        user: bool,
    },
}

/// The tab of the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsPageTab {
    /// Settings from the schema.
    #[default]
    Settings,
    /// Keyboard shortcuts.
    Keybindings,
}

/// Localized texts of the page.
#[derive(Debug, Clone, Default)]
pub struct SettingsPageTexts {
    pub(crate) values: HashMap<&'static str, SharedString>,
}

impl SettingsPageTexts {
    /// Collects the texts through `text` (an i18n lookup).
    pub fn new(text: impl Fn(&str) -> String) -> Self {
        let values = TEXT_IDS
            .iter()
            .map(|id| (*id, SharedString::from(text(id))))
            .collect();
        Self { values }
    }

    pub(crate) fn get(&self, id: &str) -> SharedString {
        self.values.get(id).cloned().unwrap_or_default()
    }
}

/// Message ids the page uses.
const TEXT_IDS: &[&str] = &[
    "settings-page-search",
    "settings-page-tab-settings",
    "settings-page-tab-keybindings",
    "settings-page-layer-user",
    "settings-page-layer-workspace",
    "settings-page-no-workspace",
    "settings-page-open-file",
    "settings-page-reset",
    "settings-page-modified",
    "settings-page-no-results",
    "settings-page-toml-only",
    "settings-page-all",
    "settings-page-on",
    "settings-page-off",
    "settings-page-invalid",
    "keybindings-search",
    "keybindings-command",
    "keybindings-keys",
    "keybindings-context",
    "keybindings-change",
    "keybindings-add",
    "keybindings-remove",
    "keybindings-record",
    "keybindings-user",
    "keybindings-open-file",
];

/// The settings page.
pub struct SettingsPage {
    pub(crate) data: SettingsPageData,
    pub(crate) texts: SettingsPageTexts,
    pub(crate) tab: SettingsPageTab,
    pub(crate) layer: LayerKind,
    /// Namespace shown; `None` shows every group.
    pub(crate) group: Option<String>,
    pub(crate) query: Entity<InputState>,
    /// Text fields of string, number and list settings, by
    /// `namespace.key`, with the value they were filled with.
    pub(crate) fields: HashMap<String, (Entity<InputState>, String, Subscription)>,
    /// Field whose text failed to parse.
    pub(crate) invalid: Option<String>,
    /// The binding being recorded: (row index).
    pub(crate) recording: Option<usize>,
    pub(crate) focus_handle: FocusHandle,
    /// The tab the search placeholder was last set for.
    pub(crate) placeholder_tab: Option<SettingsPageTab>,
    _query_events: Subscription,
}

impl EventEmitter<SettingsPageEvent> for SettingsPage {}

impl Focusable for SettingsPage {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl SettingsPage {
    /// Creates the page on `tab`.
    pub fn new(
        data: SettingsPageData,
        texts: SettingsPageTexts,
        tab: SettingsPageTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let placeholder = texts.get("settings-page-search");
        let query = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        let query_events = cx.subscribe(&query, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        Self {
            data,
            texts,
            tab,
            layer: LayerKind::User,
            group: None,
            query,
            fields: HashMap::new(),
            invalid: None,
            recording: None,
            focus_handle: cx.focus_handle(),
            placeholder_tab: None,
            _query_events: query_events,
        }
    }

    /// Replaces the data after a settings or keymap change.
    pub fn set_data(&mut self, data: SettingsPageData, cx: &mut Context<Self>) {
        if !data.has_workspace {
            self.layer = LayerKind::User;
        }
        self.data = data;
        cx.notify();
    }

    /// Replaces the texts after a language change.
    pub fn set_texts(&mut self, texts: SettingsPageTexts, cx: &mut Context<Self>) {
        self.texts = texts;
        self.placeholder_tab = None;
        cx.notify();
    }

    /// Shows `tab`.
    pub fn set_tab(&mut self, tab: SettingsPageTab, cx: &mut Context<Self>) {
        self.tab = tab;
        self.recording = None;
        cx.notify();
    }

    pub(crate) fn set_layer(&mut self, layer: LayerKind, cx: &mut Context<Self>) {
        self.layer = layer;
        // Text fields show the other layer's values now.
        self.fields.clear();
        self.invalid = None;
        cx.notify();
    }

    pub(crate) fn query_text(&self, cx: &gpui::App) -> String {
        self.query.read(cx).value().trim().to_lowercase()
    }

    pub(crate) fn change(
        &mut self,
        item: &SettingItem,
        value: Option<toml::Value>,
        cx: &mut Context<Self>,
    ) {
        cx.emit(SettingsPageEvent::Change {
            layer: self.layer,
            namespace: item.namespace.clone(),
            key: item.key.clone(),
            value,
        });
    }
}

/// Items of `groups` matching `query` (lower case) in their key, title or
/// description; with `group` set, only that namespace.
pub(crate) fn visible_items<'a>(
    groups: &'a [SettingsGroup],
    group: Option<&str>,
    query: &str,
) -> Vec<(&'a SettingsGroup, Vec<&'a SettingItem>)> {
    groups
        .iter()
        .filter(|candidate| {
            query.is_empty() && group.is_none_or(|name| name == candidate.namespace)
                || !query.is_empty()
        })
        .map(|candidate| {
            let items = candidate
                .items
                .iter()
                .filter(|item| {
                    query.is_empty()
                        || item.key.to_lowercase().contains(query)
                        || item.title.to_lowercase().contains(query)
                        || item.description.to_lowercase().contains(query)
                        || candidate.namespace.contains(query)
                })
                .collect::<Vec<_>>();
            (candidate, items)
        })
        .filter(|(_, items)| !items.is_empty())
        .collect()
}

/// `tab_size` → "Tab size".
pub fn humanize_key(key: &str) -> String {
    let words = key.replace('_', " ");
    let mut characters = words.chars();
    characters.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(characters).collect()
    })
}

#[cfg(test)]
mod tests;
