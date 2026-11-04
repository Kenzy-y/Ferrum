use std::path::PathBuf;
use std::time::SystemTime;

/// What a path is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntryKind {
    /// A regular file.
    File,
    /// A directory.
    Dir,
    /// Anything else (device, socket, FIFO) or a link whose target is
    /// missing.
    Other,
}

/// Metadata of a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Metadata {
    /// Kind of the path, or of the target for a symbolic link.
    pub kind: EntryKind,
    /// Whether the path itself is a symbolic link.
    pub is_symlink: bool,
    /// Size in bytes (of the target for a link).
    pub len: u64,
    /// Last modification time, if the platform reports it.
    pub modified: Option<SystemTime>,
    /// Whether the file is read-only.
    pub read_only: bool,
}

impl Metadata {
    /// Whether the path is (or points to) a directory.
    pub fn is_dir(&self) -> bool {
        self.kind == EntryKind::Dir
    }

    /// Whether the path is (or points to) a regular file.
    pub fn is_file(&self) -> bool {
        self.kind == EntryKind::File
    }
}

/// An entry of a directory listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    /// Full path of the entry.
    pub path: PathBuf,
    /// File name of the entry.
    pub name: String,
    /// Metadata of the entry.
    pub metadata: Metadata,
}
