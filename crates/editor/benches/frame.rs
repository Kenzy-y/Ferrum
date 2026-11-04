//! Benchmarks of a frame of the editor against the stage 3 budget: 100 000
//! lines of Rust, and one line of several megabytes.
//!
//! A frame here is the editor's whole prepaint and paint: visible rows,
//! their highlights and text runs, current line, selections, guides,
//! cursors, line numbers, scrollbar and minimap. It runs on GPUI's test
//! platform, whose text system does not rasterize glyphs, so the numbers
//! are our own work per frame; the budget for a whole frame is 8 ms.

use std::path::Path;
use std::time::{Duration, Instant};

use criterion::{Criterion, criterion_group, criterion_main};
use editor::{Editor, EditorSettings};
use gpui::{Entity, IntoElement as _, TestAppContext, VisualTestContext, point, px, size};
use syntax::{GrammarRegistry, SyntaxSettings};
use text::Document;

/// Lines in the large file.
const LINES: usize = 100_000;
/// Size of the single long line.
const LONG_LINE_BYTES: usize = 4 * 1024 * 1024;

/// About 100 000 lines of Rust (~4 MB).
fn many_lines() -> String {
    let patterns = [
        "/// Computes the value.",
        "#[inline]",
        "fn compute_value(input: &str) -> Result<u64, Error> {",
        "    let parsed = input.trim().parse::<u64>()?;",
        "    // Multiply by the configured factor and clamp.",
        "    Ok(parsed.saturating_mul(FACTOR).min(LIMIT))",
        "}",
        "",
    ];
    let mut text = String::with_capacity(LINES * 40);
    for index in 0..LINES {
        text.push_str(patterns[index % patterns.len()]);
        text.push('\n');
    }
    text
}

/// One line of several megabytes: a huge array literal.
fn long_line() -> String {
    let item = "Item::new(\"key\", 12345), ";
    format!(
        "const DATA: &[Item] = &[{}];\n",
        item.repeat(LONG_LINE_BYTES / item.len())
    )
}

/// An editor of `text` with the Rust grammar in a 1920×1080 window, parsed,
/// with the cursor in the middle.
fn open(text: &str) -> (Entity<Editor>, &'static mut VisualTestContext) {
    let cx = Box::leak(Box::new(TestAppContext::single()));
    cx.update(|cx| {
        if let Ok(theme) = theme::Theme::built_in(theme::DEFAULT_THEME, theme::Density::default()) {
            theme::set_theme(theme, cx);
        }
    });
    let mut registry = GrammarRegistry::new();
    assert!(
        lang_rust::register(&mut registry).is_ok(),
        "the Rust grammar does not register"
    );
    let entry = registry.for_path(Path::new("main.rs"));
    let text = text.to_owned();
    let (editor, cx) = cx.add_window_view(move |window, cx| {
        let mut editor = Editor::new(
            Document::from_text(&text),
            None,
            EditorSettings {
                minimap: std::env::var("NO_MINIMAP").is_err(),
                ..EditorSettings::default()
            },
            window,
            cx,
        );
        editor.set_grammar(entry, SyntaxSettings::default(), cx);
        editor
    });
    cx.simulate_resize(size(px(1920.0), px(1080.0)));
    cx.run_until_parked();
    editor.update_in(cx, |editor, window, cx| {
        let middle = editor.document().buffer().len() / 2;
        editor.go_to(middle, cx);
        editor.focus(window, cx);
    });
    draw(&editor, cx);
    cx.run_until_parked();
    (editor, cx)
}

/// One frame of the editor.
fn draw(editor: &Entity<Editor>, cx: &mut VisualTestContext) {
    let view = editor.clone();
    cx.draw(
        point(px(0.0), px(0.0)),
        size(px(1920.0), px(1080.0)),
        |_, _| view.into_any_element(),
    );
}

/// Times `frames` frames after `before` changed the editor each time; the
/// background work it started (parsing) runs outside the timing.
fn time_frames(
    editor: &Entity<Editor>,
    cx: &mut VisualTestContext,
    frames: u64,
    mut before: impl FnMut(&Entity<Editor>, &mut VisualTestContext),
) -> Duration {
    let mut total = Duration::ZERO;
    for _ in 0..frames {
        before(editor, cx);
        let started = Instant::now();
        draw(editor, cx);
        total += started.elapsed();
        cx.run_until_parked();
    }
    total
}

fn command(editor: &Entity<Editor>, cx: &mut VisualTestContext, id: &str) {
    editor.update_in(cx, |editor, window, cx| {
        editor.run_command(id, window, cx);
    });
}

fn benches(criterion: &mut Criterion) {
    for (name, text) in [("100k_lines", many_lines()), ("long_line_4mb", long_line())] {
        let (editor, cx) = open(&text);
        let mut group = criterion.benchmark_group(format!("frame_{name}"));
        group.sample_size(20);
        // A repaint with nothing changed (cursor blink).
        group.bench_function("idle", |bencher| {
            bencher.iter_custom(|frames| time_frames(&editor, cx, frames, |_, _| {}));
        });
        // The frame after typing a character.
        group.bench_function("type_character", |bencher| {
            bencher.iter_custom(|frames| {
                time_frames(&editor, cx, frames, |editor, cx| {
                    editor.update(cx, |editor, cx| {
                        editor.edit_document(cx, |document| document.insert("x"));
                    });
                })
            });
        });
        // Scrolling by a page, down and up in turn.
        group.bench_function("scroll_page", |bencher| {
            let mut down = true;
            bencher.iter_custom(|frames| {
                time_frames(&editor, cx, frames, |editor, cx| {
                    let id = if down {
                        "editor.pageDown"
                    } else {
                        "editor.pageUp"
                    };
                    down = !down;
                    command(editor, cx, id);
                })
            });
        });
        group.finish();
    }
}

criterion_group! {
    name = frame;
    config = Criterion::default().measurement_time(Duration::from_secs(5));
    targets = benches
}
criterion_main!(frame);
