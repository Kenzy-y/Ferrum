//! Mouse input: clicks place cursors, drags select, the wheel scrolls.

use std::ops::Range;

use gpui::{Context, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ScrollEvent, Window, px};
use text::{Selection, Selections};

use crate::editor::Editor;
use crate::settings::MultiCursorModifier;
use crate::viewport::max_scroll_y;

/// What a drag extends by: set by the number of clicks that started it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Granularity {
    Character,
    Word,
    Line,
}

/// A drag in progress.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Drag {
    granularity: Granularity,
    /// What the click itself selected (the word or line clicked).
    origin: Range<usize>,
}

impl Editor {
    pub(crate) fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_handle.focus(window, cx);
        let Some(layout) = self.layout.clone() else {
            return;
        };
        let buffer = self.document.buffer();
        let offset = layout.offset_at(buffer, event.position);
        let add = match self.settings.multi_cursor_modifier {
            MultiCursorModifier::Alt => event.modifiers.alt,
            MultiCursorModifier::Ctrl => event.modifiers.secondary(cx),
        };
        let granularity = match event.click_count {
            0 | 1 => Granularity::Character,
            2 => Granularity::Word,
            _ => Granularity::Line,
        };
        let range = self.unit_at(granularity, offset);
        let extend = event.modifiers.shift && !add;
        self.change(cx, |document| {
            if extend {
                let anchor = document.selections().primary().anchor;
                document.set_primary_selection(Selection::new(anchor, offset));
            } else if add {
                document.add_selection(Selection::new(range.start, range.end));
            } else {
                document
                    .set_selections(&Selections::single(Selection::new(range.start, range.end)));
            }
        });
        self.drag = Some(Drag {
            granularity,
            origin: if extend {
                let anchor = self.document.selections().primary().anchor;
                anchor..anchor
            } else {
                range
            },
        });
        cx.stop_propagation();
    }

    pub(crate) fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (Some(drag), Some(layout)) = (self.drag.clone(), self.layout.clone()) else {
            return;
        };
        if event.pressed_button != Some(gpui::MouseButton::Left) {
            self.drag = None;
            return;
        }
        let offset = layout.offset_at(self.document.buffer(), event.position);
        let unit = self.unit_at(drag.granularity, offset);
        let selection = if unit.start < drag.origin.start {
            Selection::new(drag.origin.end, unit.start)
        } else {
            Selection::new(drag.origin.start, unit.end.max(drag.origin.end))
        };
        if selection != self.document.selections().primary() {
            self.change(cx, |document| document.set_primary_selection(selection));
        }
    }

    pub(crate) fn on_mouse_up(
        &mut self,
        _event: &MouseUpEvent,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        self.drag = None;
    }

    pub(crate) fn on_scroll(
        &mut self,
        event: &ScrollEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(layout) = self.layout.clone() else {
            return;
        };
        let delta = event.delta.pixel_delta(px(layout.line_height));
        let sensitivity = self.settings.scroll_sensitivity;
        let (mut dx, dy) = (delta.x.as_f32(), delta.y.as_f32());
        let mut dy = dy;
        // Shift turns a vertical wheel into horizontal scrolling.
        if event.modifiers.shift && dx == 0.0 {
            dx = dy;
            dy = 0.0;
        }
        let max_y = max_scroll_y(self.document.buffer().line_count(), layout.line_height);
        let scroll_y = (self.scroll_y - dy * sensitivity).clamp(0.0, max_y);
        let scroll_x = (self.scroll_x - dx * sensitivity).clamp(0.0, layout.max_scroll_x);
        let moved = (scroll_y - self.scroll_y).abs() > f32::EPSILON
            || (scroll_x - self.scroll_x).abs() > f32::EPSILON;
        if moved {
            self.scroll_y = scroll_y;
            self.scroll_x = scroll_x;
            cx.notify();
        }
        cx.stop_propagation();
    }

    /// The character boundary, word or line (with its break) at `offset`.
    fn unit_at(&self, granularity: Granularity, offset: usize) -> Range<usize> {
        match granularity {
            Granularity::Character => offset..offset,
            Granularity::Word => self.document.word_at(offset),
            Granularity::Line => {
                let row = self.document.buffer().offset_to_point(offset).row;
                self.document.line_with_break(row)
            }
        }
    }
}
