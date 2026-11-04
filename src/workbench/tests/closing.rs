//! Closing the window with unsaved files asks first.

#[cfg(test)]
mod tests {
    use std::rc::Rc;
    use std::sync::Arc;
    use std::time::Duration;

    use commands::{CommandRegistry, Keymap};
    use editor::{Editor, EditorSettings};
    use fs::{FakeFs, Fs};
    use gpui::{AppContext as _, TestAppContext, WindowHandle};
    use i18n::I18n;
    use text::Document;
    use workbench::Workbench;

    fn workbench_with_dirty_file(cx: &mut TestAppContext) -> WindowHandle<Workbench> {
        cx.update(|cx| {
            let theme =
                theme::Theme::built_in(theme::DEFAULT_THEME, theme::Density::default()).unwrap();
            theme::set_theme(theme, cx);
        });
        let fs = Arc::new(FakeFs::new());
        fs.insert_file("/project/a.rs", "fn a() {}\n");
        let file: Arc<dyn Fs> = fs;
        let window = cx.add_window(|_, _| {
            Workbench::new(
                Arc::new(I18n::new("en").unwrap()),
                Rc::new(|_, _, _| {}),
                Arc::new(CommandRegistry::new()),
                Arc::new(Keymap::default()),
                Rc::new(|_, _, _| {}),
            )
        });
        window
            .update(cx, |workbench, window, cx| {
                let editor = cx.new(|cx| {
                    Editor::new(
                        Document::from_text("fn a() {}\n"),
                        Some((file, "/project/a.rs".into())),
                        EditorSettings::default(),
                        window,
                        cx,
                    )
                });
                editor.update(cx, |editor, cx| {
                    editor.edit_document(cx, |document| document.insert("// edit\n"));
                });
                workbench.open_editor(editor, window, cx);
            })
            .unwrap();
        cx.run_until_parked();
        window
    }

    #[gpui::test]
    fn closing_with_unsaved_files_asks_and_cancel_keeps_the_window(cx: &mut TestAppContext) {
        let window = workbench_with_dirty_file(cx);
        let close_now = window
            .update(cx, |workbench, window, cx| {
                workbench.should_close(window, cx)
            })
            .unwrap();
        assert!(!close_now, "the window waits for an answer");
        assert!(cx.has_pending_prompt());
        cx.simulate_prompt_answer("Cancel");
        cx.run_until_parked();
        assert!(window.update(cx, |_, _, _| ()).is_ok(), "still open");
    }

    #[gpui::test]
    fn dont_save_closes_the_window(cx: &mut TestAppContext) {
        let window = workbench_with_dirty_file(cx);
        window
            .update(cx, |workbench, window, cx| {
                let _ = workbench.should_close(window, cx);
            })
            .unwrap();
        cx.simulate_prompt_answer("Don't save");
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(10));
        cx.run_until_parked();
        assert!(window.update(cx, |_, _, _| ()).is_err(), "closed");
    }
}
