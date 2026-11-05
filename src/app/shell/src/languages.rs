//! The built-in language packs: each registers its grammar here. A new
//! language is one more line and one more dependency; nothing else
//! changes.

use language::LanguageRegistry;

/// Every built-in language.
pub(crate) fn language_registry() -> anyhow::Result<LanguageRegistry> {
    let mut registry = LanguageRegistry::new();
    registry.register(lang_rust::pack())?;
    registry.register(lang_toml::pack())?;
    Ok(registry)
}
