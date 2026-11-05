//! The built-in Rust language pack: file types, the tree-sitter grammar and
//! the highlight and outline queries. The language server part
//! (rust-analyzer) joins it in stage 4.

use language::LanguagePack;
use syntax::GrammarSpec;

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
    LanguagePack::new(grammar())
}

#[cfg(test)]
mod tests;
