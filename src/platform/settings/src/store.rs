//! Merged view over all layers.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::merge::merge_into;
use crate::{LayerKind, Settings, SettingsLayer, SettingsRegistry};

/// Namespaces whose effective values may have changed after a layer update.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SettingsChange {
    /// Affected namespaces.
    pub namespaces: BTreeSet<String>,
}

impl SettingsChange {
    /// Whether settings of type `T` may have changed.
    pub fn affects<T: Settings>(&self) -> bool {
        self.namespaces.contains(T::NAMESPACE)
    }

    /// Whether nothing changed.
    pub fn is_empty(&self) -> bool {
        self.namespaces.is_empty()
    }
}

/// A problem found in user-provided settings. Problems never stop the IDE:
/// the affected namespace falls back to its defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsDiagnostic {
    /// A layer mentions a namespace nobody registered.
    UnknownNamespace {
        /// The layer with the unknown namespace.
        layer: LayerKind,
        /// The namespace.
        namespace: String,
    },
    /// The merged values of a namespace do not match its schema.
    InvalidValue {
        /// The namespace.
        namespace: &'static str,
        /// The language whose overrides were applied, if any.
        language: Option<String>,
        /// Human-readable description of the problem.
        message: String,
    },
}

/// Settings values from all layers, merged on read.
///
/// The store itself is not shared mutable state: the application owns it
/// (in the UI layer, as a GPUI global or entity) and broadcasts the
/// [`SettingsChange`] returned by [`SettingsStore::set_layer`] to observers.
#[derive(Debug)]
pub struct SettingsStore {
    registry: Arc<SettingsRegistry>,
    layers: BTreeMap<LayerKind, SettingsLayer>,
    revision: u64,
}

impl SettingsStore {
    /// Creates a store with only built-in defaults.
    pub fn new(registry: Arc<SettingsRegistry>) -> Self {
        Self {
            registry,
            layers: BTreeMap::new(),
            revision: 0,
        }
    }

    /// The registry the store validates against.
    pub fn registry(&self) -> &SettingsRegistry {
        &self.registry
    }

    /// Increases every time a layer changes; cheap staleness check for
    /// caches.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Replaces a layer and reports which namespaces may have changed.
    pub fn set_layer(&mut self, kind: LayerKind, layer: SettingsLayer) -> SettingsChange {
        let previous = self.layers.insert(kind, layer);
        self.record_change(kind, previous.as_ref())
    }

    /// Removes a layer (for example when its file was deleted) and reports
    /// which namespaces may have changed.
    pub fn remove_layer(&mut self, kind: LayerKind) -> SettingsChange {
        let previous = self.layers.remove(&kind);
        self.record_change(kind, previous.as_ref())
    }

    fn record_change(
        &mut self,
        kind: LayerKind,
        previous: Option<&SettingsLayer>,
    ) -> SettingsChange {
        let current = self.layers.get(&kind);
        if previous == current {
            return SettingsChange::default();
        }
        self.revision += 1;
        let namespaces = previous
            .into_iter()
            .chain(current)
            .flat_map(SettingsLayer::mentioned_namespaces)
            .map(str::to_owned)
            .collect();
        SettingsChange { namespaces }
    }

    /// Effective settings of type `T`, optionally with the overrides of
    /// `language` applied.
    ///
    /// Invalid values never fail the call: the problem is logged and the
    /// defaults of `T` are returned. Use [`SettingsStore::diagnostics`] to
    /// show problems to the user.
    pub fn get<T: Settings>(&self, language: Option<&str>) -> T {
        self.try_get(language).unwrap_or_else(|message| {
            tracing::warn!(namespace = T::NAMESPACE, ?language, %message, "invalid settings, using defaults");
            T::default()
        })
    }

    fn try_get<T: Settings>(&self, language: Option<&str>) -> Result<T, String> {
        let Some(entry) = self.registry.entry_for::<T>() else {
            return Err(format!(
                "namespace `{}` is not registered for this type",
                T::NAMESPACE
            ));
        };
        self.merged(T::NAMESPACE, &entry.defaults, language)
            .try_into()
            .map_err(|error: toml::de::Error| error.message().to_owned())
    }

    /// Merges defaults, then every layer, then language overrides of every
    /// layer, for one namespace.
    fn merged(
        &self,
        namespace: &str,
        defaults: &toml::Table,
        language: Option<&str>,
    ) -> toml::Table {
        let mut merged = defaults.clone();
        for layer in self.layers.values() {
            if let Some(table) = layer.namespace(namespace) {
                merge_into(&mut merged, table);
            }
        }
        if let Some(language) = language {
            for layer in self.layers.values() {
                if let Some(table) = layer.language_namespace(language, namespace) {
                    merge_into(&mut merged, table);
                }
            }
        }
        merged
    }

