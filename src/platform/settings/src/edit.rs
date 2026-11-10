//! Changing one value in the text of a settings file, keeping everything
//! else (comments, order, formatting) as the user wrote it.

use toml_edit::{DocumentMut, Item, Table};

use crate::SettingsError;

/// Sets `path` (for example `["editor", "tab_size"]` or
/// `["lang", "rust", "editor", "tab_size"]`) to `value` in the settings file
/// `text`, or removes it when `value` is `None`. Tables left empty by a
/// removal are removed too. Returns the new text.
///
/// # Errors
///
/// Fails if `text` is not valid TOML, if `path` is empty or crosses a value
/// that is not a table, or if `value` cannot be written as TOML.
pub fn set_value(
    text: &str,
    path: &[&str],
    value: Option<&toml::Value>,
) -> Result<String, SettingsError> {
    let parse_error = |message: String| SettingsError::Parse {
        path: None,
        message,
    };
    let mut document: DocumentMut = text
        .parse()
        .map_err(|error: toml_edit::TomlError| parse_error(error.to_string()))?;
    let Some((key, tables)) = path.split_last() else {
        return Err(parse_error("empty settings path".to_owned()));
    };
    match value {
        Some(value) => {
            let value = to_edit_value(value).map_err(parse_error)?;
            let mut table = document.as_table_mut();
            for name in tables {
                let item = table.entry(name).or_insert_with(|| {
                    let mut new_table = Table::new();
                    // Intermediate tables like `lang` and `lang.rust` stay
                    // out of the text; only the innermost gets a header.
                    new_table.set_implicit(true);
                    Item::Table(new_table)
                });
                table = item
                    .as_table_mut()
                    .ok_or_else(|| parse_error(format!("`{name}` is not a table")))?;
            }
            match table.get_mut(key) {
                // Keeps the decor (comments around the value).
                Some(Item::Value(existing)) => {
                    let decor = existing.decor().clone();
                    *existing = value;
                    *existing.decor_mut() = decor;
                }
                _ => {
                    table.insert(key, Item::Value(value));
                }
            }
        }
        None => remove(document.as_table_mut(), tables, key),
    }
    let result = document.to_string();
    // A removed first table leaves the blank line that separated the next.
    if text.starts_with(['\n', '\r']) {
        Ok(result)
    } else {
        Ok(result.trim_start_matches(['\n', '\r']).to_owned())
    }
}

/// Removes `key` under `tables`, then every table the removal left empty.
fn remove(root: &mut Table, tables: &[&str], key: &str) {
    let Some((first, rest)) = tables.split_first() else {
        root.remove(key);
        return;
    };
    let Some(child) = root.get_mut(first).and_then(Item::as_table_mut) else {
        return;
    };
    remove(child, rest, key);
    if child.is_empty() {
        root.remove(first);
    }
}

/// Converts through text: `toml` writes the value, `toml_edit` reads it.
fn to_edit_value(value: &toml::Value) -> Result<toml_edit::Value, String> {
    let mut wrapper = toml::Table::new();
    wrapper.insert("value".to_owned(), value.clone());
    let text = toml::to_string(&wrapper).map_err(|error| error.to_string())?;
    let document: DocumentMut = text
        .parse()
        .map_err(|error: toml_edit::TomlError| error.to_string())?;
    document
        .get("value")
        .and_then(Item::as_value)
        .cloned()
        .map(|mut value| {
            value.decor_mut().clear();
            value
        })
        .ok_or_else(|| format!("cannot write `{value}` as a settings value"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changes_a_value_and_keeps_comments() {
        let text = "# My settings\n[editor]\n# indentation\ntab_size = 4 # four\nminimap = true\n";
        let result = set_value(
            text,
            &["editor", "tab_size"],
            Some(&toml::Value::Integer(2)),
        )
        .unwrap();
        assert_eq!(
            result,
            "# My settings\n[editor]\n# indentation\ntab_size = 2 # four\nminimap = true\n"
        );
    }

    #[test]
    fn adds_missing_tables() {
        let result = set_value(
            "",
            &["editor", "minimap"],
            Some(&toml::Value::Boolean(false)),
        )
        .unwrap();
        assert_eq!(result, "[editor]\nminimap = false\n");
        let result = set_value(
            "",
            &["lang", "rust", "editor", "tab_size"],
            Some(&toml::Value::Integer(2)),
        )
        .unwrap();
        assert_eq!(result, "[lang.rust.editor]\ntab_size = 2\n");
    }

    #[test]
    fn writes_strings_and_arrays() {
        let value = toml::Value::Array(vec![toml::Value::String("-NoLogo".into())]);
        let result = set_value("[terminal]\n", &["terminal", "shell_args"], Some(&value)).unwrap();
        assert_eq!(result, "[terminal]\nshell_args = [\"-NoLogo\"]\n");
    }

    #[test]
    fn removing_drops_empty_tables() {
        let text = "[editor]\ntab_size = 2\n\n[terminal]\nshell = \"bash\"\n";
        let result = set_value(text, &["editor", "tab_size"], None).unwrap();
        assert_eq!(result, "[terminal]\nshell = \"bash\"\n");
        let result = set_value(
            "[lang.rust.editor]\ntab_size = 2\n",
            &["lang", "rust", "editor", "tab_size"],
            None,
        )
        .unwrap();
        assert_eq!(result.trim(), "");
    }

    #[test]
    fn invalid_text_is_an_error() {
        assert!(set_value("[editor", &["editor", "x"], None).is_err());
        assert!(
            set_value(
                "editor = 1",
                &["editor", "x"],
                Some(&toml::Value::Integer(1))
            )
            .is_err()
        );
    }
}
