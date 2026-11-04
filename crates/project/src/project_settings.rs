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
