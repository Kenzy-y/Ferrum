//! Settings of the `[editor]` namespace.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Modifier that adds a cursor when held while clicking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum MultiCursorModifier {
    /// Alt (Option on macOS).
    #[default]
    Alt,
    /// Ctrl (Cmd on macOS). Useful where the window manager takes
    /// Alt+click, as i3 and other Linux window managers do.
    Ctrl,
}

/// Settings of the editor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct EditorSettings {
    /// Columns per indentation level and per tab character.
    pub tab_size: usize,
    /// Tab inserts spaces instead of a tab character.
    pub insert_spaces: bool,
    /// Whether the cursor blinks. It never blinks in an unfocused window.
    pub cursor_blink: bool,
    /// How long the cursor stays shown or hidden while blinking, in
    /// milliseconds.
    pub cursor_blink_interval_ms: u64,
    /// Modifier for adding a cursor with a click: "alt" or "ctrl".
    pub multi_cursor_modifier: MultiCursorModifier,
    /// Multiplier of mouse wheel scrolling.
    pub scroll_sensitivity: f32,
    /// Shows the minimap at the right of the text.
    pub minimap: bool,
}

impl Default for EditorSettings {
    fn default() -> Self {
        Self {
            tab_size: 4,
            insert_spaces: true,
            cursor_blink: true,
            cursor_blink_interval_ms: 530,
            multi_cursor_modifier: MultiCursorModifier::Alt,
            scroll_sensitivity: 1.0,
            minimap: true,
        }
    }
}

impl settings::Settings for EditorSettings {
    const NAMESPACE: &'static str = "editor";
}

impl EditorSettings {
    /// Indentation for Tab and Shift+Tab.
    pub fn indent(&self) -> text::Indent {
        text::Indent {
            tab_size: self.tab_size.max(1),
            spaces: self.insert_spaces,
        }
    }
}
