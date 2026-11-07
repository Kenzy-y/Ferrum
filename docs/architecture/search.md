# Search

Crate `search` (layer 1, no GPUI): project-wide search (ADR 0012).

**Running.** `search::run(fs, root, &query, &options, &cancel, on_file)`
walks `root` and calls `on_file` with the `FileMatches` of every file that
matches, in path order, then returns a `SearchSummary` (files searched,
matches, files with matches, `limit_reached`, `cancelled`). It fails up
front with `SearchError::InvalidPattern` / `InvalidGlob`. Run it on a
background executor: the walk blocks on folder reads.

**Query.** `SearchQuery { pattern, case_sensitive, whole_word, regex,
include }` becomes a `grep_regex::RegexMatcher` (literal unless `regex`,
never across lines). `include` are gitignore-style globs relative to the
root (`src/**/*.rs`, `!target`) applied with `ignore::overrides`.

**Walk** (`walk.rs`). Only through `fs::Fs` (`read_dir`, `read_to_string`).
Skips `options.hidden` (`[files] hidden`), paths ignored by any
`.gitignore` on the way down (the innermost rule wins, as in git) and
`.git/info/exclude`; linked folders are not followed. Entries are visited
in name order.

**Matching** (`workers.rs`, `run.rs`). Up to 8 worker threads of the search
read and match files (`grep-searcher`, binary files skipped at the first
NUL, files over `max_file_bytes` skipped). Jobs carry a sequence number and
`InOrder` reports them back in walk order. Each `LineMatch` has the
zero-based line, byte ranges in the line and a `Preview` (indentation
dropped, cut around the first match, highlight ranges in the preview).
`options.overrides` replaces a file's text (unsaved editors). The search
stops at `max_results`, or before the next file once `cancel` is set;
dropping the pool joins the workers.

**Settings** `[search]` (`SearchSettings`): `debounce_ms` (pause after
typing, used by the view), `max_results`, `max_file_bytes`,
`preview_chars`.

**Benchmarks.** `cargo bench -p search`: 2 000 files, 16 MB on disk.

**Extending.** Replace (stage 5) adds edits computed from `FileMatches`;
a remote `Fs` works unchanged.
