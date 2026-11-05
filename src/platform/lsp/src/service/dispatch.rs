//! What the server sends on its own: answers it owes us are matched by the
//! client; requests and notifications are handled here, on the reader
//! thread.

use std::sync::Arc;

use language::{Diagnostic, ServiceEvent};
use lsp_types::{
    NumberOrString, ProgressParams, ProgressParamsValue, PublishDiagnosticsParams, WorkDoneProgress,
};
use serde_json::Value;

use super::{Inner, Progress, lock};
use crate::client::Client;
use crate::convert::Converter;
use crate::sync::Encoding;
use crate::transport::{Incoming, METHOD_NOT_FOUND, ResponseError};
use crate::uri::uri_to_path;

/// Handles one message of server start `generation`.
pub(super) fn handle(inner: &Arc<Inner>, client: &Client, generation: u64, message: Incoming) {
    match message {
        Incoming::Response { id, result } => client.handle_response(id, result),
        Incoming::Request { id, method, params } => {
            let result = request(inner, &method, &params);
            client.respond(&id, result);
        }
        Incoming::Notification { method, params } => {
            if inner.state().generation == generation {
                notification(inner, &method, params);
            }
        }
    }
}

fn request(inner: &Inner, method: &str, params: &Value) -> Result<Value, ResponseError> {
    match method {
        "workspace/configuration" => {
            let items = params.get("items").and_then(Value::as_array);
            let sections = items.into_iter().flatten().map(|item| {
                inner
                    .adapter
                    .configuration(item.get("section").and_then(Value::as_str))
            });
            Ok(Value::Array(sections.collect()))
        }
        "workspace/inlayHint/refresh" => {
            inner.broadcast(&ServiceEvent::InlayHintsChanged);
            Ok(Value::Null)
        }
        "workspace/codeLens/refresh" => {
            inner.broadcast(&ServiceEvent::CodeLensesChanged);
            Ok(Value::Null)
        }
        "window/workDoneProgress/create"
        | "client/registerCapability"
        | "client/unregisterCapability"
        | "window/showMessageRequest"
        | "workspace/semanticTokens/refresh"
        | "workspace/diagnostic/refresh" => Ok(Value::Null),
        // Edits from the server (refactorings) arrive with code actions,
        // which are not supported yet.
        "workspace/applyEdit" => Ok(serde_json::json!({ "applied": false })),
        _ => Err(ResponseError {
            code: METHOD_NOT_FOUND,
            message: format!("{method} is not supported"),
            data: None,
        }),
    }
}

fn notification(inner: &Arc<Inner>, method: &str, params: Value) {
    match method {
        "textDocument/publishDiagnostics" => {
            if let Ok(params) = serde_json::from_value::<PublishDiagnosticsParams>(params) {
                diagnostics(inner, params);
            }
        }
        "$/progress" => {
            if let Ok(params) = serde_json::from_value::<ProgressParams>(params) {
                progress(inner, params);
            }
        }
        "window/logMessage" | "window/showMessage" => {
            let message = params.get("message").and_then(Value::as_str).unwrap_or("");
            tracing::debug!(server = inner.adapter.name(), "{message}");
        }
        _ => {
            if let Some(status) = inner.adapter.notification(method, &params) {
                inner.set_status(status);
            }
        }
    }
}

/// Converts diagnostics and shows them once the server is quiet for the
/// debounce time.
fn diagnostics(inner: &Arc<Inner>, params: PublishDiagnosticsParams) {
    let Some(path) = uri_to_path(&params.uri) else {
        return;
    };
    let encoding = inner
        .running()
        .map_or(Encoding::Utf16, |running| running.encoding);
    let text = lock(&inner.documents)
        .get(&path)
        .map(|document| document.text.rope().clone());
    let texts = text
        .map(|text| std::iter::once((path.clone(), text)).collect())
        .unwrap_or_default();
    let converter = Converter {
        encoding,
        texts: &texts,
    };
    let diagnostics: Arc<[Diagnostic]> = params
        .diagnostics
        .into_iter()
        .map(|diagnostic| converter.diagnostic(&path, diagnostic))
        .collect();
    let schedule = {
        let mut pending = lock(&inner.pending);
        pending.files.insert(path, (diagnostics, params.version));
        !std::mem::replace(&mut pending.scheduled, true)
    };
    if schedule {
        let weak = Arc::downgrade(inner);
        inner
            .timer
            .after(inner.settings().diagnostics_debounce(), move || {
                if let Some(inner) = weak.upgrade() {
                    flush_diagnostics(&inner);
                }
            });
    }
}

fn flush_diagnostics(inner: &Inner) {
    let files = {
        let mut pending = lock(&inner.pending);
        pending.scheduled = false;
        std::mem::take(&mut pending.files)
    };
    for (path, (diagnostics, version)) in files {
        {
            let mut published = lock(&inner.published);
            if diagnostics.is_empty() {
                published.remove(&path);
            } else {
                published.insert(path.clone(), (diagnostics.clone(), version));
            }
        }
        inner.broadcast(&ServiceEvent::Diagnostics {
            path,
            diagnostics,
            version,
        });
    }
}

fn progress(inner: &Inner, params: ProgressParams) {
    let token = match params.token {
        NumberOrString::Number(number) => number.to_string(),
        NumberOrString::String(text) => text,
    };
    let ProgressParamsValue::WorkDone(progress) = params.value;
    inner.update_status(|state| match progress {
        WorkDoneProgress::Begin(begin) => state.progress.push(Progress {
            token,
            title: begin.title,
            message: begin.message,
            percentage: begin.percentage,
        }),
        WorkDoneProgress::Report(report) => {
            if let Some(entry) = state.progress.iter_mut().find(|entry| entry.token == token) {
                entry.message = report.message.or(entry.message.take());
                entry.percentage = report.percentage.or(entry.percentage);
            }
        }
        WorkDoneProgress::End(_) => state.progress.retain(|entry| entry.token != token),
    });
}
