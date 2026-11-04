use std::sync::atomic::AtomicBool;
use std::time::Duration;

use syntax::{Grammar, SymbolKind, SyntaxTree};
use text::ropey::Rope;

use super::grammar;

/// The code of `docs/design/mockups/editor.svg`.
const MOCKUP: &str = r#"use clap::Parser;
use anyhow::Result;

mod config;
mod commands;

/// A small tool for building great things.
#[derive(Parser, Debug)]
#[command(name = "forge", version, about)]
struct Cli {
    /// Path to the configuration file
    #[arg(short, long, default_value = "forge.toml")]
    config: String,

    #[command(subcommand)]
    command: commands::Command,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = config::load(&cli.config)?;

    commands::run(cli.command, config)?;
    Ok(())
}
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

/// Every highlighted piece of `source` as `(text, highlight)`.
fn highlighted(source: &str) -> Vec<(String, String)> {
    let grammar = Grammar::compile(&grammar()).unwrap();
    let (rope, tree) = parse(&grammar, source);
    syntax::highlights(&grammar, &tree, &rope, 0..source.len())
        .into_iter()
        .map(|span| {
            let name = grammar.highlight_names()[span.highlight.0 as usize]
                .clone()
                .unwrap();
            (source[span.range].to_owned(), name)
        })
        .collect()
}

fn has(spans: &[(String, String)], text: &str, highlight: &str) -> bool {
    spans
        .iter()
        .any(|(span, name)| span == text && name == highlight)
}

