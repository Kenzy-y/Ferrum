//! What to search for, and the matcher built from it.

use std::path::Path;

use grep_regex::{RegexMatcher, RegexMatcherBuilder};
use ignore::overrides::{Override, OverrideBuilder};

use crate::SearchError;

/// What to search for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchQuery {
    /// Text or regular expression.
    pub pattern: String,
    /// Upper and lower case differ (the "Aa" toggle).
    pub case_sensitive: bool,
    /// Only whole words match (the "ab" toggle).
    pub whole_word: bool,
    /// `pattern` is a regular expression (the ".*" toggle).
    pub regex: bool,
    /// Globs of the files to search, relative to the root
    /// (`src/**/*.rs`); `!glob` excludes. Empty: every file.
    pub include: Vec<String>,
}

impl SearchQuery {
    /// Whether there is nothing to search for.
    pub fn is_empty(&self) -> bool {
        self.pattern.is_empty()
    }

    /// The line matcher of the query.
    pub(crate) fn matcher(&self) -> Result<RegexMatcher, SearchError> {
        RegexMatcherBuilder::new()
            .case_insensitive(!self.case_sensitive)
            .word(self.whole_word)
            .fixed_strings(!self.regex)
            // Matches never span lines.
            .line_terminator(Some(b'\n'))
            .build(&self.pattern)
            .map_err(|error| SearchError::InvalidPattern(error.to_string()))
    }

    /// The include globs, rooted at `root`.
    pub(crate) fn include_filter(&self, root: &Path) -> Result<Override, SearchError> {
        let mut builder = OverrideBuilder::new(root);
        for glob in self.include.iter().map(|glob| glob.trim()) {
            if glob.is_empty() {
                continue;
            }
            builder
                .add(glob)
                .map_err(|error| SearchError::InvalidGlob(error.to_string()))?;
        }
        builder
            .build()
            .map_err(|error| SearchError::InvalidGlob(error.to_string()))
    }
}
