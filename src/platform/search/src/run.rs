//! Running a search over a folder.

use std::collections::{BTreeMap, HashMap};
use std::io;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use fs::Fs;
use futures::StreamExt as _;
use grep_matcher::Matcher as _;
use grep_regex::RegexMatcher;
use grep_searcher::{Searcher, Sink, SinkMatch};
use tracing::Instrument as _;

use crate::walk::Walk;
use crate::workers::{Done, Job, Pool};
use crate::{Preview, SearchError, SearchQuery, SearchSettings};

/// How a search runs, besides the query.
#[derive(Debug, Clone, Default)]
pub struct SearchOptions {
    /// Limits from the settings.
    pub settings: SearchSettings,
    /// Gitignore-style patterns of paths never searched (`[files] hidden`).
    pub hidden: Vec<String>,
    /// Text searched instead of the file on disk (editors with unsaved
    /// changes).
    pub overrides: HashMap<PathBuf, Arc<str>>,
}

/// A line with matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineMatch {
    /// Zero-based line number.
    pub line: usize,
    /// Byte ranges of the matches in the line.
    pub ranges: Vec<Range<usize>>,
    /// What the results show of the line.
    pub preview: Preview,
}

/// The matches of one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMatches {
    /// The file.
    pub path: PathBuf,
    /// Its lines with matches, in order.
    pub lines: Vec<LineMatch>,
}

impl FileMatches {
    /// Number of matches in the file.
    pub fn count(&self) -> usize {
        self.lines.iter().map(|line| line.ranges.len()).sum()
    }
}

/// How a search ended.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SearchSummary {
    /// Files read and matched.
    pub files_searched: usize,
    /// Matches found.
    pub matches: usize,
    /// Files with matches.
    pub files_with_matches: usize,
    /// The search stopped at `max_results`.
    pub limit_reached: bool,
    /// The search was cancelled.
    pub cancelled: bool,
}

/// Searches every file under `root` for `query`, calling `on_file` for
/// each file with matches, in path order. Stops when `cancel` is set or
/// after `max_results` matches.
///
/// # Errors
///
/// Fails before searching if the pattern or an include glob is invalid.
pub async fn run(
    fs: Arc<dyn Fs>,
    root: &Path,
    query: &SearchQuery,
    options: &SearchOptions,
    cancel: &AtomicBool,
    on_file: impl FnMut(FileMatches),
) -> Result<SearchSummary, SearchError> {
    let span = tracing::debug_span!("search", pattern = %query.pattern);
    search(fs, root, query, options, cancel, on_file)
        .instrument(span)
        .await
}

async fn search(
    fs: Arc<dyn Fs>,
    root: &Path,
    query: &SearchQuery,
    options: &SearchOptions,
    cancel: &AtomicBool,
    on_file: impl FnMut(FileMatches),
) -> Result<SearchSummary, SearchError> {
    if query.is_empty() {
        return Ok(SearchSummary::default());
    }
    let matcher = query.matcher()?;
    let include = query.include_filter(root)?;
    let settings = &options.settings;
    let mut walk = Walk::new(fs.clone(), root, &options.hidden, include);
    let mut pool = Pool::start(fs, matcher, settings.clone());
    let mut order = InOrder::new(settings.max_results, on_file);
    let mut submitted = 0;
    while let Some(file) = walk.next().await {
        if cancel.load(Ordering::Relaxed) {
            order.summary.cancelled = true;
            return Ok(order.summary);
        }
        let text = options.overrides.get(&file.path).cloned();
        if text.is_none() && file.len > settings.max_file_bytes {
            continue;
        }
        pool.submit(Job {
            seq: submitted,
            path: file.path,
            text,
        });
        submitted += 1;
        while let Ok(done) = pool.results().try_recv() {
            if order.accept(done) {
                return Ok(order.summary);
            }
        }
    }
    pool.close();
    while order.next < submitted {
        if cancel.load(Ordering::Relaxed) {
            order.summary.cancelled = true;
            break;
        }
        let Some(done) = pool.results().next().await else {
            break;
        };
        if order.accept(done) {
            break;
        }
    }
    Ok(order.summary)
}

