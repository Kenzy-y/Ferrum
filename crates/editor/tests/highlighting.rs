//! Highlighting and the outline follow edits made while parsing runs.

use std::path::Path;
use std::time::Duration;

use editor::{Editor, EditorSettings};
use gpui::TestAppContext;
use syntax::{GrammarRegistry, SymbolKind, SyntaxSettings};
use text::Document;

#[gpui::test]
fn outline_follows_edits_made_during_a_parse(cx: &mut TestAppContext) {
    let mut registry = GrammarRegistry::new();
    lang_rust::register(&mut registry).unwrap();
    let entry = registry.for_path(Path::new("main.rs"));
    cx.update(|cx| {
        let theme =
            theme::Theme::built_in(theme::DEFAULT_THEME, theme::Density::default()).unwrap();
        theme::set_theme(theme, cx);
    });
    let window = cx.add_window(|window, cx| {
        let document = Document::from_text("fn main() {}\n");
        let mut editor = Editor::new(document, None, EditorSettings::default(), window, cx);
        editor.set_grammar(entry, SyntaxSettings::default(), cx);
        // Typed before the first parse finishes.
        editor.go_to(0, cx);
        editor.edit_document(cx, |document| document.insert("struct Cli;\n"));
        editor
    });
    cx.run_until_parked();
    // The outline waits for a pause after the last parse.
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    let names = window
        .update(cx, |editor, _, _| {
            editor
                .outline()
                .iter()
                .map(|item| (item.kind, item.name.clone()))
                .collect::<Vec<_>>()
        })
        .unwrap();
    assert_eq!(
        names,
        vec![
            (SymbolKind::Struct, "Cli".to_owned()),
            (SymbolKind::Function, "main".to_owned())
        ]
    );
}
