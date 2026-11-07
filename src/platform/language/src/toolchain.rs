//! What a pack tells about a language's tools, for the status bar and the
//! breadcrumbs (ADR 0011): the toolchain version and a short detail of a
//! file. A pack implements [`Toolchain`] with its own knowledge; the IDE
//! only shows the results.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use fs::Fs;
use process::ProcessRunner;

use crate::BoxFuture;

/// A short detail of a file, shown after the language name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDetail {
    /// Id of the message in i18n; the message takes `$value`.
    pub message_id: &'static str,
    /// The value (`2021`).
    pub value: String,
}

/// The tools of a language.
///
/// The futures may block (they run programs and read files), so they are
/// polled on a background executor, never on the UI thread.
pub trait Toolchain: Send + Sync {
    /// Version of the toolchain used in the folder `root`, or the default
    /// one without a folder; `None` if there is none.
    fn version(
        &self,
        root: Option<PathBuf>,
        process: Arc<dyn ProcessRunner>,
    ) -> BoxFuture<Option<String>>;

    /// A detail of the file at `file`, if the language has one.
    fn file_detail(&self, file: &Path, fs: Arc<dyn Fs>) -> BoxFuture<Option<FileDetail>>;
}

/// How files of a language are marked in the tree, tabs and breadcrumbs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileIconSpec {
    /// A short label (`R`).
    pub letter: &'static str,
    /// Key of the theme color (`icon.file.rust`).
    pub color_token: &'static str,
}
