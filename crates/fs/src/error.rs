use std::io;
use std::path::{Path, PathBuf};

/// Errors returned by [`crate::Fs`] operations.
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
    /// The operation needs a directory but found something else, or the
    /// other way round.
    #[error("{} is not a {expected}", path.display())]
    WrongKind {
        /// The path that was requested.
        path: PathBuf,
        /// What was expected: `file` or `directory`.
        expected: &'static str,
    },
    /// Watching for changes failed.
    #[error("cannot watch {}: {message}", path.display())]
    Watch {
        /// The watched path.
        path: PathBuf,
        /// What went wrong.
        message: String,
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
