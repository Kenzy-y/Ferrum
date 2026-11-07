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
// Each flag is an independent user setting, not a hidden state machine.
#[allow(clippy::struct_excessive_bools)]
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
    /// How long the pointer rests on a symbol before its hover shows, in
    /// milliseconds; also how long a hover stays after the pointer left.
    pub hover_delay_ms: u64,
    /// Opens the completion list when a word starts, not only after a
    /// trigger character or Ctrl Space.
    pub suggest_on_type: bool,
    /// Typing an opening bracket or quote also inserts its closing one;
    /// the language decides which characters pair.
    pub auto_close_pairs: bool,
    /// Shows the language service's inlay hints inside lines: types of
    /// bindings, names of parameters.
    pub inlay_hints: bool,
    /// Shows the language service's code lenses at the end of lines
    /// (implementations, references); a click goes there.
    pub code_lens: bool,
    /// How long after an edit or a scroll inlay hints and code lenses are
    /// asked for again, in milliseconds.
    pub language_decorations_delay_ms: u64,
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
            hover_delay_ms: 300,
            suggest_on_type: true,
            auto_close_pairs: true,
            inlay_hints: true,
            code_lens: true,
            language_decorations_delay_ms: 150,
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
