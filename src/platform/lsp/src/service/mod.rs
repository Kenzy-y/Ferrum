//! [`LspService`]: a [`LanguageService`] backed by a language server.

mod dispatch;
mod requests;
mod startup;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use language::{
    Capabilities, Diagnostic, LanguageService, ServerContext, ServerStatus, ServiceEvent,
};
use lsp_types::notification::{DidChangeConfiguration, DidCloseTextDocument, DidSaveTextDocument};
use lsp_types::{
    DidChangeConfigurationParams, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, TextDocumentIdentifier, TextDocumentItem,
    VersionedTextDocumentIdentifier,
};
use process::{ProcessControl, ProcessRunner};
use settings::SettingsStore;
use text::Buffer;
use text::ropey::Rope;

use crate::adapter::LspAdapter;
use crate::client::Client;
use crate::settings::LspSettings;
use crate::sync::{self, Encoding, SyncKind};
use crate::timer::Timer;
use crate::transport::notification;
use crate::uri::path_to_uri;

/// Starts a language service for `adapter`'s server. Returns at once; the
/// server starts on a background thread.
pub fn start(adapter: Arc<dyn LspAdapter>, context: ServerContext<'_>) -> Arc<dyn LanguageService> {
    let service = LspService::new(adapter, context);
    startup::launch(&service.inner);
    Arc::new(service)
}

/// A language service talking to a server process.
pub struct LspService {
    inner: Arc<Inner>,
}

/// A document the server has been told about.
struct OpenDocument {
    language_id: String,
    version: i32,
    text: Buffer,
}

/// The connection to a running, initialized server.
#[derive(Clone)]
struct Running {
    client: Client,
    control: Arc<dyn ProcessControl>,
    encoding: Encoding,
    sync: SyncKind,
    /// The server wants `didSave`.
    save: bool,
}

/// Work-done progress the server reported and has not ended.
struct Progress {
    token: String,
    title: String,
    message: Option<String>,
    percentage: Option<u32>,
}

impl Progress {
    /// `title: message`, or the title alone.
    fn text(&self) -> String {
        match &self.message {
            Some(message) => format!("{}: {message}", self.title),
            None => self.title.clone(),
        }
    }
}

#[derive(Default)]
struct State {
    /// Bumped on every start, so a dead server's late messages are
    /// ignored.
    generation: u64,
    running: Option<Running>,
    capabilities: Capabilities,
    /// The status without progress: what the server or its adapter says.
    status: Option<ServerStatus>,
    progress: Vec<Progress>,
    restarts: u32,
    stopping: bool,
}

impl State {
    fn shown_status(&self) -> ServerStatus {
        let status = self.status.clone().unwrap_or(ServerStatus::Starting);
        match (&status, self.progress.last()) {
            (ServerStatus::Ready | ServerStatus::Warning { .. }, Some(progress)) => {
                ServerStatus::Indexing {
                    message: Some(progress.text()),
                    percentage: progress.percentage,
                }
            }
            _ => status,
        }
    }
}

/// The diagnostics of a file and the document version they are for.
type FileDiagnostics = (Arc<[Diagnostic]>, Option<i32>);

/// Diagnostics waiting for the debounce to end.
#[derive(Default)]
struct PendingDiagnostics {
    files: HashMap<PathBuf, FileDiagnostics>,
    scheduled: bool,
}

struct Inner {
    adapter: Arc<dyn LspAdapter>,
    root: PathBuf,
    process: Arc<dyn ProcessRunner>,
    settings: Mutex<LspSettings>,
    /// Lock order: `documents`, then `state`.
    documents: Mutex<HashMap<PathBuf, OpenDocument>>,
    state: Mutex<State>,
    subscribers: Mutex<Vec<UnboundedSender<ServiceEvent>>>,
    /// The last diagnostics of every file, for new subscribers.
    published: Mutex<HashMap<PathBuf, FileDiagnostics>>,
    pending: Mutex<PendingDiagnostics>,
    /// The last line the server wrote to standard error: the reason shown
    /// when it fails.
    error_output: Mutex<Option<String>>,
    timer: Timer,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Inner {
    fn state(&self) -> MutexGuard<'_, State> {
        lock(&self.state)
    }

    fn settings(&self) -> LspSettings {
        lock(&self.settings).clone()
    }

