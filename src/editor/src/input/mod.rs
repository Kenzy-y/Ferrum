//! Text input from the platform: typed characters and input method
//! composition (Chinese, Japanese, Korean, dead keys, emoji pickers).
//! Platforms count in UTF-16 code units; the buffer converts.

pub(crate) mod mouse;

use std::ops::Range;

use gpui::{Bounds, Context, EntityInputHandler, Pixels, Point, UTF16Selection, Window};
use text::{Selection, Selections};

use crate::editor::Editor;

impl Editor {
    fn to_offsets(&self, range_utf16: &Range<usize>) -> Range<usize> {
        let buffer = self.document.buffer();
        buffer.utf16_to_offset(range_utf16.start)..buffer.utf16_to_offset(range_utf16.end)
    }

    fn to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        let buffer = self.document.buffer();
        buffer.offset_to_utf16(range.start)..buffer.offset_to_utf16(range.end)
    }
}

/// Byte offset of `utf16` code units into `text`.
fn utf16_to_byte(text: &str, utf16: usize) -> usize {
    let mut units = 0;
    for (offset, character) in text.char_indices() {
        if units >= utf16 {
            return offset;
        }
        units += character.len_utf16();
    }
    text.len()
}

impl EntityInputHandler for Editor {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.to_offsets(&range_utf16);
        *adjusted_range = Some(self.to_utf16(&range));
        Some(self.document.buffer().slice(range))
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let selection = self.document.selections().primary();
        Some(UTF16Selection {
            range: self.to_utf16(&selection.range()),
            reversed: selection.is_reversed(),
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked.as_ref().map(|marked| self.to_utf16(marked))
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.marked.take().is_some() {
            cx.notify();
        }
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .map(|range| self.to_offsets(&range))
            .or_else(|| self.marked.take());
        self.marked = None;
        match range {
            // The platform names the range (input methods, accent menus).
            Some(range) => self.change(cx, |document| {
                document.replace(range, text);
            }),
            // Plain typing: at every cursor.
            None => self.change(cx, |document| document.insert(text)),
        }
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .map(|range| self.to_offsets(&range))
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.document.selections().primary().range());
        let mut inserted = range.start..range.start;
        self.change(cx, |document| inserted = document.replace(range, new_text));
        if let Some(selected) = new_selected_range {
            let start = inserted.start + utf16_to_byte(new_text, selected.start);
            let end = inserted.start + utf16_to_byte(new_text, selected.end);
            self.document
                .set_selections(&Selections::single(Selection::new(start, end)));
        }
        self.marked = (!new_text.is_empty()).then_some(inserted);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        _element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let layout = self.layout.clone()?;
        let start = self.to_offsets(&range_utf16).start;
        Some(layout.bounds_of(self.document.buffer(), start))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let layout = self.layout.clone()?;
        let buffer = self.document.buffer();
        let offset = layout.offset_at(buffer, point);
        Some(buffer.offset_to_utf16(offset))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_offsets_inside_text() {
        assert_eq!(utf16_to_byte("a😀b", 1), 1);
        assert_eq!(utf16_to_byte("a😀b", 3), 5, "the emoji is two UTF-16 units");
        assert_eq!(utf16_to_byte("a😀b", 9), 6);
    }
}
