//! Starting, restarting and stopping the server.

use std::sync::{Arc, OnceLock, Weak};
use std::time::Duration;

use language::{
    Capabilities, CompletionCapabilities, ResolveCapability, ServerStatus, ServiceEvent,
};
use lsp_types::notification::{Exit, Initialized};
use lsp_types::request::{Initialize, Shutdown};
use lsp_types::{
    ClientCapabilities, ClientInfo, CodeLensClientCapabilities,
    CodeLensWorkspaceClientCapabilities, CompletionClientCapabilities, CompletionItemCapability,
    CompletionItemCapabilityResolveSupport, GeneralClientCapabilities, GotoCapability,
    HoverClientCapabilities, InitializeParams, InitializeResult, InitializedParams,
    InlayHintClientCapabilities, InlayHintWorkspaceClientCapabilities, MarkupKind, OneOf,
    PositionEncodingKind, PublishDiagnosticsClientCapabilities, ServerCapabilities,
    TextDocumentClientCapabilities, TextDocumentSyncCapability, TextDocumentSyncClientCapabilities,
    TextDocumentSyncKind, TextDocumentSyncSaveOptions, WindowClientCapabilities,
    WorkspaceClientCapabilities, WorkspaceFolder,
};
use process::{ProcessError, SpawnOptions};

use super::{Inner, Running, lock};
use crate::client::Client;
use crate::sync::{Encoding, SyncKind};
use crate::transport::{self, Streams};
use crate::uri::path_to_uri;

/// Pause before a crashed server is started again.
const RESTART_DELAY: Duration = Duration::from_secs(1);

/// Starts the server on a new thread.
pub(super) fn launch(inner: &Arc<Inner>) {
    let weak = Arc::downgrade(inner);
    let spawned = std::thread::Builder::new()
        .name(format!("lsp-start-{}", inner.adapter.name()))
        .spawn(move || {
            if let Some(inner) = weak.upgrade() {
                run(&inner);
            }
        });
    if let Err(error) = spawned {
        inner.set_status(ServerStatus::Failed {
            message: error.to_string(),
        });
    }
}

/// Spawns the process and initializes it. Blocks: runs on its own thread.
fn run(inner: &Arc<Inner>) {
    let generation = {
        let mut state = inner.state();
        if state.stopping {
            return;
        }
        state.generation += 1;
        state.generation
    };
    let name = inner.adapter.name().to_owned();
    let _span = tracing::info_span!("lsp.start", server = %name).entered();
    inner.set_status(ServerStatus::Starting);

    let command = inner.adapter.command(&inner.root);
    let options = SpawnOptions {
        lowered_priority: true,
    };
    let child = match inner.process.spawn(&command, options) {
        Ok(child) => child,
        Err(ProcessError::NotFound { program }) => {
            tracing::warn!(program = %program.to_string_lossy(), "language server not found");
            inner.set_status(ServerStatus::NotFound {
                program: program.to_string_lossy().into_owned(),
            });
            return;
        }
        Err(error) => {
            inner.set_status(ServerStatus::Failed {
                message: error.to_string(),
            });
            return;
        }
    };
    let control = child.control.clone();
    let client = match connect(inner, &name, generation, child) {
        Ok(client) => client,
        Err(error) => {
            let _ = control.kill();
            inner.set_status(ServerStatus::Failed {
                message: error.to_string(),
            });
            return;
        }
    };

    let result =
        futures::executor::block_on(client.request::<Initialize>(&initialize_params(inner)));
    let result = match result {
        Ok(result) => result,
        Err(error) => {
            tracing::warn!(%error, "language server did not initialize");
            let _ = control.kill();
            inner.set_status(ServerStatus::Failed {
                message: error.to_string(),
            });
            return;
        }
    };
    client.notify::<Initialized>(&InitializedParams {});
    let running = running(client, control, &result);
    let capabilities = capabilities(&result.capabilities);
    {
        let documents = lock(&inner.documents);
        let mut state = inner.state();
        if state.generation != generation || state.stopping {
            let _ = running.control.kill();
            return;
        }
        for (path, document) in documents.iter() {
            Inner::send_open(&running, path, document);
        }
        state.running = Some(running);
        state.capabilities = capabilities;
    }
    tracing::info!(server = %name, "language server ready");
    inner.broadcast(&ServiceEvent::CapabilitiesChanged);
    inner.update_status(|state| {
        if matches!(state.status, None | Some(ServerStatus::Starting)) {
            state.status = Some(ServerStatus::Ready);
        }
    });
}

