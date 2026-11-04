//! The built-in TOML language pack: file types, the tree-sitter grammar and
//! the highlight and outline queries.

use syntax::{GrammarRegistry, GrammarSpec, SyntaxError};

/// Language id.
pub const ID: &str = "toml";

/// The syntax part of the pack.
pub fn grammar() -> GrammarSpec {
    GrammarSpec {
        id: ID.to_owned(),
        name: "TOML".to_owned(),
        extensions: vec!["toml".to_owned()],
        // Lock files are TOML without the extension saying so.
        file_names: vec!["Cargo.lock".to_owned()],
        language: tree_sitter_toml_ng::LANGUAGE.into(),
        highlights: include_str!("../queries/highlights.scm").into(),
        outline: include_str!("../queries/outline.scm").into(),
    }
}

/// Registers the pack's grammar.
///
/// # Errors
///
/// Fails if a grammar with the same id is already registered.
pub fn register(registry: &mut GrammarRegistry) -> Result<(), SyntaxError> {
    registry.register(grammar())
}

#[cfg(test)]
mod tests;
