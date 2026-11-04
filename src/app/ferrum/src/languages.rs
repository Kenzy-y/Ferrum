//! The built-in language packs: each registers its grammar here. A new
//! language is one more line and one more dependency; nothing else
//! changes.

use syntax::GrammarRegistry;

/// Grammars of every built-in language.
pub(crate) fn grammar_registry() -> anyhow::Result<GrammarRegistry> {
    let mut registry = GrammarRegistry::new();
    lang_rust::register(&mut registry)?;
    lang_toml::register(&mut registry)?;
    Ok(registry)
}
