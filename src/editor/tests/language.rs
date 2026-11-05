//! The editor with a language service: edits reach the service, its
//! diagnostics follow the text, completions insert, and a definition in
//! another file is reported for the owner to open.

#[cfg(test)]
mod tests {
    use std::ops::Range;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use editor::{Editor, EditorEvent, EditorSettings};
    use fs::{FakeFs, Fs};
    use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
    use gpui::{TestAppContext, WindowHandle};
    use language::{
        BoxFuture, Capabilities, CodeLens, Completion, CompletionCapabilities, CompletionEdit,
        CompletionList, CompletionTrigger, Diagnostic, Hover, InlayHint, LanguageService, Location,
        ServerStatus, ServiceError, ServiceEvent, Severity,
    };
    use text::{Buffer, Document, Point, TextChanges};

    /// A service that answers with fixed results and records what it is
    /// told.
    #[derive(Default)]
    struct FakeService {
        subscribers: Mutex<Vec<UnboundedSender<ServiceEvent>>>,
        /// Versions and texts of `change_document`.
        changes: Mutex<Vec<(i32, String)>>,
        completions: Vec<Completion>,
        definition: Option<Location>,
    }

    impl FakeService {
        fn publish(&self, path: &Path, diagnostics: &[Diagnostic]) {
            for subscriber in self.subscribers.lock().unwrap().iter() {
                let _ = subscriber.unbounded_send(ServiceEvent::Diagnostics {
                    path: path.to_path_buf(),
                    diagnostics: diagnostics.into(),
                    version: None,
                });
            }
        }
    }

    impl LanguageService for FakeService {
        fn name(&self) -> &'static str {
            "fake"
        }

        fn capabilities(&self) -> Capabilities {
            Capabilities {
                hover: true,
                completion: Some(CompletionCapabilities {
                    trigger_characters: vec![".".to_owned()],
                    resolve: language::ResolveCapability::default(),
                }),
                definition: true,
                inlay_hints: false,
                code_lens: None,
            }
        }

        fn status(&self) -> ServerStatus {
            ServerStatus::Ready
        }

        fn events(&self) -> UnboundedReceiver<ServiceEvent> {
            let (sender, receiver) = unbounded();
            self.subscribers.lock().unwrap().push(sender);
            receiver
        }

        fn open_document(&self, _: &Path, _: &str, _: i32, _: Buffer) {}

        fn change_document(&self, _: &Path, version: i32, _: TextChanges, text: Buffer) {
            self.changes
                .lock()
                .unwrap()
                .push((version, text.slice(0..text.len())));
        }

        fn save_document(&self, _: &Path) {}

        fn close_document(&self, _: &Path) {}

        fn hover(&self, _: &Path, _: Point) -> BoxFuture<Result<Option<Hover>, ServiceError>> {
            Box::pin(async { Ok(None) })
        }

        fn completions(
            &self,
            _: &Path,
            _: Point,
            _: CompletionTrigger,
        ) -> BoxFuture<Result<CompletionList, ServiceError>> {
            let items = self.completions.clone();
            Box::pin(async move {
                Ok(CompletionList {
                    items,
                    incomplete: false,
                })
            })
        }

        fn resolve_completion(
            &self,
            completion: Completion,
        ) -> BoxFuture<Result<Completion, ServiceError>> {
            Box::pin(async move { Ok(completion) })
        }

        fn definition(&self, _: &Path, _: Point) -> BoxFuture<Result<Vec<Location>, ServiceError>> {
            let found = self.definition.clone().into_iter().collect();
            Box::pin(async move { Ok(found) })
        }

        fn inlay_hints(
            &self,
            _: &Path,
            _: Range<Point>,
        ) -> BoxFuture<Result<Vec<InlayHint>, ServiceError>> {
            Box::pin(async { Ok(Vec::new()) })
        }

        fn code_lenses(&self, _: &Path) -> BoxFuture<Result<Vec<CodeLens>, ServiceError>> {
            Box::pin(async { Ok(Vec::new()) })
        }

        fn resolve_code_lens(&self, lens: CodeLens) -> BoxFuture<Result<CodeLens, ServiceError>> {
            Box::pin(async move { Ok(lens) })
        }

