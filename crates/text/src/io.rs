//! Reading and writing documents through `fs::Fs`.

use std::path::{Path, PathBuf};

use fs::{Fs, FsError};

use crate::Buffer;

/// How many leading bytes are checked for NUL when telling binary files
/// from text: enough for any real header, cheap for huge files.
const BINARY_SNIFF_BYTES: usize = 8 * 1024;

/// Errors when opening or saving a file.
#[derive(Debug, thiserror::Error)]
pub enum TextError {
    /// The file looks binary (it contains NUL bytes).
    #[error("{} is a binary file", path.display())]
    Binary {
        /// The file.
        path: PathBuf,
    },
    /// The file is text but not UTF-8.
    #[error("{} is not UTF-8 text; other encodings are not supported yet", path.display())]
    NotUtf8 {
        /// The file.
        path: PathBuf,
    },
    /// Reading or writing failed.
    #[error(transparent)]
    Fs(#[from] FsError),
}

/// Reads `path` into a buffer. Runs on a background executor: it touches
/// the whole file.
///
/// # Errors
///
/// Fails for binary and non-UTF-8 files, and if the file cannot be read.
pub async fn load(fs: &dyn Fs, path: &Path) -> Result<Buffer, TextError> {
    let _span = tracing::debug_span!("text.load", path = %path.display()).entered();
    let bytes = fs.read(path).await?;
    if bytes.iter().take(BINARY_SNIFF_BYTES).any(|byte| *byte == 0) {
        return Err(TextError::Binary {
            path: path.to_path_buf(),
        });
    }
    let text = String::from_utf8(bytes).map_err(|_| TextError::NotUtf8 {
        path: path.to_path_buf(),
    })?;
    Ok(Buffer::from_text(&text))
}

/// Writes `buffer` to `path` atomically with its line ending and BOM.
///
/// # Errors
///
/// Fails if the file cannot be written.
pub async fn save(fs: &dyn Fs, path: &Path, buffer: &Buffer) -> Result<(), TextError> {
    let _span = tracing::debug_span!("text.save", path = %path.display()).entered();
    fs.write(path, buffer.to_text().as_bytes()).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    use fs::FakeFs;

    use super::*;
    use crate::LineEnding;

    fn ready<T>(future: impl Future<Output = T>) -> T {
        match pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("FakeFs futures complete immediately"),
        }
    }

    #[test]
    fn round_trips_line_endings() {
        let fs = FakeFs::new();
        fs.insert_file("/p/a.rs", "fn main() {\r\n}\r\n");
        let buffer = ready(load(&fs, Path::new("/p/a.rs"))).unwrap();
        assert_eq!(buffer.line_ending(), LineEnding::CrLf);
        ready(save(&fs, Path::new("/p/b.rs"), &buffer)).unwrap();
        assert_eq!(
            ready(fs.read_to_string(Path::new("/p/b.rs"))).unwrap(),
            "fn main() {\r\n}\r\n"
        );
    }

    #[test]
    fn rejects_binary_and_non_utf8_files() {
        let fs = FakeFs::new();
        fs.insert_file("/p/image.png", vec![0x89, b'P', b'N', b'G', 0, 0, 0, 13]);
        fs.insert_file("/p/latin1.txt", vec![b'c', b'a', b'f', 0xe9]);
        assert!(matches!(
            ready(load(&fs, Path::new("/p/image.png"))),
            Err(TextError::Binary { .. })
        ));
        assert!(matches!(
            ready(load(&fs, Path::new("/p/latin1.txt"))),
            Err(TextError::NotUtf8 { .. })
        ));
        assert!(matches!(
            ready(load(&fs, Path::new("/p/missing.rs"))),
            Err(TextError::Fs(_))
        ));
    }
}
