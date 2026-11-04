//! Text with theme typography.

use gpui::{
    App, FontWeight, Hsla, IntoElement, ParentElement as _, RenderOnce, SharedString, Styled as _,
    Window, div, px,
};
use theme::ActiveTheme as _;

/// Which font family a text uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontRole {
    /// Interface font (Inter).
    #[default]
    Ui,
    /// Monospace font (`JetBrains Mono`): code, paths, shortcuts.
    Mono,
}

/// A run of text. Size, weight and color are inherited unless set.
#[derive(IntoElement)]
pub struct Label {
    text: SharedString,
    role: FontRole,
    size: Option<f32>,
    weight: Option<FontWeight>,
    color: Option<Hsla>,
}

impl Label {
    /// Creates a label.
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            role: FontRole::Ui,
            size: None,
            weight: None,
            color: None,
        }
    }

    /// Uses the monospace font.
    #[must_use]
    pub fn mono(mut self) -> Self {
        self.role = FontRole::Mono;
        self
    }

    /// Font size in logical pixels (take it from `theme.metrics.font_size`).
    #[must_use]
    pub fn size(mut self, size: f32) -> Self {
        self.size = Some(size);
        self
    }

    /// Medium weight (500).
    #[must_use]
    pub fn medium(mut self) -> Self {
        self.weight = Some(FontWeight::MEDIUM);
        self
    }

    /// Semibold weight (600).
    #[must_use]
    pub fn semibold(mut self) -> Self {
        self.weight = Some(FontWeight::SEMIBOLD);
        self
    }

    /// Text color.
    #[must_use]
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }
}

impl RenderOnce for Label {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let mut element = div().flex_none().child(self.text);
        if self.role == FontRole::Mono {
            element = element.font_family(SharedString::from(theme.fonts.mono.clone()));
        }
        if let Some(size) = self.size {
            element = element.text_size(px(size));
        }
        if let Some(weight) = self.weight {
            element = element.font_weight(weight);
        }
        if let Some(color) = self.color {
            element = element.text_color(color);
        }
        element
    }
}
