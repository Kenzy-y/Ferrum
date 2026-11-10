//! Quick open: the overlay behind the title bar search (Ctrl K) and the
//! command palette (Ctrl Shift P).
//!
//! Both open the same list; the palette starts with the `>` prefix. Commands
//! are the only source for now; files, symbols and settings join as their
//! stages arrive.

use std::sync::Arc;

use commands::{CommandId, CommandRegistry, FuzzyMatch, Keymap, Platform, filter};
use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, InteractiveElement as _, IntoElement,
    KeyDownEvent, MouseButton, ParentElement as _, Render, ScrollStrategy, SharedString,
    Styled as _, Subscription, UniformListScrollHandle, Window, div, px, uniform_list,
};
use gpui_component::input::{InputEvent, InputState, Position};
use i18n::I18n;
use theme::ActiveTheme as _;
use ui::{IconName, Kbd, ListRow, TextField};

use crate::Dispatch;

/// Prefix that switches quick open to commands, as in other editors.
pub(crate) const COMMAND_PREFIX: &str = ">";

/// Rows visible without scrolling.
const VISIBLE_ROWS: f32 = 10.0;

/// What quick open was opened for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QuickOpenMode {
    /// Title bar search: everything searchable.
    Everything,
    /// Command palette.
    Commands,
}

/// Emitted when the overlay should close.
pub(crate) struct Dismiss;

/// One command in the list.
struct Entry {
    id: CommandId,
    title: SharedString,
    keys: Option<SharedString>,
}

pub(crate) struct QuickOpen {
    input: Entity<InputState>,
    entries: Vec<Entry>,
    matches: Vec<(usize, FuzzyMatch)>,
    selected: usize,
    scroll: UniformListScrollHandle,
    dispatch: Dispatch,
    no_results: SharedString,
    _input_events: Subscription,
}

impl EventEmitter<Dismiss> for QuickOpen {}

impl QuickOpen {
    pub(crate) fn new(
        mode: QuickOpenMode,
        context: &QuickOpenContext,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let placeholder = context.i18n.text(match mode {
            QuickOpenMode::Everything => "titlebar-search-placeholder",
            QuickOpenMode::Commands => "quick-open-commands-placeholder",
        });
        let initial = match mode {
            QuickOpenMode::Everything => String::new(),
            QuickOpenMode::Commands => format!("{COMMAND_PREFIX} "),
        };
        let initial_len = initial.chars().count();
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(placeholder)
                .default_value(initial)
        });
        // The cursor goes after the prefix so typed text lands behind it.
        let end = u32::try_from(initial_len).unwrap_or(0);
        input.update(cx, |input, cx| {
            input.set_cursor_position(Position::new(0, end), window, cx);
        });
        let input_events = cx.subscribe(&input, |this: &mut Self, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.refilter(cx);
            }
        });

        let mut entries: Vec<Entry> = context
            .registry
            .iter()
            .map(|command| Entry {
                id: command.id.clone(),
                title: context.i18n.text(&command.title_message).into(),
                keys: context
                    .keymap
                    .bindings_for(&command.id)
                    .next()
                    .map(|binding| binding.keys.label(Platform::current()).into()),
            })
            .collect();
        entries.sort_by(|left, right| left.title.cmp(&right.title));

        let mut quick_open = Self {
            input,
            entries,
            matches: Vec::new(),
            selected: 0,
            scroll: UniformListScrollHandle::new(),
            dispatch: context.dispatch.clone(),
            no_results: context.i18n.text("quick-open-no-results").into(),
            _input_events: input_events,
        };
        quick_open.refilter(cx);
        quick_open
    }

    /// The query without the command prefix.
    fn query(&self, cx: &App) -> String {
        let value = self.input.read(cx).value();
        value
            .trim_start()
            .trim_start_matches(COMMAND_PREFIX)
            .trim()
            .to_owned()
    }

    fn refilter(&mut self, cx: &mut Context<Self>) {
        let query = self.query(cx);
        self.matches = filter(
            &query,
            self.entries.iter().map(|entry| entry.title.as_ref()),
        );
        self.selected = 0;
        self.scroll.scroll_to_item(0, ScrollStrategy::Top);
        cx.notify();
    }

    fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        let count = self.matches.len();
        if count == 0 {
            return;
        }
        let current = isize::try_from(self.selected).unwrap_or(0);
        let count_signed = isize::try_from(count).unwrap_or(isize::MAX);
        let next = (current + delta).rem_euclid(count_signed);
        self.selected = usize::try_from(next).unwrap_or(0);
        self.scroll
            .scroll_to_item(self.selected, ScrollStrategy::Center);
        cx.notify();
    }

    fn confirm(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some((entry_index, _)) = self.matches.get(index) else {
            return;
        };
        let id = self.entries[*entry_index].id.clone();
        cx.emit(Dismiss);
        let dispatch = self.dispatch.clone();
        // Run after this update finishes so the command sees the closed
        // overlay and can open a new one.
        window.defer(cx, move |window, cx| dispatch(&id, window, cx));
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let modifiers = event.keystroke.modifiers;
        if modifiers.control || modifiers.alt || modifiers.platform {
            return;
        }
        match event.keystroke.key.as_str() {
            "up" => self.move_selection(-1, cx),
            "down" => self.move_selection(1, cx),
            "enter" => self.confirm(self.selected, window, cx),
            "escape" => cx.emit(Dismiss),
            _ => return,
        }
        cx.stop_propagation();
    }
}

