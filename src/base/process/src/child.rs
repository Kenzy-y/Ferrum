//! A running process with piped standard streams.

use std::fmt;
use std::io::{Read, Write};
use std::sync::Arc;

/// How to start a long-running process.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SpawnOptions {
    /// Run below normal priority, so a busy process (a language server
    /// indexing) never slows the IDE down.
    pub lowered_priority: bool,
}

/// Controls a running process; shared by whoever needs to stop it.
pub trait ProcessControl: Send + Sync {
    /// Kills the process. Killing a finished process is not an error.
    ///
    /// # Errors
    ///
    /// Fails if the operating system refuses.
    fn kill(&self) -> std::io::Result<()>;

    /// The exit code once the process has finished: `Some(None)` if it
    /// was killed by a signal, `None` while it runs.
    ///
    /// # Errors
    ///
    /// Fails if the operating system cannot tell.
    fn try_wait(&self) -> std::io::Result<Option<Option<i32>>>;
}

/// A started process. Each stream is meant for its own thread: reads and
/// writes block.
pub struct ChildProcess {
    /// Standard input of the process.
    pub stdin: Box<dyn Write + Send>,
    /// Standard output of the process.
    pub stdout: Box<dyn Read + Send>,
    /// Standard error of the process.
    pub stderr: Box<dyn Read + Send>,
    /// Stops the process and tells whether it has exited.
    pub control: Arc<dyn ProcessControl>,
}

impl fmt::Debug for ChildProcess {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ChildProcess")
            .finish_non_exhaustive()
    }
}
