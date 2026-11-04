//! Keystrokes for the keymap: contexts from the focused view and keys of
//! the active layout translated to the physical US keys bindings use.

use commands::LayoutKeys;
use gpui::{App, Window};

/// Context active everywhere in the main window, after the focused ones.
const WORKBENCH_CONTEXT: &str = "workbench";

/// Keybinding contexts of the focused view and its parents, innermost
/// first, then `workbench`. Views name their context with `key_context`
/// (`editor`, `file_tree`).
pub(super) fn focus_contexts(window: &Window) -> Vec<String> {
    let mut contexts: Vec<String> = window
        .context_stack()
        .iter()
        .rev()
        .filter_map(|context| context.primary().map(|entry| entry.key.to_string()))
        .collect();
    contexts.push(WORKBENCH_CONTEXT.to_owned());
    contexts
}

/// Which keys of the active layout sit where US keys are: asks the
/// platform for the key it reports on the physical key of every US key.
pub(super) fn layout_keys(cx: &App) -> LayoutKeys {
    let platform = cx.keyboard_mapper();
    LayoutKeys::new(commands::US_KEYS.iter().map(|us| {
        let keystroke = gpui::Keystroke {
            modifiers: gpui::Modifiers::default(),
            key: (*us).to_owned(),
            key_char: None,
        };
        let equivalent = platform.map_key_equivalent(keystroke, true);
        (equivalent.inner().key.clone(), (*us).to_owned())
    }))
}

/// Held keys repeat only for editing and movement (Backspace, arrows,
/// characters); a held shortcut runs once.
pub(super) fn repeats(keystroke: &gpui::Keystroke) -> bool {
    let modifiers = keystroke.modifiers;
    let shortcut = modifiers.control || modifiers.platform || modifiers.alt;
    !shortcut
        || matches!(
            keystroke.key.as_str(),
            "left" | "right" | "up" | "down" | "backspace" | "delete" | "pageup" | "pagedown"
        )
}

/// Converts a GPUI keystroke into the keymap representation; the key is
/// translated to the US key on the same physical key.
pub(super) fn to_keystroke(
    keystroke: &gpui::Keystroke,
    layout: &LayoutKeys,
) -> commands::Keystroke {
    let gpui::Modifiers {
        control,
        alt,
        shift,
        platform,
        ..
    } = keystroke.modifiers;
    let key = keystroke.key.to_lowercase();
    commands::Keystroke {
        modifiers: commands::Modifiers {
            control,
            alt,
            shift,
            platform,
        },
        key: layout.translate(&key).to_owned(),
    }
}
