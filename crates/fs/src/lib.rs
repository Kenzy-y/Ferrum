//! File system abstraction.
//!
//! All file access in the IDE goes through the [`Fs`] trait. This keeps the
//! door open for remote development (the file system lives on another
//! machine) and lets tests run against [`FakeFs`], an in-memory file system.

mod error;
mod fake;
mod real;
mod types;
mod watch;

use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;

use futures::Stream;

pub use error::FsError;
pub use fake::FakeFs;
pub use real::RealFs;
pub use types::{DirEntry, EntryKind, Metadata};
pub use watch::{FsEvent, FsEventKind, WATCH_DEBOUNCE, WatchFilter, Watcher};

/// A boxed, sendable future, used so that [`Fs`] stays object-safe.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// A boxed, sendable stream.
pub type BoxStream<T> = Pin<Box<dyn Stream<Item = T> + Send>>;

/// Access to a file system.
///
/// Implementations may block the thread that polls the returned futures,
/// so callers poll them on a background executor, never on the UI thread.
pub trait Fs: Send + Sync {
    /// Reads the whole file at `path` as UTF-8 text.
    fn read_to_string<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<String, FsError>>;

    /// Reads the whole file at `path`.
    fn read<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<Vec<u8>, FsError>>;

    /// Replaces the contents of `path` atomically: readers see the old or
    /// the new contents, never a mix. Permissions of an existing file are
    /// kept, and if `path` is a symbolic link the file it points to is
    /// written, so the link stays a link.
    fn write<'a>(
        &'a self,
        path: &'a Path,
        contents: &'a [u8],
    ) -> BoxFuture<'a, Result<(), FsError>>;

    /// Creates `path` and every missing parent directory.
    fn create_dir_all<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<(), FsError>>;

    /// Metadata of `path`; symbolic links are reported as links and
    /// describe their target. `None` if nothing exists at `path`.
    fn metadata<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<Option<Metadata>, FsError>>;

    /// Entries of the directory `path`, in no particular order.
    fn read_dir<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<Vec<DirEntry>, FsError>>;

    /// The absolute path with every symbolic link resolved. Used to detect
    /// link cycles when walking trees.
    fn canonicalize<'a>(
        &'a self,
        path: &'a Path,
    ) -> BoxFuture<'a, Result<std::path::PathBuf, FsError>>;

    /// Watches `root` recursively. Paths rejected by `filter` are dropped
    /// before they leave the watcher thread, so busy ignored directories
    /// (build output) cost almost nothing. Events arrive in batches; dropping
    /// the [`Watcher`] stops watching.
    ///
    /// # Errors
    ///
    /// Fails if the platform cannot watch `root`.
    fn watch(&self, root: &Path, filter: Arc<dyn WatchFilter>) -> Result<Watcher, FsError>;
}
