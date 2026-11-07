# LSP client

Crate `lsp` (core layer, no GPUI): a generic Language Server Protocol
client that implements `language::LanguageService`. It knows no language
and no server; ADR 0006.

**Entry point.** `lsp::start(adapter, context)` returns an `LspService` at
once and starts the server on a `lsp-start-<name>` thread:
1. `ProcessRunner::spawn` with lowered priority;
2. `initialize`, then `initialized`;
3. `didOpen` for every document opened meanwhile.

The status goes Starting → Ready (or NotFound / Failed). The stderr thread
keeps the server's last error line. `Failed` shows it instead of a generic
error, even when the line arrives after the failure, and
`LspAdapter::failure_hint` may add an i18n hint id (ADR 0011).

**Threads** (`transport/`), per server, each blocked on its stream, so an
idle server costs no CPU:
- the writer sends `WriterJob`s in order. `Build` jobs build their body
  there, so document changes are converted off the UI thread;
- the reader parses frames (`framing.rs`, `Content-Length`) and
  dispatches every message;
- stderr goes to `tracing` (`lsp::stderr`);
- one timer (`timer.rs`) runs request timeouts, the diagnostics debounce
  and restart delays. It sleeps until the next deadline.

**Client** (`client.rs`). Requests get ids and wait on a oneshot. A
`RequestFuture` that is dropped before its answer sends
`$/cancelRequest`. Answers are deserialized where the future is
awaited. `RequestCancelled`, `ServerCancelled` and `ContentModified`
become `ServiceError::Cancelled`. A request times out after
`[lsp] request_timeout_ms`.

**Sync** (`sync.rs`). `positionEncoding` is negotiated, `utf-8` first.
With utf-8, `TextEdit` points go out as they are. With utf-16, the
writer replays the edits on a copy of the previous rope, which costs a
logarithmic amount per edit, to convert each position in its own line.
Full sync, and `TextChanges::replaced`, send the whole text. Answers are
converted with the open documents' texts (`convert.rs`). Files that are
not open take UTF-16 columns as bytes.

**Service** (`service/`).
- `mod.rs` holds the documents, the state and the subscribers. Lock
  order: documents, then state.
- `dispatch.rs` handles server requests: `workspace/configuration`
  (answered by the adapter), refreshes as events, `applyEdit` refused,
  unknown methods `MethodNotFound`. It also handles notifications:
  `publishDiagnostics` is debounced by `[lsp] diagnostics_debounce_ms`,
  `$/progress` makes the status `Indexing`, and anything else goes to the
  adapter, which may report a status.
- `requests.rs` holds the document requests.
- `startup.rs` handles start, restart (after 1 s, up to
  `[lsp] max_restarts`) and `shutdown` (`shutdown`, `exit`, then a kill
  after `[lsp] shutdown_timeout_ms`). Dropping the last reference kills
  the server.

**Adapter** (`LspAdapter`) is what differs per server: the command,
`initializationOptions`, `workspace/configuration` sections, extra
client capabilities, unknown notifications (a status), code lens
commands (`LensAction`) and settings changes. It lives in the language
pack (`lang_rust` for rust-analyzer).

Completions declare `snippetSupport: true`. Snippet items are parsed with
`language::Snippet` into text and tab stops (ADR 0008).

**Not supported yet:** `applyEdit` and
code actions, semantic tokens.

**Tests.** `tests/service.rs` runs the service against a scripted server
(`tests/support`) on `FakeProcessRunner` pipes. It covers sync, UTF-16
positions, cancellation, timeouts, the debounce, restarts and a missing
program.
