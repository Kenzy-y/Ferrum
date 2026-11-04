//! The workspace: the folder open in a window, its settings layer and the
//! list of recently opened projects.
//!
//! Everything goes through `fs::Fs`, so the crate is tested with
//! `fs::FakeFs` and works unchanged with a remote file system later.

mod project_settings;
mod recent;
mod workspace;

pub use project_settings::ProjectSettings;
pub use recent::{RECENT_PROJECTS_FILE_NAME, RecentProject, RecentProjects};
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
