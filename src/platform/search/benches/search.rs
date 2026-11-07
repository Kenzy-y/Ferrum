//! Benchmark of project-wide search on a generated project: 2 000 files of
//! 200 lines (~400 000 lines, ~16 MB) in nested folders, written once to
//! the temporary folder through `RealFs` (the in-memory `FakeFs` lists
//! folders in linear time and would dominate the numbers).
//!
//! Budget (AGENTS.md, section 8): the search runs in the background, so
//! it has no frame budget; results of a medium project should arrive in
//! well under a second.

#![allow(clippy::expect_used)]

use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use criterion::{Criterion, criterion_group, criterion_main};
use fs::{Fs, RealFs};
use futures::executor::block_on;
use search::{SearchOptions, SearchQuery};

/// Folder of the generated project, under the temporary folder.
const PROJECT: &str = "ferrum-search-bench";
const FILES: usize = 2_000;
const LINES: usize = 200;

fn root() -> PathBuf {
    std::env::temp_dir().join(PROJECT)
}

fn project() -> Arc<dyn Fs> {
    let fs: Arc<dyn Fs> = Arc::new(RealFs);
    let patterns = [
        "fn compute_value(input: &str) -> Result<u64, Error> {",
        "    let parsed = input.trim().parse::<u64>()?;",
        "    // Multiply by the configured factor and clamp.",
        "    Ok(parsed.saturating_mul(FACTOR).min(LIMIT))",
        "}",
        "",
    ];
    let mut text = String::new();
    for index in 0..LINES {
        text.push_str(patterns[index % patterns.len()]);
        text.push('\n');
    }
    let root = root();
    block_on(async {
        for index in 0..FILES {
            let folder = root.join(format!("crate_{}/src/module_{}", index % 20, index % 7));
            let path = folder.join(format!("file_{index}.rs"));
            if fs.metadata(&path).await.ok().flatten().is_some() {
                continue;
            }
            fs.create_dir_all(&folder).await.expect("bench folder");
            fs.write(&path, text.as_bytes()).await.expect("bench file");
        }
    });
    fs
}

fn search(fs: &Arc<dyn Fs>, query: &SearchQuery) -> usize {
    let options = SearchOptions::default();
    let cancel = AtomicBool::new(false);
    let mut files = 0;
    let summary = block_on(search::run(
        fs.clone(),
        Path::new(&root()),
        query,
        &options,
        &cancel,
        |_| files += 1,
    ));
    black_box(summary.map(|summary| summary.matches).unwrap_or_default()) + files
}

fn benches(criterion: &mut Criterion) {
    let fs = project();
    let mut group = criterion.benchmark_group("search");
    group.sample_size(10);
    // A word on every sixth line: many results, stops at `max_results`.
    group.bench_function("frequent literal", |bench| {
        let query = SearchQuery {
            pattern: "parsed".to_owned(),
            ..SearchQuery::default()
        };
        bench.iter(|| search(&fs, &query));
    });
    // No match anywhere: every byte is read and scanned.
    group.bench_function("absent literal", |bench| {
        let query = SearchQuery {
            pattern: "nowhere_to_be_found".to_owned(),
            ..SearchQuery::default()
        };
        bench.iter(|| search(&fs, &query));
    });
    group.bench_function("absent literal, match case", |bench| {
        let query = SearchQuery {
            pattern: "nowhere_to_be_found".to_owned(),
            case_sensitive: true,
            ..SearchQuery::default()
        };
        bench.iter(|| search(&fs, &query));
    });
    group.bench_function("regex whole word", |bench| {
        let query = SearchQuery {
            pattern: r"FACT\w+".to_owned(),
            regex: true,
            whole_word: true,
            case_sensitive: true,
            ..SearchQuery::default()
        };
        bench.iter(|| search(&fs, &query));
    });
    group.finish();
}

criterion_group!(search_benches, benches);
criterion_main!(search_benches);
