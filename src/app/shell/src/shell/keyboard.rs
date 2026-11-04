//! Keyboard routing: turns key presses into command ids through the
//! keymap. Contexts come from the focused view; keys of the active layout
//! are translated to the physical US keys bindings use.

use std::sync::Arc;
use std::time::Duration;

use commands::{CommandId, Keymap, KeymapMatcher, LayoutKeys, MatchResult};
use gpui::{App, KeyDownEvent, Task, Window};

/// What a key press means for the keymap.
pub(super) enum KeyOutcome {
    /// The keys complete a binding: run the command.
    Run(CommandId),
    /// The keys start a sequence. `chord_timeout` is set when the pending
    /// keys are also a complete binding of their own, which runs if no
    /// further key arrives in time.
    Pending { chord_timeout: Option<Duration> },
    /// Not a binding; the focused view handles the key.
    NoMatch,
}

/// Keymap state of a window: the bindings, the pending key sequence and
/// the active keyboard layout.
pub(super) struct KeyboardDispatcher {
    keymap: Arc<Keymap>,
    matcher: KeymapMatcher,
    /// Runs the pending sequence's own command if no further key arrives.
    chord_timer: Option<Task<()>>,
    /// Contexts of the keystroke that started the pending sequence.
    chord_contexts: Vec<String>,
    chord_timeout: Duration,
    /// Keys of the active keyboard layout to the US keys of keybindings.
    layout_keys: LayoutKeys,
}

impl KeyboardDispatcher {
    pub(super) fn new(keymap: Arc<Keymap>, chord_timeout: Duration, cx: &App) -> Self {
        Self {
            keymap,
            matcher: KeymapMatcher::new(),
            chord_timer: None,
            chord_contexts: Vec::new(),
            chord_timeout,
            layout_keys: layout_keys(cx),
        }
    }

    /// Replaces the bindings and drops any pending sequence.
    pub(super) fn set_keymap(&mut self, keymap: Arc<Keymap>) {
        self.keymap = keymap;
        self.matcher.reset();
        self.chord_timer = None;
    }

    pub(super) fn set_chord_timeout(&mut self, timeout: Duration) {
        self.chord_timeout = timeout;
    }

    /// Re-reads the active keyboard layout.
    pub(super) fn layout_changed(&mut self, cx: &App) {
        self.layout_keys = layout_keys(cx);
    }

    /// Feeds a key press to the keymap. `None`: a held key that does not
    /// repeat, ignored entirely.
    pub(super) fn key_down(&mut self, event: &KeyDownEvent, window: &Window) -> Option<KeyOutcome> {
        if event.is_held && !repeats(&event.keystroke) {
            return None;
        }
        self.chord_timer = None;
        let keystroke = to_keystroke(&event.keystroke, &self.layout_keys);
        let contexts = focus_contexts(window);
        let context_names: Vec<&str> = contexts.iter().map(String::as_str).collect();
        Some(
            match self.matcher.push(&self.keymap, keystroke, &context_names) {
                MatchResult::Command(id) => KeyOutcome::Run(id),
                MatchResult::Pending => {
                    let complete = self
                        .matcher
                        .pending_command(&self.keymap, &context_names)
                        .is_some();
                    self.chord_contexts = contexts;
                    KeyOutcome::Pending {
                        chord_timeout: complete.then_some(self.chord_timeout),
                    }
                }
                MatchResult::NoMatch => KeyOutcome::NoMatch,
            },
        )
    }

    /// Keeps the timer that ends the pending sequence; dropping it cancels.
    pub(super) fn set_chord_timer(&mut self, timer: Task<()>) {
        self.chord_timer = Some(timer);
    }

    /// The chord timeout elapsed: ends the pending sequence and returns its
    /// own command, if any.
    pub(super) fn chord_timed_out(&mut self) -> Option<CommandId> {
        self.chord_timer = None;
        let contexts: Vec<&str> = self.chord_contexts.iter().map(String::as_str).collect();
        let command = self.matcher.pending_command(&self.keymap, &contexts);
        self.matcher.reset();
        command
    }
}

/// Context active everywhere in the main window, after the focused ones.
const WORKBENCH_CONTEXT: &str = "workbench";

/// Keybinding contexts of the focused view and its parents, innermost
/// first, then `workbench`. Views name their context with `key_context`
/// (`editor`, `file_tree`).
fn focus_contexts(window: &Window) -> Vec<String> {
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
fn layout_keys(cx: &App) -> LayoutKeys {
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
fn repeats(keystroke: &gpui::Keystroke) -> bool {
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
fn to_keystroke(keystroke: &gpui::Keystroke, layout: &LayoutKeys) -> commands::Keystroke {
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