    fn running(&self) -> Option<Running> {
        self.state().running.clone()
    }

    fn broadcast(&self, event: &ServiceEvent) {
        lock(&self.subscribers)
            .retain(|subscriber| subscriber.unbounded_send(event.clone()).is_ok());
    }

    /// Changes the status and tells subscribers if what is shown changed.
    fn update_status(&self, change: impl FnOnce(&mut State)) {
        let (before, after) = {
            let mut state = self.state();
            let before = state.shown_status();
            change(&mut state);
            (before, state.shown_status())
        };
        if before != after {
            tracing::info!(server = self.adapter.name(), status = ?after, "language server status");
            self.broadcast(&ServiceEvent::Status(after));
        }
    }

    fn set_status(&self, status: ServerStatus) {
        self.update_status(|state| state.status = Some(status));
    }

    /// The server stopped after `error`. The reason shown is its last
    /// error output when it wrote one, else `error`.
    fn set_failed(&self, error: impl Into<String>) {
        let message = lock(&self.error_output)
            .clone()
            .unwrap_or_else(|| error.into());
        let hint = self.adapter.failure_hint(&message);
        self.set_status(ServerStatus::Failed { message, hint });
    }

    /// A line the server wrote to standard error. A failure shown before
    /// the line arrived gets it as its reason.
    fn note_error_output(&self, line: &str) {
        let line = line.trim();
        if line.is_empty() {
            return;
        }
        *lock(&self.error_output) = Some(line.to_owned());
        let status = self.state().status.clone();
        if let Some(ServerStatus::Failed { message, .. }) = status {
            self.set_failed(message);
        }
    }

    /// Texts of the open documents, for converting positions.
    fn texts(&self) -> HashMap<PathBuf, Rope> {
        lock(&self.documents)
            .iter()
            .map(|(path, document)| (path.clone(), document.text.rope().clone()))
            .collect()
    }

    /// Tells a running server about an open document.
    fn send_open(running: &Running, path: &Path, document: &OpenDocument) {
        let Some(uri) = path_to_uri(path) else {
            return;
        };
        let language_id = document.language_id.clone();
        let version = document.version;
        let text = document.text.clone();
        running.client.send_built(move || {
            let params = DidOpenTextDocumentParams {
                text_document: TextDocumentItem::new(
                    uri,
                    language_id,
                    version,
                    text.rope().to_string(),
                ),
            };
            Some(notification(
                "textDocument/didOpen",
                &serde_json::to_value(params).ok()?,
            ))
        });
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        let running = self
            .state
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner)
            .running
            .take();
        if let Some(running) = running {
            let _ = running.control.kill();
        }
    }
}

impl LspService {
    fn new(adapter: Arc<dyn LspAdapter>, context: ServerContext<'_>) -> Self {
        let timer = Timer::start(adapter.name());
        Self {
            inner: Arc::new(Inner {
                adapter,
                root: context.root,
                process: context.process,
                settings: Mutex::new(context.settings.get::<LspSettings>(None)),
                documents: Mutex::new(HashMap::new()),
                state: Mutex::new(State::default()),
                subscribers: Mutex::new(Vec::new()),
                published: Mutex::new(HashMap::new()),
                pending: Mutex::new(PendingDiagnostics::default()),
                error_output: Mutex::new(None),
                timer,
            }),
        }
    }
}

impl LanguageService for LspService {
    fn name(&self) -> &str {
        self.inner.adapter.name()
    }

    fn capabilities(&self) -> Capabilities {
        self.inner.state().capabilities.clone()
    }

    fn status(&self) -> ServerStatus {
        self.inner.state().shown_status()
    }

    fn events(&self) -> UnboundedReceiver<ServiceEvent> {
        let (sender, receiver) = unbounded();
        let _ = sender.unbounded_send(ServiceEvent::Status(self.status()));
        for (path, (diagnostics, version)) in lock(&self.inner.published).iter() {
            let _ = sender.unbounded_send(ServiceEvent::Diagnostics {
                path: path.clone(),
                diagnostics: diagnostics.clone(),
                version: *version,
            });
        }
        lock(&self.inner.subscribers).push(sender);
        receiver
    }

