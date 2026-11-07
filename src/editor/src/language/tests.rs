//! Inlay hints and code lenses through a drawn frame: asked for after the
//! delay, drawn in and after the lines, followed by clicks.

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fs::{FakeFs, Fs};
use futures::channel::mpsc::{UnboundedReceiver, unbounded};
use gpui::{
    Entity, IntoElement as _, Modifiers, TestAppContext, VisualTestContext, point, px, size,
};
use language::{
    BoxFuture, Capabilities, CodeLens, Completion, CompletionList, CompletionTrigger, Hover,
    InlayHint, InlayHintKind, LanguageService, LensAction, Location, ResolveCapability,
    ResolveToken, ServerStatus, ServiceError, ServiceEvent,
};
use text::{Buffer, Document, Point, TextChanges};

use crate::editor::{Editor, EditorEvent};
use crate::settings::EditorSettings;

const PATH: &str = "/project/src/main.rs";

/// Answers with fixed hints and lenses and counts the requests.
#[derive(Default)]
struct FakeService {
    hints: Vec<InlayHint>,
    lenses: Vec<CodeLens>,
    /// What resolving a lens gives.
    resolved: Option<CodeLens>,
    hint_requests: Mutex<Vec<Range<Point>>>,
    lens_requests: Mutex<usize>,
    completions: Vec<Completion>,
}

impl LanguageService for FakeService {
    fn name(&self) -> &'static str {
        "fake"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            hover: false,
            completion: Some(language::CompletionCapabilities {
                trigger_characters: Vec::new(),
                resolve: ResolveCapability::default(),
            }),
            definition: false,
            inlay_hints: true,
            code_lens: Some(ResolveCapability { resolve: true }),
        }
    }

    fn status(&self) -> ServerStatus {
        ServerStatus::Ready
    }

    fn events(&self) -> UnboundedReceiver<ServiceEvent> {
        unbounded().1
    }

    fn open_document(&self, _: &Path, _: &str, _: i32, _: Buffer) {}

    fn change_document(&self, _: &Path, _: i32, _: TextChanges, _: Buffer) {}

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

    fn resolve_completion(&self, item: Completion) -> BoxFuture<Result<Completion, ServiceError>> {
        Box::pin(async move { Ok(item) })
    }

    fn definition(&self, _: &Path, _: Point) -> BoxFuture<Result<Vec<Location>, ServiceError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn inlay_hints(
        &self,
        _: &Path,
        range: Range<Point>,
    ) -> BoxFuture<Result<Vec<InlayHint>, ServiceError>> {
        self.hint_requests.lock().unwrap().push(range);
        let hints = self.hints.clone();
        Box::pin(async move { Ok(hints) })
    }

    fn code_lenses(&self, _: &Path) -> BoxFuture<Result<Vec<CodeLens>, ServiceError>> {
        *self.lens_requests.lock().unwrap() += 1;
        let lenses = self.lenses.clone();
        Box::pin(async move { Ok(lenses) })
    }

    fn resolve_code_lens(&self, lens: CodeLens) -> BoxFuture<Result<CodeLens, ServiceError>> {
        let resolved = self.resolved.clone().unwrap_or(lens);
        Box::pin(async move { Ok(resolved) })
    }

    fn shutdown(&self) -> BoxFuture<()> {
        Box::pin(async {})
    }
}

fn location(path: &str, row: usize) -> Location {
    Location {
        path: PathBuf::from(path),
        range: Point::new(row, 0)..Point::new(row, 0),
    }
}

fn lens(row: usize, title: Option<&str>, locations: Vec<Location>) -> CodeLens {
    CodeLens {
        range: Point::new(row, 0)..Point::new(row, 2),
        title: title.map(str::to_owned),
        action: if title.is_some() {
            LensAction::ShowLocations(locations)
        } else {
            LensAction::Unsupported
        },
        resolve: title.is_none().then(|| ResolveToken(Arc::new(()))),
    }
}

fn open<'a>(
    text: &str,
    service: &Arc<FakeService>,
    cx: &'a mut TestAppContext,
) -> (Entity<Editor>, &'a mut VisualTestContext) {
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
    let (editor, cx) = cx.add_window_view(move |window, cx| {
        let mut editor = Editor::new(
            Document::from_text(&text),
            Some((file, PATH.into())),
            EditorSettings {
                minimap: false,
                ..EditorSettings::default()
            },
            window,
            cx,
        );
        editor.set_language_service(service, "rust", cx);
        editor
    });
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    settle(&editor, cx);
    (editor, cx)
}

