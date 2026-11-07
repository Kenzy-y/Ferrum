//! Settings of the `[terminal]` namespace.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Settings of the integrated terminal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct TerminalSettings {
    /// Shell of new terminals: the name of a detected shell ("PowerShell",
    /// "bash") or the path of a program. Empty: the first detected shell
    /// (PowerShell on Windows, `$SHELL` elsewhere).
    pub shell: String,
    /// Arguments for `shell` when it is a program path.
    pub shell_args: Vec<String>,
    /// Lines kept above the screen for scrolling back.
    pub scrollback_lines: usize,
    /// Whether the cursor blinks. It never blinks in an unfocused terminal.
    pub cursor_blink: bool,
    /// How long the cursor stays shown or hidden while blinking, in
    /// milliseconds.
    pub cursor_blink_interval_ms: u64,
    /// Copies the selection to the clipboard as soon as it is made.
    pub copy_on_select: bool,
    /// Multiplier of mouse wheel scrolling.
    pub scroll_sensitivity: f32,
    /// Closes a terminal when its shell exits.
    pub close_on_exit: bool,
}

impl Default for TerminalSettings {
    fn default() -> Self {
        Self {
            shell: String::new(),
            shell_args: Vec::new(),
            scrollback_lines: 10_000,
            cursor_blink: true,
            cursor_blink_interval_ms: 530,
            copy_on_select: false,
            scroll_sensitivity: 1.0,
            close_on_exit: true,
        }
    }
}

impl settings::Settings for TerminalSettings {
    const NAMESPACE: &'static str = "terminal";
}
