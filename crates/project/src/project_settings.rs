use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Default of [`ProjectSettings::recent_projects_limit`].
const DEFAULT_RECENT_PROJECTS_LIMIT: usize = 10;

/// Settings of the `[project]` namespace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ProjectSettings {
    /// How many recently opened folders to remember.
    pub recent_projects_limit: usize,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            recent_projects_limit: DEFAULT_RECENT_PROJECTS_LIMIT,
        }
    }
}

impl settings::Settings for ProjectSettings {
    const NAMESPACE: &'static str = "project";
}

/// Settings of the `[files]` namespace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct FilesSettings {
    /// Paths never shown in the file tree nor watched, in `.gitignore`
    /// syntax: `.git` hides every `.git`, `/dist` only the one at the root.
    pub hidden: Vec<String>,
}

impl Default for FilesSettings {
    fn default() -> Self {
        Self {
            hidden: crate::ignore_rules::DEFAULT_HIDDEN
                .iter()
                .map(|pattern| (*pattern).to_owned())
                .collect(),
        }
    }
}

impl settings::Settings for FilesSettings {
    const NAMESPACE: &'static str = "files";
}
