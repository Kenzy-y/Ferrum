//! The service against a scripted server.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::time::{Duration, Instant};

use futures::executor::block_on;
use language::{CompletionTrigger, ServerStatus, ServiceError, ServiceEvent};
use serde_json::json;
use support::{Mock, PATIENCE, root, start, start_failing, wait_for_status};
use text::{Buffer, Document, Point, Selection};

fn ready(status: &ServerStatus) -> bool {
    *status == ServerStatus::Ready
}

#[test]
fn documents_opened_early_are_synced_once_the_server_is_ready() {
    let mock = Mock::new(true);
    let (service, _) = start(Some(&mock), "");
    let path = root().join("main.rs");
    let mut document = Document::new(Buffer::from_text("fn main() {}\n"));
    document.track_changes();
    service.open_document(&path, "rust", 1, document.buffer().clone());
    wait_for_status(service.as_ref(), ready);

    let open = mock.wait_for_method("textDocument/didOpen");
    assert_eq!(open["params"]["textDocument"]["text"], "fn main() {}\n");
    assert_eq!(open["params"]["textDocument"]["languageId"], "rust");

    document.set_primary_selection(Selection::new(11, 11));
    document.insert("x");
    service.change_document(&path, 2, document.take_changes(), document.buffer().clone());
    let change = mock.wait_for_method("textDocument/didChange");
    assert_eq!(change["params"]["textDocument"]["version"], 2);
    assert_eq!(
        change["params"]["contentChanges"],
        json!([{
            "range": {"start": {"line": 0, "character": 11}, "end": {"line": 0, "character": 11}},
            "text": "x",
        }])
    );

    service.save_document(&path);
    mock.wait_for_method("textDocument/didSave");
    service.close_document(&path);
    mock.wait_for_method("textDocument/didClose");

    // The server asked for its configuration; the adapter answered.
    let answer = mock.wait_for("the configuration", |message| {
        message["id"] == "configuration"
    });
    assert_eq!(answer["result"], json!([{"section": "mock"}]));

    block_on(service.shutdown());
    mock.wait_for_method("exit");
    assert_eq!(service.status(), ServerStatus::Stopped);
}

#[test]
fn answers_come_back_in_the_ides_terms() {
    let mock = Mock::new(false);
    let (service, _) = start(Some(&mock), "");
    let path = root().join("lib.rs");
    service.open_document(&path, "rust", 1, Buffer::from_text("😀x.len()\n"));
    wait_for_status(service.as_ref(), ready);
    let capabilities = service.capabilities();
    assert!(capabilities.hover && capabilities.definition && capabilities.inlay_hints);
    assert_eq!(capabilities.completion.unwrap().trigger_characters, ["."]);

    // The server counts UTF-16: byte 4, after the emoji, is character 2.
    let hover = block_on(service.hover(&path, Point::new(0, 4)))
        .unwrap()
        .unwrap();
    assert_eq!(hover.contents, "**docs**");
    let request = mock.wait_for_method("textDocument/hover");
    assert_eq!(
        request["params"]["position"],
        json!({"line": 0, "character": 2})
    );

    let list = block_on(service.completions(
        &path,
        Point::new(0, 6),
        CompletionTrigger::Character(".".to_owned()),
    ))
    .unwrap();
    assert_eq!(list.items[0].label, "len");
    assert_eq!(list.items[0].kind, Some(language::CompletionKind::Method));
    let request = mock.wait_for_method("textDocument/completion");
    assert_eq!(request["params"]["context"]["triggerCharacter"], ".");

    // Requests about files that are not open are not sent.
    assert_eq!(
        block_on(service.hover(&root().join("other.rs"), Point::new(0, 0))),
        Err(ServiceError::NotRunning)
    );
}

#[test]
fn dropping_a_request_cancels_it_and_slow_ones_time_out() {
    let mock = Mock::new(true);
    let (service, _) = start(Some(&mock), "[lsp]\nrequest_timeout_ms = 200\n");
    let path = root().join("main.rs");
    service.open_document(&path, "rust", 1, Buffer::from_text("main\n"));
    wait_for_status(service.as_ref(), ready);

    let request = service.definition(&path, Point::new(0, 1));
    let sent = mock.wait_for_method("textDocument/definition");
    drop(request);
    let cancel = mock.wait_for_method("$/cancelRequest");
    assert_eq!(cancel["params"]["id"], sent["id"]);

    let started = Instant::now();
    assert_eq!(
        block_on(service.definition(&path, Point::new(0, 1))),
        Err(ServiceError::Timeout)
    );
    assert!(started.elapsed() >= Duration::from_millis(200));
}

#[test]
fn a_burst_of_diagnostics_arrives_as_one_event() {
    let mock = Mock::new(true);
    let (service, _) = start(Some(&mock), "[lsp]\ndiagnostics_debounce_ms = 100\n");
    let mut events = service.events();
    let path = root().join("diagnostics.rs");
    service.open_document(&path, "rust", 1, Buffer::from_text("ab\n"));
    wait_for_status(service.as_ref(), ready);

    let deadline = Instant::now() + PATIENCE;
    let mut published = Vec::new();
    while Instant::now() < deadline {
        while let Ok(event) = events.try_recv() {
            if let ServiceEvent::Diagnostics { diagnostics, .. } = event {
                published.push(diagnostics);
            }
        }
        if !published.is_empty() {
            // Anything else would arrive within the debounce.
            std::thread::sleep(Duration::from_millis(300));
            while let Ok(event) = events.try_recv() {
                if let ServiceEvent::Diagnostics { diagnostics, .. } = event {
                    published.push(diagnostics);
                }
            }
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(published.len(), 1, "{published:?}");
    assert_eq!(published[0][0].message, "third");
    assert_eq!(published[0][0].range, Point::new(0, 0)..Point::new(0, 2));

    // A later subscriber gets the current diagnostics at once.
    let mut late = service.events();
    let replayed: Vec<_> = std::iter::from_fn(|| late.try_recv().ok()).collect();
    assert!(
        replayed
            .iter()
            .any(|event| matches!(event, ServiceEvent::Diagnostics { .. }))
    );
}

#[test]
fn a_crashed_server_is_started_again_with_its_documents() {
    let mock = Mock::new(true);
    let (service, runner) = start(Some(&mock), "[lsp]\nmax_restarts = 1\n");
    let crash = root().join("crash.rs");
    service.open_document(&crash, "rust", 1, Buffer::from_text("boom\n"));
    // The first server dies on the document; the second gets it again and
    // dies too; then no more restarts.
    wait_for_status(service.as_ref(), |status| {
        matches!(status, ServerStatus::Failed { .. })
    });
    assert_eq!(runner.runs().len(), 2);
    let opens = mock
        .received()
        .iter()
        .filter(|message| message["method"] == "textDocument/didOpen")
        .count();
    assert_eq!(opens, 2);
}

#[test]
fn a_missing_server_is_reported() {
    let (service, _) = start(None, "");
    wait_for_status(
        service.as_ref(),
        |status| matches!(status, ServerStatus::NotFound { program } if program == "mock-ls"),
    );
    assert_eq!(service.capabilities(), language::Capabilities::default());
}

#[test]
fn a_failure_shows_the_server_error_output_and_the_hint() {
    let service = start_failing("error: mock-ls is not installed for this toolchain");
    wait_for_status(service.as_ref(), |status| {
        *status
            == ServerStatus::Failed {
                message: "error: mock-ls is not installed for this toolchain".to_owned(),
                hint: Some("mock-not-installed".to_owned()),
            }
    });
}
