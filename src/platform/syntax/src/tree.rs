//! Parsing a rope into a syntax tree, incrementally after edits.

use std::ops::{ControlFlow, Range};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use ropey::Rope;
use text::TextEdit;
use tree_sitter::{InputEdit, ParseOptions, Parser, Point, Tree};

use crate::{Grammar, SyntaxError};

/// A syntax tree of a text. Cloning is cheap (the nodes are shared).
#[derive(Clone)]
pub struct SyntaxTree {
    pub(crate) tree: Tree,
}

impl std::fmt::Debug for SyntaxTree {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SyntaxTree")
            .field("root", &self.tree.root_node().kind())
            .finish()
    }
}

/// Why parsing stopped without a tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseStop {
    /// The cancel flag was set.
    Cancelled,
    /// It took longer than allowed.
    TimedOut,
}

impl SyntaxTree {
    /// Parses `rope`. With `old`, a tree of an earlier version of the text
    /// already moved by [`Self::edit`] through every edit since, only the
    /// changed parts are parsed again. Runs in the background; stops when
    /// `cancel` is set or after `timeout`.
    ///
    /// # Errors
    ///
    /// [`SyntaxError::Parse`] when parsing stopped (cancelled or timed
    /// out), [`SyntaxError::Language`] when the grammar is incompatible
    /// with the tree-sitter library.
    pub fn parse(
        grammar: &Grammar,
        rope: &Rope,
        old: Option<&SyntaxTree>,
        timeout: Duration,
        cancel: &AtomicBool,
    ) -> Result<Self, SyntaxError> {
        let _span = tracing::debug_span!(
            "syntax.parse",
            grammar = grammar.id(),
            bytes = rope.len_bytes(),
            incremental = old.is_some()
        )
        .entered();
        let mut parser = parser(grammar)?;
        run(&mut parser, rope, old, timeout, cancel)
    }

    /// Parses only `range` of `rope` as a text of its own; nodes keep the
    /// offsets of the rope. For a quick tree of the text around an edit
    /// while the whole text is parsed again: `range` should be a span that
    /// parses on its own ([`Self::top_level_span`]).
    ///
    /// # Errors
    ///
    /// As [`Self::parse`].
    pub fn parse_range(
        grammar: &Grammar,
        rope: &Rope,
        range: Range<usize>,
        timeout: Duration,
        cancel: &AtomicBool,
    ) -> Result<Self, SyntaxError> {
        let _span = tracing::trace_span!(
            "syntax.parse_range",
            grammar = grammar.id(),
            bytes = range.len()
        )
        .entered();
        let mut parser = parser(grammar)?;
        let end = range.end.min(rope.len_bytes());
        let start = range.start.min(end);
        let point_at = |offset: usize| {
            let row = rope.byte_to_line(offset);
            Point::new(row, offset - rope.line_to_byte(row))
        };
        parser
            .set_included_ranges(&[tree_sitter::Range {
                start_byte: start,
                end_byte: end,
                start_point: point_at(start),
                end_point: point_at(end),
            }])
            .map_err(|error| SyntaxError::Language {
                grammar: grammar.id().to_owned(),
                message: error.to_string(),
            })?;
        run(&mut parser, rope, None, timeout, cancel)
    }

    /// The span of the top-level nodes that overlap `range`: whole items
    /// (functions, tables), which parse correctly on their own. Logarithmic
    /// in the number of items.
    pub fn top_level_span(&self, range: Range<usize>) -> Range<usize> {
        let root = self.tree.root_node();
        let child_at = |offset: usize| {
            let mut cursor = root.walk();
            cursor
                .goto_first_child_for_byte(offset)
                .map(|_| cursor.node().byte_range())
        };
        let start = child_at(range.start).map_or(range.start, |node| node.start.min(range.start));
        let end = child_at(range.end.saturating_sub(1).max(range.start))
            .map_or(range.end, |node| node.end.max(range.end));
        start..end
    }

    /// Moves the tree through `edit`, so its nodes keep matching the text
    /// until it is parsed again: unchanged nodes shift, the edited ones are
    /// marked for the next parse. Cheap; fine on the UI thread.
    pub fn edit(&mut self, edit: &TextEdit) {
        self.tree.edit(&input_edit(edit));
    }

    /// Whether the tree contains syntax errors.
    pub fn has_errors(&self) -> bool {
        self.tree.root_node().has_error()
    }

    /// The tree as an S-expression (tests, debugging).
    pub fn to_sexp(&self) -> String {
        self.tree.root_node().to_sexp()
    }
}

fn parser(grammar: &Grammar) -> Result<Parser, SyntaxError> {
    let mut parser = Parser::new();
    parser
        .set_language(&grammar.language)
        .map_err(|error| SyntaxError::Language {
            grammar: grammar.id().to_owned(),
            message: error.to_string(),
        })?;
    Ok(parser)
}

/// Parses with `parser`, stopping on `cancel` or after `timeout`.
fn run(
    parser: &mut Parser,
    rope: &Rope,
    old: Option<&SyntaxTree>,
    timeout: Duration,
    cancel: &AtomicBool,
) -> Result<SyntaxTree, SyntaxError> {
    let started = Instant::now();
    let mut stop = None;
    let mut progress = |_: &tree_sitter::ParseState| {
        if cancel.load(Ordering::Relaxed) {
            stop = Some(ParseStop::Cancelled);
            ControlFlow::Break(())
        } else if started.elapsed() > timeout {
            stop = Some(ParseStop::TimedOut);
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    };
    let mut read = |offset: usize, _: Point| chunk_at(rope, offset);
    let tree = parser.parse_with_options(
        &mut read,
        old.map(|old| &old.tree),
        Some(ParseOptions::new().progress_callback(&mut progress)),
    );
    match tree {
        Some(tree) => Ok(SyntaxTree { tree }),
        None => Err(SyntaxError::Parse(stop.unwrap_or(ParseStop::Cancelled))),
    }
}

/// The rest of the rope chunk containing `offset`, as tree-sitter reads it.
fn chunk_at(rope: &Rope, offset: usize) -> &[u8] {
    if offset >= rope.len_bytes() {
        return &[];
    }
    let (chunk, chunk_start, _, _) = rope.chunk_at_byte(offset);
    &chunk.as_bytes()[offset - chunk_start..]
}

fn point(point: text::Point) -> Point {
    Point::new(point.row, point.column)
}

/// `edit` for tree-sitter. Rows follow the buffer's lines; the trees are
/// only queried by byte offsets, so rows of rare line breaks other than
/// `\n` (which tree-sitter does not count) do not matter.
fn input_edit(edit: &TextEdit) -> InputEdit {
    InputEdit {
        start_byte: edit.start,
        old_end_byte: edit.old_end,
        new_end_byte: edit.new_end,
        start_position: point(edit.start_point),
        old_end_position: point(edit.old_end_point),
        new_end_position: point(edit.new_end_point),
    }
}