        fn shutdown(&self) -> BoxFuture<()> {
            Box::pin(async {})
        }
    }

    const PATH: &str = "/project/src/main.rs";

    fn completion(label: &str) -> Completion {
        Completion {
            label: label.to_owned(),
            kind: None,
            detail: None,
            documentation: None,
            filter_text: None,
            sort_text: None,
            edit: CompletionEdit {
                range: None,
                new_text: label.to_owned(),
            },
            additional_edits: Vec::new(),
            resolve: None,
        }
    }

    fn open(
        text: &str,
        service: &Arc<FakeService>,
        cx: &mut TestAppContext,
    ) -> WindowHandle<Editor> {
        cx.update(|cx| {
            let theme =
                theme::Theme::built_in(theme::DEFAULT_THEME, theme::Density::default()).unwrap();
            theme::set_theme(theme, cx);
        });
        let fs = Arc::new(FakeFs::new());
        fs.insert_file(PATH, text);
        let file: Arc<dyn Fs> = fs;
        let text = text.to_owned();
        let service: Arc<dyn LanguageService> = service.clone();
        let window = cx.add_window(move |window, cx| {
            let mut editor = Editor::new(
                Document::from_text(&text),
                Some((file, PATH.into())),
                EditorSettings::default(),
                window,
                cx,
            );
            editor.set_language_service(service, "rust", cx);
            editor
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

    fn text_of(window: WindowHandle<Editor>, cx: &mut TestAppContext) -> String {
        window
            .read_with(cx, |editor, _| {
                let buffer = editor.document().buffer();
                buffer.slice(0..buffer.len())
            })
            .unwrap()
    }

    fn command(window: WindowHandle<Editor>, id: &str, cx: &mut TestAppContext) {
        window
            .update(cx, |editor, window, cx| {
                assert!(editor.run_command(id, window, cx));
            })
            .unwrap();
    }

    #[gpui::test]
    fn edits_reach_the_service_and_diagnostics_are_counted(cx: &mut TestAppContext) {
        let service = Arc::new(FakeService::default());
        let window = open("fn main() {}\n", &service, cx);
        window
            .update(cx, |editor, _, cx| {
                editor.edit_document(cx, |document| document.insert("x"));
            })
            .unwrap();
        assert_eq!(
            service.changes.lock().unwrap().last().cloned(),
            Some((2, "xfn main() {}\n".to_owned()))
        );

        service.publish(
            Path::new(PATH),
            &[Diagnostic {
                range: Point::new(0, 0)..Point::new(0, 1),
                severity: Severity::Error,
                message: "unknown".to_owned(),
                source: Some("rustc".to_owned()),
                code: None,
            }],
        );
        settle(cx);
        let counts = window
            .read_with(cx, |editor, _| editor.problem_counts())
            .unwrap();
        assert_eq!(counts, (1, 0));
    }

    #[gpui::test]
    fn ctrl_space_lists_and_enter_accepts(cx: &mut TestAppContext) {
        let service = Arc::new(FakeService {
            completions: vec![completion("println"), completion("print")],
            ..FakeService::default()
        });
        let window = open("pri", &service, cx);
        window
            .update(cx, |editor, _, cx| {
                editor.edit_document(cx, |document| {
                    document.set_primary_selection(text::Selection::cursor(3));
                });
            })
            .unwrap();
        command(window, "editor.triggerSuggest", cx);
        settle(cx);
        // "print" is the shorter, tighter match of "pri".
        command(window, "editor.moveDown", cx);
        command(window, "editor.newline", cx);
        let text = text_of(window, cx);
        assert!(text == "println" || text == "print", "{text}");
        assert!(!text.contains('\n'), "Enter accepted instead of a new line");
    }

    #[gpui::test]
    fn definition_in_another_file_is_reported(cx: &mut TestAppContext) {
        let other = Location {
            path: PathBuf::from("/project/src/lib.rs"),
            range: Point::new(3, 4)..Point::new(3, 8),
        };
        let service = Arc::new(FakeService {
            definition: Some(other.clone()),
            ..FakeService::default()
        });
        let window = open("use lib::run;\n", &service, cx);
        let events = Arc::new(Mutex::new(Vec::new()));
        let recorded = events.clone();
        window
            .update(cx, |_, _, cx| {
                let editor = cx.entity();
                cx.subscribe(&editor, move |_, _, event: &EditorEvent, _| {
                    recorded.lock().unwrap().push(event.clone());
                })
                .detach();
            })
            .unwrap();
        command(window, "editor.goToDefinition", cx);
        settle(cx);
        assert!(
            events
                .lock()
                .unwrap()
                .contains(&EditorEvent::OpenLocation(other))
        );
    }
}
