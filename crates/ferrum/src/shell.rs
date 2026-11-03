//! Root view of the main window. For now it is empty and only routes
//! keystrokes to commands; the workbench layout replaces its content later.

use std::collections::HashMap;
use std::sync::Arc;

use commands::{CommandId, Keymap, KeymapMatcher, MatchResult};
use gpui::{
    Context, FocusHandle, InteractiveElement as _, IntoElement, KeyDownEvent, Render, Styled as _,
    Window, div,
};
use gpui_component::ActiveTheme as _;

use crate::app_commands::Handler;

/// Keybinding context active everywhere in the main window.
const WORKBENCH_CONTEXT: &str = "workbench";

pub(crate) struct Shell {
    focus_handle: FocusHandle,
    keymap: Arc<Keymap>,
    handlers: Arc<HashMap<CommandId, Handler>>,
    matcher: KeymapMatcher,
}

impl Shell {
    pub(crate) fn new(
        keymap: Arc<Keymap>,
        handlers: Arc<HashMap<CommandId, Handler>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        Self {
            focus_handle,
            keymap,
            handlers,
            matcher: KeymapMatcher::new(),
        }
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.is_held {
            return;
        }
        let keystroke = to_keystroke(&event.keystroke);
        match self
            .matcher
            .push(&self.keymap, keystroke, &[WORKBENCH_CONTEXT])
        {
            MatchResult::Command(id) => {
                cx.stop_propagation();
                if let Some(handler) = self.handlers.get(&id) {
                    tracing::debug!(command = %id, "running command");
                    handler(window, cx);
                } else {
                    tracing::warn!(command = %id, "no handler for command");
                }
            }
            MatchResult::Pending => cx.stop_propagation(),
            MatchResult::NoMatch => {}
        }
    }
}

impl Render for Shell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
            .size_full()
            .bg(cx.theme().background)
    }
}

/// Converts a GPUI keystroke into the keymap representation.
fn to_keystroke(keystroke: &gpui::Keystroke) -> commands::Keystroke {
    let gpui::Modifiers {
        control,
        alt,
        shift,
        platform,
        ..
    } = keystroke.modifiers;
    commands::Keystroke {
        modifiers: commands::Modifiers {
            control,
            alt,
            shift,
            platform,
        },
        key: keystroke.key.to_ascii_lowercase(),
    }
}