/// Puts the files the workers finish back in walk order and reports
/// them.
struct InOrder<F> {
    next: usize,
    waiting: BTreeMap<usize, Done>,
    max_results: usize,
    summary: SearchSummary,
    on_file: F,
}

impl<F: FnMut(FileMatches)> InOrder<F> {
    fn new(max_results: usize, on_file: F) -> Self {
        Self {
            next: 0,
            waiting: BTreeMap::new(),
            max_results,
            summary: SearchSummary::default(),
            on_file,
        }
    }

    /// Takes a finished file; returns whether the limit is reached.
    fn accept(&mut self, done: Done) -> bool {
        self.waiting.insert(done.seq, done);
        while let Some(done) = self.waiting.remove(&self.next) {
            self.next += 1;
            let Some(mut lines) = done.lines else {
                continue;
            };
            self.summary.files_searched += 1;
            if lines.is_empty() {
                continue;
            }
            let count = truncate(&mut lines, self.max_results - self.summary.matches);
            self.summary.matches += count;
            self.summary.files_with_matches += 1;
            (self.on_file)(FileMatches {
                path: done.path,
                lines,
            });
            if self.summary.matches >= self.max_results {
                self.summary.limit_reached = true;
                return true;
            }
        }
        false
    }
}

/// Keeps at most `limit` matches in `lines`; returns how many are kept.
fn truncate(lines: &mut Vec<LineMatch>, limit: usize) -> usize {
    let mut kept = 0;
    let mut keep_lines = 0;
    for line in lines.iter_mut() {
        if kept >= limit {
            break;
        }
        line.ranges.truncate(limit - kept);
        kept += line.ranges.len();
        keep_lines += 1;
    }
    lines.truncate(keep_lines);
    kept
}

/// The matching lines of `bytes`, `None` for binary data.
pub(crate) fn search_bytes(
    searcher: &mut Searcher,
    matcher: &RegexMatcher,
    bytes: &[u8],
    settings: &SearchSettings,
) -> Option<Vec<LineMatch>> {
    let mut sink = LineSink {
        matcher,
        preview_chars: settings.preview_chars,
        lines: Vec::new(),
        binary: false,
    };
    if let Err(error) = searcher.search_slice(matcher, bytes, &mut sink) {
        tracing::debug!(%error, "search failed");
        return None;
    }
    (!sink.binary).then_some(sink.lines)
}

/// Collects matching lines with the ranges of every match in them.
struct LineSink<'a> {
    matcher: &'a RegexMatcher,
    preview_chars: usize,
    lines: Vec<LineMatch>,
    binary: bool,
}

impl Sink for LineSink<'_> {
    type Error = io::Error;

    fn matched(&mut self, _searcher: &Searcher, found: &SinkMatch<'_>) -> Result<bool, io::Error> {
        let raw = found.bytes();
        let raw = raw.strip_suffix(b"\n").unwrap_or(raw);
        let raw = raw.strip_suffix(b"\r").unwrap_or(raw);
        let text = String::from_utf8_lossy(raw);
        let mut ranges = Vec::new();
        // Ranges are found in the lossy text, so they stay on character
        // boundaries even for invalid UTF-8.
        self.matcher
            .find_iter(text.as_bytes(), |found| {
                if !found.is_empty() {
                    ranges.push(found.start()..found.end());
                }
                true
            })
            .map_err(io::Error::other)?;
        if ranges.is_empty() {
            return Ok(true);
        }
        let line = found
            .line_number()
            .and_then(|number| usize::try_from(number).ok())
            .map_or(0, |number| number.saturating_sub(1));
        let preview = Preview::new(&text, &ranges, self.preview_chars);
        self.lines.push(LineMatch {
            line,
            ranges,
            preview,
        });
        Ok(true)
    }

    fn binary_data(&mut self, _searcher: &Searcher, _offset: u64) -> Result<bool, io::Error> {
        self.binary = true;
        Ok(false)
    }
}
