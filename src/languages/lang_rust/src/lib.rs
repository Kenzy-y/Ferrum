//! The built-in Rust language pack: file types, the tree-sitter grammar,
//! the highlight and outline queries, rust-analyzer, and the toolchain
//! (`rustc` version, crate edition).

mod rust_analyzer;
mod toolchain;

use std::sync::Arc;

use language::{AutoPair, FileIconSpec, LanguagePack};
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

/// Pairs of Rust: no single quote, which also starts lifetimes (`'a`).
const PAIRS: &[AutoPair] = &[
    AutoPair::new('(', ')'),
    AutoPair::new('[', ']'),
    AutoPair::new('{', '}'),
    AutoPair::new('"', '"'),
];

/// The language pack.
pub fn pack() -> LanguagePack {
    LanguagePack::new(grammar())
        .with_pairs(PAIRS)
        .with_server(Arc::new(rust_analyzer::RustAnalyzerFactory))
        .with_toolchain(Arc::new(toolchain::RustToolchain))
        .with_file_icon(FileIconSpec {
            letter: "R",
            color_token: "icon.file.rust",
        })
}

#[cfg(test)]
mod tests;
