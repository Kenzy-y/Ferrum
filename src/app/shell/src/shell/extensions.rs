//! The installed extensions shown in the Extensions view. Until the plugin
//! system (stage 9) this is one sample entry that shows how an extension
//! page looks; it does nothing.

use i18n::I18n;
use ui::IconName;
use workbench::{ExtensionEntry, ExtensionItem, ExtensionOverview};

/// Id of the sample entry.
const SAMPLE_ID: &str = "ferrum.sample";

fn item(i18n: &I18n, title: &str, detail: &str) -> ExtensionItem {
    ExtensionItem {
        title: i18n.text(title).into(),
        detail: i18n.text(detail).into(),
    }
}

/// The sample extension, localized; `enabled` keeps what the user chose.
pub(super) fn sample(i18n: &I18n, enabled: bool) -> ExtensionEntry {
    ExtensionEntry {
        id: SAMPLE_ID.into(),
        name: i18n.text("sample-extension-name").into(),
        publisher: i18n.text("sample-extension-publisher").into(),
        summary: i18n.text("sample-extension-summary").into(),
        description: i18n.text("sample-extension-description").into(),
        icon: IconName::Braces,
        enabled,
        overview: ExtensionOverview {
            title: i18n.text("sample-extension-overview-title").into(),
            text: i18n.text("sample-extension-overview-text").into(),
            features: vec![
                item(
                    i18n,
                    "sample-extension-commands",
                    "sample-extension-commands-detail",
                ),
                item(
                    i18n,
                    "sample-extension-views",
                    "sample-extension-views-detail",
                ),
                item(
                    i18n,
                    "sample-extension-languages",
                    "sample-extension-languages-detail",
                ),
            ],
            sample: vec![
                i18n.text("sample-extension-code-comment").into(),
                format!("id = \"{SAMPLE_ID}\"").into(),
                "api = \"0.1\"".into(),
            ],
            side_title: i18n.text("sample-extension-side-title").into(),
            side: vec![
                item(
                    i18n,
                    "sample-extension-manifest",
                    "sample-extension-manifest-detail",
                ),
                item(
                    i18n,
                    "sample-extension-sandbox",
                    "sample-extension-sandbox-detail",
                ),
                item(
                    i18n,
                    "sample-extension-permissions",
                    "sample-extension-permissions-detail",
                ),
            ],
        },
        contributions: vec![
            item(
                i18n,
                "sample-extension-contribution-commands",
                "sample-extension-contribution-none",
            ),
            item(
                i18n,
                "sample-extension-contribution-settings",
                "sample-extension-contribution-none",
            ),
            item(
                i18n,
                "sample-extension-contribution-views",
                "sample-extension-contribution-none",
            ),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sample_is_localized_in_every_locale() {
        for locale in i18n::AVAILABLE_LOCALES {
            let i18n = I18n::new(locale).unwrap();
            let entry = sample(&i18n, true);
            let texts = [
                &entry.name,
                &entry.summary,
                &entry.description,
                &entry.overview.title,
                &entry.overview.side_title,
            ];
            for text in texts {
                assert!(!text.starts_with("sample-extension"), "{locale}: {text}");
            }
            for item in entry.overview.features.iter().chain(&entry.overview.side) {
                assert!(!item.title.starts_with("sample-extension"), "{locale}");
            }
        }
    }
}
