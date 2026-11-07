//! The workspace: the folder open in a window, its settings layer and the
//! list of recently opened projects.
//!
//! Everything goes through `fs::Fs`, so the crate is tested with
//! `fs::FakeFs` and works unchanged with a remote file system later.

mod file_tree;
mod git_excludes;
mod ignore_rules;
mod project_settings;
mod recent;
mod window_state;
mod workspace;

pub use file_tree::{DirListing, FileTree, RowState, TreeEntry, TreeRow, load_dir};
pub use git_excludes::{GitEnvironment, RepositoryExcludes, read_repository_excludes};
pub use ignore_rules::{DEFAULT_HIDDEN, IGNORE_FILE_NAME, IgnoreRules, Visibility};
pub use project_settings::{FilesSettings, ProjectSettings};
pub use recent::{RECENT_PROJECTS_FILE_NAME, RecentProject, RecentProjects};
pub use window_state::{WINDOW_STATE_FILE_NAME, WindowState};
pub use workspace::Workspace;

use std::path::PathBuf;

/// Errors of workspace operations.
#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    /// The path to open does not exist.
    #[error("{} does not exist", path.display())]
    NotFound {
        /// The requested path.
        path: PathBuf,
    },
    /// The path to open is not a folder.
    #[error("{} is not a folder", path.display())]
    NotAFolder {
        /// The requested path.
        path: PathBuf,
    },
    /// The recent projects file cannot be written.
    #[error("cannot save recent projects: {0}")]
    Serialize(String),
    /// A file system operation failed.
    #[error(transparent)]
    Fs(#[from] fs::FsError),
}