/// Starts the threads of a spawned server and the client on them.
fn connect(
    inner: &Arc<Inner>,
    name: &str,
    generation: u64,
    child: process::ChildProcess,
) -> std::io::Result<Client> {
    let slot: Arc<OnceLock<Client>> = Arc::new(OnceLock::new());
    let streams = Streams {
        stdin: child.stdin,
        stdout: child.stdout,
        stderr: child.stderr,
    };
    let on_message = {
        let weak = Arc::downgrade(inner);
        let slot = slot.clone();
        move |message| {
            if let (Some(inner), Some(client)) = (weak.upgrade(), slot.get()) {
                super::dispatch::handle(&inner, client, generation, message);
            }
        }
    };
    let on_closed = {
        let weak = Arc::downgrade(inner);
        let slot = slot.clone();
        move || {
            if let Some(client) = slot.get() {
                client.fail_all();
            }
            closed(&weak, generation);
        }
    };
    let writer = transport::start(name, streams, on_message, on_closed)?;
    let client = Client::new(
        writer,
        inner.timer.clone(),
        inner.settings().request_timeout(),
    );
    let _ = slot.set(client.clone());
    Ok(client)
}

/// The server's output ended: it exited or was killed.
fn closed(weak: &Weak<Inner>, generation: u64) {
    let Some(inner) = weak.upgrade() else {
        return;
    };
    let max_restarts = inner.settings().max_restarts;
    let (running, stopping, restart) = {
        let mut state = inner.state();
        if state.generation != generation {
            return;
        }
        let running = state.running.take();
        state.capabilities = Capabilities::default();
        state.progress.clear();
        let restart = !state.stopping && state.restarts < max_restarts;
        if restart {
            state.restarts += 1;
        }
        (running, state.stopping, restart)
    };
    let exit_code = running.as_ref().and_then(|running| {
        let _ = running.control.kill();
        running.control.try_wait().ok().flatten().flatten()
    });
    inner.broadcast(&ServiceEvent::CapabilitiesChanged);
    if stopping {
        inner.set_status(ServerStatus::Stopped);
        return;
    }
    tracing::warn!(
        server = inner.adapter.name(),
        ?exit_code,
        restart,
        "language server exited"
    );
    if restart {
        inner.set_status(ServerStatus::Starting);
        let weak = Arc::downgrade(&inner);
        inner.timer.after(RESTART_DELAY, move || {
            if let Some(inner) = weak.upgrade() {
                launch(&inner);
            }
        });
    } else if !matches!(
        inner.state().status,
        Some(ServerStatus::Failed { .. } | ServerStatus::NotFound { .. })
    ) {
        inner.set_status(ServerStatus::Failed {
            message: match exit_code {
                Some(code) => format!("exited with code {code}"),
                None => "exited".to_owned(),
            },
        });
    }
}

/// Asks the server to shut down, then to exit, and kills it if it does not
/// in time.
pub(super) fn shutdown(inner: &Arc<Inner>) -> language::BoxFuture<()> {
    let running = {
        let mut state = inner.state();
        state.stopping = true;
        state.running.take()
    };
    let Some(running) = running else {
        inner.set_status(ServerStatus::Stopped);
        return Box::pin(async {});
    };
    let timeout = inner.settings().shutdown_timeout();
    let request = running
        .client
        .request_with_timeout::<Shutdown>(&(), timeout);
    let inner = inner.clone();
    Box::pin(async move {
        let _ = request.await;
        running.client.notify::<Exit>(&());
        let control = running.control.clone();
        inner.timer.after(timeout, move || {
            let _ = control.kill();
        });
        inner.set_status(ServerStatus::Stopped);
    })
}

fn running(
    client: Client,
    control: Arc<dyn process::ProcessControl>,
    result: &InitializeResult,
) -> Running {
    let capabilities = &result.capabilities;
    let encoding = if capabilities.position_encoding == Some(PositionEncodingKind::UTF8) {
        Encoding::Utf8
    } else {
        Encoding::Utf16
    };
    let kind = |kind: TextDocumentSyncKind| match kind {
        TextDocumentSyncKind::FULL => SyncKind::Full,
        TextDocumentSyncKind::INCREMENTAL => SyncKind::Incremental,
        _ => SyncKind::None,
    };
    let (sync, save) = match &capabilities.text_document_sync {
        Some(TextDocumentSyncCapability::Kind(sync)) => (kind(*sync), false),
        Some(TextDocumentSyncCapability::Options(options)) => (
            options.change.map_or(SyncKind::None, kind),
            match &options.save {
                Some(TextDocumentSyncSaveOptions::Supported(save)) => *save,
                Some(TextDocumentSyncSaveOptions::SaveOptions(_)) => true,
                None => false,
            },
        ),
        None => (SyncKind::None, false),
    };
    Running {
        client,
        control,
        encoding,
        sync,
        save,
    }
}

