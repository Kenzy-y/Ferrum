//! Root view of the main window. It routes keystrokes to commands and, for
//! stage 1, shows the component gallery; the workbench layout replaces it.

use std::collections::HashMap;
use std::sync::Arc;

use commands::{CommandId, Keymap, KeymapMatcher, MatchResult};
use gpui::{
    AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement,
    KeyDownEvent, ParentElement as _, Render, SharedString, Styled as _, Window, div, px,
};
use i18n::I18n;
use theme::ActiveTheme as _;

use crate::app_commands::Handler;
use crate::gallery::Gallery;

/// Keybinding context active everywhere in the main window.
const WORKBENCH_CONTEXT: &str = "workbench";

pub(crate) struct Shell {
    focus_handle: FocusHandle,
    keymap: Arc<Keymap>,
    handlers: Arc<HashMap<CommandId, Handler>>,
    matcher: KeymapMatcher,
    gallery: Entity<Gallery>,
}

impl Shell {
    pub(crate) fn new(
        keymap: Arc<Keymap>,
        handlers: Arc<HashMap<CommandId, Handler>>,
        i18n: Arc<I18n>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        let gallery = cx.new(|cx| Gallery::new(i18n, window, cx));
        Self {
            focus_handle,
            keymap,
            handlers,
            matcher: KeymapMatcher::new(),
            gallery,
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
        let theme = cx.theme();
        div()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
            .size_full()
            .font_family(SharedString::from(theme.fonts.ui.clone()))
            .text_size(px(theme.metrics.font_size.base))
            .text_color(theme.colors.text_primary)
            .bg(theme.colors.bg_app)
            .child(self.gallery.clone())
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
