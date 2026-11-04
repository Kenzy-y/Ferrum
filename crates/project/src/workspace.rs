//! The folder open in a window.

use std::path::{Path, PathBuf};

use fs::Fs;
use settings::SettingsLayer;

use crate::ProjectError;

/// An open folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    root: PathBuf,
    name: String,
}

impl Workspace {
    /// Opens the folder at `path`: checks that it is a folder and resolves
    /// symbolic links, so the same folder opened by different paths is one
    /// workspace.
    ///
    /// # Errors
    ///
    /// Fails if `path` does not exist or is not a folder.
    pub async fn open(fs: &dyn Fs, path: &Path) -> Result<Self, ProjectError> {
        let metadata = fs
            .metadata(path)
            .await?
            .ok_or_else(|| ProjectError::NotFound {
                path: path.to_path_buf(),
            })?;
        if !metadata.is_dir() {
            return Err(ProjectError::NotAFolder {
                path: path.to_path_buf(),
            });
        }
        let root = fs.canonicalize(path).await?;
        Ok(Self::from_root(root))
    }

    /// A workspace for an already validated root.
    pub fn from_root(root: PathBuf) -> Self {
        let name = display_name(&root);
        Self { root, name }
    }

    /// The folder.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Name shown to the user: the folder name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The root for display, without the `\\?\` prefix Windows adds to
    /// canonical paths.
    pub fn display_root(&self) -> String {
        display_path(&self.root)
    }

    /// The workspace settings file (`.ferrum/settings.toml`; the folder
    /// name comes from `app_identity`).
    pub fn settings_file(&self) -> PathBuf {
        settings::paths::workspace_settings(&self.root)
    }

    /// Reads the workspace settings layer. A missing file is `None`; an
    /// invalid file is logged and also `None`, so the user settings apply.
    pub async fn load_settings(&self, fs: &dyn Fs) -> Option<SettingsLayer> {
        let path = self.settings_file();
        match settings::load::load_layer(fs, &path).await {
            Ok(layer) => layer,
            Err(error) => {
                tracing::warn!("{error}; workspace settings ignored");
                None
            }
        }
    }
}

/// The last component of `root`, or the whole path for a drive root.
fn display_name(root: &Path) -> String {
    root.file_name().map_or_else(
        || display_path(root),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// `path` as shown to users: Windows' verbatim prefix (`\\?\C:\...`) is
/// dropped, it only matters to the file system API.
pub(crate) fn display_path(path: &Path) -> String {
    let text = path.to_string_lossy();
    text.strip_prefix(r"\\?\UNC\")
        .map(|rest| format!(r"\\{rest}"))
        .or_else(|| text.strip_prefix(r"\\?\").map(str::to_owned))
        .unwrap_or_else(|| text.into_owned())
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    use fs::FakeFs;

    use super::*;

    fn ready<T>(future: impl Future<Output = T>) -> T {
        match pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("FakeFs futures complete immediately"),
        }
    }

    #[test]
    fn opens_folders_only() {
        let fs = FakeFs::new();
        fs.insert_file("/work/forge/Cargo.toml", "[package]");

        let workspace = ready(Workspace::open(&fs, Path::new("/work/forge"))).unwrap();
        assert_eq!(workspace.name(), "forge");
        assert_eq!(workspace.root(), Path::new("/work/forge"));

        assert!(matches!(
            ready(Workspace::open(&fs, Path::new("/work/forge/Cargo.toml"))),
            Err(ProjectError::NotAFolder { .. })
        ));
        assert!(matches!(
            ready(Workspace::open(&fs, Path::new("/work/missing"))),
            Err(ProjectError::NotFound { .. })
        ));
    }

    #[test]
    fn links_resolve_to_one_workspace() {
        let fs = FakeFs::new();
        fs.insert_dir("/work/forge");
        fs.insert_symlink("/home/forge", "/work/forge");
        let workspace = ready(Workspace::open(&fs, Path::new("/home/forge"))).unwrap();
        assert_eq!(workspace.root(), Path::new("/work/forge"));
    }

    #[test]
    fn loads_workspace_settings() {
        let fs = FakeFs::new();
        let workspace = Workspace::from_root(PathBuf::from("/work/forge"));
        fs.insert_file(workspace.settings_file(), "[editor]\ntab_size = 2");
        assert!(ready(workspace.load_settings(&fs)).is_some());

        fs.insert_file(workspace.settings_file(), "[editor");
        assert!(
            ready(workspace.load_settings(&fs)).is_none(),
            "invalid files are ignored"
        );
    }

    #[test]
    fn display_paths_drop_the_verbatim_prefix() {
        assert_eq!(
            display_path(Path::new(r"\\?\C:\projects\forge")),
            r"C:\projects\forge"
        );
        assert_eq!(
            display_path(Path::new(r"\\?\UNC\server\share")),
            r"\\server\share"
        );
        assert_eq!(display_path(Path::new("/work/forge")), "/work/forge");
    }
}
