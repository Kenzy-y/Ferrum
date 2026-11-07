//! [`Fs`] implementation backed by the local file system.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::watch::watch_real;
use crate::{
    BoxFuture, DirEntry, EntryKind, Fs, FsError, Metadata, WatchFilter, WatchMode, Watcher,
};

/// The local file system.
///
/// Operations block the thread that polls them; see [`Fs`] for where to
/// poll.
#[derive(Debug, Default, Clone, Copy)]
pub struct RealFs;

/// Makes temporary file names unique within the process.
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

impl Fs for RealFs {
    fn read_to_string<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<String, FsError>> {
        Box::pin(async move {
            let _span = tracing::debug_span!("fs.read_to_string", path = %path.display()).entered();
            std::fs::read_to_string(path).map_err(|error| FsError::from_io(path, error))
        })
    }

    fn read<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<Vec<u8>, FsError>> {
        Box::pin(async move {
            let _span = tracing::debug_span!("fs.read", path = %path.display()).entered();
            std::fs::read(path).map_err(|error| FsError::from_io(path, error))
        })
    }

    fn write<'a>(
        &'a self,
        path: &'a Path,
        contents: &'a [u8],
    ) -> BoxFuture<'a, Result<(), FsError>> {
        Box::pin(async move {
            let _span = tracing::debug_span!("fs.write", path = %path.display()).entered();
            write_atomic(path, contents)
        })
    }

    fn create_dir_all<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<(), FsError>> {
        Box::pin(async move {
            std::fs::create_dir_all(path).map_err(|error| FsError::from_io(path, error))
        })
    }

    fn metadata<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<Option<Metadata>, FsError>> {
        Box::pin(async move { metadata(path) })
    }

    fn read_dir<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<Vec<DirEntry>, FsError>> {
        Box::pin(async move {
            let _span = tracing::debug_span!("fs.read_dir", path = %path.display()).entered();
            let reader = std::fs::read_dir(path).map_err(|error| FsError::from_io(path, error))?;
            let mut entries = Vec::new();
            for entry in reader {
                let entry = entry.map_err(|error| FsError::from_io(path, error))?;
                let entry_path = entry.path();
                // The entry may vanish between listing and stat; skip it.
                let Some(metadata) = metadata(&entry_path)? else {
                    continue;
                };
                entries.push(DirEntry {
                    name: entry.file_name().to_string_lossy().into_owned(),
                    path: entry_path,
                    metadata,
                });
            }
            Ok(entries)
        })
    }

    fn canonicalize<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<PathBuf, FsError>> {
        Box::pin(async move {
            std::fs::canonicalize(path)
                .map(without_verbatim_prefix)
                .map_err(|error| FsError::from_io(path, error))
        })
    }

    fn watch(
        &self,
        root: &Path,
        mode: WatchMode,
        filter: Arc<dyn WatchFilter>,
    ) -> Result<Watcher, FsError> {
        watch_real(root, mode, filter)
    }
}

fn metadata(path: &Path) -> Result<Option<Metadata>, FsError> {
    let link = match std::fs::symlink_metadata(path) {
        Ok(link) => link,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(FsError::from_io(path, error)),
    };
    let is_symlink = link.file_type().is_symlink();
    // For a link, describe its target; a dangling link is `Other`.
    let target = if is_symlink {
        std::fs::metadata(path).ok()
    } else {
        Some(link)
    };
    let Some(target) = target else {
        return Ok(Some(Metadata {
            kind: EntryKind::Other,
            is_symlink,
            len: 0,
            modified: None,
            read_only: false,
        }));
    };
    let kind = if target.is_dir() {
        EntryKind::Dir
    } else if target.is_file() {
        EntryKind::File
    } else {
        EntryKind::Other
    };
    Ok(Some(Metadata {
        kind,
        is_symlink,
        len: target.len(),
        modified: target.modified().ok(),
        read_only: target.permissions().readonly(),
    }))
}

