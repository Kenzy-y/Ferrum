//! Benchmarks of syntax work on the files of the stage 3 budget: 100 000
//! lines of Rust, and one line of several megabytes.
//!
//! Budget (AGENTS.md, section 8): work on the UI thread per keystroke or
//! frame (moving the tree through an edit, highlighting the visible rows)
//! stays far below 1 ms; parsing and the outline run in the background.

use std::hint::black_box;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use syntax::{Grammar, SyntaxTree};
use text::{Document, Selection, Selections};

/// Lines in the large file.
const LINES: usize = 100_000;
/// Size of the single long line.
const LONG_LINE_BYTES: usize = 4 * 1024 * 1024;
/// Rows on a screen.
const VISIBLE_ROWS: usize = 60;
/// Bytes of a long line visible on a screen.
const VISIBLE_COLUMNS: usize = 300;

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

/// The value, or stops the benchmark with the error.
fn ok<T>(result: Result<T, syntax::SyntaxError>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("benchmark setup failed: {error}"),
    }
}

fn parse(grammar: &Grammar, document: &Document, old: Option<&SyntaxTree>) -> SyntaxTree {
    ok(SyntaxTree::parse(
        grammar,
        document.buffer().rope(),
        old,
        Duration::from_secs(60),
        &AtomicBool::new(false),
    ))
}

fn document_with_cursor_at(text: &str, offset: usize) -> Document {
    let mut document = Document::from_text(text);
    let offset = document.buffer().clamp(offset);
    document.set_selections(&Selections::single(Selection::cursor(offset)));
    document.track_changes();
    document
}

fn benches(criterion: &mut Criterion) {
    let grammar = ok(Grammar::compile(&lang_rust::grammar()));
    criterion.bench_function("compile_queries", |bencher| {
        let spec = lang_rust::grammar();
        bencher.iter(|| ok(Grammar::compile(black_box(&spec))));
    });
    for (name, text) in [("100k_lines", many_lines()), ("long_line_4mb", long_line())] {
        let mut group = criterion.benchmark_group(name);
        group.sample_size(10);
        let document = Document::from_text(&text);
        let tree = parse(&grammar, &document, None);
        let buffer = document.buffer();
        // The visible text in the middle of the file.
        let middle = buffer.clamp(buffer.len() / 2);
        let visible = if buffer.line_count() > 1 {
            let row = buffer.offset_to_point(middle).row;
            buffer.line_range(row).start..buffer.line_range(row + VISIBLE_ROWS).end
        } else {
            middle..buffer.clamp(middle + VISIBLE_COLUMNS)
        };

        // Background work.
        group.bench_function("parse", |bencher| {
            bencher.iter(|| parse(&grammar, &document, None));
        });
        group.bench_function("outline", |bencher| {
            bencher.iter(|| syntax::outline(&grammar, &tree, buffer.rope()));
        });
        group.bench_function("reparse_after_typing", |bencher| {
            bencher.iter_batched(
                || {
                    let mut document = document_with_cursor_at(&text, middle);
                    let mut edited = tree.clone();
                    document.insert("x");
                    for edit in document.take_changes().edits {
                        edited.edit(&edit);
                    }
                    (document, edited)
                },
                |(document, edited)| parse(&grammar, &document, Some(&edited)),
                BatchSize::LargeInput,
            );
        });

        // UI thread work.
        group.bench_function("edit_tree", |bencher| {
            bencher.iter_batched(
                || {
                    let mut document = document_with_cursor_at(&text, middle);
                    document.insert("x");
                    (tree.clone(), document.take_changes().edits)
                },
                |(mut edited, edits)| {
                    for edit in &edits {
                        edited.edit(edit);
                    }
                    edited
                },
                BatchSize::LargeInput,
            );
        });
        group.bench_function("highlight_visible", |bencher| {
            bencher.iter(|| syntax::highlights(&grammar, &tree, buffer.rope(), visible.clone()));
        });
        group.finish();
    }
}

criterion_group!(syntax_benches, benches);
criterion_main!(syntax_benches);
