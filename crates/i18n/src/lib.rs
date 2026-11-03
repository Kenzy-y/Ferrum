//! Localization of user-visible strings.
//!
//! Strings live in Fluent files under `locales/<locale>/`, embedded into the
//! binary. UI code never contains user-facing literals: it asks an
//! [`I18n`] for a message by its id.
//!
//! The product name is not written in the `.ftl` files: every bundle gets a
//! `-product-name` term built from `app_identity`, so messages reference it
//! as `{ -product-name }`.

mod locale;
mod localization_settings;

use fluent_bundle::FluentResource;
use fluent_bundle::concurrent::FluentBundle;
use unic_langid::LanguageIdentifier;

pub use fluent_bundle::{FluentArgs, FluentValue};
pub use locale::{AVAILABLE_LOCALES, DEFAULT_LOCALE, resolve_locale, system_locale};
pub use localization_settings::LocalizationSettings;

/// Errors while building localization bundles. They indicate a broken
/// built-in `.ftl` file and are caught by tests.
#[derive(Debug, thiserror::Error)]
pub enum I18nError {
    /// The locale is not one of [`AVAILABLE_LOCALES`].
    #[error("locale `{0}` is not available")]
    UnknownLocale(String),
    /// A Fluent file could not be parsed or loaded.
    #[error("invalid Fluent resource for locale `{locale}`: {message}")]
    InvalidResource {
        /// The locale of the broken file.
        locale: &'static str,
        /// Description of the problems.
        message: String,
    },
}

type Bundle = FluentBundle<FluentResource>;

/// Localized strings for one locale, with [`DEFAULT_LOCALE`] as fallback
/// for messages the locale does not have.
pub struct I18n {
    locale: &'static str,
    primary: Bundle,
    fallback: Option<Bundle>,
}

impl std::fmt::Debug for I18n {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("I18n")
            .field("locale", &self.locale)
            .finish_non_exhaustive()
    }
}

impl I18n {
    /// Loads the strings of `locale`, one of [`AVAILABLE_LOCALES`].
    ///
    /// # Errors
    ///
    /// Fails if the locale is unknown or its built-in files are broken.
    pub fn new(locale: &str) -> Result<Self, I18nError> {
        let locale = AVAILABLE_LOCALES
            .iter()
            .copied()
            .find(|available| *available == locale)
            .ok_or_else(|| I18nError::UnknownLocale(locale.to_owned()))?;
        let primary = build_bundle(locale)?;
        let fallback = if locale == DEFAULT_LOCALE {
            None
        } else {
            Some(build_bundle(DEFAULT_LOCALE)?)
        };
        Ok(Self {
            locale,
            primary,
            fallback,
        })
    }

    /// The locale of the strings.
    pub fn locale(&self) -> &'static str {
        self.locale
    }

    /// The message `id` without arguments.
    pub fn text(&self, id: &str) -> String {
        self.format(id, None)
    }

    /// The message `id` with `args` substituted.
    pub fn text_with(&self, id: &str, args: &FluentArgs<'_>) -> String {
        self.format(id, Some(args))
    }

    /// Whether `id` exists in this locale or the fallback.
    pub fn has(&self, id: &str) -> bool {
        self.bundles().any(|bundle| bundle.has_message(id))
    }

    fn bundles(&self) -> impl Iterator<Item = &Bundle> {
        std::iter::once(&self.primary).chain(&self.fallback)
    }

    fn format(&self, id: &str, args: Option<&FluentArgs<'_>>) -> String {
        for bundle in self.bundles() {
            let Some(pattern) = bundle.get_message(id).and_then(|message| message.value()) else {
                continue;
            };
            let mut errors = Vec::new();
            let text = bundle.format_pattern(pattern, args, &mut errors);
            if !errors.is_empty() {
                tracing::warn!(
                    id,
                    locale = self.locale,
                    ?errors,
                    "errors while formatting message"
                );
            }
            return text.into_owned();
        }
        // A missing message is a bug, but the UI must keep working: show
        // the id so the problem is visible and easy to find.
        tracing::warn!(id, locale = self.locale, "missing localized message");
        id.to_owned()
    }
}

/// Fluent sources embedded for `locale`.
fn sources(locale: &str) -> &'static [&'static str] {
    match locale {
        "en" => &[include_str!("../locales/en/main.ftl")],
        "ru" => &[include_str!("../locales/ru/main.ftl")],
        _ => &[],
    }
}

fn build_bundle(locale: &'static str) -> Result<Bundle, I18nError> {
    let invalid = |message: String| I18nError::InvalidResource { locale, message };
    let language: LanguageIdentifier = locale
        .parse()
        .map_err(|error| invalid(format!("bad language identifier: {error}")))?;
    let mut bundle = Bundle::new_concurrent(vec![language]);
    // Unicode isolation marks around placeables help bidirectional text,
    // but the text renderer does not handle them yet and would show them.
    bundle.set_use_isolating(false);

    let identity = format!("-product-name = {}\n", app_identity::PRODUCT_NAME);
    for source in std::iter::once(identity.as_str()).chain(sources(locale).iter().copied()) {
        let resource = FluentResource::try_new(source.to_owned())
            .map_err(|(_, errors)| invalid(format!("{errors:?}")))?;
        bundle
            .add_resource(resource)
            .map_err(|errors| invalid(format!("{errors:?}")))?;
    }
    Ok(bundle)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    /// Message ids defined in the embedded sources of `locale`.
    fn message_ids(locale: &str) -> BTreeSet<String> {
        sources(locale)
            .iter()
            .flat_map(|source| source.lines())
            .filter(|line| line.starts_with(|first: char| first.is_ascii_lowercase()))
            .filter_map(|line| line.split_once('=').map(|(id, _)| id.trim().to_owned()))
            .collect()
    }

    #[test]
    fn every_locale_loads() {
        for locale in AVAILABLE_LOCALES {
            I18n::new(locale).unwrap();
        }
    }

    #[test]
    fn locales_define_the_same_messages() {
        let reference = message_ids(DEFAULT_LOCALE);
        assert!(!reference.is_empty());
        for locale in AVAILABLE_LOCALES {
            assert_eq!(
                message_ids(locale),
                reference,
                "messages of `{locale}` differ from `{DEFAULT_LOCALE}`"
            );
        }
    }

    #[test]
    fn product_name_comes_from_app_identity() {
        for locale in AVAILABLE_LOCALES {
            assert_eq!(
                I18n::new(locale).unwrap().text("app-title"),
                app_identity::PRODUCT_NAME
            );
        }
    }

    #[test]
    fn localized_text_differs_per_locale() {
        let english = I18n::new("en").unwrap().text("command-app-quit");
        let russian = I18n::new("ru").unwrap().text("command-app-quit");
        assert_eq!(english, format!("Quit {}", app_identity::PRODUCT_NAME));
        assert_ne!(english, russian);
    }

    #[test]
    fn missing_message_shows_its_id() {
        let i18n = I18n::new("ru").unwrap();
        assert!(!i18n.has("no-such-message"));
        assert_eq!(i18n.text("no-such-message"), "no-such-message");
    }

    #[test]
    fn unknown_locale_is_an_error() {
        assert!(matches!(I18n::new("xx"), Err(I18nError::UnknownLocale(_))));
    }
}
