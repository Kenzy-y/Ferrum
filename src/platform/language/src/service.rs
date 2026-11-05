//! The [`LanguageService`] trait.

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

use futures::channel::mpsc::UnboundedReceiver;
use settings::SettingsStore;
use text::{Buffer, Point, TextChanges};

use crate::{
    CodeLens, Completion, CompletionList, Diagnostic, Hover, InlayHint, Location, ServerStatus,
};

/// A boxed, sendable future that owns what it needs, so the
/// [`LanguageService`] trait stays object-safe. Dropping it cancels the
/// request.
pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

/// What a service can do. The UI shows only what is supported.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Capabilities {
    /// [`LanguageService::hover`].
    pub hover: bool,
    /// [`LanguageService::completions`].
    pub completion: Option<CompletionCapabilities>,
    /// [`LanguageService::definition`].
    pub definition: bool,
    /// [`LanguageService::inlay_hints`].
    pub inlay_hints: bool,
    /// [`LanguageService::code_lenses`].
    pub code_lens: Option<ResolveCapability>,
}

/// How a service completes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompletionCapabilities {
    /// Characters that open the list (`.`, `:`).
    pub trigger_characters: Vec<String>,
    /// Whether [`LanguageService::resolve_completion`] fills in details.
    pub resolve: ResolveCapability,
}

/// Whether items can be resolved later (details filled in on demand).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ResolveCapability {
    /// The service resolves items.
    pub resolve: bool,
}

/// Why the completion list was asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionTrigger {
    /// The user asked (Ctrl Space).
    Invoked,
    /// A trigger character was typed.
    Character(String),
    /// The previous list was incomplete and the user kept typing.
    Incomplete,
}

/// Something the service tells on its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceEvent {
    /// New diagnostics of a file; they replace the previous ones.
    Diagnostics {
        /// The file.
        path: PathBuf,
        /// Its problems, in the coordinates of document `version`.
        diagnostics: Arc<[Diagnostic]>,
        /// The document version they are for, if the service tells.
        version: Option<i32>,
    },
    /// The status changed.
    Status(ServerStatus),
    /// The capabilities are known (after startup) or changed.
    CapabilitiesChanged,
    /// Inlay hints of open documents are stale: ask again.
    InlayHintsChanged,
    /// Code lenses of open documents are stale: ask again.
    CodeLensesChanged,
}

/// Why a request failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ServiceError {
    /// The service is not running (not started, crashed, stopped).
    #[error("the language server is not running")]
    NotRunning,
    /// No answer in time.
    #[error("the language server did not answer in time")]
    Timeout,
    /// The request was cancelled, by the user or by the server.
    #[error("the request was cancelled")]
    Cancelled,
    /// The service does not support the request.
    #[error("the language server does not support this")]
    Unsupported,
    /// The server answered with an error.
    #[error("the language server failed: {0}")]
    Server(String),
    /// The answer could not be understood.
    #[error("unexpected answer from the language server: {0}")]
    Protocol(String),
}

/// Language support for the documents of one workspace.
///
/// Document notifications are cheap and return at once; the service does
/// the work in the background, in the order it was told. Requests return
/// futures that own what they need; dropping one cancels the request.
/// Positions are in the document as last told to the service.
pub trait LanguageService: Send + Sync {
    /// Name shown to the user (`rust-analyzer`).
    fn name(&self) -> &str;

    /// What the service can do now. Before startup finishes it can do
    /// nothing; [`ServiceEvent::CapabilitiesChanged`] tells when that
    /// changes.
    fn capabilities(&self) -> Capabilities;

    /// The current status.
    fn status(&self) -> ServerStatus;

    /// A new stream of the service's events. Every call gets every event
    /// from then on.
    fn events(&self) -> UnboundedReceiver<ServiceEvent>;

    /// A document was opened. `version` starts any number and grows with
    /// every change.
    fn open_document(&self, path: &Path, language_id: &str, version: i32, text: Buffer);

    /// A document changed: `changes` lead from the previous version to
    /// `text`, which is version `version`.
    fn change_document(&self, path: &Path, version: i32, changes: TextChanges, text: Buffer);

    /// A document was saved.
    fn save_document(&self, path: &Path);

    /// A document was closed.
    fn close_document(&self, path: &Path);

    /// Settings changed; the service picks what concerns it.
    fn configure(&self, settings: &SettingsStore) {
        let _ = settings;
    }

    /// Information about the symbol at `position`.
    fn hover(&self, path: &Path, position: Point)
    -> BoxFuture<Result<Option<Hover>, ServiceError>>;

    /// Completions at `position`.
    fn completions(
        &self,
        path: &Path,
        position: Point,
        trigger: CompletionTrigger,
    ) -> BoxFuture<Result<CompletionList, ServiceError>>;

    /// `completion` with its details filled in.
    fn resolve_completion(
        &self,
        completion: Completion,
    ) -> BoxFuture<Result<Completion, ServiceError>>;

    /// Where the symbol at `position` is defined.
    fn definition(
        &self,
        path: &Path,
        position: Point,
    ) -> BoxFuture<Result<Vec<Location>, ServiceError>>;

    /// Inlay hints of `range`, normally the visible rows.
    fn inlay_hints(
        &self,
        path: &Path,
        range: Range<Point>,
    ) -> BoxFuture<Result<Vec<InlayHint>, ServiceError>>;

    /// The code lenses of a document.
    fn code_lenses(&self, path: &Path) -> BoxFuture<Result<Vec<CodeLens>, ServiceError>>;

    /// `lens` with its title and action filled in.
    fn resolve_code_lens(&self, lens: CodeLens) -> BoxFuture<Result<CodeLens, ServiceError>>;

    /// Stops the service; the future ends when it has stopped.
    fn shutdown(&self) -> BoxFuture<()>;
}
