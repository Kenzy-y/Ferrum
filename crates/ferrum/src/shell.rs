//! Root view of the main window: routes keystrokes and button clicks to
//! command owners and hosts the workbench.

use std::rc::Rc;
use std::sync::Arc;

use commands::{CommandId, Keymap, KeymapMatcher, MatchResult};
use gpui::{
    AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement,
    KeyDownEvent, ParentElement as _, Render, SharedString, Styled as _, Window, div, px,
};
use i18n::I18n;
use theme::ActiveTheme as _;
use workbench::{Dispatch, EditorContent, Workbench};

use crate::app_commands;
use crate::gallery::Gallery;

/// Keybinding context active everywhere in the main window.
const WORKBENCH_CONTEXT: &str = "workbench";

pub(crate) struct Shell {
    focus_handle: FocusHandle,
    keymap: Arc<Keymap>,
    matcher: KeymapMatcher,
    i18n: Arc<I18n>,
    workbench: Entity<Workbench>,
    gallery: Option<Entity<Gallery>>,
}

impl Shell {
    pub(crate) fn new(
        keymap: Arc<Keymap>,
        i18n: Arc<I18n>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        let shell = cx.entity().downgrade();
        let dispatch: Dispatch = Rc::new(move |id, window, cx| {
            let id = id.clone();
            let _ = shell.update(cx, |shell, cx| shell.run_command(&id, window, cx));
        });
        let workbench_i18n = i18n.clone();
        let workbench = cx.new(|_| Workbench::new(workbench_i18n, dispatch));
        Self {
            focus_handle,
            keymap,
            matcher: KeymapMatcher::new(),
            i18n,
            workbench,
            gallery: None,
        }
    }

    /// Runs a command by id, whoever owns it.
    fn run_command(&mut self, id: &CommandId, window: &mut Window, cx: &mut Context<Self>) {
        tracing::debug!(command = %id, "running command");
        match id.as_str() {
            app_commands::QUIT => cx.quit(),
            app_commands::TOGGLE_COMPONENT_GALLERY => self.toggle_gallery(window, cx),
            _ => {
                let handled = self
                    .workbench
                    .update(cx, |workbench, cx| workbench.run_command(id, cx));
                if !handled {
                    tracing::debug!(command = %id, "command is not implemented yet");
                }
            }
        }
    }

    fn toggle_gallery(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let showing = self.workbench.read(cx).has_editor_content();
        let content = if showing {
            None
        } else {
            let i18n = self.i18n.clone();
            let gallery = self
                .gallery
                .get_or_insert_with(|| cx.new(|cx| Gallery::new(i18n, window, cx)))
                .clone();
            Some(EditorContent {
                title: self.i18n.text("gallery-title").into(),
                view: gallery.into(),
            })
        };
        self.workbench.update(cx, |workbench, cx| {
            workbench.set_editor_content(content, cx);
        });
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
                self.run_command(&id, window, cx);
            }
            MatchResult::Pending => cx.stop_propagation(),
            MatchResult::NoMatch => {}
        }
    }
}

impl Render for Shell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let content = div()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
            .size_full()
            .font_family(SharedString::from(theme.fonts.ui.clone()))
            .text_size(px(theme.metrics.font_size.base))
            .text_color(theme.colors.text_primary)
            .bg(theme.colors.bg_app)
            .child(self.workbench.clone());
        // Draws the resize frame and shadow when the window decorates itself
        // (Linux); a plain container elsewhere.
        gpui_component::window_border().child(content)
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
