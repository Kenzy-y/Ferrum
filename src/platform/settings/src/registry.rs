//! Registry of settings namespaces, populated at startup and read-only
//! afterwards.

use std::any::TypeId;
use std::collections::BTreeMap;

use schemars::Schema;

use crate::{Settings, SettingsError};

/// What the registry knows about one namespace.
#[derive(Debug)]
pub(crate) struct Entry {
    pub(crate) type_id: TypeId,
    pub(crate) schema: Schema,
    pub(crate) defaults: toml::Table,
}

/// All registered settings namespaces with their schemas and defaults.
///
/// Built once with [`SettingsRegistry::builder`]; immutable afterwards.
#[derive(Debug, Default)]
pub struct SettingsRegistry {
    entries: BTreeMap<&'static str, Entry>,
}

impl SettingsRegistry {
    /// Starts building a registry.
    pub fn builder() -> RegistryBuilder {
        RegistryBuilder::default()
    }

    /// Registered namespaces in alphabetical order.
    pub fn namespaces(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.entries.keys().copied()
    }

    /// The JSON schema of `namespace`, used to generate the settings UI.
    pub fn schema(&self, namespace: &str) -> Option<&Schema> {
        self.entries.get(namespace).map(|entry| &entry.schema)
    }

    /// Fields of `namespace` in declaration order, with their defaults; empty
    /// if the namespace is unknown.
    pub fn fields(&self, namespace: &str) -> Vec<crate::SettingField> {
        self.entries.get(namespace).map_or_else(Vec::new, |entry| {
            crate::fields::fields(entry.schema.as_value(), &entry.defaults)
        })
    }

    /// Whether `namespace` is registered.
    pub fn contains(&self, namespace: &str) -> bool {
        self.entries.contains_key(namespace)
    }

    pub(crate) fn defaults(&self, namespace: &str) -> Option<&toml::Table> {
        self.entries.get(namespace).map(|entry| &entry.defaults)
    }

    pub(crate) fn entry_for<T: Settings>(&self) -> Option<&Entry> {
        self.entries
            .get(T::NAMESPACE)
            .filter(|entry| entry.type_id == TypeId::of::<T>())
    }
}

/// Collects [`Settings`] types into a [`SettingsRegistry`].
#[derive(Debug, Default)]
pub struct RegistryBuilder {
    registry: SettingsRegistry,
}

impl RegistryBuilder {
    /// Registers `T` under [`Settings::NAMESPACE`].
    ///
    /// # Errors
    ///
    /// Fails if the namespace is invalid or already taken, or if the
    /// defaults of `T` cannot be represented as a TOML table.
    pub fn register<T: Settings>(mut self) -> Result<Self, SettingsError> {
        let namespace = T::NAMESPACE;
        if !is_valid_namespace(namespace) {
            return Err(SettingsError::InvalidNamespace(namespace));
        }
        if self.registry.entries.contains_key(namespace) {
            return Err(SettingsError::DuplicateNamespace(namespace));
        }
        let defaults = toml::Table::try_from(T::default())
            .map_err(|source| SettingsError::SerializeDefaults { namespace, source })?;
        let entry = Entry {
            type_id: TypeId::of::<T>(),
            schema: schemars::schema_for!(T),
            defaults,
        };
        self.registry.entries.insert(namespace, entry);
        Ok(self)
    }

    /// Finishes the registry.
    pub fn build(self) -> SettingsRegistry {
        self.registry
    }
}

/// Namespaces are lowercase identifiers so they map onto plain TOML keys.
/// The language overrides key is reserved.
fn is_valid_namespace(namespace: &str) -> bool {
    let mut characters = namespace.chars();
    namespace != crate::LANGUAGE_OVERRIDES_KEY
        && characters
            .next()
            .is_some_and(|first| first.is_ascii_lowercase())
        && characters.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

#[cfg(test)]
mod tests {
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
    #[serde(default)]
    struct Editor {
        /// Width of a tab in columns.
        tab_size: u32,
    }

    impl Settings for Editor {
        const NAMESPACE: &'static str = "editor";
    }

    #[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
    struct AlsoEditor {}

    impl Settings for AlsoEditor {
        const NAMESPACE: &'static str = "editor";
    }

    #[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
    struct Reserved {}

    impl Settings for Reserved {
        const NAMESPACE: &'static str = "lang";
    }

    #[test]
    fn registers_schema_with_field_docs() {
        let registry = SettingsRegistry::builder()
            .register::<Editor>()
            .unwrap()
            .build();

        assert!(registry.contains("editor"));
        let schema = registry.schema("editor").unwrap().as_value().to_string();
        assert!(schema.contains("Width of a tab in columns."), "{schema}");
    }

    #[test]
    fn rejects_duplicate_and_reserved_namespaces() {
        let duplicate = SettingsRegistry::builder()
            .register::<Editor>()
            .unwrap()
            .register::<AlsoEditor>();
        assert!(matches!(
            duplicate,
            Err(SettingsError::DuplicateNamespace("editor"))
        ));

        let reserved = SettingsRegistry::builder().register::<Reserved>();
        assert!(matches!(
            reserved,
            Err(SettingsError::InvalidNamespace("lang"))
        ));
    }

    #[test]
    fn namespace_rules() {
        assert!(is_valid_namespace("editor"));
        assert!(is_valid_namespace("git_blame2"));
        assert!(!is_valid_namespace(""));
        assert!(!is_valid_namespace("Editor"));
        assert!(!is_valid_namespace("2editor"));
        assert!(!is_valid_namespace("editor.font"));
    }
}
