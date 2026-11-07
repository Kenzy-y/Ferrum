//! Languages by id and by file. Filled at startup from the packs, read-only
//! afterwards.

use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::sync::Arc;
use text::AutoPair;

use syntax::{GrammarEntry, GrammarRegistry, SyntaxError};

use crate::{FileIconSpec, LanguagePack, ServerFactory, Toolchain};

/// Why a pack could not be registered.
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    /// A language with this id is already registered.
    #[error("language `{0}` is registered twice")]
    Duplicate(String),
    /// The grammar could not be registered.
    #[error(transparent)]
    Grammar(#[from] SyntaxError),
}

/// A registered language.
pub struct LanguageEntry {
    id: String,
    name: String,
    grammar: Option<Arc<GrammarEntry>>,
    server: Option<Arc<dyn ServerFactory>>,
    pairs: Arc<[AutoPair]>,
    extensions: Vec<String>,
    file_names: Vec<String>,
    toolchain: Option<Arc<dyn Toolchain>>,
    file_icon: Option<FileIconSpec>,
}

impl fmt::Debug for LanguageEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LanguageEntry")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl LanguageEntry {
    /// Stable id (`rust`).
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Brackets and quotes that close themselves when typed.
    pub fn pairs(&self) -> &Arc<[AutoPair]> {
        &self.pairs
    }

    /// Name shown to the user (`Rust`).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The grammar, if the language has one.
    pub fn grammar(&self) -> Option<&Arc<GrammarEntry>> {
        self.grammar.as_ref()
    }

    /// The language server, if the language has one.
    pub fn server(&self) -> Option<&Arc<dyn ServerFactory>> {
        self.server.as_ref()
    }

    /// The toolchain, if the pack declares one.
    pub fn toolchain(&self) -> Option<&Arc<dyn Toolchain>> {
        self.toolchain.as_ref()
    }

    /// How the language's files are marked, if the pack says.
    pub fn file_icon(&self) -> Option<FileIconSpec> {
        self.file_icon
    }

    /// File extensions of the language, without the dot.
    pub fn extensions(&self) -> &[String] {
        &self.extensions
    }

    /// Exact file names of the language (`Cargo.lock`).
    pub fn file_names(&self) -> &[String] {
        &self.file_names
    }
}

/// Every language, by id, exact file name and extension.
#[derive(Debug, Default)]
pub struct LanguageRegistry {
    entries: Vec<Arc<LanguageEntry>>,
    by_file_name: HashMap<String, usize>,
    by_extension: HashMap<String, usize>,
    grammars: GrammarRegistry,
}

impl LanguageRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a pack.
    ///
    /// # Errors
    ///
    /// Fails if a language with the same id is already registered.
    pub fn register(&mut self, pack: LanguagePack) -> Result<(), RegistryError> {
        if self.get(&pack.id).is_some() {
            return Err(RegistryError::Duplicate(pack.id));
        }
        let grammar = match pack.grammar {
            Some(mut spec) => {
                // The pack's identity is the one source of truth.
                spec.id.clone_from(&pack.id);
                spec.name.clone_from(&pack.name);
                spec.extensions.clone_from(&pack.extensions);
                spec.file_names.clone_from(&pack.file_names);
                self.grammars.register(spec)?;
                self.grammars.get(&pack.id)
            }
            None => None,
        };
        let index = self.entries.len();
        for name in &pack.file_names {
            self.by_file_name.insert(name.clone(), index);
        }
        for extension in &pack.extensions {
            self.by_extension.insert(extension.to_lowercase(), index);
        }
        self.entries.push(Arc::new(LanguageEntry {
            id: pack.id,
            name: pack.name,
            grammar,
            server: pack.server,
            pairs: pack.pairs.into(),
            extensions: pack.extensions,
            file_names: pack.file_names,
            toolchain: pack.toolchain,
            file_icon: pack.file_icon,
        }));
        Ok(())
    }

    /// The language with id `id`.
    pub fn get(&self, id: &str) -> Option<&Arc<LanguageEntry>> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    /// The language of the file at `path`: by its exact name, then by its
    /// extension, ignoring case.
    pub fn for_path(&self, path: &Path) -> Option<&Arc<LanguageEntry>> {
        let name = path.file_name()?.to_str()?;
        let index = self.by_file_name.get(name).copied().or_else(|| {
            let (_, extension) = name.rsplit_once('.')?;
            self.by_extension.get(&extension.to_lowercase()).copied()
        })?;
        self.entries.get(index)
    }

    /// The grammars of the registered languages.
    pub fn grammars(&self) -> &GrammarRegistry {
        &self.grammars
    }

    /// Every registered language.
    pub fn iter(&self) -> impl Iterator<Item = &Arc<LanguageEntry>> {
        self.entries.iter()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn plain(id: &str, extensions: &[&str], file_names: &[&str]) -> LanguagePack {
        let strings = |items: &[&str]| items.iter().map(|item| (*item).to_owned()).collect();
        LanguagePack::without_grammar(
            id,
            id.to_uppercase(),
            strings(extensions),
            strings(file_names),
        )
    }

    #[test]
    fn finds_languages_by_file_name_then_extension() {
        let mut registry = LanguageRegistry::new();
        registry
            .register(plain("toml", &["toml"], &["Cargo.lock"]))
            .unwrap();
        registry.register(plain("lock", &["lock"], &[])).unwrap();
        let id = |path: &str| {
            registry
                .for_path(&PathBuf::from(path))
                .map(|entry| entry.id().to_owned())
        };
        assert_eq!(id("project/Cargo.lock").as_deref(), Some("toml"));
        assert_eq!(id("yarn.lock").as_deref(), Some("lock"));
        assert_eq!(id("CONFIG.TOML").as_deref(), Some("toml"));
        assert_eq!(id("README"), None);
        assert!(
            registry
                .get("toml")
                .is_some_and(|entry| entry.grammar().is_none())
        );
    }

    #[test]
    fn rejects_a_second_pack_with_the_same_id() {
        let mut registry = LanguageRegistry::new();
        registry.register(plain("toml", &["toml"], &[])).unwrap();
        assert!(matches!(
            registry.register(plain("toml", &["tml"], &[])),
            Err(RegistryError::Duplicate(_))
        ));
    }
}
