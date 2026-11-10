//! What to search for, and the matcher built from it.

use std::ops::Range;
use std::path::Path;

use grep_matcher::{Captures as _, Matcher as _};

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

    /// Byte ranges of the matches of the query in `text`, in order, at most
    /// `limit` of them (the find widget of an editor). An empty pattern
    /// matches nothing.
    ///
    /// # Errors
    ///
    /// Fails if the pattern is not a valid regular expression.
    pub fn find_all(&self, text: &str, limit: usize) -> Result<Vec<Range<usize>>, SearchError> {
        if self.pattern.is_empty() {
            return Ok(Vec::new());
        }
        let matcher = self.matcher()?;
        let mut found = Vec::new();
        let mut at = 0;
        while found.len() < limit && at <= text.len() {
            let Ok(Some(hit)) = matcher.find_at(text.as_bytes(), at) else {
                break;
            };
            if hit.is_empty() {
                // An empty match (`^`, ``) moves on by one character.
                at = text[hit.end()..]
                    .chars()
                    .next()
                    .map_or(text.len() + 1, |next| hit.end() + next.len_utf8());
                continue;
            }
            found.push(hit.start()..hit.end());
            at = hit.end();
        }
        Ok(found)
    }

    /// Every match of the query in `text` (at most `limit`) with what
    /// replaces it: `replacement` as is, or for a regular expression with
    /// `$1`, `${name}` filled from the match (`$$` is a dollar sign).
    ///
    /// # Errors
    ///
    /// Fails if the pattern is not a valid regular expression.
    pub fn replacements(
        &self,
        text: &str,
        replacement: &str,
        limit: usize,
    ) -> Result<Vec<(Range<usize>, String)>, SearchError> {
        if !self.regex {
            return Ok(self
                .find_all(text, limit)?
                .into_iter()
                .map(|range| (range, replacement.to_owned()))
                .collect());
        }
        if self.pattern.is_empty() {
            return Ok(Vec::new());
        }
        let matcher = self.matcher()?;
        let haystack = text.as_bytes();
        let invalid = |error: grep_matcher::NoError| SearchError::InvalidPattern(error.to_string());
        let mut captures = matcher.new_captures().map_err(invalid)?;
        let mut found = Vec::new();
        let mut at = 0;
        while found.len() < limit && at <= text.len() {
            if !matcher
                .captures_at(haystack, at, &mut captures)
                .map_err(invalid)?
            {
                break;
            }
            let Some(hit) = captures.get(0) else {
                break;
            };
            if hit.is_empty() {
                at = text[hit.end()..]
                    .chars()
                    .next()
                    .map_or(text.len() + 1, |next| hit.end() + next.len_utf8());
                continue;
            }
            let mut filled = Vec::new();
            captures.interpolate(
                |name| matcher.capture_index(name),
                haystack,
                replacement.as_bytes(),
                &mut filled,
            );
            found.push((
                hit.start()..hit.end(),
                String::from_utf8_lossy(&filled).into_owned(),
            ));
            at = hit.end();
        }
        Ok(found)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn query(pattern: &str) -> SearchQuery {
        SearchQuery {
            pattern: pattern.to_owned(),
            ..SearchQuery::default()
        }
    }

    #[test]
    fn finds_every_match_with_options() {
        let text = "Foo foo food
foo";
        assert_eq!(
            query("foo").find_all(text, 100).unwrap(),
            [0..3, 4..7, 8..11, 13..16]
        );
        let word = SearchQuery {
            whole_word: true,
            case_sensitive: true,
            ..query("foo")
        };
        assert_eq!(word.find_all(text, 100).unwrap(), [4..7, 13..16]);
        let regex = SearchQuery {
            regex: true,
            ..query("fo+d")
        };
        assert_eq!(regex.find_all(text, 100).unwrap(), vec![8..12_usize]);
        assert_eq!(query("foo").find_all(text, 2).unwrap().len(), 2);
        assert!(query("").find_all(text, 100).unwrap().is_empty());
        let empty = SearchQuery {
            regex: true,
            ..query("^")
        };
        assert!(empty.find_all(text, 100).unwrap().is_empty());
        let invalid = SearchQuery {
            regex: true,
            ..query("(")
        };
        assert!(invalid.find_all(text, 100).is_err());
    }

    #[test]
    fn replacements_fill_capture_groups() {
        let literal = query("foo");
        assert_eq!(
            literal.replacements("a foo b", "$1x", 10).unwrap(),
            [(2..5, "$1x".to_owned())]
        );
        let regex = SearchQuery {
            regex: true,
            ..query(r"(\w+)=(?P<value>\d+)")
        };
        assert_eq!(
            regex
                .replacements("a=1 bb=22", "${value}:$1$$", 10)
                .unwrap(),
            [(0..3, "1:a$".to_owned()), (4..9, "22:bb$".to_owned())]
        );
    }
}
