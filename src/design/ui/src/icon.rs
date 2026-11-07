//! Icons: Lucide SVGs embedded in the binary (`icons/lucide`, ISC license).

use gpui::{App, Hsla, IntoElement, RenderOnce, Styled as _, Window, px, svg};
use theme::ActiveTheme as _;

macro_rules! icon_names {
    ($( $(#[$meta:meta])* $variant:ident => $file:literal, )*) => {
        /// Icons available to the interface.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum IconName {
            $( $(#[$meta])* #[doc = concat!("Lucide `", $file, "`.")] $variant, )*
        }

        impl IconName {
            /// Every icon, for the gallery and tests.
            pub const ALL: &[Self] = &[$(Self::$variant),*];

            /// Asset path of the SVG.
            pub fn path(self) -> &'static str {
                match self {
                    $( Self::$variant => concat!("icons/", $file, ".svg"), )*
                }
            }

            pub(crate) fn source(self) -> &'static [u8] {
                match self {
                    $( Self::$variant => include_bytes!(concat!("../icons/lucide/", $file, ".svg")), )*
                }
            }
        }
    };
}

icon_names! {
    ArrowDown => "arrow-down",
    ArrowDownToLine => "arrow-down-to-line",
    ArrowRight => "arrow-right",
    ArrowUp => "arrow-up",
    ArrowUpFromLine => "arrow-up-from-line",
    Bell => "bell",
    BookOpen => "book-open",
    Braces => "braces",
    CaseSensitive => "case-sensitive",
    Check => "check",
    ChevronDown => "chevron-down",
    ChevronRight => "chevron-right",
    ChevronUp => "chevron-up",
    Circle => "circle",
    CircleX => "circle-x",
    Columns => "columns-2",
    Copy => "copy",
    Cpu => "cpu",
    Ellipsis => "ellipsis",
    Eye => "eye",
    File => "file",
    FileText => "file-text",
    FileX => "file-x",
    Files => "files",
    Folder => "folder",
    FolderOpen => "folder-open",
    GitBranch => "git-branch",
    Info => "info",
    Minus => "minus",
    Package => "package",
    PanelBottom => "panel-bottom",
    PanelLeft => "panel-left",
    Play => "play",
    Plus => "plus",
    Redo => "redo",
    RefreshCw => "refresh-cw",
    Regex => "regex",
    RotateCw => "rotate-cw",
    Search => "search",
    Settings => "settings",
    Square => "square",
    Terminal => "terminal",
    Trash => "trash",
    TriangleAlert => "triangle-alert",
    WholeWord => "whole-word",
    X => "x",
}

/// An icon element. Defaults to the small icon size and the `icon` color.
#[derive(IntoElement)]
pub struct Icon {
    name: IconName,
    size: Option<f32>,
    color: Option<Hsla>,
}

impl Icon {
    /// Creates an icon.
    pub fn new(name: IconName) -> Self {
        Self {
            name,
            size: None,
            color: None,
        }
    }

    /// Uses the medium icon size (activity bar).
    #[must_use]
    pub fn medium(mut self, cx: &App) -> Self {
        self.size = Some(cx.theme().metrics.size.icon_medium);
        self
    }

    /// Overrides the size in logical pixels.
    #[must_use]
    pub fn size(mut self, size: f32) -> Self {
        self.size = Some(size);
        self
    }

    /// Overrides the color.
    #[must_use]
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }
}

impl RenderOnce for Icon {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let size = px(self.size.unwrap_or(theme.metrics.size.icon_small));
        svg()
            .path(self.name.path())
            .size(size)
            .flex_none()
            .text_color(self.color.unwrap_or_else(|| theme.colors.icon.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_is_an_svg() {
        for icon in IconName::ALL {
            let source = std::str::from_utf8(icon.source()).unwrap();
            assert!(source.contains("<svg"), "{icon:?}");
            assert!(icon.path().starts_with("icons/"));
        }
    }
}
