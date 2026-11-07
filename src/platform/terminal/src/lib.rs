//! Terminal sessions without UI.
//!
//! A [`TerminalSession`] runs a shell in a pseudo-terminal (started through
//! `process::ProcessRunner`), feeds its output to the `alacritty_terminal`
//! emulator on the emulator's own thread and tells the UI about changes
//! through [`TerminalEvent`]s. The UI draws a [`Snapshot`] of the visible
//! screen and turns key presses into bytes with [`keys::encode`].
//! [`profiles`] finds the shells of the machine.

pub mod keys;
mod palette;
pub mod profiles;
mod session;
mod settings;
mod snapshot;

pub use crate::palette::{Palette, Rgb};
pub use crate::profiles::ShellProfile;
pub use crate::session::{
    CellPoint, LaunchOptions, ScrollTarget, SelectionKind, TerminalError, TerminalEvent,
    TerminalModes, TerminalSession,
};
pub use crate::settings::TerminalSettings;
pub use crate::snapshot::{CellStyle, Cursor, CursorShape, Snapshot, SnapshotRow, Span};
