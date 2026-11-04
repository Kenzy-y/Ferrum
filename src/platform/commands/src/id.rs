//! Stable command identifiers.

use std::fmt;
use std::sync::Arc;

/// Identifier of a command: dot-separated segments, at least two
/// (`area.action`, e.g. `editor.save`, `workbench.toggleTerminal`).
///
/// The first segment (the area) is lowercase; later segments are
/// lower camel case. Only ASCII letters and digits are allowed.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CommandId(Arc<str>);

/// A string that is not a valid [`CommandId`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("`{0}` is not a valid command id: expected `area.action` with ASCII letters and digits")]
pub struct InvalidCommandId(pub String);

impl CommandId {
    /// Validates and wraps `id`.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidCommandId`] if `id` does not follow the format.
    pub fn new(id: &str) -> Result<Self, InvalidCommandId> {
        if is_valid(id) {
            Ok(Self(id.into()))
        } else {
            Err(InvalidCommandId(id.to_owned()))
        }
    }

    /// The id as a string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The area (first segment), e.g. `editor` for `editor.save`.
    pub fn area(&self) -> &str {
        self.0.split('.').next().unwrap_or_default()
    }
}

impl fmt::Display for CommandId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::str::FromStr for CommandId {
    type Err = InvalidCommandId;

    fn from_str(id: &str) -> Result<Self, Self::Err> {
        Self::new(id)
    }
}

fn is_valid(id: &str) -> bool {
    let mut segments = id.split('.');
    let Some(area) = segments.next() else {
        return false;
    };
    let area_valid = area.starts_with(|first: char| first.is_ascii_lowercase())
        && area
            .chars()
            .all(|character| character.is_ascii_lowercase() || character.is_ascii_digit());
    let mut rest = segments.peekable();
    let has_action = rest.peek().is_some();
    area_valid
        && has_action
        && rest.all(|segment| {
            segment.starts_with(|first: char| first.is_ascii_lowercase())
                && segment
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_area_action_ids() {
        for id in [
            "editor.save",
            "workbench.toggleTerminal",
            "git.stash.pop",
            "lang2.run1",
        ] {
            assert_eq!(CommandId::new(id).unwrap().as_str(), id);
        }
        assert_eq!(CommandId::new("editor.save").unwrap().area(), "editor");
    }

    #[test]
    fn rejects_malformed_ids() {
        for id in [
            "",
            "save",
            "editor.",
            ".save",
            "Editor.save",
            "editor.Save",
            "editor.save-all",
            "editor..save",
            "éditor.save",
        ] {
            assert!(CommandId::new(id).is_err(), "{id} must be rejected");
        }
    }
}
