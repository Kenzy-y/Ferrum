//! Settings: schema-first registry, layered values and loading of settings
//! files.
//!
//! Every crate (and later every plugin) declares its settings as a struct
//! implementing [`Settings`] and registers it in a [`SettingsRegistry`] under
//! a namespace. Values come from layers that override each other in order:
//!
//! 1. built-in defaults (`Default` of each struct);
//! 2. the user `settings.toml` in the configuration directory;
//! 3. the workspace `.ferrum/settings.toml` (name from `app_identity`);
//! 4. per-language overrides (`[lang.<id>.<namespace>]`) from the user layer,
//!    then from the workspace layer.
//!
//! The [`SettingsStore`] merges the layers and hands out typed values. This
//! is the only crate that reads configuration files; see [`load`].

mod error;
mod layer;
pub mod load;
mod merge;
pub mod paths;
mod registry;
mod store;

pub use error::SettingsError;
pub use layer::{LANGUAGE_OVERRIDES_KEY, LayerKind, SettingsLayer};
pub use registry::{RegistryBuilder, SettingsRegistry};
pub use store::{SettingsChange, SettingsDiagnostic, SettingsStore};

use schemars::JsonSchema;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// A group of settings owned by one crate or plugin.
///
/// Implementors are plain structs. Doc comments on fields become the
/// descriptions shown in the settings UI, so write them for users. Mark the
/// struct `#[serde(default)]` so a partially filled table is valid.
pub trait Settings:
    Serialize + DeserializeOwned + JsonSchema + Default + Send + Sync + 'static
{
    /// Namespace under which the settings live in `settings.toml`
    /// (for example `editor` for `[editor]`).
    const NAMESPACE: &'static str;
}
