//! The syntax tree of an editor's text: parsed in the background, moved by
//! every edit on the UI thread, queried for the visible highlights; the
//! outline is computed from it after a pause.
//!
//! At most one parse runs at a time. Edits made while it runs are kept and
//! replayed on its result, then the next parse starts from that tree, so
//! fast typing batches itself.

use std::cell::RefCell;
use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use gpui::{Context, Hsla, Task};
use syntax::{
    Grammar, GrammarEntry, HighlightSpan, OutlineItem, SyntaxError, SyntaxSettings, SyntaxTree,
};
use text::{Buffer, TextChanges, TextEdit};
use theme::SyntaxTokens;

use crate::editor::{Editor, EditorEvent};

/// Pause after the last parse before the outline is computed again.
const OUTLINE_DEBOUNCE: Duration = Duration::from_millis(150);

/// Highlights are cached for the visible text plus this many bytes on each
/// side at most, so small scrolls reuse them.
const CACHE_MARGIN_BYTES: usize = 16 * 1024;

/// Visible text longer than this (rows of huge lines) is highlighted per
/// row instead of cached as a whole.
const MAX_CACHED_BYTES: usize = 256 * 1024;

/// Whether highlighting works for the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    /// Waiting for the grammar or the first tree.
    Loading,
    Ready,
    /// Too large, too slow to parse, or the grammar is broken: plain text.
    Disabled,
}

/// A parse running in the background; dropping it cancels the parse.
struct ParseJob {
    cancel: Arc<AtomicBool>,
    task: Option<Task<()>>,
}

impl Drop for ParseJob {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// Highlights of a byte range for one version of the tree.
struct HighlightCache {
    generation: u64,
    range: Range<usize>,
    spans: Vec<HighlightSpan>,
}

pub(crate) struct SyntaxState {
    entry: Arc<GrammarEntry>,
    settings: SyntaxSettings,
    status: Status,
    grammar: Option<Arc<Grammar>>,
    /// The tree, moved through every edit since it was parsed.
    tree: Option<SyntaxTree>,
    /// Changes with every change of `tree`; highlights cached for another
    /// generation are stale.
    generation: u64,
    job: Option<ParseJob>,
    /// Edits since the running parse took its snapshot of the text.
    pending: Vec<TextEdit>,
    /// The whole text was replaced since the running parse started.
    pending_replaced: bool,
    outline: Arc<[OutlineItem]>,
    /// The waiting or running outline computation.
    outline_task: Option<Task<()>>,
    cache: RefCell<Option<HighlightCache>>,
}

impl SyntaxState {
    /// Highlights of `range` (bytes) of line `row`, sorted. Served from the
    /// cache when it covers the range; lines longer than
    /// `max_line_length` get none.
    pub(crate) fn highlights(
        &self,
        buffer: &Buffer,
        row: usize,
        range: Range<usize>,
    ) -> Vec<HighlightSpan> {
        let (Some(grammar), Some(tree)) = (&self.grammar, &self.tree) else {
            return Vec::new();
        };
        if buffer.line_range(row).len() > self.settings.max_line_length {
            return Vec::new();
        }
        if let Some(cache) = self.cache.borrow().as_ref()
            && cache.generation == self.generation
            && cache.range.start <= range.start
            && range.end <= cache.range.end
        {
            let first = cache
                .spans
                .partition_point(|span| span.range.end <= range.start);
            return cache.spans[first..]
                .iter()
                .take_while(|span| span.range.start < range.end)
                .cloned()
                .collect();
        }
        syntax::highlights(grammar, tree, buffer.rope(), range)
    }

