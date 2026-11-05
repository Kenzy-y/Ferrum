//! Highlighting and the outline follow edits made while parsing runs.

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::time::Duration;

    use editor::{Editor, EditorSettings};
    use gpui::{TestAppContext, WindowHandle};
    use syntax::{GrammarRegistry, SymbolKind, SyntaxSettings};
    use text::Document;

    /// An editor of `text` with the Rust grammar.
    fn rust_editor(text: &str, cx: &mut TestAppContext) -> WindowHandle<Editor> {
        let mut registry = GrammarRegistry::new();
        registry.register(lang_rust::grammar()).unwrap();
        let entry = registry.for_path(Path::new("main.rs"));
        cx.update(|cx| {
            let theme =
                theme::Theme::built_in(theme::DEFAULT_THEME, theme::Density::default()).unwrap();
            theme::set_theme(theme, cx);
        });
        let text = text.to_owned();
        cx.add_window(move |window, cx| {
            let document = Document::from_text(&text);
            let mut editor = Editor::new(document, None, EditorSettings::default(), window, cx);
            editor.set_grammar(entry, SyntaxSettings::default(), cx);
            editor
        })
    }

    /// Lets parses finish and the outline (which waits for a pause) follow.
    fn settle(cx: &mut TestAppContext) {
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
    }

    fn outline(window: WindowHandle<Editor>, cx: &mut TestAppContext) -> Vec<(SymbolKind, String)> {
        window
            .update(cx, |editor, _, _| {
                editor
                    .outline()
                    .iter()
                    .map(|item| (item.kind, item.name.clone()))
                    .collect()
            })
            .unwrap()
    }

    #[gpui::test]
    fn outline_follows_edits_made_during_a_parse(cx: &mut TestAppContext) {
        let window = rust_editor("fn main() {}\n", cx);
        // Typed before the first parse finishes.
        window
            .update(cx, |editor, _, cx| {
                editor.go_to(0, cx);
                editor.edit_document(cx, |document| document.insert("struct Cli;\n"));
            })
            .unwrap();
        settle(cx);
        assert_eq!(
            outline(window, cx),
            vec![
                (SymbolKind::Struct, "Cli".to_owned()),
                (SymbolKind::Function, "main".to_owned())
            ]
        );
    }

    #[gpui::test]
    fn deleting_a_large_text_starts_over_instead_of_editing_the_tree(cx: &mut TestAppContext) {
        // More than the bytes an edit may remove and still move the tree.
        let window = rust_editor(&"fn f() {}\n".repeat(10_000), cx);
        settle(cx);
        assert_eq!(outline(window, cx).len(), 10_000);
        window
            .update(cx, |editor, _, cx| {
                editor.edit_document(cx, |document| {
                    document.select_all();
                    document.delete_backward();
                    document.insert("fn g() {}");
                });
            })
            .unwrap();
        settle(cx);
        assert_eq!(
            outline(window, cx),
            vec![(SymbolKind::Function, "g".to_owned())]
        );
    }
}
