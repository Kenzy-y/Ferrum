//! The fields of a namespace, read from its JSON schema, for generating the
//! settings UI.

use serde_json::{Map, Value};

/// What kind of control edits a field.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldKind {
    /// `true` / `false`.
    Bool,
    /// A whole number, with optional bounds.
    Integer {
        /// Smallest allowed value.
        minimum: Option<i64>,
        /// Largest allowed value.
        maximum: Option<i64>,
    },
    /// A number with a fraction.
    Number,
    /// Free text.
    String,
    /// One of fixed strings.
    Choice(Vec<FieldChoice>),
    /// A list of strings.
    StringList,
    /// Anything else: edited in `settings.toml` only.
    Other,
}

/// One option of a [`FieldKind::Choice`].
#[derive(Debug, Clone, PartialEq)]
pub struct FieldChoice {
    /// The value written to the file.
    pub value: String,
    /// The doc comment of the variant.
    pub description: String,
}

/// One setting of a namespace.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingField {
    /// Key inside the namespace table.
    pub key: String,
    /// The field's doc comment.
    pub description: String,
    /// The control that edits it.
    pub kind: FieldKind,
    /// The built-in default.
    pub default: Option<toml::Value>,
}

/// Fields of a namespace schema in declaration order, with defaults from
/// `defaults`.
pub(crate) fn fields(schema: &Value, defaults: &toml::Table) -> Vec<SettingField> {
    let definitions = schema
        .get("$defs")
        .or_else(|| schema.get("definitions"))
        .and_then(Value::as_object);
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return Vec::new();
    };
    properties
        .iter()
        .map(|(key, property)| {
            let resolved = resolve(property, definitions);
            let description = text(property, "description")
                .or_else(|| resolved.and_then(|resolved| text(resolved, "description")))
                .unwrap_or_default();
            SettingField {
                key: key.clone(),
                description,
                kind: resolved.map_or(FieldKind::Other, |resolved| kind(resolved, definitions)),
                default: defaults.get(key).cloned(),
            }
        })
        .collect()
}

fn text(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_owned)
}

/// Follows `$ref` (directly or in a one-item `allOf`).
fn resolve<'a>(value: &'a Value, definitions: Option<&'a Map<String, Value>>) -> Option<&'a Value> {
    let reference = value.get("$ref").and_then(Value::as_str).or_else(|| {
        value
            .get("allOf")
            .and_then(Value::as_array)
            .filter(|all| all.len() == 1)
            .and_then(|all| all[0].get("$ref"))
            .and_then(Value::as_str)
    });
    match reference {
        Some(reference) => {
            let name = reference.rsplit('/').next()?;
            definitions?.get(name)
        }
        None => Some(value),
    }
}

fn kind(schema: &Value, definitions: Option<&Map<String, Value>>) -> FieldKind {
    if let Some(choices) = choices(schema) {
        return FieldKind::Choice(choices);
    }
    let types: Vec<&str> = match schema.get("type") {
        Some(Value::String(single)) => vec![single.as_str()],
        Some(Value::Array(many)) => many
            .iter()
            .filter_map(Value::as_str)
            .filter(|name| *name != "null")
            .collect(),
        _ => Vec::new(),
    };
    match types.as_slice() {
        ["boolean"] => FieldKind::Bool,
        ["integer"] => FieldKind::Integer {
            minimum: bound(schema, "minimum"),
            maximum: bound(schema, "maximum"),
        },
        ["number"] => FieldKind::Number,
        ["string"] => FieldKind::String,
        ["array"] => {
            let item = schema
                .get("items")
                .and_then(|items| resolve(items, definitions));
            let is_string = item
                .and_then(|item| item.get("type"))
                .and_then(Value::as_str)
                == Some("string");
            if is_string {
                FieldKind::StringList
            } else {
                FieldKind::Other
            }
        }
        _ => FieldKind::Other,
    }
}

fn bound(schema: &Value, key: &str) -> Option<i64> {
    let value = schema.get(key)?;
    value.as_i64().or_else(|| {
        // `u64` bounds above `i64::MAX` are as good as none.
        value.as_u64().map(|_| i64::MAX)
    })
}

/// Unit enums: `enum: [...]` or `oneOf` of `const` strings.
fn choices(schema: &Value) -> Option<Vec<FieldChoice>> {
    if let Some(values) = schema.get("enum").and_then(Value::as_array) {
        return values
            .iter()
            .map(|value| {
                Some(FieldChoice {
                    value: value.as_str()?.to_owned(),
                    description: String::new(),
                })
            })
            .collect();
    }
    let variants = schema
        .get("oneOf")
        .or_else(|| schema.get("anyOf"))
        .and_then(Value::as_array)?;
    let mut result = Vec::new();
    for variant in variants {
        let description = text(variant, "description").unwrap_or_default();
        if let Some(value) = variant.get("const").and_then(Value::as_str) {
            result.push(FieldChoice {
                value: value.to_owned(),
                description,
            });
        } else {
            let values = variant.get("enum").and_then(Value::as_array)?;
            for value in values {
                result.push(FieldChoice {
                    value: value.as_str()?.to_owned(),
                    description: description.clone(),
                });
            }
        }
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    use crate::{FieldKind, Settings, SettingsRegistry};

    #[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema)]
    #[serde(rename_all = "lowercase")]
    enum Modifier {
        /// The Alt key.
        #[default]
        Alt,
        /// The Ctrl key.
        Ctrl,
    }

    #[derive(Debug, Serialize, Deserialize, JsonSchema)]
    #[serde(default)]
    struct Sample {
        /// Width of a tab.
        tab_size: u32,
        /// Shows the minimap.
        minimap: bool,
        /// Wheel speed.
        speed: f32,
        /// Shell name.
        shell: String,
        /// Arguments.
        args: Vec<String>,
        /// Click modifier.
        modifier: Modifier,
    }

    impl Default for Sample {
        fn default() -> Self {
            Self {
                tab_size: 4,
                minimap: true,
                speed: 1.0,
                shell: String::new(),
                args: Vec::new(),
                modifier: Modifier::Alt,
            }
        }
    }

    impl Settings for Sample {
        const NAMESPACE: &'static str = "sample";
    }

    #[test]
    fn reads_fields_in_order_with_kinds_and_defaults() {
        let registry = SettingsRegistry::builder()
            .register::<Sample>()
            .unwrap()
            .build();
        let fields = registry.fields("sample");
        let keys: Vec<&str> = fields.iter().map(|field| field.key.as_str()).collect();
        assert_eq!(
            keys,
            ["tab_size", "minimap", "speed", "shell", "args", "modifier"]
        );
        assert_eq!(fields[0].description, "Width of a tab.");
        assert!(matches!(
            fields[0].kind,
            FieldKind::Integer {
                minimum: Some(0),
                ..
            }
        ));
        assert_eq!(fields[0].default, Some(toml::Value::Integer(4)));
        assert_eq!(fields[1].kind, FieldKind::Bool);
        assert_eq!(fields[2].kind, FieldKind::Number);
        assert_eq!(fields[3].kind, FieldKind::String);
        assert_eq!(fields[4].kind, FieldKind::StringList);
        let FieldKind::Choice(choices) = &fields[5].kind else {
            panic!("{:?}", fields[5].kind);
        };
        assert_eq!(choices[0].value, "alt");
        assert_eq!(choices[1].description, "The Ctrl key.");
        assert_eq!(fields[5].description, "Click modifier.");
    }
}