    /// Makes the cache cover `visible` (the bytes of the visible rows), if
    /// it is not too long; called once per frame before rows are shaped.
    pub(crate) fn prepare(&self, buffer: &Buffer, visible: Range<usize>) {
        let (Some(grammar), Some(tree)) = (&self.grammar, &self.tree) else {
            return;
        };
        if visible.len() > MAX_CACHED_BYTES {
            return;
        }
        let covered = self.cache.borrow().as_ref().is_some_and(|cache| {
            cache.generation == self.generation
                && cache.range.start <= visible.start
                && visible.end <= cache.range.end
        });
        if covered {
            return;
        }
        let _span = tracing::trace_span!("editor.highlight", bytes = visible.len()).entered();
        let margin = visible.len().min(CACHE_MARGIN_BYTES);
        let range = visible.start.saturating_sub(margin)..(visible.end + margin).min(buffer.len());
        let spans = syntax::highlights(grammar, tree, buffer.rope(), range.clone());
        *self.cache.borrow_mut() = Some(HighlightCache {
            generation: self.generation,
            range,
            spans,
        });
    }

    /// The color of every highlight id in `tokens`: a dotted name falls
    /// back to its prefix (`function.method` → `function`); names the
    /// theme does not know are drawn plain.
    pub(crate) fn colors(&self, tokens: &SyntaxTokens) -> Vec<Option<Hsla>> {
        let Some(grammar) = &self.grammar else {
            return Vec::new();
        };
        let entries = tokens.entries();
        grammar
            .highlight_names()
            .iter()
            .map(|name| {
                let mut name = name.as_deref()?;
                loop {
                    if let Some((_, color)) = entries.iter().find(|(key, _)| *key == name) {
                        return Some(Hsla::from(*color));
                    }
                    name = name.rsplit_once('.')?.0;
                }
            })
            .collect()
    }
}

impl Editor {
    /// Highlights the text and computes its outline with the grammar of
    /// `entry` (`None`: plain text).
    pub fn set_grammar(
        &mut self,
        entry: Option<Arc<GrammarEntry>>,
        settings: SyntaxSettings,
        cx: &mut Context<Self>,
    ) {
        self.document.track_changes();
        self.document.take_changes();
        self.syntax = entry.map(|entry| SyntaxState {
            entry,
            settings,
            status: Status::Loading,
            grammar: None,
            tree: None,
            generation: 0,
            job: None,
            pending: Vec::new(),
            pending_replaced: false,
            outline: Arc::from([]),
            outline_task: None,
            cache: RefCell::new(None),
        });
        self.start_parse(cx);
        cx.emit(EditorEvent::OutlineChanged);
        cx.notify();
    }

    /// Applies changed `[syntax]` settings to the next parse.
    pub fn set_syntax_settings(&mut self, settings: SyntaxSettings) {
        if let Some(syntax) = &mut self.syntax {
            syntax.settings = settings;
        }
    }

    /// Id of the language of the text (for `[lang.<id>]` settings), if it
    /// has a grammar.
    pub fn language_id(&self) -> Option<&str> {
        self.syntax.as_ref().map(|syntax| syntax.entry.id())
    }

    /// Name of the language of the text, if it has a grammar.
    pub fn language_name(&self) -> Option<&str> {
        self.syntax.as_ref().map(|syntax| syntax.entry.name())
    }

    /// The outline of the text (empty without a grammar).
    pub fn outline(&self) -> Arc<[OutlineItem]> {
        self.syntax
            .as_ref()
            .map_or_else(|| Arc::from([]), |syntax| syntax.outline.clone())
    }

    /// The outline items around the primary cursor, outermost first.
    pub fn symbols_at_cursor(&self) -> Vec<OutlineItem> {
        let Some(syntax) = &self.syntax else {
            return Vec::new();
        };
        let head = self.document.selections().primary().head;
        syntax::items_at(&syntax.outline, head)
            .into_iter()
            .cloned()
            .collect()
    }

    /// Moves the tree through the text's changes; called after every
    /// edit.
    pub(crate) fn follow_text_changes(&mut self, cx: &mut Context<Self>) {
        let changes: TextChanges = self.document.take_changes();
        let Some(syntax) = &mut self.syntax else {
            return;
        };
        if changes.is_empty() || syntax.status == Status::Disabled {
            return;
        }
        if changes.replaced {
            syntax.tree = None;
            syntax.pending.clear();
            syntax.pending_replaced = syntax.job.is_some();
        } else {
            if let Some(tree) = &mut syntax.tree {
                for edit in &changes.edits {
                    tree.edit(edit);
                }
            }
            if syntax.job.is_some() {
                syntax.pending.extend(changes.edits);
            }
        }
        syntax.generation += 1;
        self.start_parse(cx);
    }

