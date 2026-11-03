//! Product name and every identifier derived from it.
//!
//! The product name is a working title. Nothing else in the code base spells
//! it out: other crates take it from here, and user-visible text gets it via
//! `i18n`. Renaming the product means changing this file only (plus the
//! documentation).

/// Human-readable product name shown to the user.
pub const PRODUCT_NAME: &str = "Ferrum";

/// Reverse-DNS application identifier.
///
/// Used where the platform wants a stable unique id: the Wayland/X11
/// `app_id`, the Windows `AppUserModelID`, the macOS bundle identifier.
pub const APP_ID: &str = "dev.ferrum.Ferrum";

/// Qualifier part of [`APP_ID`].
pub const QUALIFIER: &str = "dev";

/// Organization part of [`APP_ID`].
pub const ORGANIZATION: &str = "Ferrum";

/// Application part of [`APP_ID`]; also the name of the per-user
/// directories on Windows and macOS (`%APPDATA%\Ferrum`,
/// `~/Library/Application Support/Ferrum`).
pub const APPLICATION: &str = "Ferrum";

/// Lowercase name for places where Unix conventions expect one: the
/// executable and the per-user directories on Linux (`~/.config/ferrum`).
pub const LOWERCASE_NAME: &str = "ferrum";

/// Name of the per-workspace folder that holds project-local configuration
/// (for example `.ferrum/settings.toml`).
pub const WORKSPACE_DIR_NAME: &str = ".ferrum";

/// Namespace of the WIT package that defines the plugin API
/// (`<namespace>:extension@<version>`).
pub const WIT_NAMESPACE: &str = "ferrum";

/// URL scheme the application registers for deep links (`<scheme>://...`).
pub const URL_SCHEME: &str = "ferrum";

/// Name of the environment variable prefix used for overrides
/// (for example `FERRUM_LOG`).
pub const ENV_PREFIX: &str = "FERRUM";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_id_is_composed_of_its_parts() {
        assert_eq!(
            APP_ID,
            format!("{QUALIFIER}.{}.{APPLICATION}", ORGANIZATION.to_lowercase())
        );
    }

    #[test]
    fn lowercase_name_matches_application() {
        assert_eq!(LOWERCASE_NAME, APPLICATION.to_lowercase());
    }

    #[test]
    fn workspace_dir_is_hidden() {
        assert!(WORKSPACE_DIR_NAME.starts_with('.'));
    }

    #[test]
    fn identifiers_are_lowercase_ascii() {
        for identifier in [WIT_NAMESPACE, URL_SCHEME] {
            assert!(
                identifier
                    .chars()
                    .all(|character| character.is_ascii_lowercase() || character == '-'),
                "{identifier} must be lowercase ASCII"
            );
        }
    }
}
