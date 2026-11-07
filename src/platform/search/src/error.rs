use thiserror::Error;

/// Why a search could not start.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SearchError {
    /// The pattern is not a valid regular expression.
    #[error("invalid search pattern: {0}")]
    InvalidPattern(String),
    /// A "files to include" glob is invalid.
    #[error("invalid file pattern: {0}")]
    InvalidGlob(String),
}
