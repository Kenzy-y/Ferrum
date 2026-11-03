//! Asset source for embedded SVG icons.

use std::borrow::Cow;

use gpui::{AssetSource, SharedString};

use crate::IconName;

/// Serves the embedded icons to GPUI. Install with
/// `Application::with_assets(ui::Assets)`.
///
/// Paths follow the Lucide names (`icons/<name>.svg`), which are also the
/// names gpui-component uses for its own icons.
#[derive(Debug, Clone, Copy, Default)]
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        Ok(IconName::ALL
            .iter()
            .find(|icon| icon.path() == path)
            .map(|icon| Cow::Borrowed(icon.source())))
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(IconName::ALL
            .iter()
            .map(|icon| icon.path())
            .filter(|asset| asset.starts_with(path))
            .map(SharedString::from)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serves_every_icon() {
        for icon in IconName::ALL {
            assert!(Assets.load(icon.path()).unwrap().is_some(), "{icon:?}");
        }
        assert!(Assets.load("icons/missing.svg").unwrap().is_none());
        assert_eq!(Assets.list("icons/").unwrap().len(), IconName::ALL.len());
    }
}