/// What the server can do, in the IDE's terms.
fn capabilities(server: &ServerCapabilities) -> Capabilities {
    let enabled = |provider: Option<&OneOf<bool, _>>| match provider {
        Some(OneOf::Left(enabled)) => *enabled,
        Some(OneOf::Right(_)) => true,
        None => false,
    };
    Capabilities {
        hover: match &server.hover_provider {
            Some(lsp_types::HoverProviderCapability::Simple(enabled)) => *enabled,
            Some(lsp_types::HoverProviderCapability::Options(_)) => true,
            None => false,
        },
        completion: server
            .completion_provider
            .as_ref()
            .map(|options| CompletionCapabilities {
                trigger_characters: options.trigger_characters.clone().unwrap_or_default(),
                resolve: ResolveCapability {
                    resolve: options.resolve_provider.unwrap_or(false),
                },
            }),
        definition: enabled(server.definition_provider.as_ref()),
        inlay_hints: match &server.inlay_hint_provider {
            Some(OneOf::Left(enabled)) => *enabled,
            Some(OneOf::Right(_)) => true,
            None => false,
        },
        code_lens: server
            .code_lens_provider
            .as_ref()
            .map(|options| ResolveCapability {
                resolve: options.resolve_provider.unwrap_or(false),
            }),
    }
}

fn initialize_params(inner: &Inner) -> InitializeParams {
    let workspace_folders = path_to_uri(&inner.root).map(|uri| {
        vec![WorkspaceFolder {
            uri,
            name: inner
                .root
                .file_name()
                .map_or_else(String::new, |name| name.to_string_lossy().into_owned()),
        }]
    });
    InitializeParams {
        process_id: Some(std::process::id()),
        workspace_folders,
        initialization_options: inner.adapter.initialization_options(),
        capabilities: client_capabilities(inner),
        client_info: Some(ClientInfo {
            name: app_identity::PRODUCT_NAME.to_owned(),
            version: Some(env!("CARGO_PKG_VERSION").to_owned()),
        }),
        ..InitializeParams::default()
    }
}

fn client_capabilities(inner: &Inner) -> ClientCapabilities {
    let markdown = Some(vec![MarkupKind::Markdown, MarkupKind::PlainText]);
    let mut capabilities = ClientCapabilities {
        general: Some(GeneralClientCapabilities {
            position_encodings: Some(vec![
                PositionEncodingKind::UTF8,
                PositionEncodingKind::UTF16,
            ]),
            ..GeneralClientCapabilities::default()
        }),
        text_document: Some(TextDocumentClientCapabilities {
            synchronization: Some(TextDocumentSyncClientCapabilities {
                did_save: Some(true),
                ..TextDocumentSyncClientCapabilities::default()
            }),
            hover: Some(HoverClientCapabilities {
                content_format: markdown.clone(),
                ..HoverClientCapabilities::default()
            }),
            completion: Some(CompletionClientCapabilities {
                completion_item: Some(CompletionItemCapability {
                    // Snippet sessions are a ROADMAP item.
                    snippet_support: Some(false),
                    documentation_format: markdown,
                    resolve_support: Some(CompletionItemCapabilityResolveSupport {
                        properties: ["documentation", "detail", "additionalTextEdits"]
                            .map(str::to_owned)
                            .to_vec(),
                    }),
                    ..CompletionItemCapability::default()
                }),
                context_support: Some(true),
                ..CompletionClientCapabilities::default()
            }),
            definition: Some(GotoCapability {
                link_support: Some(true),
                ..GotoCapability::default()
            }),
            publish_diagnostics: Some(PublishDiagnosticsClientCapabilities {
                version_support: Some(true),
                ..PublishDiagnosticsClientCapabilities::default()
            }),
            inlay_hint: Some(InlayHintClientCapabilities::default()),
            code_lens: Some(CodeLensClientCapabilities::default()),
            ..TextDocumentClientCapabilities::default()
        }),
        workspace: Some(WorkspaceClientCapabilities {
            configuration: Some(true),
            workspace_folders: Some(true),
            inlay_hint: Some(InlayHintWorkspaceClientCapabilities {
                refresh_support: Some(true),
            }),
            code_lens: Some(CodeLensWorkspaceClientCapabilities {
                refresh_support: Some(true),
            }),
            ..WorkspaceClientCapabilities::default()
        }),
        window: Some(WindowClientCapabilities {
            work_done_progress: Some(true),
            ..WindowClientCapabilities::default()
        }),
        ..ClientCapabilities::default()
    };
    inner.adapter.extend_capabilities(&mut capabilities);
    capabilities
}
