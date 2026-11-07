//! A real shell in a real pseudo-terminal.

use std::time::{Duration, Instant};

use process::{PtySize, RealProcessRunner};
use terminal::{LaunchOptions, Palette, Rgb, ShellProfile, Snapshot, TerminalSession};

/// How long the shell may take to start and print.
const DEADLINE: Duration = Duration::from_secs(20);

fn shell() -> ShellProfile {
    if cfg!(windows) {
        ShellProfile::from_program("cmd.exe", Vec::new())
    } else {
        ShellProfile::from_program("/bin/sh", Vec::new())
    }
}

fn screen_text(snapshot: &Snapshot) -> String {
    snapshot
        .rows
        .iter()
        .map(|row| {
            row.spans
                .iter()
                .map(|span| span.text.as_str())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn wait_for(session: &TerminalSession, text: &str) -> bool {
    let start = Instant::now();
    while start.elapsed() < DEADLINE {
        if screen_text(&session.snapshot()).contains(text) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

#[test]
fn typed_commands_run_and_their_output_is_emulated() {
    let options = LaunchOptions {
        profile: shell(),
        working_directory: None,
        size: PtySize {
            columns: 80,
            rows: 24,
            cell_width: 8,
            cell_height: 16,
        },
        scrollback_lines: 100,
        palette: Palette {
            foreground: Rgb::new(200, 200, 200),
            background: Rgb::new(0, 0, 0),
            cursor: Rgb::new(255, 255, 255),
            ansi: [Rgb::new(128, 128, 128); 16],
        },
    };
    let (mut session, _events) = TerminalSession::spawn(&RealProcessRunner, options).unwrap();
    // The marker is assembled by the shell, so the echo of the typed
    // command alone does not match.
    session.write(b"echo ferrum-%OS%-$((40+2))-done\r".to_vec());
    let marker = if cfg!(windows) {
        "ferrum-Windows_NT-$((40+2))-done"
    } else {
        "ferrum-%OS%-42-done"
    };
    assert!(
        wait_for(&session, marker),
        "{}",
        screen_text(&session.snapshot())
    );

    session.resize(PtySize {
        columns: 40,
        rows: 10,
        cell_width: 8,
        cell_height: 16,
    });
    let snapshot = session.snapshot();
    assert_eq!(snapshot.columns, 40);
    assert_eq!(snapshot.rows.len(), 10);
}