    /// Starts a parse of the current text unless one runs (its result is
    /// followed by another parse when the text changed meanwhile).
    fn start_parse(&mut self, cx: &mut Context<Self>) {
        let length = self.document.buffer().len();
        let Some(syntax) = &mut self.syntax else {
            return;
        };
        if syntax.job.is_some() || syntax.status == Status::Disabled {
            return;
        }
        if length > syntax.settings.max_file_bytes() {
            tracing::info!(bytes = length, "file too large to highlight");
            disable(syntax);
            return;
        }
        let entry = syntax.entry.clone();
        let old = syntax.tree.clone();
        let rope = self.document.buffer().rope().clone();
        let timeout = syntax.settings.parse_timeout();
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let task = cx.spawn(async move |editor, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let grammar = entry.load()?;
                    let tree = SyntaxTree::parse(&grammar, &rope, old.as_ref(), timeout, &flag)?;
                    Ok::<_, SyntaxError>((grammar, tree))
                })
                .await;
            let _ = editor.update(cx, |editor, cx| editor.finish_parse(result, cx));
        });
        syntax.job = Some(ParseJob {
            cancel,
            task: Some(task),
        });
    }

    fn finish_parse(
        &mut self,
        result: Result<(Arc<Grammar>, SyntaxTree), SyntaxError>,
        cx: &mut Context<Self>,
    ) {
        let Some(syntax) = &mut self.syntax else {
            return;
        };
        // This runs inside the job's own task: let it finish rather than
        // cancel it.
        if let Some(task) = syntax.job.take().and_then(|mut job| job.task.take()) {
            task.detach();
        }
        let replaced = std::mem::take(&mut syntax.pending_replaced);
        let pending = std::mem::take(&mut syntax.pending);
        match result {
            Ok((grammar, mut tree)) => {
                syntax.grammar = Some(grammar);
                syntax.status = Status::Ready;
                if replaced {
                    syntax.tree = None;
                } else {
                    for edit in &pending {
                        tree.edit(edit);
                    }
                    syntax.tree = Some(tree);
                }
                syntax.generation += 1;
                if replaced || !pending.is_empty() {
                    self.start_parse(cx);
                } else {
                    self.schedule_outline(cx);
                }
                cx.notify();
            }
            Err(SyntaxError::Parse(syntax::ParseStop::Cancelled)) => {}
            Err(error) => {
                tracing::warn!(%error, "highlighting is off for this file");
                disable(syntax);
                cx.emit(EditorEvent::OutlineChanged);
                cx.notify();
            }
        }
    }

    /// Computes the outline in the background after a pause; a newer
    /// parse replaces the waiting computation.
    fn schedule_outline(&mut self, cx: &mut Context<Self>) {
        let rope = self.document.buffer().rope().clone();
        let Some(syntax) = &mut self.syntax else {
            return;
        };
        let (Some(grammar), Some(tree)) = (syntax.grammar.clone(), syntax.tree.clone()) else {
            return;
        };
        let task = cx.spawn(async move |editor, cx| {
            cx.background_executor().timer(OUTLINE_DEBOUNCE).await;
            let items = cx
                .background_executor()
                .spawn(async move { syntax::outline(&grammar, &tree, &rope) })
                .await;
            let _ = editor.update(cx, |editor, cx| {
                if let Some(syntax) = &mut editor.syntax {
                    syntax.outline = items.into();
                    cx.emit(EditorEvent::OutlineChanged);
                }
            });
        });
        syntax.outline_task = Some(task);
    }
}

fn disable(syntax: &mut SyntaxState) {
    syntax.status = Status::Disabled;
    syntax.tree = None;
    syntax.job = None;
    syntax.outline = Arc::from([]);
    syntax.outline_task = None;
    *syntax.cache.borrow_mut() = None;
}
