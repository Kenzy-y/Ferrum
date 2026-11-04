//! A small dialog to type a folder path: the fallback when the system
//! folder picker is unavailable (Linux without xdg-desktop-portal), and the
//! place to report why a folder could not be opened.

use gpui::{
    AppContext as _, Context, Entity, EventEmitter, InteractiveElement as _, IntoElement,
    KeyDownEvent, MouseButton, ParentElement as _, Render, SharedString, Styled as _, Subscription,
    Window, div, px,
};
use gpui_component::input::{InputEvent, InputState};
use theme::ActiveTheme as _;
use ui::{Button, ButtonVariant, IconName, Label, TextField};

/// What the dialog asks the workbench to do.
pub(crate) enum PathPromptEvent {
    /// Open this path.
    Confirm(String),
    /// Close without opening.
    Dismiss,
}

/// Texts of the dialog, already localized.
pub(crate) struct PathPromptTexts {
    pub(crate) title: SharedString,
    pub(crate) placeholder: SharedString,
    pub(crate) open: SharedString,
    pub(crate) cancel: SharedString,
}

pub(crate) struct PathPrompt {
    input: Entity<InputState>,
    texts: PathPromptTexts,
    error: Option<SharedString>,
    _input_events: Subscription,
}

impl EventEmitter<PathPromptEvent> for PathPrompt {}

impl PathPrompt {
    pub(crate) fn new(
        texts: PathPromptTexts,
        initial: Option<String>,
        error: Option<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let placeholder = texts.placeholder.clone();
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(placeholder)
                .default_value(initial.unwrap_or_default())
        });
        input.update(cx, |input, cx| input.focus(window, cx));
        let input_events = cx.subscribe(&input, |this: &mut Self, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                this.confirm(cx);
            }
        });
        Self {
            input,
            texts,
            error,
            _input_events: input_events,
        }
    }

    fn confirm(&mut self, cx: &mut Context<Self>) {
        let path = self.input.read(cx).value().trim().to_owned();
        if !path.is_empty() {
            cx.emit(PathPromptEvent::Confirm(path));
        }
    }

    fn on_key_down(
        _: &mut Self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.key == "escape" {
            cx.stop_propagation();
            cx.emit(PathPromptEvent::Dismiss);
        }
    }
}

impl Render for PathPrompt {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity().downgrade();
        let confirm_entity = entity.clone();
        let theme = cx.theme();
        let colors = &theme.colors;
        let metrics = &theme.metrics;
        let error = self.error.clone().map(|error| {
            div()
                .text_size(px(metrics.font_size.md))
                .text_color(colors.text_error)
                .child(error)
        });
        let buttons = div()
            .flex()
            .justify_end()
            .gap(px(metrics.space.sm))
            .child(
                Button::new("path-prompt-cancel", self.texts.cancel.clone())
                    .variant(ButtonVariant::Secondary)
                    .on_activate(move |_, cx| {
                        let _ = entity.update(cx, |_, cx| cx.emit(PathPromptEvent::Dismiss));
                    }),
            )
            .child(
                Button::new("path-prompt-open", self.texts.open.clone()).on_activate(
                    move |_, cx| {
                        let _ = confirm_entity.update(cx, PathPrompt::confirm);
                    },
                ),
            );
        div()
            .id("path-prompt")
            .flex()
            .flex_col()
            .w(px(metrics.size.dialog_width))
            .p(px(metrics.space.lg))
            .gap(px(metrics.space.md))
            .rounded(px(metrics.radius.lg))
            .border_1()
            .border_color(colors.border_overlay)
            .bg(colors.bg_overlay)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .capture_key_down(cx.listener(Self::on_key_down))
            .child(
                Label::new(self.texts.title.clone())
                    .size(metrics.font_size.lg)
                    .semibold()
                    .color(colors.text_primary),
            )
            .children(error)
            .child(TextField::new(&self.input).icon(IconName::Folder))
            .child(buttons)
    }
}
