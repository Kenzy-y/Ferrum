//! A scripted language server on [`FakeProcessRunner`] pipes.

#![allow(clippy::unwrap_used, clippy::expect_used, dead_code)]

use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use language::{LanguageService, ServerContext, ServerStatus};
use lsp::LspAdapter;
use process::{CommandSpec, FakeProcessRunner, ServerSide};
use serde_json::{Value, json};
use settings::{LayerKind, SettingsLayer, SettingsRegistry, SettingsStore};

/// How long tests wait for something to happen.
pub const PATIENCE: Duration = Duration::from_secs(10);

/// What the mock server saw and how it behaves.
#[derive(Default)]
pub struct Mock {
    received: Mutex<Vec<Value>>,
    changed: Condvar,
    /// Agree to UTF-8 positions.
    pub utf8: bool,
}

impl Mock {
    pub fn new(utf8: bool) -> Arc<Self> {
        Arc::new(Self {
            utf8,
            ..Self::default()
        })
    }

    /// Waits for a message the server received.
    pub fn wait_for(&self, what: &str, matches: impl Fn(&Value) -> bool) -> Value {
        let deadline = Instant::now() + PATIENCE;
        let mut received = self.received.lock().unwrap();
        loop {
            if let Some(message) = received.iter().find(|message| matches(message)) {
                return message.clone();
            }
            let left = deadline.saturating_duration_since(Instant::now());
            assert!(!left.is_zero(), "the server never received {what}");
            received = self.changed.wait_timeout(received, left).unwrap().0;
        }
    }

    /// Waits for a message with `method`.
    pub fn wait_for_method(&self, method: &str) -> Value {
        self.wait_for(method, |message| message["method"] == method)
    }

    pub fn received(&self) -> Vec<Value> {
        self.received.lock().unwrap().clone()
    }

    fn record(&self, message: &Value) {
        self.received.lock().unwrap().push(message.clone());
        self.changed.notify_all();
    }
}

fn read(reader: &mut impl std::io::BufRead) -> Option<Value> {
    let mut length = 0;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let line = line.trim();
        if line.is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length:") {
            length = value.trim().parse().ok()?;
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;
    serde_json::from_slice(&body).ok()
}

fn write(output: &mut impl Write, message: &Value) {
    let body = serde_json::to_vec(message).unwrap();
    let _ = write!(output, "Content-Length: {}\r\n\r\n", body.len());
    let _ = output.write_all(&body);
    let _ = output.flush();
}

fn diagnostic(message: &str) -> Value {
    json!({
        "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 2}},
        "severity": 1,
        "message": message,
    })
}

fn serve(mock: &Mock, side: ServerSide) {
    let mut input = BufReader::new(side.stdin);
    let mut output = side.stdout;
    while let Some(message) = read(&mut input) {
        mock.record(&message);
        let id = message.get("id").cloned();
        let reply = |output: &mut Box<dyn Write + Send>, result: Value| {
            write(
                output,
                &json!({"jsonrpc": "2.0", "id": id, "result": result}),
            );
        };
        let uri = message["params"]["textDocument"]["uri"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        match message["method"].as_str().unwrap_or_default() {
            "initialize" => {
                let mut capabilities = json!({
                    "textDocumentSync": {"openClose": true, "change": 2, "save": true},
                    "hoverProvider": true,
                    "completionProvider": {"triggerCharacters": ["."], "resolveProvider": true},
                    "definitionProvider": true,
                    "inlayHintProvider": true,
                    "codeLensProvider": {"resolveProvider": false},
                });
                if mock.utf8 {
                    capabilities["positionEncoding"] = json!("utf-8");
                }
                reply(&mut output, json!({"capabilities": capabilities}));
            }
            "initialized" => write(
                &mut output,
                &json!({
                    "jsonrpc": "2.0",
                    "id": "configuration",
                    "method": "workspace/configuration",
                    "params": {"items": [{"section": "mock"}]},
                }),
            ),
            "textDocument/didOpen" if uri.ends_with("crash.rs") => return,
            "textDocument/didOpen" if uri.ends_with("diagnostics.rs") => {
                for text in ["first", "second", "third"] {
                    write(
                        &mut output,
                        &json!({
                            "jsonrpc": "2.0",
                            "method": "textDocument/publishDiagnostics",
                            "params": {"uri": uri, "diagnostics": [diagnostic(text)]},
                        }),
                    );
                }
            }
            "textDocument/hover" => reply(
                &mut output,
                json!({"contents": {"kind": "markdown", "value": "**docs**"}}),
            ),
            "textDocument/completion" => reply(
                &mut output,
                json!({"isIncomplete": false, "items": [{"label": "len", "kind": 2}]}),
            ),
            "shutdown" => reply(&mut output, Value::Null),
            "exit" => return,
            // Everything else, `textDocument/definition` included, is not
            // answered: for cancellation and timeouts.
            _ => {}
        }
    }
}

pub struct MockAdapter;

impl LspAdapter for MockAdapter {
    fn name(&self) -> &'static str {
        "mock"
    }

    fn command(&self, root: &Path) -> CommandSpec {
        CommandSpec::new("mock-ls").current_dir(root)
    }

    fn configuration(&self, section: Option<&str>) -> Value {
        json!({ "section": section })
    }
}

/// A workspace folder; nothing is written there.
pub fn root() -> PathBuf {
    std::env::temp_dir().join("ferrum-lsp-tests")
}

pub fn settings(toml: &str) -> SettingsStore {
    let registry = SettingsRegistry::builder()
        .register::<lsp::LspSettings>()
        .unwrap()
        .build();
    let mut store = SettingsStore::new(Arc::new(registry));
    store.set_layer(LayerKind::User, SettingsLayer::parse(toml, None).unwrap());
    store
}

/// A service talking to `mock`, or to nothing if `mock` is `None`.
pub fn start(
    mock: Option<&Arc<Mock>>,
    settings_toml: &str,
) -> (Arc<dyn LanguageService>, Arc<FakeProcessRunner>) {
    let runner = Arc::new(FakeProcessRunner::new());
    if let Some(mock) = mock {
        let mock = mock.clone();
        runner.serve("mock-ls", Arc::new(move |side| serve(&mock, side)));
    }
    let settings = settings(settings_toml);
    let service = lsp::start(
        Arc::new(MockAdapter),
        ServerContext {
            root: root(),
            process: runner.clone(),
            settings: &settings,
        },
    );
    (service, runner)
}

/// Waits until `service` has status `status`.
pub fn wait_for_status(service: &dyn LanguageService, matches: impl Fn(&ServerStatus) -> bool) {
    let deadline = Instant::now() + PATIENCE;
    while !matches(&service.status()) {
        assert!(
            Instant::now() < deadline,
            "status stayed {:?}",
            service.status()
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}
