//! A single source of settings values, parsed from one file.

use std::collections::BTreeMap;
use std::path::Path;

use crate::SettingsError;

/// Top-level key that holds per-language overrides:
/// `[lang.<language id>.<namespace>]`.
pub const LANGUAGE_OVERRIDES_KEY: &str = "lang";

/// Where a layer comes from. Later variants override earlier ones.
///
/// Built-in defaults are not a stored layer: they come from the registry.
/// New layers (for example per folder) are added as new variants in their
/// place in the order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum LayerKind {
    /// The user `settings.toml` in the configuration directory.
    User,
    /// The workspace settings file inside the workspace folder.
    Workspace,
}

/// Values of one layer: namespaced tables plus per-language overrides.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SettingsLayer {
    namespaces: toml::Table,
    languages: BTreeMap<String, toml::Table>,
}

impl SettingsLayer {
    /// Parses the text of a settings file. `path` is only used in errors.
    ///
    /// # Errors
    ///
    /// Returns [`SettingsError::Parse`] if the text is not valid TOML or if
    /// a namespace or language override is not a table.
    pub fn parse(text: &str, path: Option<&Path>) -> Result<Self, SettingsError> {
        let parse_error = |message: String| SettingsError::Parse {
            path: path.map(Path::to_path_buf),
            message,
        };
        let mut namespaces: toml::Table = text
            .parse()
            .map_err(|error: toml::de::Error| parse_error(error.to_string()))?;

        let mut languages = BTreeMap::new();
        if let Some(overrides) = namespaces.remove(LANGUAGE_OVERRIDES_KEY) {
            let toml::Value::Table(overrides) = overrides else {
                return Err(parse_error(format!(
                    "`{LANGUAGE_OVERRIDES_KEY}` must be a table of languages"
                )));
            };
            for (language, table) in overrides {
                let toml::Value::Table(table) = table else {
                    return Err(parse_error(format!(
                        "`{LANGUAGE_OVERRIDES_KEY}.{language}` must be a table"
                    )));
                };
                languages.insert(language, table);
            }
        }

        if let Some((namespace, _)) = namespaces.iter().find(|(_, value)| !value.is_table()) {
            return Err(parse_error(format!("`{namespace}` must be a table")));
        }

        Ok(Self {
            namespaces,
            languages,
        })
    }

    /// The table of `namespace` in this layer, if present.
    pub(crate) fn namespace(&self, namespace: &str) -> Option<&toml::Table> {
        self.namespaces
            .get(namespace)
            .and_then(toml::Value::as_table)
    }

    /// The table of `namespace` inside the overrides for `language`.
    pub(crate) fn language_namespace(
        &self,
        language: &str,
        namespace: &str,
    ) -> Option<&toml::Table> {
        self.languages
            .get(language)
            .and_then(|table| table.get(namespace))
            .and_then(toml::Value::as_table)
    }

    /// Languages that have overrides in this layer.
    pub(crate) fn languages(&self) -> impl Iterator<Item = &str> {
        self.languages.keys().map(String::as_str)
    }

    /// Names of all namespaces mentioned in this layer, including inside
    /// language overrides.
    pub(crate) fn mentioned_namespaces(&self) -> impl Iterator<Item = &str> {
        self.namespaces
            .keys()
            .chain(self.languages.values().flat_map(toml::Table::keys))
            .map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_namespaces_and_language_overrides() {
        let layer = SettingsLayer::parse(
            r"
            [editor]
            tab_size = 4

            [lang.rust.editor]
            tab_size = 2
            ",
            None,
        )
        .unwrap();

        assert_eq!(
            layer.namespace("editor").unwrap()["tab_size"].as_integer(),
            Some(4)
        );
        assert_eq!(
            layer.language_namespace("rust", "editor").unwrap()["tab_size"].as_integer(),
            Some(2)
        );
        assert!(layer.namespace(LANGUAGE_OVERRIDES_KEY).is_none());
    }

    #[test]
    fn empty_text_is_an_empty_layer() {
        assert_eq!(
            SettingsLayer::parse("", None).unwrap(),
            SettingsLayer::default()
        );
    }

    #[test]
    fn rejects_invalid_toml_with_path() {
        let error = SettingsLayer::parse("[editor", Some(Path::new("settings.toml"))).unwrap_err();
        assert!(error.to_string().contains("settings.toml"), "{error}");
    }

    #[test]
    fn rejects_non_table_namespace() {
        assert!(SettingsLayer::parse("editor = 1", None).is_err());
        assert!(SettingsLayer::parse("lang = 1", None).is_err());
        assert!(SettingsLayer::parse("[lang]\nrust = 1", None).is_err());
    }
}