fn draw(editor: &Entity<Editor>, cx: &mut VisualTestContext) {
    let view = editor.clone();
    cx.draw(
        point(px(0.0), px(0.0)),
        size(px(1200.0), px(800.0)),
        |_, _| view.into_any_element(),
    );
}

/// Draws, lets the delayed requests answer, and draws again until the
/// frames are stable.
fn settle(editor: &Entity<Editor>, cx: &mut VisualTestContext) {
    for _ in 0..4 {
        draw(editor, cx);
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
    }
    draw(editor, cx);
}

/// The drawn text of `row`.
fn drawn(editor: &Entity<Editor>, row: usize, cx: &mut VisualTestContext) -> String {
    editor.read_with(cx, |editor, _| {
        let layout = editor.layout.as_ref().unwrap();
        let shaped = layout.rows.iter().find(|shaped| shaped.row == row).unwrap();
        shaped.display.text.clone()
    })
}

fn hint(row: usize, column: usize, label: &str) -> InlayHint {
    InlayHint {
        position: Point::new(row, column),
        label: label.to_owned(),
        kind: InlayHintKind::Type,
        padding_left: false,
        padding_right: false,
    }
}

#[gpui::test]
fn hints_are_asked_for_the_view_and_drawn_in_the_line(cx: &mut TestAppContext) {
    let service = Arc::new(FakeService {
        hints: vec![hint(0, 7, ": i32")],
        ..FakeService::default()
    });
    let (editor, cx) = open("let cli = 1;\nlet x = 2;\n", &service, cx);
    assert_eq!(drawn(&editor, 0, cx), "let cli: i32 = 1;");
    assert_eq!(drawn(&editor, 1, cx), "let x = 2;");
    let requests = service.hint_requests.lock().unwrap().clone();
    assert_eq!(requests.len(), 1, "a current view is not asked again");
    assert_eq!(requests[0].start, Point::new(0, 0));

    // The cursor at the hint stays before it; typing there moves the hint
    // along and asks again.
    editor.update(cx, |editor, cx| {
        editor.go_to(7, cx);
        editor.edit_document(cx, |document| document.insert("s"));
    });
    draw(&editor, cx);
    assert_eq!(drawn(&editor, 0, cx), "let clis: i32 = 1;");
    settle(&editor, cx);
    assert_eq!(service.hint_requests.lock().unwrap().len(), 2);
}

#[gpui::test]
fn hints_can_be_turned_off(cx: &mut TestAppContext) {
    let service = Arc::new(FakeService {
        hints: vec![hint(0, 5, ": i32")],
        ..FakeService::default()
    });
    let (editor, cx) = open("let x = 1;\n", &service, cx);
    editor.update(cx, |editor, cx| {
        let settings = EditorSettings {
            inlay_hints: false,
            ..editor.settings.clone()
        };
        editor.set_settings(settings, cx);
    });
    settle(&editor, cx);
    assert_eq!(drawn(&editor, 0, cx), "let x = 1;");
}

/// Clicks the first drawn lens.
fn click_lens(editor: &Entity<Editor>, cx: &mut VisualTestContext) {
    let bounds = editor.read_with(cx, |editor, _| {
        editor.layout.as_ref().unwrap().lenses.first().unwrap().0
    });
    cx.simulate_click(bounds.center(), Modifiers::default());
    cx.run_until_parked();
}

#[gpui::test]
fn a_resolved_lens_is_drawn_and_a_click_goes_to_its_place(cx: &mut TestAppContext) {
    let target = location("/project/src/lib.rs", 3);
    // The second lens is a Run lens: the IDE cannot run yet, so it is not
    // drawn.
    let run = CodeLens {
        action: LensAction::Unsupported,
        ..lens(1, Some("Run"), Vec::new())
    };
    let service = Arc::new(FakeService {
        lenses: vec![lens(0, None, Vec::new()), run],
        resolved: Some(lens(0, Some("1 implementation"), vec![target.clone()])),
        ..FakeService::default()
    });
    let (editor, cx) = open("fn main() {}\nfn run() {}\n", &service, cx);
    let drawn_lenses = editor.read_with(cx, |editor, _| {
        let layout = editor.layout.as_ref().unwrap();
        let titles: Vec<(usize, String)> = editor
            .code_lenses_on(0)
            .into_iter()
            .map(|(index, title)| (index, title.to_owned()))
            .collect();
        (layout.lenses.len(), titles)
    });
    assert_eq!(drawn_lenses.0, 1);
    assert_eq!(drawn_lenses.1, vec![(0, "1 implementation".to_owned())]);
    assert_eq!(*service.lens_requests.lock().unwrap(), 1);

    let opened = Arc::new(Mutex::new(None));
    let sink = opened.clone();
    cx.update(|_, cx| {
        cx.subscribe(&editor, move |_, event: &EditorEvent, _| {
            if let EditorEvent::OpenLocation(location) = event {
                *sink.lock().unwrap() = Some(location.clone());
            }
        })
        .detach();
    });
    click_lens(&editor, cx);
    assert_eq!(opened.lock().unwrap().clone(), Some(target));
}

