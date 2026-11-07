//! Searching a fake project.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use fs::{FakeFs, Fs};
use futures::executor::block_on;
use search::{FileMatches, SearchError, SearchOptions, SearchQuery, SearchSettings, SearchSummary};

const ROOT: &str = "/work/forge";

fn project() -> FakeFs {
    let fs = FakeFs::new();
    fs.insert_file("/work/forge/.gitignore", "target/\n*.log\n");
    fs.insert_file(
        "/work/forge/src/main.rs",
        "mod config;\n\nfn main() {\n    let config = config::load();\n    Config::new();\n}\n",
    );
    fs.insert_file(
        "/work/forge/src/config.rs",
        "pub fn load() -> Config {\n    todo!()\n}\n",
    );
    fs.insert_file("/work/forge/README.md", "Configure with forge.toml\n");
    fs.insert_file("/work/forge/target/debug/out.rs", "config\n");
    fs.insert_file("/work/forge/build.log", "config\n");
    fs.insert_file("/work/forge/.git/config", "[core]\n");
    fs.insert_file("/work/forge/logo.png", b"\x89PNG\0\0config".to_vec());
    fs
}

fn query(pattern: &str) -> SearchQuery {
    SearchQuery {
        pattern: pattern.to_owned(),
        ..SearchQuery::default()
    }
}

fn options() -> SearchOptions {
    SearchOptions {
        hidden: vec![".git".to_owned()],
        ..SearchOptions::default()
    }
}

fn search_with(
    fs: FakeFs,
    query: &SearchQuery,
    options: &SearchOptions,
) -> Result<(Vec<FileMatches>, SearchSummary), SearchError> {
    let fs: Arc<dyn Fs> = Arc::new(fs);
    let cancel = AtomicBool::new(false);
    let mut files = Vec::new();
    let summary = block_on(search::run(
        fs,
        Path::new(ROOT),
        query,
        options,
        &cancel,
        |file| files.push(file),
    ))?;
    Ok((files, summary))
}

fn found(query: &SearchQuery) -> Vec<(String, usize, Vec<String>)> {
    let fs = project();
    let (files, _) = search_with(fs, query, &options()).unwrap();
    let mut rows = Vec::new();
    for file in files {
        let name = file
            .path
            .strip_prefix(ROOT)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        for line in file.lines {
            let texts = line
                .preview
                .highlights
                .iter()
                .map(|range| line.preview.text[range.clone()].to_owned())
                .collect();
            rows.push((name.clone(), line.line, texts));
        }
    }
    rows
}

fn names(query: &SearchQuery) -> Vec<String> {
    let mut names: Vec<_> = found(query).into_iter().map(|(name, ..)| name).collect();
    names.dedup();
    names
}

#[test]
fn finds_literal_text_case_insensitively_by_default() {
    let rows = found(&query("config"));
    assert_eq!(
        rows,
        vec![
            ("README.md".to_owned(), 0, vec!["Config".to_owned()]),
            ("src/config.rs".to_owned(), 0, vec!["Config".to_owned()]),
            ("src/main.rs".to_owned(), 0, vec!["config".to_owned()]),
            (
                "src/main.rs".to_owned(),
                3,
                vec!["config".to_owned(), "config".to_owned()]
            ),
            ("src/main.rs".to_owned(), 4, vec!["Config".to_owned()]),
        ]
    );
}

#[test]
fn ignored_hidden_and_binary_files_are_skipped() {
    let names = names(&query("config"));
    assert!(!names.iter().any(|name| name.starts_with("target")));
    assert!(!names.iter().any(|name| name == "build.log"));
    assert!(!names.iter().any(|name| name.starts_with(".git/")));
    assert!(!names.iter().any(|name| name == "logo.png"));
}

#[test]
fn case_sensitive_and_whole_word_narrow_the_matches() {
    let mut sensitive = query("Config");
    sensitive.case_sensitive = true;
    let rows = found(&sensitive);
    assert!(
        rows.iter()
            .all(|(_, _, texts)| texts.iter().all(|text| text == "Config"))
    );
    assert_eq!(rows.len(), 3, "README.md (Configure), config.rs, main.rs");

    let mut word = query("config");
    word.whole_word = true;
    // "Configure" is not the whole word.
    assert_eq!(names(&word), vec!["src/config.rs", "src/main.rs"]);
}

#[test]
fn regular_expressions_match_when_enabled() {
    let mut regex = query(r"fn \w+\(\)");
    assert!(
        found(&regex).is_empty(),
        "the pattern is literal text by default"
    );
    regex.regex = true;
    let rows = found(&regex);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].2, vec!["fn load()".to_owned()]);
}

#[test]
fn invalid_patterns_and_globs_are_errors() {
    let mut regex = query("(unclosed");
    regex.regex = true;
    let result = search_with(project(), &regex, &options());
    assert!(matches!(result, Err(SearchError::InvalidPattern(_))));

    let mut glob = query("config");
    glob.include = vec!["src/{".to_owned()];
    let result = search_with(project(), &glob, &options());
    assert!(matches!(result, Err(SearchError::InvalidGlob(_))));
}

#[test]
fn include_globs_pick_the_files() {
    let mut include = query("config");
    include.include = vec!["src/**/*.rs".to_owned()];
    assert_eq!(names(&include), vec!["src/config.rs", "src/main.rs"]);

    include.include = vec!["!src/main.rs".to_owned()];
    assert_eq!(names(&include), vec!["README.md", "src/config.rs"]);
}

#[test]
fn unsaved_text_is_searched_instead_of_the_file() {
    let mut options = options();
    options.overrides = HashMap::from([(
        PathBuf::from("/work/forge/README.md"),
        Arc::<str>::from("nothing here\nunsaved config\n"),
    )]);
    let (files, _) = search_with(project(), &query("config"), &options).unwrap();
    let readme = files
        .iter()
        .find(|file| file.path.ends_with("README.md"))
        .unwrap();
    assert_eq!(readme.lines.len(), 1);
    assert_eq!(readme.lines[0].line, 1);
    assert_eq!(readme.lines[0].ranges, vec![8..14]);
}

#[test]
fn the_search_stops_at_the_limit() {
    let mut options = options();
    options.settings = SearchSettings {
        max_results: 3,
        ..SearchSettings::default()
    };
    let (files, summary) = search_with(project(), &query("config"), &options).unwrap();
    assert!(summary.limit_reached);
    assert_eq!(summary.matches, 3);
    assert_eq!(files.iter().map(FileMatches::count).sum::<usize>(), 3);
}

#[test]
fn large_files_are_skipped() {
    let mut options = options();
    // README.md has 26 bytes, every other matching file more.
    options.settings.max_file_bytes = 30;
    let (files, _) = search_with(project(), &query("config"), &options).unwrap();
    assert_eq!(files.len(), 1);
    assert!(files[0].path.ends_with("README.md"));
}

#[test]
fn a_cancelled_search_reports_nothing_more() {
    let fs: Arc<dyn Fs> = Arc::new(project());
    let cancel = AtomicBool::new(true);
    let mut files = 0;
    let summary = block_on(search::run(
        fs,
        Path::new(ROOT),
        &query("config"),
        &options(),
        &cancel,
        |_| files += 1,
    ))
    .unwrap();
    assert!(summary.cancelled);
    assert_eq!(files, 0);
}

#[test]
fn an_empty_pattern_searches_nothing() {
    let (files, summary) = search_with(project(), &query(""), &options()).unwrap();
    assert!(files.is_empty());
    assert_eq!(summary, SearchSummary::default());
}
