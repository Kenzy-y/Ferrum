//! A list of places to go to (a code lens with several implementations
//! or references), below or above the line, styled like the completion
//! list. Up and Down choose, Enter and Tab go, Escape closes, a click
//! goes.

use gpui::{
    AnyElement, Context, InteractiveElement as _, MouseButton, ParentElement as _, ScrollEvent,
    SharedString, Styled as _, div, px,
};
use language::Location;
use theme::Theme;

use crate::editor::Editor;
use crate::language::completion::{Step, completion_rows};
use crate::language::popup;

/// An open list of places.
pub(crate) struct LocationList {
    locations: Vec<Location>,
    /// The text the list belongs to; it opens at this offset.
    offset: usize,
    selected: usize,
    /// First row shown.
    first_row: usize,
}

impl LocationList {
    fn step(&mut self, step: Step, rows: usize) {
        let count = self.locations.len();
        if count == 0 {
            return;
        }
        self.selected = match step {
            Step::Up => (self.selected + count - 1) % count,
            Step::Down => (self.selected + 1) % count,
            Step::PageUp => self.selected.saturating_sub(rows.max(1)),
            Step::PageDown => (self.selected + rows.max(1)).min(count - 1),
        };
        if self.selected < self.first_row {
            self.first_row = self.selected;
        } else if self.selected >= self.first_row + rows {
            self.first_row = self.selected + 1 - rows;
        }
    }
}

/// File name and line of `location`, and its folder.
fn describe(location: &Location) -> (String, String) {
    let name = location.path.file_name().map_or_else(
        || location.path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    let folder = location
        .path
        .parent()
        .map(|parent| parent.display().to_string())
        .unwrap_or_default();
    (format!("{name}:{}", location.range.start.row + 1), folder)
}

impl Editor {
    /// Opens the list of `locations` at `offset`.
    pub(crate) fn show_locations(
        &mut self,
        offset: usize,
        locations: Vec<Location>,
        cx: &mut Context<Self>,
    ) {
        self.dismiss_popups(cx);
        if let Some(language) = &mut self.language {
            language.locations = Some(LocationList {
                locations,
                offset,
                selected: 0,
                first_row: 0,
            });
            cx.notify();
        }
    }

    /// Whether the list of places is open.
    pub(crate) fn locations_open(&self) -> bool {
        self.language
            .as_ref()
            .is_some_and(|language| language.locations.is_some())
    }

    /// Moves the selection in the list of places.
    pub(crate) fn step_locations(&mut self, step: Step, rows: usize, cx: &mut Context<Self>) {
        if let Some(list) = self.language.as_mut().and_then(|l| l.locations.as_mut()) {
            list.step(step, rows);
            cx.notify();
        }
    }

    /// Goes to the selected place (or place `pick`) and closes the list.
    pub(crate) fn accept_location(&mut self, pick: Option<usize>, cx: &mut Context<Self>) {
        let Some(list) = self.language.as_mut().and_then(|l| l.locations.take()) else {
            return;
        };
        cx.notify();
        if let Some(location) = list.locations.get(pick.unwrap_or(list.selected)) {
            self.go_to_location(location.clone(), cx);
        }
    }

    /// The open list of places.
    pub(crate) fn render_locations(
        &self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let list = self.language.as_ref()?.locations.as_ref()?;
        let sizes = &theme.metrics.size;
        let metrics = &theme.metrics;
        let rows = completion_rows(theme);
        let (anchor, side) = self.popup_anchor(list.offset, sizes.editor_popup_gap)?;
        let mono = SharedString::from(theme.fonts.mono.clone());
        let items = list
            .locations
            .iter()
            .enumerate()
            .skip(list.first_row)
            .take(rows)
            .map(|(position, location)| {
                let (place, folder) = describe(location);
                let row = div()
                    .id(position)
                    .flex()
                    .items_center()
                    .gap(px(metrics.space.sm))
                    .h(px(sizes.outline_row_height))
                    .px(px(metrics.space.sm))
                    .rounded(px(metrics.radius.sm))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |editor, _, _, cx| {
                            cx.stop_propagation();
                            editor.accept_location(Some(position), cx);
                        }),
                    )
                    .child(
                        div()
                            .flex_none()
                            .font_family(mono.clone())
                            .text_color(theme.colors.text_primary)
                            .child(place),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_color(theme.colors.text_muted)
                            .text_size(px(metrics.font_size.sm))
                            .child(folder),
                    );
                if position == list.selected {
                    row.bg(theme.colors.bg_list_selected)
                } else {
                    row.hover(|style| style.bg(theme.colors.bg_list_hover))
                }
            });
        let body = popup::container(theme)
            .id("location-list")
            .w(px(sizes.completion_width))
            .p(px(metrics.space.xs))
            .on_scroll(cx.listener(move |editor, event: &ScrollEvent, window, cx| {
                cx.stop_propagation();
                let delta = event.delta.pixel_delta(window.line_height()).y.as_f32();
                let step = if delta < 0.0 { Step::Down } else { Step::Up };
                editor.step_locations(step, rows, cx);
            }))
            .children(items);
        Some(popup::place(body, anchor, side))
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use text::Point;

    use super::*;

    fn at(path: &str, row: usize) -> Location {
        Location {
            path: PathBuf::from(path),
            range: Point::new(row, 0)..Point::new(row, 1),
        }
    }

    #[test]
    fn places_show_the_file_line_and_folder() {
        let (place, folder) = describe(&at("/project/src/lib.rs", 41));
        assert_eq!(place, "lib.rs:42");
        assert_eq!(PathBuf::from(folder), PathBuf::from("/project/src"));
    }

    #[test]
    fn the_selection_wraps_and_the_rows_follow_it() {
        let mut list = LocationList {
            locations: (0..5).map(|row| at("/a.rs", row)).collect(),
            offset: 0,
            selected: 0,
            first_row: 0,
        };
        list.step(Step::Up, 3);
        assert_eq!((list.selected, list.first_row), (4, 2));
        list.step(Step::Down, 3);
        assert_eq!((list.selected, list.first_row), (0, 0));
    }
}