#[gpui::test]
fn a_lens_with_several_places_lists_them(cx: &mut TestAppContext) {
    let places = vec![location(PATH, 1), location("/project/src/lib.rs", 7)];
    let service = Arc::new(FakeService {
        lenses: vec![lens(0, Some("2 implementations"), places)],
        ..FakeService::default()
    });
    let (editor, cx) = open("trait T {}\nimpl T for () {}\n", &service, cx);
    click_lens(&editor, cx);
    assert!(editor.read_with(cx, |editor, _| editor.locations_open()));
    // Enter on the first place, in this file, moves the cursor there.
    editor.update_in(cx, |editor, window, cx| {
        editor.run_command("editor.newline", window, cx);
    });
    let (open, head, text) = editor.read_with(cx, |editor, _| {
        let document = editor.document();
        (
            editor.locations_open(),
            document.selections().primary().head,
            document.buffer().slice(0..document.buffer().len()),
        )
    });
    assert!(!open);
    assert_eq!(head, 11, "the start of the second line");
    assert_eq!(text, "trait T {}\nimpl T for () {}\n", "Enter did not type");
}

#[gpui::test]
fn a_click_on_a_lens_does_not_move_the_cursor(cx: &mut TestAppContext) {
    let service = Arc::new(FakeService {
        lenses: vec![lens(0, Some("0 references"), Vec::new())],
        ..FakeService::default()
    });
    let (editor, cx) = open("fn f() {}\n", &service, cx);
    click_lens(&editor, cx);
    let head = editor.read_with(cx, |editor, _| {
        editor.document().selections().primary().head
    });
    assert_eq!(head, 0);
}

fn completion(label: &str, label_detail: Option<&str>, description: Option<&str>) -> Completion {
    Completion {
        label: label.to_owned(),
        kind: Some(language::CompletionKind::Function),
        detail: Some("fn(*const T)".to_owned()),
        label_detail: label_detail.map(str::to_owned),
        description: description.map(str::to_owned),
        documentation: None,
        filter_text: None,
        sort_text: None,
        edit: language::CompletionEdit {
            range: None,
            new_text: label.to_owned(),
            tab_stops: Vec::new(),
        },
        additional_edits: Vec::new(),
        resolve: None,
    }
}

#[gpui::test]
fn long_completions_are_drawn_in_one_line(cx: &mut TestAppContext) {
    let long = "prefetch_write_instruction_with_a_much_longer_name_than_fits";
    let service = Arc::new(FakeService {
        completions: vec![
            completion("print!(…)", None, Some("macro_rules! print")),
            completion("prelude::", Some("(use core::prelude)"), None),
            completion(
                "prefetch_read_data(…)",
                Some("(use std::intrinsics::prefetch_read_data)"),
                Some("const fn(*const T)"),
            ),
            completion(
                long,
                Some("(use std::intrinsics::very::long::path::to::it)"),
                None,
            ),
            completion("Product", Some("(use std::iter::Product)"), None),
        ],
        ..FakeService::default()
    });
    let (editor, cx) = open(
        "fn main() {
    pr
}
",
        &service,
        cx,
    );
    editor.update(cx, |editor, cx| {
        editor.edit_document(cx, |document| {
            document.set_primary_selection(text::Selection::cursor(18));
        });
        editor.trigger_completion(CompletionTrigger::Invoked, cx);
    });
    // The window draws itself; popups are deferred, which a test draw of
    // a lone element does not support.
    frames(cx);
    assert!(editor.read_with(cx, |editor, _| editor.completion_open()));
    // Typing on filters and redraws without a panic.
    editor.update(cx, |editor, cx| {
        editor.edit_document(cx, |document| document.insert("e"));
    });
    frames(cx);
    assert!(editor.read_with(cx, |editor, _| editor.completion_open()));
}

fn frames(cx: &mut VisualTestContext) {
    for _ in 0..3 {
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
}
