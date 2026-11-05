use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use language::LanguageRegistry;
use syntax::{Grammar, SymbolKind, SyntaxTree};
use text::ropey::Rope;

use super::{ID, grammar, pack};

const CARGO_TOML: &str = r#"# The package
[package]
name = "forge"
version = "0.1.0"
edition = 2021

[dependencies]
clap = { version = "4", features = ["derive"] }

[[bin]]
name = "forge"
"#;

fn parse(grammar: &Grammar, source: &str) -> (Rope, SyntaxTree) {
    let rope = Rope::from_str(source);
    let tree = SyntaxTree::parse(
        grammar,
        &rope,
        None,
        Duration::from_secs(5),
        &AtomicBool::new(false),
    )
    .unwrap();
    (rope, tree)
}

#[test]
fn registers_for_toml_files_and_lock_files() {
    let mut registry = LanguageRegistry::new();
    registry.register(pack()).unwrap();
    for path in ["Cargo.toml", "dir/config.TOML", "Cargo.lock"] {
        assert_eq!(
            registry
                .for_path(Path::new(path))
                .map(|entry| entry.id().to_owned()),
            Some(ID.to_owned()),
            "{path}"
        );
    }
    assert!(registry.for_path(Path::new("other.lock")).is_none());
    assert!(registry.register(pack()).is_err(), "ids are unique");
    assert!(
        registry
            .get(ID)
            .is_some_and(|entry| entry.grammar().is_some())
    );
}

#[test]
fn highlights_tables_keys_and_values() {
    let grammar = Grammar::compile(&grammar()).unwrap();
    let (rope, tree) = parse(&grammar, CARGO_TOML);
    let spans: Vec<(String, String)> =
        syntax::highlights(&grammar, &tree, &rope, 0..CARGO_TOML.len())
            .into_iter()
            .map(|span| {
                let name = grammar.highlight_names()[span.highlight.0 as usize]
                    .clone()
                    .unwrap();
                (CARGO_TOML[span.range].to_owned(), name)
            })
            .collect();
    for (text, highlight) in [
        ("# The package", "comment"),
        ("package", "type"),
        ("bin", "type"),
        ("name", "attribute"),
        ("features", "attribute"),
        ("\"forge\"", "string"),
        ("2021", "number"),
    ] {
        assert!(
            spans
                .iter()
                .any(|(span, name)| span == text && name == highlight),
            "{text:?} as {highlight}: {spans:#?}"
        );
    }
}

#[test]
fn outline_nests_keys_in_tables() {
    let grammar = Grammar::compile(&grammar()).unwrap();
    let (rope, tree) = parse(&grammar, CARGO_TOML);
    let items: Vec<(SymbolKind, String, usize)> = syntax::outline(&grammar, &tree, &rope)
        .into_iter()
        .map(|item| (item.kind, item.name, item.depth))
        .collect();
    let table = |name: &str| (SymbolKind::Table, name.to_owned(), 0);
    let key = |name: &str, depth| (SymbolKind::Key, name.to_owned(), depth);
    assert_eq!(
        items,
        vec![
            table("package"),
            key("name", 1),
            key("version", 1),
            key("edition", 1),
            table("dependencies"),
            key("clap", 1),
            key("version", 2),
            key("features", 2),
            table("bin"),
            key("name", 1),
        ]
    );
}
