//! Benchmarks of the text buffer on the files of the stage 3 budget:
//! 100 000 lines of code, and one line of several megabytes.
//!
//! Budget (AGENTS.md, section 8): operations run on a keystroke stay far
//! below 1 ms (the target here is under 100 µs) and do not grow with the
//! document; loading and saving touch the whole text and run in the
//! background.

use std::hint::black_box;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use text::{Buffer, Document, Movement, Point, Selection, Selections};

/// Lines in the large file.
const LINES: usize = 100_000;
/// Size of the single long line.
const LONG_LINE_BYTES: usize = 4 * 1024 * 1024;

/// About 100 000 lines that look like Rust code (~4 MB).
fn many_lines() -> String {
    let patterns = [
        "fn compute_value(input: &str) -> Result<u64, Error> {",
        "    let parsed = input.trim().parse::<u64>()?;",
        "    // Multiply by the configured factor and clamp.",
        "    Ok(parsed.saturating_mul(FACTOR).min(LIMIT))",
        "}",
        "",
    ];
    let mut text = String::with_capacity(LINES * 48);
    for index in 0..LINES {
        text.push_str(patterns[index % patterns.len()]);
        text.push('\n');
    }
    text
}

/// One line of several megabytes (minified data, logs).
fn long_line() -> String {
    "{\"key\":\"value\",\"n\":12345},".repeat(LONG_LINE_BYTES / 26)
}

fn document_with_cursor_in_middle(text: &str) -> Document {
    let mut document = Document::from_text(text);
    let middle = document.buffer().len() / 2;
    let middle = document.buffer().clamp(middle);
    document.set_selections(&Selections::single(Selection::cursor(middle)));
    document
}

fn benches(criterion: &mut Criterion) {
    for (name, text) in [("100k_lines", many_lines()), ("long_line_4mb", long_line())] {
        let mut group = criterion.benchmark_group(name);

        // Background work: building the buffer from file contents.
        group.bench_function("open", |bencher| {
            bencher.iter(|| Buffer::from_text(black_box(&text)));
        });
        group.bench_function("save_to_text", |bencher| {
            let buffer = Buffer::from_text(&text);
            bencher.iter(|| black_box(buffer.to_text()));
        });

        // Keystroke work: must not depend on the document size.
        group.bench_function("type_character", |bencher| {
            let mut document = document_with_cursor_in_middle(&text);
            bencher.iter(|| document.insert(black_box("x")));
        });
        // With changes tracked, as in an editor with a grammar: each edit
        // also logs its points, and the editor takes them after it.
        group.bench_function("type_character_tracked", |bencher| {
            let mut document = document_with_cursor_in_middle(&text);
            document.track_changes();
            bencher.iter(|| {
                document.insert(black_box("x"));
                black_box(document.take_changes())
            });
        });
        group.bench_function("backspace", |bencher| {
            bencher.iter_batched_ref(
                || document_with_cursor_in_middle(&text),
                Document::delete_backward,
                BatchSize::LargeInput,
            );
        });
        group.bench_function("type_then_undo_redo", |bencher| {
            let mut document = document_with_cursor_in_middle(&text);
            bencher.iter(|| {
                document.insert("x");
                document.undo();
                document.redo();
            });
        });
        group.bench_function("move_page_down_and_up", |bencher| {
            let mut document = document_with_cursor_in_middle(&text);
            bencher.iter(|| {
                document.move_cursors(Movement::PageDown(50), false);
                document.move_cursors(Movement::PageUp(50), false);
            });
        });
        group.bench_function("move_right_left", |bencher| {
            let mut document = document_with_cursor_in_middle(&text);
            bencher.iter(|| {
                document.move_cursors(Movement::Right, false);
                document.move_cursors(Movement::Left, false);
            });
        });
        group.bench_function("offset_to_point_and_back", |bencher| {
            let buffer = Buffer::from_text(&text);
            let offset = buffer.clamp(buffer.len() * 3 / 4);
            bencher.iter(|| {
                let point: Point = buffer.offset_to_point(black_box(offset));
                black_box(buffer.point_to_offset(point))
            });
        });
        group.finish();
    }
}

/// A document of 100 000 lines with `count` cursors spread evenly, each
/// after the first character of its line.
fn document_with_cursors(text: &str, count: usize) -> Document {
    let mut document = Document::from_text(text);
    let buffer = document.buffer();
    let step = buffer.line_count() / count;
    let cursors = (0..count)
        .map(|index| Selection::cursor(buffer.line_range(index * step).start + 1))
        .collect();
    document.set_selections(&Selections::from_vec(cursors));
    document
}

/// Editing at many cursors: time must grow linearly with the number of
/// cursors (compare 1000 and 5000), not quadratically.
fn many_cursors(criterion: &mut Criterion) {
    let text = many_lines();
    let mut group = criterion.benchmark_group("cursors");
    for count in [1000, 5000] {
        group.bench_function(format!("type_character_{count}"), |bencher| {
            bencher.iter_batched_ref(
                || document_with_cursors(&text, count),
                |document| document.insert("x"),
                BatchSize::LargeInput,
            );
        });
        group.bench_function(format!("type_character_{count}_tracked"), |bencher| {
            bencher.iter_batched_ref(
                || {
                    let mut document = document_with_cursors(&text, count);
                    document.track_changes();
                    document
                },
                |document| {
                    document.insert("x");
                    black_box(document.take_changes())
                },
                BatchSize::LargeInput,
            );
        });
        group.bench_function(format!("backspace_{count}"), |bencher| {
            bencher.iter_batched_ref(
                || document_with_cursors(&text, count),
                Document::delete_backward,
                BatchSize::LargeInput,
            );
        });
    }
    group.finish();
}

criterion_group!(buffer, benches, many_cursors);
criterion_main!(buffer);
