//! The shell picker under the panel's shell button: each row opens a new
//! terminal with that shell.

use gpui::{
    Context, EventEmitter, FocusHandle, Focusable, InteractiveElement as _, IntoElement,
    KeyDownEvent, MouseButton, ParentElement as _, Render, SharedString, Styled as _, Window, div,
    px,
};
use theme::ActiveTheme as _;
use ui::ListRow;

/// What the picker asks of the panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PickerEvent {
    /// Open a terminal with shell `index`.
    Picked(usize),
    /// Close the picker.
    Dismiss,
}

pub(crate) struct ShellPicker {
    names: Vec<SharedString>,
    /// Shown while shells are still being detected or none were found.
    empty: SharedString,
    selected: usize,
    focus_handle: FocusHandle,
}

impl EventEmitter<PickerEvent> for ShellPicker {}

impl Focusable for ShellPicker {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl ShellPicker {
    /// A focused picker of `names`, with `selected` highlighted.
    pub(crate) fn new(
        names: Vec<SharedString>,
        empty: SharedString,
        selected: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        Self {
            selected: selected.min(names.len().saturating_sub(1)),
            names,
            empty,
            focus_handle,
        }
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let modifiers = event.keystroke.modifiers;
        if modifiers.control || modifiers.alt || modifiers.platform {
            return;
        }
        let count = self.names.len();
        match event.keystroke.key.as_str() {
            "up" if count > 0 => self.selected = (self.selected + count - 1) % count,
            "down" if count > 0 => self.selected = (self.selected + 1) % count,
            "enter" | "space" if count > 0 => cx.emit(PickerEvent::Picked(self.selected)),
            "escape" => cx.emit(PickerEvent::Dismiss),
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }
}

impl Render for ShellPicker {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let metrics = &theme.metrics;
        let entity = cx.entity().downgrade();
        let rows = self.names.iter().enumerate().map(|(index, name)| {
            let entity = entity.clone();
            ListRow::new(("shell-row", index), name.clone())
                .flat()
                .selected(self.selected == index)
                .on_activate(move |_, cx| {
                    let _ = entity.update(cx, |_, cx| cx.emit(PickerEvent::Picked(index)));
                })
                .into_any_element()
        });
        let empty = self.names.is_empty().then(|| {
            div()
                .px(px(metrics.space.md))
                .py(px(metrics.space.sm))
                .text_color(theme.colors.text_muted)
                .child(self.empty.clone())
        });
        div()
            .id("shell-picker")
            .debug_selector(|| "shell-picker".to_owned())
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
            .flex()
            .flex_col()
            .min_w(px(metrics.size.menu_width))
            .p(px(metrics.space.xs))
            .rounded(px(metrics.radius.md))
            .border_1()
            .border_color(theme.colors.border_overlay)
            .bg(theme.colors.bg_overlay)
            .occlude()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .children(rows)
            .children(empty)
    }
}
