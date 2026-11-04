//! Shortcuts by physical key: keybindings are written with the keys of the
//! US layout, and keys of other layouts (Russian, German, …) are translated
//! back to them, so Ctrl+S or Ctrl+Backquote work whatever layout is active.

use std::collections::HashMap;

/// Keys of the US layout that other layouts put different characters on.
/// Letters are included: some platforms report the layout's letter.
pub const US_KEYS: &[&str] = &[
    "`", "-", "=", "[", "]", "\\", ";", "'", ",", ".", "/", "a", "b", "c", "d", "e", "f", "g", "h",
    "i", "j", "k", "l", "m", "n", "o", "p", "q", "r", "s", "t", "u", "v", "w", "x", "y", "z", "0",
    "1", "2", "3", "4", "5", "6", "7", "8", "9",
];

/// Translation from keys of the active layout to US keys.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LayoutKeys {
    to_us: HashMap<String, String>,
}

impl LayoutKeys {
    /// A translation from pairs of (key of the active layout, US key on
    /// the same physical key). Pairs where both are the same are dropped.
    /// When two US keys claim one layout key, the first one wins.
    pub fn new(pairs: impl IntoIterator<Item = (String, String)>) -> Self {
        let mut to_us = HashMap::new();
        for (layout, us) in pairs {
            let layout = layout.to_lowercase();
            if !layout.is_empty() && layout != us {
                to_us.entry(layout).or_insert(us);
            }
        }
        Self { to_us }
    }

    /// The US key for `key`; keys of the US layout and named keys (`enter`,
    /// `f5`) come back unchanged.
    pub fn translate<'a>(&'a self, key: &'a str) -> &'a str {
        self.to_us.get(key).map_or(key, String::as_str)
    }

    /// Whether the active layout differs from US anywhere.
    pub fn is_identity(&self) -> bool {
        self.to_us.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn russian() -> LayoutKeys {
        LayoutKeys::new([
            ("ы".to_owned(), "s".to_owned()),
            ("ё".to_owned(), "`".to_owned()),
            ("я".to_owned(), "z".to_owned()),
            ("s".to_owned(), "s".to_owned()),
        ])
    }

    #[test]
    fn layout_keys_become_us_keys() {
        let keys = russian();
        assert_eq!(keys.translate("ы"), "s");
        assert_eq!(keys.translate("ё"), "`");
        assert_eq!(keys.translate("я"), "z");
    }

    #[test]
    fn us_and_named_keys_are_unchanged() {
        let keys = russian();
        assert_eq!(keys.translate("s"), "s");
        assert_eq!(keys.translate("enter"), "enter");
        assert!(LayoutKeys::new([("a".to_owned(), "a".to_owned())]).is_identity());
    }

    #[test]
    fn upper_case_layout_keys_match() {
        let keys = LayoutKeys::new([("Ё".to_owned(), "`".to_owned())]);
        assert_eq!(keys.translate("ё"), "`");
    }
}
