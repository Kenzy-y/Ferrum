//! The built-in Rust language pack: file types, the tree-sitter grammar,
//! the highlight and outline queries, and rust-analyzer.

mod rust_analyzer;

use std::sync::Arc;

use language::LanguagePack;
use syntax::GrammarSpec;

pub use rust_analyzer::RustAnalyzerSettings;

/// Language id.
pub const ID: &str = "rust";

/// The syntax part of the pack.
pub fn grammar() -> GrammarSpec {
    GrammarSpec {
        id: ID.to_owned(),
        name: "Rust".to_owned(),
        extensions: vec!["rs".to_owned()],
        file_names: Vec::new(),
        language: tree_sitter_rust::LANGUAGE.into(),
        highlights: include_str!("../queries/highlights.scm").into(),
        outline: include_str!("../queries/outline.scm").into(),
    }
}

/// The language pack.
pub fn pack() -> LanguagePack {
    LanguagePack::new(grammar()).with_server(Arc::new(rust_analyzer::RustAnalyzerFactory))
}

#[cfg(test)]
mod tests;
