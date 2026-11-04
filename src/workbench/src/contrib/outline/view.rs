//! The Outline section of the Explorer: the symbols of the active editor
//! (`syntax::outline`), virtualized; clicking or Enter goes to one.

use std::sync::Arc;

use gpui::{
    Context, EventEmitter, FocusHandle, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, Render, ScrollStrategy, SharedString, Styled as _, UniformListScrollHandle,
    Window, div, px, uniform_list,
};
use syntax::OutlineItem;
use theme::ActiveTheme as _;
use ui::{Label, ListRow};

use crate::contrib::outline::symbol_icons;

/// Emitted when an item is activated: go to its name in the editor.
pub struct GoToSymbol {
    /// Byte offset of the item's name.
    pub offset: usize,
    /// Move the focus to the editor too (Enter), not only scroll it
    /// (click).
    pub focus_editor: bool,
}

/// The outline list.
pub struct OutlineView {
    items: Arc<[OutlineItem]>,
    selected: Option<usize>,
    /// Shown when there are no items.
    empty_text: SharedString,
    focus_handle: FocusHandle,
    scroll: UniformListScrollHandle,
}

impl EventEmitter<GoToSymbol> for OutlineView {}

impl OutlineView {
    /// An empty outline that says `empty_text`.
    pub fn new(empty_text: SharedString, cx: &mut Context<Self>) -> Self {
        Self {
            items: Arc::from([]),
            selected: None,
            empty_text,
            focus_handle: cx.focus_handle(),
            scroll: UniformListScrollHandle::new(),
        }
    }

    /// Shows `items`; the selection is kept if the items are the same
    /// length (the file was edited), dropped otherwise.
    pub fn set_items(&mut self, items: Arc<[OutlineItem]>, cx: &mut Context<Self>) {
        if Arc::ptr_eq(&items, &self.items) {
            return;
        }
        if items.len() != self.items.len() {
            self.selected = None;
        }
        self.items = items;
        cx.notify();
    }

    /// Replaces the text shown without items (language change).
    pub fn set_empty_text(&mut self, text: SharedString, cx: &mut Context<Self>) {
        self.empty_text = text;
        cx.notify();
    }

    /// Moves keyboard focus to the list, selecting the first item.
    pub fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_handle.focus(window, cx);
        if self.selected.is_none() && !self.items.is_empty() {
            self.select(0, cx);
        }
    }

    fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.items.len() {
            self.selected = Some(index);
            self.scroll.scroll_to_item(index, ScrollStrategy::Nearest);
            cx.notify();
        }
    }

    fn activate(&mut self, index: usize, focus_editor: bool, cx: &mut Context<Self>) {
        let Some(item) = self.items.get(index) else {
            return;
        };
        let offset = item.name_start;
        self.select(index, cx);
        cx.emit(GoToSymbol {
            offset,
            focus_editor,
        });
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let modifiers = event.keystroke.modifiers;
        if modifiers.control || modifiers.alt || modifiers.platform || modifiers.shift {
            return;
        }
        let count = self.items.len();
        if count == 0 {
            return;
        }
        let current = self.selected;
        match event.keystroke.key.as_str() {
            "down" => self.select(current.map_or(0, |index| (index + 1).min(count - 1)), cx),
            "up" => self.select(current.map_or(0, |index| index.saturating_sub(1)), cx),
            "home" => self.select(0, cx),
            "end" => self.select(count - 1, cx),
            "enter" => self.activate(current.unwrap_or(0), true, cx),
            "space" => self.activate(current.unwrap_or(0), false, cx),
            _ => return,
        }
        cx.stop_propagation();
    }
}

impl Render for OutlineView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let metrics = &theme.metrics;
        let ring = theme.colors.border_focus;
        let container = div()
            .id("outline-container")
            .key_context("outline")
            .track_focus(&self.focus_handle.clone().tab_stop(true))
            .on_key_down(cx.listener(Self::on_key_down))
            .flex()
            .flex_none()
            .px(px(metrics.space.sm))
            .border_1()
            .border_color(gpui::transparent_black())
            .rounded(px(metrics.radius.md))
            .focus_visible(move |style| style.border_color(ring));
        if self.items.is_empty() {
            return container.child(
                div().px(px(metrics.size.section_header_inset)).child(
                    Label::new(self.empty_text.clone())
                        .size(metrics.font_size.md)
                        .color(theme.colors.text_muted),
                ),
            );
        }
        let row_height = metrics.size.outline_row_height;
        let height =
            (row_height * count_as_f32(self.items.len())).min(metrics.size.outline_max_height);
        let glyph_size = metrics.font_size.md;
        let items = self.items.clone();
        let selected = self.selected;
        let entity = cx.entity().downgrade();
        let list = uniform_list("outline", items.len(), move |range, _window, cx| {
            range
                .map(|index| {
                    let item = &items[index];
                    let entity = entity.clone();
                    ListRow::new(("outline-row", index), item.name.clone())
                        .flat()
                        .code()
                        .depth(item.depth)
                        .leading(symbol_icons::render(item.kind, glyph_size, cx))
                        .selected(selected == Some(index))
                        .tab_stop(false)
                        .on_activate(move |_window, cx| {
                            let _ = entity.update(cx, |view, cx| view.activate(index, false, cx));
                        })
                })
                .collect()
        })
        .track_scroll(&self.scroll)
        .w_full()
        .h(px(height));
        container.child(list)
    }
}

/// A count as a multiplier of a size.
fn count_as_f32(count: usize) -> f32 {
    u16::try_from(count).map_or(f32::from(u16::MAX), f32::from)
}
