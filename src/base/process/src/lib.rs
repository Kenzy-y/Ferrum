//! Starting programs.
//!
//! All process spawning in the IDE goes through [`ProcessRunner`], never
//! `std::process` directly. Like `fs::Fs`, this is the seam for remote
//! development (processes run on another machine) and for tests
//! ([`FakeProcessRunner`]).

mod child;
mod fake;
mod pipe;
mod real;

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::pin::Pin;

pub use child::{ChildProcess, ProcessControl, SpawnOptions};
pub use fake::{FakeProcessRunner, FakeServer, ServerSide};
pub use real::RealProcessRunner;

/// A boxed, sendable future, used so that [`ProcessRunner`] stays
/// object-safe.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// What to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    program: OsString,
    args: Vec<OsString>,
    current_dir: Option<PathBuf>,
    env: Vec<(OsString, Option<OsString>)>,
}

impl CommandSpec {
    /// Runs `program`, looked up in `PATH` unless it is a path.
    pub fn new(program: impl AsRef<OsStr>) -> Self {
        Self {
            program: program.as_ref().to_owned(),
            args: Vec::new(),
            current_dir: None,
            env: Vec::new(),
        }
    }

    /// Appends an argument.
    #[must_use]
    pub fn arg(mut self, arg: impl AsRef<OsStr>) -> Self {
        self.args.push(arg.as_ref().to_owned());
        self
    }

    /// Appends arguments.
    #[must_use]
    pub fn args(mut self, args: impl IntoIterator<Item = impl AsRef<OsStr>>) -> Self {
        self.args
            .extend(args.into_iter().map(|arg| arg.as_ref().to_owned()));
        self
    }

    /// Working directory of the process.
    #[must_use]
    pub fn current_dir(mut self, directory: impl AsRef<Path>) -> Self {
        self.current_dir = Some(directory.as_ref().to_path_buf());
        self
    }

    /// Sets an environment variable.
    #[must_use]
    pub fn env(mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Self {
        self.env
            .push((key.as_ref().to_owned(), Some(value.as_ref().to_owned())));
        self
    }

    /// Removes an inherited environment variable.
    #[must_use]
    pub fn env_remove(mut self, key: impl AsRef<OsStr>) -> Self {
        self.env.push((key.as_ref().to_owned(), None));
        self
    }

    /// The program.
    pub fn program(&self) -> &OsStr {
        &self.program
    }

    /// The arguments.
    pub fn arguments(&self) -> &[OsString] {
        &self.args
    }

    /// The working directory, if set.
    pub fn working_directory(&self) -> Option<&Path> {
        self.current_dir.as_deref()
    }
}

/// Result of a finished process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    /// Exit code; `None` if the process was killed by a signal.
    pub code: Option<i32>,
    /// Everything written to standard output.
    pub stdout: Vec<u8>,
    /// Everything written to standard error.
    pub stderr: Vec<u8>,
}

impl Output {
    /// Whether the process exited with code 0.
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }
}

/// Errors when starting or waiting for a process.
#[derive(Debug, thiserror::Error)]
pub enum ProcessError {
    /// The program does not exist.
    #[error("program `{}` not found", program.to_string_lossy())]
    NotFound {
        /// The program that was requested.
        program: OsString,
    },
    /// The program exists but could not be started or waited for.
    #[error("cannot run `{}`: {source}", program.to_string_lossy())]
    Io {
        /// The program that was requested.
        program: OsString,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },
}

/// Starts programs.
///
/// Implementations may block the thread that polls the returned futures,
/// so callers poll them on a background executor, never on the UI thread.
pub trait ProcessRunner: Send + Sync {
    /// Runs `command` to completion and collects its output.
    fn run<'a>(&'a self, command: &'a CommandSpec) -> BoxFuture<'a, Result<Output, ProcessError>>;

    /// Starts a long-running process (a language server) with piped
    /// standard streams. Looks the program up in `PATH`, so call it off
    /// the UI thread.
    ///
    /// # Errors
    ///
    /// [`ProcessError::NotFound`] if the program does not exist,
    /// [`ProcessError::Io`] if it cannot be started.
    fn spawn(
        &self,
        command: &CommandSpec,
        options: SpawnOptions,
    ) -> Result<ChildProcess, ProcessError>;
}
