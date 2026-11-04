//! Applying changed settings and keybindings to a running window: the
//! interface language, keyboard settings, editor settings and the file
//! tree take effect without a restart.

use std::sync::Arc;

use gpui::{Context, Window};
use settings::LayerKind;

use super::{Shell, window_title};
use crate::config;
use crate::live_config::{Reloaded, UserSettings};

impl Shell {
    /// Applies configuration files that changed on disk.
    pub(crate) fn apply_config(
        &mut self,
        reloaded: Reloaded,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let change = match reloaded.settings {
            UserSettings::Untouched => None,
            UserSettings::Present(layer) => Some(self.settings.set_layer(LayerKind::User, layer)),
            UserSettings::Absent => Some(self.settings.remove_layer(LayerKind::User)),
        };
        if let Some(change) = change
            && !change.is_empty()
        {
            self.settings_changed(window, cx);
        }
        if let Some(keymap) = reloaded.keymap {
            let keymap = Arc::new(keymap);
            self.keyboard.set_keymap(keymap.clone());
            self.workbench
                .update(cx, |workbench, cx| workbench.set_keymap(keymap, cx));
            tracing::info!("applied keymap changes");
        }
    }

    /// Re-applies everything that depends on settings.
    pub(super) fn settings_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        config::report_settings(&self.settings);
        self.keyboard
            .set_chord_timeout(config::chord_timeout(&self.settings));
        self.apply_language(window, cx);
        let editors: Vec<_> = self.workbench.read(cx).editors().cloned().collect();
        for editor in editors {
            editor.update(cx, |editor, cx| {
                // `[lang.<id>]` overrides apply to files of that language.
                let language = editor.language_id().map(str::to_owned);
                let language = language.as_deref();
                editor.set_settings(self.settings.get::<editor::EditorSettings>(language), cx);
                editor.set_syntax_settings(self.settings.get::<syntax::SyntaxSettings>(language));
            });
        }
        let hidden = self.settings.get::<project::FilesSettings>(None).hidden;
        if hidden != self.hidden
            && let Some(tree) = &self.file_tree
        {
            tree.update(cx, |tree, cx| tree.set_hidden(&hidden, cx));
            self.hidden = hidden;
        }
    }

    fn apply_language(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let i18n = match config::localization(&self.settings) {
            Ok(i18n) => i18n,
            Err(error) => {
                tracing::error!("{error:#}");
                return;
            }
        };
        if i18n.locale() == self.i18n.locale() {
            return;
        }
        let i18n = Arc::new(i18n);
        window.set_window_title(&window_title(&i18n, self.workspace.as_ref()));
        self.i18n = i18n.clone();
        self.workbench
            .update(cx, |workbench, cx| workbench.set_i18n(i18n, cx));
    }
}