#[test]
fn highlights_the_mockup_as_designed() {
    let spans = highlighted(MOCKUP);
    let expected = [
        ("use", "keyword"),
        ("Parser", "type"),
        ("Result", "type"),
        ("mod", "keyword"),
        (
            "/// A small tool for building great things.\n",
            "doc_comment",
        ),
        ("#[derive(Parser, Debug)]", "attribute"),
        // Strings inside attributes keep the attribute color.
        (r#"#[command(name = "forge", version, about)]"#, "attribute"),
        ("struct", "keyword"),
        ("Cli", "type"),
        ("String", "type"),
        ("Command", "type"),
        ("fn", "keyword"),
        ("main", "function"),
        ("let", "keyword"),
        ("parse", "function"),
        ("load", "function"),
        ("run", "function"),
        ("Ok", "function"),
    ];
    for (text, highlight) in expected {
        assert!(
            has(&spans, text, highlight),
            "{text:?} as {highlight}: {spans:#?}"
        );
    }
    // Plain text stays plain: modules, variables, fields, punctuation.
    for plain in ["clap", "config", "cli", "commands", "->", "?"] {
        assert!(
            !spans.iter().any(|(span, _)| span == plain),
            "{plain:?} is plain"
        );
    }
}

#[test]
fn highlights_literals_macros_and_conventions() {
    let spans = highlighted(
        "const MAX_SIZE: u32 = 0x10;\nfn f<'a>(x: &'a str) { println!(\"{}\", true); let _ = None; 'outer: loop {} }",
    );
    for (text, highlight) in [
        ("MAX_SIZE", "constant"),
        ("u32", "type"),
        ("0x10", "number"),
        ("'a", "lifetime"),
        ("println!", "macro"),
        ("\"{}\"", "string"),
        ("true", "constant"),
        ("None", "type"),
        ("'outer", "lifetime"),
    ] {
        assert!(
            has(&spans, text, highlight),
            "{text:?} as {highlight}: {spans:#?}"
        );
    }
}

#[test]
fn outline_lists_items_with_nesting() {
    let grammar = Grammar::compile(&grammar()).unwrap();
    let source = "struct Cli;\n\
        impl std::fmt::Display for Cli {\n    fn fmt(&self) {}\n}\n\
        impl<T> Wrapper<T> {}\n\
        mod inner {\n    const LIMIT: u8 = 1;\n    trait Named {\n        fn name(&self);\n    }\n}\n\
        fn main() {}\n\
        macro_rules! m { () => {} }\n";
    let (rope, tree) = parse(&grammar, source);
    let items: Vec<(SymbolKind, String, usize)> = syntax::outline(&grammar, &tree, &rope)
        .into_iter()
        .map(|item| (item.kind, item.name, item.depth))
        .collect();
    let item = |kind, name: &str, depth| (kind, name.to_owned(), depth);
    assert_eq!(
        items,
        vec![
            item(SymbolKind::Struct, "Cli", 0),
            item(SymbolKind::Impl, "impl std::fmt::Display for Cli", 0),
            item(SymbolKind::Function, "fmt", 1),
            item(SymbolKind::Impl, "impl Wrapper<T>", 0),
            item(SymbolKind::Module, "inner", 0),
            item(SymbolKind::Constant, "LIMIT", 1),
            item(SymbolKind::Trait, "Named", 1),
            item(SymbolKind::Function, "name", 2),
            item(SymbolKind::Function, "main", 0),
            item(SymbolKind::Macro, "m", 0),
        ]
    );
}

#[test]
fn incremental_parse_matches_a_full_parse() {
    let grammar = Grammar::compile(&grammar()).unwrap();
    let mut document = text::Document::from_text(MOCKUP);
    document.track_changes();
    let (_, mut tree) = parse(&grammar, MOCKUP);
    // Type at two places at once, undo and redo.
    document.set_selections(&text::Selections::from_vec(vec![
        text::Selection::cursor(MOCKUP.find("mod config").unwrap()),
        text::Selection::cursor(MOCKUP.find("Ok(())").unwrap()),
    ]));
    document.insert("let x = 1; ");
    document.undo();
    document.redo();
    for edit in document.take_changes().edits {
        tree.edit(&edit);
    }
    let rope = document.buffer().rope().clone();
    let incremental = SyntaxTree::parse(
        &grammar,
        &rope,
        Some(&tree),
        Duration::from_secs(5),
        &AtomicBool::new(false),
    )
    .unwrap();
    let (_, full) = parse(&grammar, &rope.to_string());
    assert_eq!(incremental.to_sexp(), full.to_sexp());
}

#[test]
fn incremental_parse_after_typing_at_many_cursors_matches_a_full_parse() {
    let grammar = Grammar::compile(&grammar()).unwrap();
    let source = "fn f() { g(1); }\n".repeat(2000);
    let mut document = text::Document::from_text(&source);
    document.track_changes();
    let (_, mut tree) = parse(&grammar, &source);
    // One cursor per line, more than are logged one by one.
    let buffer = document.buffer();
    let cursors = (0..2000)
        .map(|row| text::Selection::cursor(buffer.line_range(row).start + 9))
        .collect();
    document.set_selections(&text::Selections::from_vec(cursors));
    document.insert("let x = 1; ");
    document.undo();
    document.redo();
    let changes = document.take_changes();
    assert!(
        changes.edits.len() < 2000,
        "large steps are logged as one edit"
    );
    for edit in changes.edits {
        tree.edit(&edit);
    }
    let rope = document.buffer().rope().clone();
    let incremental = SyntaxTree::parse(
        &grammar,
        &rope,
        Some(&tree),
        Duration::from_secs(5),
        &AtomicBool::new(false),
    )
    .unwrap();
    let (_, full) = parse(&grammar, &rope.to_string());
    assert_eq!(incremental.to_sexp(), full.to_sexp());
}

#[test]
fn a_set_cancel_flag_stops_parsing() {
    let grammar = Grammar::compile(&grammar()).unwrap();
    let rope = Rope::from_str(&"fn f() {}\n".repeat(50_000));
    let result = SyntaxTree::parse(
        &grammar,
        &rope,
        None,
        Duration::from_secs(5),
        &AtomicBool::new(true),
    );
    assert_eq!(
        result.unwrap_err(),
        syntax::SyntaxError::Parse(syntax::ParseStop::Cancelled)
    );
}

#[test]
fn a_top_level_span_parses_like_the_whole_file() {
    let grammar = Grammar::compile(&grammar()).unwrap();
    let (rope, full) = parse(&grammar, MOCKUP);
    // Inside `main`: the span is the whole function.
    let inside = MOCKUP.find("Cli::parse").unwrap();
    let span = full.top_level_span(inside..inside + 1);
    assert_eq!(&MOCKUP[span.clone()][..7], "fn main");
    assert!(MOCKUP[span.clone()].ends_with('}'));
    let part = SyntaxTree::parse_range(
        &grammar,
        &rope,
        span.clone(),
        Duration::from_secs(5),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(
        syntax::highlights(&grammar, &part, &rope, span.clone()),
        syntax::highlights(&grammar, &full, &rope, span)
    );
}
