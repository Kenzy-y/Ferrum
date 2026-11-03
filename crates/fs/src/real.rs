//! [`Fs`] implementation backed by the local file system.

use std::path::Path;

use crate::{BoxFuture, Fs, FsError};

/// The local file system.
///
/// Operations block the thread that polls them; see [`Fs`] for where to
/// poll.
#[derive(Debug, Default, Clone, Copy)]
pub struct RealFs;

impl Fs for RealFs {
    fn read_to_string<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<String, FsError>> {
        Box::pin(async move {
            let _span = tracing::debug_span!("fs.read_to_string", path = %path.display()).entered();
            std::fs::read_to_string(path).map_err(|error| FsError::from_io(path, error))
        })
    }
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::path::PathBuf;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    use super::*;

    /// Polls a future that is expected to complete immediately.
    fn poll_ready<T>(future: impl Future<Output = T>) -> T {
        let mut future = pin!(future);
        match future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("RealFs futures complete on first poll"),
        }
    }

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("fs-test-{name}-{}", std::process::id()));
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
    }

    #[test]
    fn missing_file_is_not_found() {
        let dir = TempDir::new("missing");
        let error = poll_ready(RealFs.read_to_string(&dir.0.join("absent.txt"))).unwrap_err();

        assert!(error.is_not_found(), "{error}");
    }

    #[test]
    fn non_utf8_file_is_reported() {
        let dir = TempDir::new("utf8");
        let path = dir.0.join("binary.bin");
        std::fs::write(&path, [0xff, 0xfe, 0xfd]).unwrap();

        let error = poll_ready(RealFs.read_to_string(&path)).unwrap_err();
        assert!(matches!(error, FsError::InvalidUtf8 { .. }), "{error}");
    }
}
