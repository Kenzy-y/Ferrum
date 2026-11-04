//! Icons of files in the tree, by file name or extension.
//!
//! A contribution point: language packs and icon themes register icons at
//! startup (the Rust pack adds its "R" for `.rs` in stage 4); everything
//! else gets the generic file icon. The registry is read-only afterwards.

use std::collections::HashMap;

use gpui::{AnyElement, App, IntoElement, SharedString};
use theme::ActiveTheme as _;
use ui::{Icon, IconName, Label};

/// How a file type is drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileIcon {
    /// An icon from the icon set.
    Icon(IconName),
    /// A short monospace label (such as `R`) in a theme color, given by its
    /// token key (for example `icon.file.rust`).
    Letter {
        /// The label.
        text: SharedString,
        /// Key of the color token.
        color_token: &'static str,
    },
}

/// File icons registered by contributors.
#[derive(Debug, Default, Clone)]
pub struct FileIcons {
    by_name: HashMap<String, FileIcon>,
    by_extension: HashMap<String, FileIcon>,
}

impl FileIcons {
    /// An empty registry: every file gets the generic icon.
    pub fn new() -> Self {
        Self::default()
    }

    /// Uses `icon` for files with `extension` (without the dot, any case).
    pub fn register_extension(&mut self, extension: &str, icon: FileIcon) {
        self.by_extension.insert(extension.to_lowercase(), icon);
    }

    /// Uses `icon` for files called exactly `name` (such as `Cargo.toml`);
    /// wins over the extension.
    pub fn register_name(&mut self, name: &str, icon: FileIcon) {
        self.by_name.insert(name.to_owned(), icon);
    }

    /// The icon of a file called `name`.
    pub fn for_file(&self, name: &str) -> FileIcon {
        if let Some(icon) = self.by_name.get(name) {
            return icon.clone();
        }
        name.rsplit_once('.')
            .and_then(|(_, extension)| self.by_extension.get(&extension.to_lowercase()))
            .cloned()
            .unwrap_or(FileIcon::Icon(IconName::File))
    }
}

/// Draws `icon` at the small icon size.
pub(crate) fn render(icon: &FileIcon, cx: &App) -> AnyElement {
    match icon {
        FileIcon::Icon(name) => Icon::new(*name).into_any_element(),
        FileIcon::Letter { text, color_token } => {
            let theme = cx.theme();
            let color = theme
                .colors
                .entries()
                .into_iter()
                .find(|(key, _)| key == color_token)
                .map_or(theme.colors.icon, |(_, color)| color);
            Label::new(text.clone())
                .mono()
                .medium()
                .size(theme.metrics.font_size.base)
                .color(color)
                .into_any_element()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_win_over_extensions_and_unknown_files_are_generic() {
        let mut icons = FileIcons::new();
        let letter = FileIcon::Letter {
            text: "R".into(),
            color_token: "icon.file.rust",
        };
        icons.register_extension("RS", letter.clone());
        icons.register_name("Cargo.toml", FileIcon::Icon(IconName::Package));

        assert_eq!(icons.for_file("main.rs"), letter);
        assert_eq!(
            icons.for_file("Cargo.toml"),
            FileIcon::Icon(IconName::Package)
        );
        assert_eq!(icons.for_file("notes.toml"), FileIcon::Icon(IconName::File));
        assert_eq!(icons.for_file("Makefile"), FileIcon::Icon(IconName::File));
    }
}
