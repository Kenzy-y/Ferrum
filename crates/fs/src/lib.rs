//! File system abstraction.
//!
//! All file access in the IDE goes through the [`Fs`] trait. This keeps the
//! door open for remote development (the file system lives on another
//! machine) and for tests that run against an in-memory file system.
//!
//! The trait is intentionally minimal for now and grows as features need it.

mod real;

use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;

pub use real::RealFs;

/// A boxed, sendable future, used so that [`Fs`] stays object-safe.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Errors returned by [`Fs`] operations.
#[derive(Debug, thiserror::Error)]
pub enum FsError {
    /// The file or directory does not exist.
    #[error("{} not found", path.display())]
    NotFound {
        /// The path that was requested.
        path: PathBuf,
    },
    /// The file exists but is not valid UTF-8 text.
    #[error("{} is not valid UTF-8", path.display())]
    InvalidUtf8 {
        /// The path that was requested.
        path: PathBuf,
    },
    /// Any other I/O failure.
    #[error("I/O error on {}: {source}", path.display())]
    Io {
        /// The path that was requested.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: io::Error,
    },
}

impl FsError {
    /// Wraps an [`io::Error`] for `path`, mapping well-known kinds to
    /// dedicated variants.
    pub fn from_io(path: &Path, source: io::Error) -> Self {
        match source.kind() {
            io::ErrorKind::NotFound => Self::NotFound {
                path: path.to_path_buf(),
            },
            io::ErrorKind::InvalidData => Self::InvalidUtf8 {
                path: path.to_path_buf(),
            },
            _ => Self::Io {
                path: path.to_path_buf(),
                source,
            },
        }
    }

    /// Returns `true` if the error means the path does not exist.
    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::NotFound { .. })
    }
}

/// Access to a file system.
///
/// Implementations may perform blocking work while the returned futures are
/// polled, so callers must poll them on a background executor, never on the
/// UI thread.
pub trait Fs: Send + Sync {
    /// Reads the whole file at `path` as UTF-8 text.
    fn read_to_string<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<String, FsError>>;
}
