//! The grammars of all languages, by file. Filled at startup by language
//! packs (built-in ones now, plugins later), read-only afterwards.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use crate::{Grammar, GrammarSpec, SyntaxError};

/// A registered grammar; its queries compile on first use.
#[derive(Debug)]
pub struct GrammarEntry {
    spec: GrammarSpec,
    compiled: OnceLock<Result<Arc<Grammar>, SyntaxError>>,
}

impl GrammarEntry {
    /// Id of the language.
    pub fn id(&self) -> &str {
        &self.spec.id
    }

    /// Name of the language.
    pub fn name(&self) -> &str {
        &self.spec.name
    }

    /// The compiled grammar. The first call compiles the queries, which
    /// takes milliseconds: call it in the background.
    ///
    /// # Errors
    ///
    /// Fails (every time) if the queries do not compile.
    pub fn load(&self) -> Result<Arc<Grammar>, SyntaxError> {
        self.compiled
            .get_or_init(|| Grammar::compile(&self.spec).map(Arc::new))
            .clone()
    }
}

/// Grammars by language id, file name and extension.
#[derive(Debug, Default)]
pub struct GrammarRegistry {
    entries: Vec<Arc<GrammarEntry>>,
    by_file_name: HashMap<String, usize>,
    by_extension: HashMap<String, usize>,
}

impl GrammarRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a grammar. Nothing is compiled yet.
    ///
    /// # Errors
    ///
    /// Fails if the language id is already registered.
    pub fn register(&mut self, spec: GrammarSpec) -> Result<(), SyntaxError> {
        if self.get(&spec.id).is_some() {
            return Err(SyntaxError::Duplicate(spec.id));
        }
        let index = self.entries.len();
        for name in &spec.file_names {
            self.by_file_name.insert(name.clone(), index);
        }
        for extension in &spec.extensions {
            self.by_extension.insert(extension.to_lowercase(), index);
        }
        self.entries.push(Arc::new(GrammarEntry {
            spec,
            compiled: OnceLock::new(),
        }));
        Ok(())
    }

    /// The grammar with language id `id`.
    pub fn get(&self, id: &str) -> Option<Arc<GrammarEntry>> {
        self.entries.iter().find(|entry| entry.id() == id).cloned()
    }

    /// The grammar for the file at `path`: by its exact name, then by its
    /// extension.
    pub fn for_path(&self, path: &Path) -> Option<Arc<GrammarEntry>> {
        let name = path.file_name()?.to_str()?;
        let index = self.by_file_name.get(name).copied().or_else(|| {
            let (_, extension) = name.rsplit_once('.')?;
            self.by_extension.get(&extension.to_lowercase()).copied()
        })?;
        self.entries.get(index).cloned()
    }

    /// Every registered grammar.
    pub fn iter(&self) -> impl Iterator<Item = &Arc<GrammarEntry>> {
        self.entries.iter()
    }
}
