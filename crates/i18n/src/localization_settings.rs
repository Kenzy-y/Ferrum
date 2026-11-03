use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Settings of the `[localization]` namespace.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct LocalizationSettings {
    /// Language of the user interface, for example `"en"` or `"ru"`.
    /// When not set, the system language is used if it is available,
    /// otherwise English.
    pub language: Option<String>,
}

impl settings::Settings for LocalizationSettings {
    const NAMESPACE: &'static str = "localization";
}
