//! An open menu: its rows with shortcuts, driven by the keyboard or the
//! mouse. It tells the workbench to close it ([`MenuEvent::Dismiss`]) or
//! to open a neighbour ([`MenuEvent::Switch`]).

use commands::{CommandId, Platform};
use gpui::{
    Context, EventEmitter, FocusHandle, Focusable, InteractiveElement as _, IntoElement,
    KeyDownEvent, MouseButton, ParentElement as _, Render, SharedString, Styled as _, Window, div,
    px,
};
use theme::ActiveTheme as _;
use ui::{Kbd, ListRow};

use super::{MENUS, MenuItem};
use crate::contrib::quick_open::QuickOpenContext;

/// What an open menu asks of the workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MenuEvent {
    /// Close the menu.
    Dismiss,
    /// Open the menu `offset` places away (Left: -1, Right: 1).
    Switch(isize),
}

/// A row as shown.
enum Row {
    Command {
        id: CommandId,
        title: SharedString,
        keys: Option<SharedString>,
    },
    Separator,
}

/// The open menu.
pub(crate) struct MenuView {
    rows: Vec<Row>,
    /// Index of the selected command row, if any.
    selected: Option<usize>,
    focus_handle: FocusHandle,
    context: QuickOpenContext,
}

impl EventEmitter<MenuEvent> for MenuView {}

impl Focusable for MenuView {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl MenuView {
    /// Menu `index` of [`MENUS`], focused. With `select_first` (opened
    /// from the keyboard) its first row is selected.
    pub(crate) fn new(
        index: usize,
        select_first: bool,
        context: &QuickOpenContext,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut rows: Vec<Row> = Vec::new();
        for item in MENUS[index].items {
            match item {
                MenuItem::Separator => {
                    // No separator first, last or twice.
                    if rows
                        .last()
                        .is_some_and(|row| matches!(row, Row::Command { .. }))
                    {
                        rows.push(Row::Separator);
                    }
                }
                MenuItem::Command(id) => {
                    let Ok(id) = CommandId::new(id) else {
                        continue;
                    };
                    let Some(descriptor) = context.registry.get(&id) else {
                        continue;
                    };
                    let keys = context
                        .keymap
                        .bindings_for(&id)
                        .next()
                        .map(|binding| binding.keys.label(Platform::current()).into());
                    rows.push(Row::Command {
                        title: context.i18n.text(&descriptor.title_message).into(),
                        keys,
                        id,
                    });
                }
            }
        }
        if matches!(rows.last(), Some(Row::Separator)) {
            rows.pop();
        }
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        let mut view = Self {
            rows,
            selected: None,
            focus_handle,
            context: context.clone(),
        };
        if select_first {
            view.step(1);
        }
        view
    }

    /// Moves the selection to the next command row in `direction`.
    fn step(&mut self, direction: isize) {
        self.selected = next_command(&self.rows, self.selected, direction).or(self.selected);
    }

    /// Runs the command of row `index` once the menu has closed and the
    /// focus is back where it was.
    fn run(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Row::Command { id, .. }) = self.rows.get(index) else {
            return;
        };
        let id = id.clone();
        cx.emit(MenuEvent::Dismiss);
        let dispatch = self.context.dispatch.clone();
        window.defer(cx, move |window, cx| dispatch(&id, window, cx));
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let modifiers = event.keystroke.modifiers;
        if modifiers.control || modifiers.alt || modifiers.platform {
            return;
        }
        match event.keystroke.key.as_str() {
            "up" => self.step(-1),
            "down" => self.step(1),
            "left" => cx.emit(MenuEvent::Switch(-1)),
            "right" => cx.emit(MenuEvent::Switch(1)),
            "enter" | "space" => {
                if let Some(index) = self.selected {
                    self.run(index, window, cx);
                }
            }
            "escape" => cx.emit(MenuEvent::Dismiss),
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }
}

impl Render for MenuView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let metrics = &theme.metrics;
        let entity = cx.entity().downgrade();
        let rows = self.rows.iter().enumerate().map(|(index, row)| match row {
            Row::Separator => div()
                .my(px(metrics.space.xs))
                .h(px(1.0))
                .bg(theme.colors.border_divider)
                .into_any_element(),
            Row::Command { title, keys, .. } => {
                let entity = entity.clone();
                let mut row = ListRow::new(("menu-row", index), title.clone())
                    .flat()
                    .selected(self.selected == Some(index))
                    .on_activate(move |window, cx| {
                        let _ = entity.update(cx, |menu, cx| menu.run(index, window, cx));
                    });
                if let Some(keys) = keys {
                    row = row.trailing(Kbd::new(keys.clone()).plain());
                }
                row.into_any_element()
            }
        });
        div()
            .id("menu")
            .debug_selector(|| "menu".to_owned())
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
            // Clicks and the wheel stay in the menu.
            .occlude()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .children(rows)
    }
}

/// The command row after (`direction` > 0) or before `selected`,
/// wrapping around and skipping separators.
fn next_command(rows: &[Row], selected: Option<usize>, direction: isize) -> Option<usize> {
    let count = rows.len();
    if count == 0 {
        return None;
    }
    let mut index = match (selected, direction > 0) {
        (Some(index), _) => index,
        (None, true) => count - 1,
        (None, false) => 0,
    };
    for _ in 0..count {
        index = if direction > 0 {
            (index + 1) % count
        } else {
            (index + count - 1) % count
        };
        if matches!(rows[index], Row::Command { .. }) {
            return Some(index);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str) -> Row {
        Row::Command {
            id: CommandId::new(id).unwrap(),
            title: id.to_owned().into(),
            keys: None,
        }
    }

    #[test]
    fn steps_skip_separators_and_wrap() {
        let rows = [row("a.one"), Row::Separator, row("a.two")];
        assert_eq!(next_command(&rows, None, 1), Some(0));
        assert_eq!(
            next_command(&rows, Some(0), 1),
            Some(2),
            "skips the separator"
        );
        assert_eq!(next_command(&rows, Some(2), 1), Some(0), "wraps");
        assert_eq!(next_command(&rows, Some(0), -1), Some(2));
        assert_eq!(next_command(&[Row::Separator], None, 1), None);
    }
}
