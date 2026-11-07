//! Creating a file from the Explorer: where it goes and writing it.

use std::path::{Component, Path, PathBuf};

use fs::{Fs, FsError};

/// Why a file could not be created.
#[derive(Debug, thiserror::Error)]
pub(crate) enum NewFileError {
    /// No name was typed.
    #[error("the name is empty")]
    Empty,
    /// The name leaves the folder (`..`, an absolute path).
    #[error("the name leaves the folder")]
    OutsideFolder,
    /// Something already exists at the path.
    #[error("{} already exists", .0.display())]
    Exists(PathBuf),
    /// The file system refused.
    #[error(transparent)]
    Fs(#[from] FsError),
}

impl NewFileError {
    /// Message id of the error in i18n.
    pub(crate) fn message_id(&self) -> &'static str {
        match self {
            Self::Empty => "new-file-error-empty",
            Self::OutsideFolder => "new-file-error-outside",
            Self::Exists(_) => "new-file-error-exists",
            Self::Fs(_) => "new-file-error-failed",
        }
    }
}

/// The path of a new file named `name` in `folder`. The name may contain
/// folders (`src/main.rs`), which are created too, but must stay inside
/// `folder`.
pub(crate) fn target_path(folder: &Path, name: &str) -> Result<PathBuf, NewFileError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(NewFileError::Empty);
    }
    let relative = Path::new(name);
    let mut path = folder.to_path_buf();
    let mut parts = 0;
    for component in relative.components() {
        match component {
            Component::Normal(part) => {
                path.push(part);
                parts += 1;
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(NewFileError::OutsideFolder);
            }
        }
    }
    if parts == 0 {
        return Err(NewFileError::Empty);
    }
    Ok(path)
}

/// Creates an empty file at `path` with its missing folders. Does file
/// I/O: poll on a background executor.
pub(crate) async fn create(fs: &dyn Fs, path: &Path) -> Result<(), NewFileError> {
    if fs.metadata(path).await?.is_some() {
        return Err(NewFileError::Exists(path.to_path_buf()));
    }
    if let Some(parent) = path.parent() {
        fs.create_dir_all(parent).await?;
    }
    fs.write(path, b"").await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    use fs::FakeFs;

    use super::*;

    fn ready<T>(future: impl Future<Output = T>) -> T {
        match pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("FakeFs futures complete on first poll"),
        }
    }

    #[test]
    fn names_stay_inside_the_folder() {
        let folder = Path::new("/project/src");
        assert_eq!(
            target_path(folder, " lib.rs ").unwrap(),
            folder.join("lib.rs")
        );
        assert_eq!(
            target_path(folder, "net/http.rs").unwrap(),
            folder.join("net").join("http.rs")
        );
        assert!(matches!(
            target_path(folder, "  "),
            Err(NewFileError::Empty)
        ));
        assert!(matches!(target_path(folder, "."), Err(NewFileError::Empty)));
        assert!(matches!(
            target_path(folder, "../secret"),
            Err(NewFileError::OutsideFolder)
        ));
        assert!(matches!(
            target_path(folder, "/etc/passwd"),
            Err(NewFileError::OutsideFolder)
        ));
    }

    #[test]
    fn creates_the_file_and_its_folders_but_never_overwrites() {
        let fs = FakeFs::new();
        fs.insert_dir("/project");
        let path = Path::new("/project/src/new.rs");
        ready(create(&fs, path)).unwrap();
        assert_eq!(ready(fs.read_to_string(path)).unwrap(), "");
        fs.insert_file("/project/taken.rs", "keep");
        let taken = Path::new("/project/taken.rs");
        assert!(matches!(
            ready(create(&fs, taken)),
            Err(NewFileError::Exists(_))
        ));
        assert_eq!(ready(fs.read_to_string(taken)).unwrap(), "keep");
    }
}