    fn open_document(&self, path: &Path, language_id: &str, version: i32, text: Buffer) {
        let document = OpenDocument {
            language_id: language_id.to_owned(),
            version,
            text,
        };
        let mut documents = lock(&self.inner.documents);
        if let Some(running) = self.inner.running() {
            Inner::send_open(&running, path, &document);
        }
        documents.insert(path.to_path_buf(), document);
    }

    fn change_document(&self, path: &Path, version: i32, changes: text::TextChanges, text: Buffer) {
        let mut documents = lock(&self.inner.documents);
        let Some(document) = documents.get_mut(path) else {
            return;
        };
        let previous = std::mem::replace(&mut document.text, text.clone());
        document.version = version;
        let Some(running) = self.inner.running() else {
            return;
        };
        let Some(uri) = path_to_uri(path) else {
            return;
        };
        let (encoding, kind) = (running.encoding, running.sync);
        running.client.send_built(move || {
            let content_changes = sync::content_changes(&previous, &changes, &text, encoding, kind);
            if content_changes.is_empty() {
                return None;
            }
            let params = DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier::new(uri, version),
                content_changes,
            };
            Some(notification(
                "textDocument/didChange",
                &serde_json::to_value(params).ok()?,
            ))
        });
    }

    fn save_document(&self, path: &Path) {
        let Some(running) = self.inner.running() else {
            return;
        };
        if !running.save || !lock(&self.inner.documents).contains_key(path) {
            return;
        }
        if let Some(uri) = path_to_uri(path) {
            running
                .client
                .notify::<DidSaveTextDocument>(&DidSaveTextDocumentParams {
                    text_document: TextDocumentIdentifier::new(uri),
                    text: None,
                });
        }
    }

    fn close_document(&self, path: &Path) {
        let mut documents = lock(&self.inner.documents);
        if documents.remove(path).is_none() {
            return;
        }
        if let (Some(running), Some(uri)) = (self.inner.running(), path_to_uri(path)) {
            running
                .client
                .notify::<DidCloseTextDocument>(&DidCloseTextDocumentParams {
                    text_document: TextDocumentIdentifier::new(uri),
                });
        }
    }

    fn configure(&self, settings: &SettingsStore) {
        *lock(&self.inner.settings) = settings.get::<LspSettings>(None);
        if self.inner.adapter.configure(settings)
            && let Some(running) = self.inner.running()
        {
            running
                .client
                .notify::<DidChangeConfiguration>(&DidChangeConfigurationParams {
                    settings: serde_json::Value::Null,
                });
        }
    }

    fn hover(
        &self,
        path: &Path,
        position: text::Point,
    ) -> language::BoxFuture<Result<Option<language::Hover>, language::ServiceError>> {
        requests::hover(&self.inner, path, position)
    }

    fn completions(
        &self,
        path: &Path,
        position: text::Point,
        trigger: language::CompletionTrigger,
    ) -> language::BoxFuture<Result<language::CompletionList, language::ServiceError>> {
        requests::completions(&self.inner, path, position, trigger)
    }

    fn resolve_completion(
        &self,
        completion: language::Completion,
    ) -> language::BoxFuture<Result<language::Completion, language::ServiceError>> {
        requests::resolve_completion(&self.inner, completion)
    }

    fn definition(
        &self,
        path: &Path,
        position: text::Point,
    ) -> language::BoxFuture<Result<Vec<language::Location>, language::ServiceError>> {
        requests::definition(&self.inner, path, position)
    }

    fn inlay_hints(
        &self,
        path: &Path,
        range: std::ops::Range<text::Point>,
    ) -> language::BoxFuture<Result<Vec<language::InlayHint>, language::ServiceError>> {
        requests::inlay_hints(&self.inner, path, range)
    }

    fn code_lenses(
        &self,
        path: &Path,
    ) -> language::BoxFuture<Result<Vec<language::CodeLens>, language::ServiceError>> {
        requests::code_lenses(&self.inner, path)
    }

    fn resolve_code_lens(
        &self,
        lens: language::CodeLens,
    ) -> language::BoxFuture<Result<language::CodeLens, language::ServiceError>> {
        requests::resolve_code_lens(&self.inner, lens)
    }

    fn shutdown(&self) -> language::BoxFuture<()> {
        startup::shutdown(&self.inner)
    }
}