/// What quick open needs from the workbench.
#[derive(Clone)]
pub(crate) struct QuickOpenContext {
    pub(crate) i18n: Arc<I18n>,
    pub(crate) registry: Arc<CommandRegistry>,
    pub(crate) keymap: Arc<Keymap>,
    pub(crate) dispatch: Dispatch,
}

impl Render for QuickOpen {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity().downgrade();
        let rows = self
            .matches
            .iter()
            .map(|(entry_index, found)| {
                let entry = &self.entries[*entry_index];
                let highlights = found
                    .positions
                    .iter()
                    .filter_map(|start| {
                        let length = entry.title[*start..].chars().next()?.len_utf8();
                        Some(*start..*start + length)
                    })
                    .collect::<Vec<_>>();
                (entry.title.clone(), entry.keys.clone(), highlights)
            })
            .collect::<Vec<_>>();
        let selected = self.selected;
        let count = rows.len();

        let theme = cx.theme();
        let metrics = &theme.metrics;
        let row_height = metrics.size.list_row_height;
        let list_height =
            row_height * VISIBLE_ROWS.min(f32::from(u16::try_from(count).unwrap_or(u16::MAX)));
        let list = uniform_list("quick-open-list", count, move |range, _window, _cx| {
            range
                .map(|index| {
                    let (title, keys, highlights) = rows[index].clone();
                    let entity = entity.clone();
                    let mut row = ListRow::new(("quick-open-row", index), title)
                        .flat()
                        .highlights(highlights)
                        .selected(index == selected)
                        .on_activate(move |window, cx| {
                            let _ = entity
                                .update(cx, |quick_open, cx| quick_open.confirm(index, window, cx));
                        });
                    if let Some(keys) = keys {
                        row = row.trailing(Kbd::new(keys).plain());
                    }
                    row
                })
                .collect()
        })
        .track_scroll(&self.scroll)
        .h(px(list_height));

        let empty = (count == 0).then(|| {
            div()
                .px(px(metrics.space.lg))
                .py(px(metrics.space.md))
                .text_size(px(metrics.font_size.md))
                .text_color(theme.colors.text_muted)
                .child(self.no_results.clone())
        });

        let results = div()
            .flex()
            .flex_col()
            .p(px(metrics.space.xs))
            .border_t_1()
            .border_color(theme.colors.border_divider)
            .child(list)
            .children(empty);
        // Same width, corner radius and field height as the title bar search
        // field it opens over, so the field stays in place and the list
        // unfolds below it.
        div()
            .id("quick-open")
            .flex()
            .flex_col()
            .w_full()
            .overflow_hidden()
            .rounded(px(metrics.radius.md))
            .border_1()
            .border_color(theme.colors.border_focus)
            .bg(theme.colors.bg_overlay)
            // Wheel and clicks over the box stay in it (the list scrolls,
            // the editor below does not).
            .occlude()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .capture_key_down(cx.listener(Self::on_key_down))
            .child(
                TextField::new(&self.input)
                    .icon(IconName::Search)
                    .embedded(),
            )
            .child(results)
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;
    use std::sync::Arc;

    use commands::{CommandRegistry, Keymap};
    use gpui::TestAppContext;
    use i18n::I18n;

    use super::{QuickOpen, QuickOpenContext, QuickOpenMode};

    /// Regression: the palette opened with the cursor before `> `, so typed
    /// text went in front of the prefix.
    #[gpui::test]
    fn palette_cursor_starts_after_the_prefix(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            let theme =
                theme::Theme::built_in(theme::DEFAULT_THEME, theme::Density::default()).unwrap();
            theme::set_theme(theme, cx);
        });
        let context = QuickOpenContext {
            i18n: Arc::new(I18n::new("en").unwrap()),
            registry: Arc::new(CommandRegistry::new()),
            keymap: Arc::new(Keymap::default()),
            dispatch: Rc::new(|_, _, _| {}),
        };
        let window = cx
            .add_window(|window, cx| QuickOpen::new(QuickOpenMode::Commands, &context, window, cx));
        let position = window
            .update(cx, |quick_open, _, cx| {
                quick_open.input.read(cx).cursor_position()
            })
            .unwrap();
        assert_eq!(position.character, 2);
    }
}
