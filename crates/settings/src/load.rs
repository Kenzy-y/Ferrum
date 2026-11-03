//! Reading configuration files through [`Fs`].
//!
//! Every configuration file the IDE reads (settings, keymaps) goes through
//! this module. The futures do file I/O through [`Fs`], so poll them on a
//! background executor.

use std::path::Path;

use fs::Fs;

use crate::{SettingsError, SettingsLayer};

/// Reads a configuration file as text. A missing file is not an error: it
/// yields `None`.
///
/// # Errors
///
/// Returns [`SettingsError::Fs`] if the file exists but cannot be read.
pub async fn read_config_file(fs: &dyn Fs, path: &Path) -> Result<Option<String>, SettingsError> {
    match fs.read_to_string(path).await {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.is_not_found() => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// Reads and parses a settings file. A missing file yields `None`.
///
/// # Errors
///
/// Fails if the file cannot be read or is not a valid settings file.
pub async fn load_layer(fs: &dyn Fs, path: &Path) -> Result<Option<SettingsLayer>, SettingsError> {
    let _span = tracing::debug_span!("settings.load_layer", path = %path.display()).entered();
    read_config_file(fs, path)
        .await?
        .map(|text| SettingsLayer::parse(&text, Some(path)))
        .transpose()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::future::Future;
    use std::path::PathBuf;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    use fs::{BoxFuture, FsError};

    use super::*;

    /// In-memory file system for tests; `None` content means "unreadable".
    struct MemoryFs(HashMap<PathBuf, Option<String>>);

    impl Fs for MemoryFs {
        fn read_to_string<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Result<String, FsError>> {
            let result = match self.0.get(path) {
                Some(Some(text)) => Ok(text.clone()),
                Some(None) => Err(FsError::from_io(path, std::io::Error::other("denied"))),
                None => Err(FsError::from_io(path, std::io::ErrorKind::NotFound.into())),
            };
            Box::pin(async move { result })
        }
    }

    fn poll_ready<T>(future: impl Future<Output = T>) -> T {
        match pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("in-memory futures complete on first poll"),
        }
    }

    fn memory_fs(files: &[(&str, Option<&str>)]) -> MemoryFs {
        MemoryFs(
            files
                .iter()
                .map(|(path, text)| (PathBuf::from(path), text.map(str::to_owned)))
                .collect(),
        )
    }

    #[test]
    fn missing_file_is_none() {
        let fs = memory_fs(&[]);
        assert!(
            poll_ready(load_layer(&fs, Path::new("settings.toml")))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn existing_file_is_parsed() {
        let fs = memory_fs(&[("settings.toml", Some("[editor]\ntab_size = 2"))]);
        let layer = poll_ready(load_layer(&fs, Path::new("settings.toml")))
            .unwrap()
            .unwrap();
        assert_eq!(
            layer,
            SettingsLayer::parse("[editor]\ntab_size = 2", None).unwrap()
        );
    }

    #[test]
    fn unreadable_and_invalid_files_are_errors() {
        let fs = memory_fs(&[("denied.toml", None), ("broken.toml", Some("[editor"))]);
        assert!(matches!(
            poll_ready(load_layer(&fs, Path::new("denied.toml"))),
            Err(SettingsError::Fs(_))
        ));
        assert!(matches!(
            poll_ready(load_layer(&fs, Path::new("broken.toml"))),
            Err(SettingsError::Parse { .. })
        ));
    }
}
