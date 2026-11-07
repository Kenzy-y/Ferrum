use std::path::PathBuf;
use std::str::FromStr as _;

use language::Location;
use serde_json::json;

use super::*;

#[test]
fn server_status_tells_loading_problems_and_readiness() {
    assert_eq!(
        server_status(&json!({"health": "ok", "quiescent": true})),
        ServerStatus::Ready
    );
    assert_eq!(
        server_status(&json!({"health": "ok", "quiescent": false, "message": "Loading"})),
        ServerStatus::Indexing {
            message: Some("Loading".to_owned()),
            percentage: None
        }
    );
    assert_eq!(
        server_status(&json!({"health": "warning", "quiescent": true, "message": "no Cargo.toml"})),
        ServerStatus::Warning {
            message: "no Cargo.toml".to_owned()
        }
    );
}

#[test]
fn configuration_comes_from_settings() {
    let mut settings = RustAnalyzerSettings::default();
    settings
        .config
        .insert("check".to_owned(), json!({"command": "clippy"}));
    settings.path = "/opt/ra".to_owned();
    settings.args = vec!["--log-file".to_owned(), "ra.log".to_owned()];
    let adapter = RustAnalyzerAdapter::new(settings);
    assert_eq!(
        adapter.configuration(Some("rust-analyzer")),
        json!({"check": {"command": "clippy"}})
    );
    assert_eq!(
        adapter.configuration(None),
        json!({"rust-analyzer": {"check": {"command": "clippy"}}})
    );
    assert_eq!(adapter.configuration(Some("other")), Value::Null);
    let command = adapter.command(Path::new("/project"));
    assert_eq!(command.program(), "/opt/ra");
    assert_eq!(command.arguments().len(), 2);

    let mut capabilities = ClientCapabilities::default();
    adapter.extend_capabilities(&mut capabilities);
    assert_eq!(
        capabilities.experimental,
        Some(json!({
            "serverStatusNotification": true,
            // Lenses carry only the commands listed here.
            "commands": {"commands": ["rust-analyzer.showReferences"]},
        }))
    );
}

struct Locations;

impl LocationConverter for Locations {
    fn location(&self, location: &lsp_types::Location) -> Option<Location> {
        let point = |position: lsp_types::Position| {
            text::Point::new(position.line as usize, position.character as usize)
        };
        Some(Location {
            path: PathBuf::from(location.uri.as_str()),
            range: point(location.range.start)..point(location.range.end),
        })
    }
}

#[test]
fn reference_lenses_show_their_locations_and_run_lenses_wait() {
    let adapter = RustAnalyzerAdapter::new(RustAnalyzerSettings::default());
    let uri = lsp_types::Uri::from_str("file:///a.rs").unwrap();
    let position = lsp_types::Position::new(3, 4);
    let found = lsp_types::Location::new(uri.clone(), lsp_types::Range::new(position, position));
    let show = lsp_types::Command {
        title: "1 implementation".to_owned(),
        command: "rust-analyzer.showReferences".to_owned(),
        arguments: Some(vec![json!(uri), json!(position), json!([found])]),
    };
    let LensAction::ShowLocations(locations) = adapter.lens_action(&show, &Locations) else {
        panic!("not a locations lens");
    };
    assert_eq!(locations[0].range.start, text::Point::new(3, 4));
    let run = lsp_types::Command {
        title: "Run".to_owned(),
        command: "rust-analyzer.runSingle".to_owned(),
        arguments: None,
    };
    assert_eq!(
        adapter.lens_action(&run, &Locations),
        LensAction::Unsupported
    );
}

#[test]
fn a_toolchain_without_rust_analyzer_is_recognized() {
    assert_eq!(
        failure_hint(
            "error: Unknown binary 'rust-analyzer.exe' in official toolchain 'stable-x86_64-pc-windows-msvc'."
        ),
        Some(HINT_NOT_INSTALLED)
    );
    assert_eq!(
        failure_hint("error: 'rust-analyzer' is not installed for the toolchain 'stable'"),
        Some(HINT_NOT_INSTALLED)
    );
    assert_eq!(failure_hint("thread 'main' panicked"), None);
}
