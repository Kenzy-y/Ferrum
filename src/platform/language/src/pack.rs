//! What a language pack declares.

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use text::{AutoPair, DEFAULT_PAIRS};

use process::ProcessRunner;
use settings::SettingsStore;
use syntax::GrammarSpec;

use crate::LanguageService;

/// What a service starts with.
pub struct ServerContext<'a> {
    /// The workspace folder.
    pub root: PathBuf,
    /// Starts the server process.
    pub process: Arc<dyn ProcessRunner>,
    /// The settings when the service starts; later changes come through
    /// [`LanguageService::configure`].
    pub settings: &'a SettingsStore,
}

/// Starts the language service of a pack.
pub trait ServerFactory: Send + Sync {
    /// Stable id of the server (`rust-analyzer`); one service runs per
    /// server and workspace.
    fn id(&self) -> &str;

    /// Starts a service for a workspace. Returns at once, without
    /// blocking: the service starts in the background and tells how it
    /// goes with [`crate::ServiceEvent::Status`]. A program that is not
    /// found ends as [`crate::ServerStatus::NotFound`], not as a panic or
    /// an error here.
    fn start(&self, context: ServerContext<'_>) -> Arc<dyn LanguageService>;
}

/// A language: its files, grammar and server. Built-in packs are Rust
/// crates; plugins declare the same in their manifest (stage 9).
#[derive(Clone)]
pub struct LanguagePack {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) extensions: Vec<String>,
    pub(crate) file_names: Vec<String>,
    pub(crate) grammar: Option<GrammarSpec>,
    pub(crate) server: Option<Arc<dyn ServerFactory>>,
    pub(crate) pairs: Vec<AutoPair>,
}

impl fmt::Debug for LanguagePack {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LanguagePack")
            .field("id", &self.id)
            .field("server", &self.server.as_ref().map(|server| server.id()))
            .finish_non_exhaustive()
    }
}

impl LanguagePack {
    /// A pack with a grammar; the language's id, name and files are the
    /// grammar's.
    pub fn new(grammar: GrammarSpec) -> Self {
        Self {
            id: grammar.id.clone(),
            name: grammar.name.clone(),
            extensions: grammar.extensions.clone(),
            file_names: grammar.file_names.clone(),
            grammar: Some(grammar),
            server: None,
            pairs: DEFAULT_PAIRS.to_vec(),
        }
    }

    /// A pack without a grammar: files are plain text, but a server may
    /// still serve them.
    pub fn without_grammar(
        id: impl Into<String>,
        name: impl Into<String>,
        extensions: Vec<String>,
        file_names: Vec<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            extensions,
            file_names,
            grammar: None,
            server: None,
            pairs: DEFAULT_PAIRS.to_vec(),
        }
    }

    /// Adds the language server.
    #[must_use]
    pub fn with_server(mut self, server: Arc<dyn ServerFactory>) -> Self {
        self.server = Some(server);
        self
    }

    /// Replaces the auto-closing pairs (by default
    /// [`text::DEFAULT_PAIRS`]).
    #[must_use]
    pub fn with_pairs(mut self, pairs: &[AutoPair]) -> Self {
        self.pairs = pairs.to_vec();
        self
    }

    /// Stable id of the language (`rust`).
    pub fn id(&self) -> &str {
        &self.id
    }
}
