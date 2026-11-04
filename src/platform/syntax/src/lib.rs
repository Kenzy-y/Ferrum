//! Syntax trees with tree-sitter: highlighting and the outline.
//!
//! - [`GrammarSpec`] — what a language pack declares: the grammar, file
//!   names and extensions, highlight and outline queries.
//! - [`GrammarRegistry`] — grammars by file, filled at startup; queries
//!   compile on first use ([`GrammarEntry::load`], in the background).
//! - [`SyntaxTree`] — parsed in the background, incrementally after edits
//!   ([`SyntaxTree::edit`] with the [`text::TextEdit`]s of a document).
//! - [`highlights`] — colored spans of a byte range (what is visible).
//! - [`outline`] — named items with their nesting; [`items_at`] gives the
//!   items around the cursor.
//!
//! The crate knows no language: languages come from packs.

mod grammar;
mod highlight;
mod outline;
mod registry;
mod settings;
mod tree;

pub use grammar::{Grammar, GrammarSpec};
pub use highlight::{HighlightId, HighlightSpan, MAX_HIGHLIGHT_RANGE, highlights};
pub use outline::{OutlineItem, SymbolKind, items_at, outline};
pub use registry::{GrammarEntry, GrammarRegistry};
pub use settings::SyntaxSettings;
pub use tree::{ParseStop, SyntaxTree};
/// The grammar type packs give in [`GrammarSpec::language`]; grammar
/// crates convert into it (`tree_sitter_x::LANGUAGE.into()`).
pub use tree_sitter::Language;

/// Errors of grammars and parsing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SyntaxError {
    /// A query of a grammar does not compile.
    #[error("the {query} query of grammar `{grammar}` is invalid: {message}")]
    Query {
        /// Language id.
        grammar: String,
        /// Which query (`highlights`, `outline`).
        query: &'static str,
        /// What is wrong.
        message: String,
    },
    /// The grammar does not work with this version of tree-sitter.
    #[error("grammar `{grammar}` cannot be used: {message}")]
    Language {
        /// Language id.
        grammar: String,
        /// What is wrong.
        message: String,
    },
    /// Two grammars with one language id.
    #[error("grammar `{0}` is already registered")]
    Duplicate(String),
    /// Parsing stopped without a tree.
    #[error("parsing stopped: {0:?}")]
    Parse(ParseStop),
}
