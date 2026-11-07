//! What a window remembers between runs: the sizes the user dragged its
//! panels to. The root for session restore (open files, layout) later.

use std::path::Path;

use fs::Fs;
use serde::{Deserialize, Serialize};

use crate::ProjectError;

/// File name of the window state in the data directory.
pub const WINDOW_STATE_FILE_NAME: &str = "window-state.toml";

/// Sizes of resizable panels, in logical pixels; `None` keeps the
/// theme's default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowState {
    /// Width of the sidebar.
    pub sidebar_width: Option<f32>,
    /// Height of the bottom panel.
    pub panel_height: Option<f32>,
}

impl WindowState {
    /// Reads `file`; a missing or damaged file gives the defaults (damage is
    /// logged).
    pub async fn load(fs: &dyn Fs, file: &Path) -> Self {
        match fs.read_to_string(file).await {
            Ok(text) => toml::from_str(&text).unwrap_or_else(|error| {
                tracing::warn!(path = %file.display(), %error, "window state file is damaged");
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    /// Writes the state to `file`, creating its folder.
    ///
    /// # Errors
    ///
    /// Fails if the file cannot be written.
    pub async fn save(&self, fs: &dyn Fs, file: &Path) -> Result<(), ProjectError> {
        let text =
            toml::to_string(self).map_err(|error| ProjectError::Serialize(error.to_string()))?;
        if let Some(directory) = file.parent() {
            fs.create_dir_all(directory).await?;
        }
        fs.write(file, text.as_bytes()).await?;
        Ok(())
    }
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
    fn round_trips_and_defaults_when_missing_or_damaged() {
        let fs = FakeFs::new();
        let file = Path::new("/data/window-state.toml");
        assert_eq!(ready(WindowState::load(&fs, file)), WindowState::default());
        let state = WindowState {
            sidebar_width: Some(320.0),
            panel_height: None,
        };
        ready(state.save(&fs, file)).unwrap();
        assert_eq!(ready(WindowState::load(&fs, file)), state);
        fs.insert_file(file, "sidebar_width = 'wide'");
        assert_eq!(ready(WindowState::load(&fs, file)), WindowState::default());
    }
}
