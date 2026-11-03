use std::path::PathBuf;

/// Errors produced by the settings system.
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    /// A namespace was registered twice.
    #[error("settings namespace `{0}` is registered more than once")]
    DuplicateNamespace(&'static str),
    /// A namespace does not follow the naming rules.
    #[error(
        "settings namespace `{0}` is invalid: use lowercase ASCII letters, digits and `_`, starting with a letter"
    )]
    InvalidNamespace(&'static str),
    /// The defaults of a settings struct could not be represented as TOML.
    #[error("defaults of settings namespace `{namespace}` cannot be serialized: {source}")]
    SerializeDefaults {
        /// The namespace whose defaults failed.
        namespace: &'static str,
        /// The underlying error.
        #[source]
        source: toml::ser::Error,
    },
    /// A settings file is not valid TOML or has the wrong shape.
    #[error("cannot parse settings{}: {message}", path.as_ref().map(|path| format!(" in {}", path.display())).unwrap_or_default())]
    Parse {
        /// The file, if the text came from one.
        path: Option<PathBuf>,
        /// Human-readable description of the problem.
        message: String,
    },
    /// Reading a settings file failed.
    #[error(transparent)]
    Fs(#[from] fs::FsError),
}
