//! Programs in a pseudo-terminal, for the integrated terminal.
//!
//! The pseudo-terminal comes from `alacritty_terminal` (ADR 0010): `ConPTY` on
//! Windows, `openpty` on Unix. Its reading loop and the emulator live in the
//! `terminal` crate; this module only starts the program, so that process
//! spawning stays in one crate.

use std::collections::HashMap;
use std::fmt;

use alacritty_terminal::event::WindowSize;
use alacritty_terminal::tty;

use crate::{CommandSpec, ProcessError};

/// Size of a pseudo-terminal: its grid and the pixel size of one cell
/// (some programs draw images by pixels).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PtySize {
    /// Columns of the grid.
    pub columns: u16,
    /// Rows of the grid.
    pub rows: u16,
    /// Width of a cell in pixels.
    pub cell_width: u16,
    /// Height of a cell in pixels.
    pub cell_height: u16,
}

impl PtySize {
    /// The size as `alacritty_terminal` takes it.
    pub fn window_size(self) -> WindowSize {
        WindowSize {
            num_lines: self.rows.max(1),
            num_cols: self.columns.max(1),
            cell_width: self.cell_width.max(1),
            cell_height: self.cell_height.max(1),
        }
    }
}

/// A program running in a pseudo-terminal. Dropping it ends the program.
pub struct Pty(tty::Pty);

impl Pty {
    /// The pseudo-terminal, for the `alacritty_terminal` event loop that
    /// reads and writes it.
    pub fn into_inner(self) -> tty::Pty {
        self.0
    }
}

impl fmt::Debug for Pty {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Pty").finish_non_exhaustive()
    }
}

/// Starts `command` in a new pseudo-terminal of `size`. Environment
/// removals of `command` are not supported by the pseudo-terminal and are
/// ignored.
pub(crate) fn spawn(command: &CommandSpec, size: PtySize) -> Result<Pty, ProcessError> {
    let _span = tracing::debug_span!(
        "process.spawn_pty",
        program = %command.program.to_string_lossy()
    )
    .entered();
    let program = command.program.to_string_lossy().into_owned();
    let args = command
        .args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let env: HashMap<String, String> = command
        .env
        .iter()
        .filter_map(|(key, value)| {
            let value = value.as_ref()?;
            Some((
                key.to_string_lossy().into_owned(),
                value.to_string_lossy().into_owned(),
            ))
        })
        .collect();
    let options = tty::Options {
        shell: Some(tty::Shell::new(program, args)),
        working_directory: command.current_dir.clone(),
        drain_on_exit: true,
        env,
        #[cfg(windows)]
        escape_args: true,
    };
    // The id only names the window to the program (`WINDOWID`); there is
    // no platform window per terminal.
    tty::new(&options, size.window_size(), 0)
        .map(Pty)
        .map_err(|source| {
            if source.kind() == std::io::ErrorKind::NotFound {
                ProcessError::NotFound {
                    program: command.program.clone(),
                }
            } else {
                ProcessError::Io {
                    program: command.program.clone(),
                    source,
                }
            }
        })
}
