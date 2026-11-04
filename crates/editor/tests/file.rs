//! The editor follows its file on disk: renames, deletions.

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Arc;
    use std::time::Duration;

    use editor::{Editor, EditorSettings};
    use fs::{FakeFs, Fs};
    use gpui::{TestAppContext, WindowHandle};
    use text::Document;

    fn open(fs: &Arc<FakeFs>, path: &str, cx: &mut TestAppContext) -> WindowHandle<Editor> {
        cx.update(|cx| {
            let theme =
                theme::Theme::built_in(theme::DEFAULT_THEME, theme::Density::default()).unwrap();
            theme::set_theme(theme, cx);
        });
        let text =
            String::from_utf8(futures::executor::block_on(fs.read(Path::new(path))).unwrap())
                .unwrap();
        let file: Arc<dyn Fs> = fs.clone();
        let path = path.to_owned();
        let window = cx.add_window(move |window, cx| {
            Editor::new(
                Document::from_text(&text),
                Some((file, path.into())),
                EditorSettings::default(),
                window,
                cx,
            )
        });
        settle(cx);
        window
    }

    fn settle(cx: &mut TestAppContext) {
        for _ in 0..3 {
            cx.run_until_parked();
            cx.executor().advance_clock(Duration::from_secs(1));
        }
        cx.run_until_parked();
    }

    fn type_text(window: WindowHandle<Editor>, text: &str, cx: &mut TestAppContext) {
        window
            .update(cx, |editor, _, cx| {
                editor.edit_document(cx, |document| document.insert(text));
            })
            .unwrap();
    }

    /// Name of the editor's file (fake paths are joined with the platform
    /// separator, so whole paths differ in spelling).
    fn file_name(window: WindowHandle<Editor>, cx: &mut TestAppContext) -> String {
        window
            .update(cx, |editor, _, _| {
                let path = editor.path().unwrap();
                path.file_name().unwrap().to_string_lossy().into_owned()
            })
            .unwrap()
    }

    #[gpui::test]
    fn a_renamed_file_with_unsaved_edits_is_saved_under_its_new_name(cx: &mut TestAppContext) {
        let fs = Arc::new(FakeFs::new());
        fs.insert_file("/project/a.rs", "fn a() {}\n");
        let window = open(&fs, "/project/a.rs", cx);
        type_text(window, "// edited\n", cx);

        // A rename: the new name exists when the old one is reported gone.
        fs.insert_file("/project/b.rs", "fn a() {}\n");
        fs.remove("/project/a.rs");
        settle(cx);
        assert_eq!(file_name(window, cx), "b.rs");

        window.update(cx, |editor, _, cx| editor.save(cx)).unwrap();
        settle(cx);
        let saved = futures::executor::block_on(fs.read(Path::new("/project/b.rs"))).unwrap();
        assert_eq!(saved, b"// edited\nfn a() {}\n");
        let old = futures::executor::block_on(fs.metadata(Path::new("/project/a.rs"))).unwrap();
        assert!(old.is_none(), "the old name is not written again");
    }

    #[gpui::test]
    fn a_deleted_file_is_not_mistaken_for_another_one(cx: &mut TestAppContext) {
        let fs = Arc::new(FakeFs::new());
        fs.insert_file("/project/a.rs", "fn a() {}\n");
        let window = open(&fs, "/project/a.rs", cx);
        fs.insert_file("/project/other.rs", "fn other() {}\n");
        fs.remove("/project/a.rs");
        settle(cx);
        assert_eq!(file_name(window, cx), "a.rs");
    }
}