/// The file a write to `path` must change: `path` itself, or the final
/// target if `path` is a symbolic link (possibly a chain, possibly
/// dangling).
fn write_target(path: &Path) -> Result<PathBuf, FsError> {
    let mut current = path.to_path_buf();
    // Bounded like the operating system bounds link resolution.
    for _ in 0..MAX_LINK_DEPTH {
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                let target = std::fs::read_link(&current)
                    .map_err(|error| FsError::from_io(&current, error))?;
                current = match current.parent() {
                    Some(parent) if target.is_relative() => parent.join(target),
                    _ => target,
                };
            }
            Ok(_) => return Ok(current),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(current),
            Err(error) => return Err(FsError::from_io(&current, error)),
        }
    }
    Err(FsError::Io {
        path: path.to_path_buf(),
        source: std::io::Error::other("too many levels of symbolic links"),
    })
}

/// Maximum number of symbolic links followed when resolving a write target.
const MAX_LINK_DEPTH: usize = 40;

/// Writes `contents` to a temporary file next to the target, copies the
/// target's permissions onto it and renames it over the target.
fn write_atomic(path: &Path, contents: &[u8]) -> Result<(), FsError> {
    let target = write_target(path)?;
    let directory = target
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let file_name = target
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temp = directory.join(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let existing_permissions = std::fs::metadata(&target)
        .ok()
        .map(|metadata| metadata.permissions());

    let result = (|| {
        let mut file = std::fs::File::create(&temp)?;
        file.write_all(contents)?;
        file.sync_all()?;
        drop(file);
        if let Some(permissions) = existing_permissions {
            std::fs::set_permissions(&temp, permissions)?;
        }
        std::fs::rename(&temp, &target)
    })();
    if let Err(error) = result {
        let _ = std::fs::remove_file(&temp);
        return Err(FsError::from_io(&target, error));
    }
    Ok(())
}

/// `MAX_PATH` (260) less the terminating null: longer paths need the
/// verbatim form.
const MAX_PLAIN_PATH: usize = 259;

/// Windows canonical paths are verbatim (`\\?\C:\a`); the rest of the IDE,
/// language servers and users expect `C:\a`, and `\\server\share` for
/// network paths. Paths that need the verbatim form (too long, or not on a
/// drive or share) keep it.
fn without_verbatim_prefix(path: PathBuf) -> PathBuf {
    let Some(text) = path.to_str() else {
        return path;
    };
    let plain = if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = text.strip_prefix(r"\\?\") {
        let on_drive = matches!(
            rest.as_bytes(),
            [drive, b':', b'\\', ..] if drive.is_ascii_alphabetic()
        ) || matches!(rest.as_bytes(), [drive, b':'] if drive.is_ascii_alphabetic());
        if !on_drive {
            return path;
        }
        rest.to_owned()
    } else {
        return path;
    };
    if plain.len() > MAX_PLAIN_PATH {
        return path;
    }
    PathBuf::from(plain)
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    use super::*;

    /// Polls a future that is expected to complete immediately.
    fn poll_ready<T>(future: impl Future<Output = T>) -> T {
        match pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("RealFs futures complete on first poll"),
        }
    }

    #[test]
    fn canonical_paths_lose_the_verbatim_prefix() {
        let plain = |text: &str| without_verbatim_prefix(PathBuf::from(text));
        assert_eq!(plain(r"\\?\C:\Projects\a"), PathBuf::from(r"C:\Projects\a"));
        assert_eq!(
            plain(r"\\?\UNC\host\share\a"),
            PathBuf::from(r"\\host\share\a")
        );
        assert_eq!(plain(r"\\?\Volume{1}\a"), PathBuf::from(r"\\?\Volume{1}\a"));
        assert_eq!(plain("/home/a"), PathBuf::from("/home/a"));
        let long = format!(r"\\?\C:\{}", "a".repeat(300));
        assert_eq!(
            plain(&long),
            PathBuf::from(&long),
            "too long for the plain form"
        );
    }

    /// Regression: a verbatim root made language servers ignore every
    /// document (`file:////%3F/C:/...`).
    #[cfg(windows)]
    #[test]
    fn real_canonical_paths_are_plain() {
        let canonical = poll_ready(RealFs.canonicalize(&std::env::temp_dir())).unwrap();
        assert!(
            !canonical.to_string_lossy().starts_with(r"\\?\"),
            "{}",
            canonical.display()
        );
    }

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("fs-test-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn reads_existing_file() {
        let dir = TempDir::new("read");
        let path = dir.0.join("file.txt");
        std::fs::write(&path, "héllo").unwrap();

        assert_eq!(poll_ready(RealFs.read_to_string(&path)).unwrap(), "héllo");
        assert_eq!(poll_ready(RealFs.read(&path)).unwrap(), "héllo".as_bytes());
    }

    #[test]
    fn missing_file_is_not_found() {
        let dir = TempDir::new("missing");
        let error = poll_ready(RealFs.read_to_string(&dir.0.join("absent.txt"))).unwrap_err();
        assert!(error.is_not_found(), "{error}");
        assert_eq!(
            poll_ready(RealFs.metadata(&dir.0.join("absent.txt"))).unwrap(),
            None
        );
    }

    #[test]
    fn non_utf8_file_is_reported() {
        let dir = TempDir::new("utf8");
        let path = dir.0.join("binary.bin");
        std::fs::write(&path, [0xff, 0xfe, 0xfd]).unwrap();

        let error = poll_ready(RealFs.read_to_string(&path)).unwrap_err();
        assert!(matches!(error, FsError::InvalidUtf8 { .. }), "{error}");
    }

    #[test]
    fn writes_new_and_existing_files_without_leftovers() {
        let dir = TempDir::new("write");
        let path = dir.0.join("nested").join("file.txt");
        poll_ready(RealFs.create_dir_all(path.parent().unwrap())).unwrap();
        poll_ready(RealFs.write(&path, b"one")).unwrap();
        poll_ready(RealFs.write(&path, b"two")).unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "two");
        let names: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(
            names,
            vec![std::ffi::OsString::from("file.txt")],
            "no temporary files remain"
        );
    }

    #[cfg(unix)]
    #[test]
    fn write_keeps_permissions() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = TempDir::new("permissions");
        let path = dir.0.join("script.sh");
        std::fs::write(&path, "old").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o750)).unwrap();

        poll_ready(RealFs.write(&path, b"new")).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o750);
    }

    #[cfg(unix)]
    #[test]
    fn write_through_symlink_keeps_the_link() {
        let dir = TempDir::new("symlink");
        let target = dir.0.join("target.txt");
        let link = dir.0.join("link.txt");
        std::fs::write(&target, "old").unwrap();
        std::os::unix::fs::symlink("target.txt", &link).unwrap();

        poll_ready(RealFs.write(&link, b"new")).unwrap();
        assert!(
            std::fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "new");
        let metadata = poll_ready(RealFs.metadata(&link)).unwrap().unwrap();
        assert!(metadata.is_symlink && metadata.is_file());
    }

    #[test]
    fn lists_directories_with_kinds() {
        let dir = TempDir::new("list");
        std::fs::create_dir(dir.0.join("sub")).unwrap();
        std::fs::write(dir.0.join("file.txt"), "x").unwrap();

        let mut entries = poll_ready(RealFs.read_dir(&dir.0)).unwrap();
        entries.sort_by(|left, right| left.name.cmp(&right.name));
        let kinds: Vec<_> = entries
            .iter()
            .map(|entry| (entry.name.as_str(), entry.metadata.kind))
            .collect();
        assert_eq!(
            kinds,
            vec![("file.txt", EntryKind::File), ("sub", EntryKind::Dir)]
        );
    }
}
