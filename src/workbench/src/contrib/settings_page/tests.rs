use settings::FieldKind;

use super::controls::{parse_text, value_text};
use super::{SettingItem, SettingsGroup, humanize_key, visible_items};

fn item(namespace: &str, key: &str, description: &str) -> SettingItem {
    SettingItem {
        namespace: namespace.into(),
        key: key.into(),
        title: humanize_key(key).into(),
        description: description.to_owned().into(),
        kind: FieldKind::Bool,
        default: None,
        user: None,
        workspace: None,
    }
}

#[test]
fn humanizes_keys() {
    assert_eq!(humanize_key("tab_size"), "Tab size");
    assert_eq!(humanize_key(""), "");
}

#[test]
fn search_matches_key_title_and_description_across_groups() {
    let groups = vec![
        SettingsGroup {
            namespace: "editor".into(),
            title: "Editor".into(),
            items: vec![
                item("editor", "tab_size", "Columns"),
                item("editor", "minimap", "Shows"),
            ],
        },
        SettingsGroup {
            namespace: "terminal".into(),
            title: "Terminal".into(),
            items: vec![item("terminal", "shell", "Program")],
        },
    ];
    let all = visible_items(&groups, None, "");
    assert_eq!(all.len(), 2);
    let one = visible_items(&groups, Some("terminal"), "");
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].0.namespace, "terminal");
    // A search looks in every group, whatever is selected.
    let found = visible_items(&groups, Some("terminal"), "columns");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].1[0].key, "tab_size");
    assert!(visible_items(&groups, None, "nothing").is_empty());
}

#[test]
fn layer_values_fall_back_to_lower_layers() {
    let mut setting = item("editor", "tab_size", "");
    setting.default = Some(toml::Value::Integer(4));
    setting.user = Some(toml::Value::Integer(2));
    assert_eq!(
        setting.shown_value(settings::LayerKind::Workspace),
        Some(&toml::Value::Integer(2))
    );
    assert_eq!(setting.in_layer(settings::LayerKind::Workspace), None);
}

#[test]
fn typed_text_becomes_values() {
    let bounded = FieldKind::Integer {
        minimum: Some(0),
        maximum: None,
    };
    assert_eq!(parse_text(&bounded, " 8 "), Some(toml::Value::Integer(8)));
    assert_eq!(parse_text(&bounded, "-1"), None);
    assert_eq!(parse_text(&bounded, "x"), None);
    assert_eq!(
        parse_text(&FieldKind::Number, "1.5"),
        Some(toml::Value::Float(1.5))
    );
    let list = parse_text(&FieldKind::StringList, "a, b,").unwrap();
    assert_eq!(value_text(&list), "a, b");
    assert_eq!(value_text(&toml::Value::String("bash".into())), "bash");
}