    /// The value of `key` in `namespace` as written in one layer (with the
    /// overrides of `language` when given), if the layer sets it.
    pub fn layer_value(
        &self,
        kind: LayerKind,
        namespace: &str,
        language: Option<&str>,
        key: &str,
    ) -> Option<&toml::Value> {
        let layer = self.layers.get(&kind)?;
        let table = match language {
            Some(language) => layer.language_namespace(language, namespace),
            None => layer.namespace(namespace),
        };
        table?.get(key)
    }

    /// The merged value of `key` in `namespace` (defaults and every layer,
    /// no language overrides).
    pub fn effective_value(&self, namespace: &str, key: &str) -> Option<toml::Value> {
        let defaults = self.registry.defaults(namespace)?;
        self.merged(namespace, defaults, None).remove(key)
    }

    /// Namespaces mentioned in the layers that nobody registered (usually a
    /// typo or a setting of a plugin that is not installed).
    pub fn unknown_namespaces(&self) -> Vec<SettingsDiagnostic> {
        self.layers
            .iter()
            .flat_map(|(kind, layer)| {
                layer
                    .mentioned_namespaces()
                    .filter(|namespace| !self.registry.contains(namespace))
                    .map(|namespace| SettingsDiagnostic::UnknownNamespace {
                        layer: *kind,
                        namespace: namespace.to_owned(),
                    })
            })
            .collect()
    }

    /// Checks that the merged values of `T` are valid, without and with the
    /// overrides of every language mentioned in the layers.
    pub fn check<T: Settings>(&self) -> Vec<SettingsDiagnostic> {
        let languages: BTreeSet<&str> = self
            .layers
            .values()
            .flat_map(SettingsLayer::languages)
            .collect();
        std::iter::once(None)
            .chain(languages.into_iter().map(Some))
            .filter_map(|language| {
                self.try_get::<T>(language)
                    .err()
                    .map(|message| SettingsDiagnostic::InvalidValue {
                        namespace: T::NAMESPACE,
                        language: language.map(str::to_owned),
                        message,
                    })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
    #[serde(default)]
    struct Editor {
        tab_size: u32,
        font_family: String,
    }

    impl Default for Editor {
        fn default() -> Self {
            Self {
                tab_size: 4,
                font_family: "mono".into(),
            }
        }
    }

    impl Settings for Editor {
        const NAMESPACE: &'static str = "editor";
    }

    fn store() -> SettingsStore {
        let registry = SettingsRegistry::builder()
            .register::<Editor>()
            .unwrap()
            .build();
        SettingsStore::new(Arc::new(registry))
    }

    fn layer(text: &str) -> SettingsLayer {
        SettingsLayer::parse(text, None).unwrap()
    }

    #[test]
    fn defaults_without_layers() {
        assert_eq!(store().get::<Editor>(None), Editor::default());
    }

    #[test]
    fn layers_override_in_order_and_language_last() {
        let mut store = store();
        store.set_layer(
            LayerKind::Workspace,
            layer("[editor]\ntab_size = 3\n[lang.rust.editor]\ntab_size = 5"),
        );
        store.set_layer(
            LayerKind::User,
            layer(
                "[editor]\ntab_size = 2\nfont_family = \"user\"\n[lang.rust.editor]\ntab_size = 9",
            ),
        );

        let plain = store.get::<Editor>(None);
        assert_eq!(plain.tab_size, 3, "workspace overrides user");
        assert_eq!(
            plain.font_family, "user",
            "unset keys come from lower layers"
        );

        assert_eq!(
            store.get::<Editor>(Some("rust")).tab_size,
            5,
            "workspace language override wins"
        );
        assert_eq!(store.get::<Editor>(Some("toml")).tab_size, 3);
    }

    #[test]
    fn invalid_values_fall_back_to_defaults_and_are_reported() {
        let mut store = store();
        store.set_layer(LayerKind::User, layer("[editor]\ntab_size = \"wide\""));

        assert_eq!(store.get::<Editor>(None), Editor::default());
        let diagnostics = store.check::<Editor>();
        assert!(matches!(
            diagnostics.as_slice(),
            [SettingsDiagnostic::InvalidValue {
                namespace: "editor",
                language: None,
                ..
            }]
        ));
    }

    #[test]
    fn unknown_namespaces_are_reported() {
        let mut store = store();
        store.set_layer(LayerKind::User, layer("[editr]\ntab_size = 2"));

        assert_eq!(
            store.unknown_namespaces(),
            vec![SettingsDiagnostic::UnknownNamespace {
                layer: LayerKind::User,
                namespace: "editr".into()
            }]
        );
    }

    #[test]
    fn set_layer_reports_changes() {
        let mut store = store();
        let change = store.set_layer(LayerKind::User, layer("[editor]\ntab_size = 2"));
        assert!(change.affects::<Editor>());
        assert_eq!(store.revision(), 1);

        let unchanged = store.set_layer(LayerKind::User, layer("[editor]\ntab_size = 2"));
        assert!(unchanged.is_empty());
        assert_eq!(store.revision(), 1);

        let removed = store.remove_layer(LayerKind::User);
        assert!(removed.affects::<Editor>());
        assert_eq!(store.get::<Editor>(None), Editor::default());
    }
}
