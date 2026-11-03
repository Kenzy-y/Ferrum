//! Choosing the UI locale.

/// Locale used when nothing better is available. Every message exists in it.
pub const DEFAULT_LOCALE: &str = "en";

/// Locales with built-in strings.
pub const AVAILABLE_LOCALES: &[&str] = &["en", "ru"];

/// The operating system's preferred locale (a BCP 47 tag such as `ru-RU`),
/// if the platform reports one.
pub fn system_locale() -> Option<String> {
    sys_locale::get_locale()
}

/// Picks the UI locale: the explicitly configured one if it is available,
/// otherwise the system locale if available, otherwise
/// [`DEFAULT_LOCALE`].
///
/// Tags are matched by their language subtag, so `ru-RU` and `ru_RU.UTF-8`
/// select `ru`.
pub fn resolve_locale(configured: Option<&str>, system: Option<&str>) -> &'static str {
    configured
        .and_then(available_locale)
        .or_else(|| system.and_then(available_locale))
        .unwrap_or(DEFAULT_LOCALE)
}

fn available_locale(tag: &str) -> Option<&'static str> {
    let language = tag
        .split(['-', '_', '.', '@'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    AVAILABLE_LOCALES
        .iter()
        .copied()
        .find(|locale| *locale == language)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_locale_wins() {
        assert_eq!(resolve_locale(Some("en"), Some("ru-RU")), "en");
    }

    #[test]
    fn system_locale_is_used_when_available() {
        assert_eq!(resolve_locale(None, Some("ru-RU")), "ru");
        assert_eq!(resolve_locale(None, Some("ru_RU.UTF-8")), "ru");
        assert_eq!(resolve_locale(Some("xx"), Some("RU")), "ru");
    }

    #[test]
    fn falls_back_to_default() {
        assert_eq!(resolve_locale(None, Some("de-DE")), DEFAULT_LOCALE);
        assert_eq!(resolve_locale(None, None), DEFAULT_LOCALE);
        assert_eq!(resolve_locale(Some(""), Some("")), DEFAULT_LOCALE);
    }
}
