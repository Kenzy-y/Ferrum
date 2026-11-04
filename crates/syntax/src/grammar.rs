//! Grammars: what a language pack declares and what it compiles into.

use std::borrow::Cow;

use tree_sitter::{Language, Query};

use crate::SyntaxError;
use crate::outline::OutlineQuery;

/// What a language pack declares about the syntax of its language.
#[derive(Debug, Clone)]
pub struct GrammarSpec {
    /// Stable id of the language (`rust`, `toml`).
    pub id: String,
    /// Name shown to the user (`Rust`, `TOML`).
    pub name: String,
    /// File extensions without the dot, matched without regard to case.
    pub extensions: Vec<String>,
    /// Exact file names (`Cargo.lock`); they win over extensions.
    pub file_names: Vec<String>,
    /// The tree-sitter grammar.
    pub language: Language,
    /// Highlight query. Capture names are syntax token keys of the theme
    /// (`keyword`, `type`, `doc_comment`, …); a dotted name falls back to
    /// its prefix (`function.method` → `function`). Captures starting with
    /// `_` only serve predicates. Where captures overlap, the pattern
    /// written earlier in the query wins.
    pub highlights: Cow<'static, str>,
    /// Outline query: `@outline.<kind>` captures the whole item (see
    /// [`crate::SymbolKind`]), `@name` its name; `@context` nodes are
    /// shown with the name (`impl Display for Cli`). Empty: no outline.
    pub outline: Cow<'static, str>,
}

/// A compiled grammar: the language and its queries.
pub struct Grammar {
    id: String,
    name: String,
    pub(crate) language: Language,
    pub(crate) highlights: Query,
    /// Highlight name of every capture index; `None` for helper captures.
    highlight_names: Vec<Option<String>>,
    pub(crate) outline: Option<OutlineQuery>,
}

impl std::fmt::Debug for Grammar {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Grammar")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl Grammar {
    /// Compiles the queries of `spec`. Takes milliseconds: run it in the
    /// background.
    ///
    /// # Errors
    ///
    /// Fails if a query does not compile or names an unknown outline kind.
    pub fn compile(spec: &GrammarSpec) -> Result<Self, SyntaxError> {
        let _span = tracing::debug_span!("syntax.compile", grammar = %spec.id).entered();
        let query_error = |query: &'static str, message: String| SyntaxError::Query {
            grammar: spec.id.clone(),
            query,
            message,
        };
        let highlights = Query::new(&spec.language, &spec.highlights)
            .map_err(|error| query_error("highlights", error.to_string()))?;
        let highlight_names = highlights
            .capture_names()
            .iter()
            .map(|name| (!name.starts_with('_')).then(|| (*name).to_owned()))
            .collect();
        let outline = if spec.outline.trim().is_empty() {
            None
        } else {
            let query = Query::new(&spec.language, &spec.outline)
                .map_err(|error| query_error("outline", error.to_string()))?;
            Some(OutlineQuery::new(query).map_err(|message| query_error("outline", message))?)
        };
        Ok(Self {
            id: spec.id.clone(),
            name: spec.name.clone(),
            language: spec.language.clone(),
            highlights,
            highlight_names,
            outline,
        })
    }

    /// Id of the language.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Name of the language.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The highlight name of every [`crate::HighlightId`], by index;
    /// `None` for captures that never color text.
    pub fn highlight_names(&self) -> &[Option<String>] {
        &self.highlight_names
    }
}
