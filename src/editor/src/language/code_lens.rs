//! Code lenses: notes the language service attaches to code ("1
//! implementation"), drawn at the end of their line as in `editor.svg`.
//! The lenses of the whole document are asked for after `[editor]
//! language_decorations_delay_ms` once edits pause, and the ones in view
//! are resolved when the service fills in titles on request. Only lenses
//! the IDE can act on are shown; a click shows their places.

use std::ops::Range;
use std::time::Duration;

use gpui::{Context, Task};
use language::{CodeLens, LensAction, Location};
use text::{Buffer, TextChanges};

use crate::editor::Editor;
use crate::highlighting::patch::map_anchor;

/// A lens placed in the text.
#[derive(Debug, Clone)]
pub(crate) struct PlacedLens {
    /// Start of the code it is about; it is drawn on this row.
    pub(crate) offset: usize,
    pub(crate) lens: CodeLens,
    /// Resolving was asked for; a lens is resolved at most once, so one the
    /// service cannot fill in is not asked about on every frame.
    resolve_asked: bool,
}

impl PlacedLens {
    /// The text to draw, once the lens is known and can be acted on.
    fn shown_title(&self) -> Option<&str> {
        match self.lens.action {
            LensAction::ShowLocations(_) => self.lens.title.as_deref(),
            LensAction::Unsupported => None,
        }
    }
}

/// The lenses of a document and the requests for them.
#[derive(Default)]
pub(crate) struct CodeLenses {
    /// Sorted by offset.
    pub(crate) lenses: Vec<PlacedLens>,
    /// The lenses are current; `false` asks again.
    fresh: bool,
    /// The request for the lenses is in flight.
    requesting: bool,
    /// Changes whenever `lenses` is replaced or reordered, so late
    /// resolve answers do not land on another lens.
    generation: u64,
    task: Option<Task<()>>,
    resolve_tasks: Vec<Task<()>>,
}

impl CodeLenses {
    /// Moves lenses through edits and asks again; a lens inside replaced
    /// text goes, a whole-text change drops them all.
    pub(crate) fn follow(&mut self, changes: &TextChanges, buffer: &Buffer) {
        self.invalidate();
        if changes.replaced {
            self.lenses.clear();
            return;
        }
        self.lenses.retain_mut(|placed| {
            for edit in &changes.edits {
                let Some(offset) = map_anchor(placed.offset, edit) else {
                    return false;
                };
                placed.offset = offset;
            }
            placed.offset = buffer.clamp(placed.offset);
            placed.resolve_asked = false;
            true
        });
        self.lenses.sort_by_key(|placed| placed.offset);
    }

    /// The service has new lenses, or the settings changed: the next
    /// frame asks again.
    pub(crate) fn invalidate(&mut self) {
        self.fresh = false;
        self.requesting = false;
        self.task = None;
        self.generation += 1;
        self.resolve_tasks.clear();
        for placed in &mut self.lenses {
            placed.resolve_asked = false;
        }
    }

    /// Indices of the lenses on `rows` that are not resolved yet.
    fn unresolved_in(&self, rows: &Range<usize>, buffer: &Buffer) -> Vec<usize> {
        let (start, end) = row_offsets(rows, buffer);
        let from = self.lenses.partition_point(|placed| placed.offset < start);
        (from..self.lenses.len())
            .take_while(|index| self.lenses[*index].offset <= end)
            .filter(|index| {
                let placed = &self.lenses[*index];
                placed.lens.title.is_none()
                    && placed.lens.resolve.is_some()
                    && !placed.resolve_asked
            })
            .collect()
    }
}

/// Buffer offsets of the start of the first and the end of the last of
/// `rows` (not empty).
fn row_offsets(rows: &Range<usize>, buffer: &Buffer) -> (usize, usize) {
    let last = rows.end.max(rows.start + 1) - 1;
    (
        buffer.line_range(rows.start).start,
        buffer.line_range(last.min(buffer.line_count() - 1)).end,
    )
}

/// Places `lenses` in `buffer`, sorted by offset.
fn place(lenses: Vec<CodeLens>, buffer: &Buffer) -> Vec<PlacedLens> {
    let mut placed: Vec<PlacedLens> = lenses
        .into_iter()
        .map(|lens| PlacedLens {
            offset: buffer.point_to_offset(lens.range.start),
            lens,
            resolve_asked: false,
        })
        .collect();
    placed.sort_by_key(|placed| placed.offset);
    placed
}

impl Editor {
    /// The lenses to draw on `row`: index and title, in order; none when
    /// they are off.
    pub(crate) fn code_lenses_on(&self, row: usize) -> Vec<(usize, &str)> {
        let Some(language) = self.language.as_ref().filter(|_| self.settings.code_lens) else {
            return Vec::new();
        };
        let buffer = self.document.buffer();
        let (start, end) = row_offsets(&(row..row + 1), buffer);
        let lenses = &language.code_lenses.lenses;
        let from = lenses.partition_point(|placed| placed.offset < start);
        lenses[from..]
            .iter()
            .enumerate()
            .take_while(|(_, placed)| placed.offset <= end)
            .filter_map(|(index, placed)| Some((from + index, placed.shown_title()?)))
            .collect()
    }

    /// After a frame that showed `visible` rows: asks for the lenses once
    /// edits pause, and resolves the ones in view.
    pub(crate) fn request_code_lenses(&mut self, visible: &Range<usize>, cx: &mut Context<Self>) {
        let enabled = self.settings.code_lens;
        let delay = Duration::from_millis(self.settings.language_decorations_delay_ms);
        let buffer = self.document.buffer();
        let Some(language) = &mut self.language else {
            return;
        };
        let Some(capability) = language.capabilities.code_lens else {
            return;
        };
        if !enabled || visible.is_empty() {
            return;
        }
        let state = &mut language.code_lenses;
        if !state.fresh {
            if !state.requesting {
                state.requesting = true;
                let version = language.version;
                let service = language.service.clone();
                let path = language.path.clone();
                state.task = Some(cx.spawn(async move |editor, cx| {
                    cx.background_executor().timer(delay).await;
                    let answer = service.code_lenses(&path).await;
                    let _ = editor.update(cx, |editor, cx| {
                        let buffer = editor.document.buffer();
                        let Some(language) = &mut editor.language else {
                            return;
                        };
                        // A failed request is not repeated until an edit,
                        // a refresh or a status change. This task is not
                        // dropped from inside it.
                        let (Ok(lenses), true) = (answer, language.version == version) else {
                            return;
                        };
                        let state = &mut language.code_lenses;
                        state.requesting = false;
                        state.generation += 1;
                        state.resolve_tasks.clear();
                        state.lenses = place(lenses, buffer);
                        state.fresh = true;
                        cx.notify();
                    });
                }));
            }
            return;
        }
        if !capability.resolve {
            return;
        }
        for index in state.unresolved_in(visible, buffer) {
            let placed = &mut state.lenses[index];
            placed.resolve_asked = true;
            let request = language.service.resolve_code_lens(placed.lens.clone());
            let generation = state.generation;
            state.resolve_tasks.push(cx.spawn(async move |editor, cx| {
                let Ok(resolved) = request.await else {
                    return;
                };
                let _ = editor.update(cx, |editor, cx| {
                    let Some(language) = &mut editor.language else {
                        return;
                    };
                    let state = &mut language.code_lenses;
                    if state.generation != generation {
                        return;
                    }
                    if let Some(placed) = state.lenses.get_mut(index) {
                        placed.lens = resolved;
                        cx.notify();
                    }
                });
            }));
        }
    }

    /// A click on lens `index`: one place is gone to, several are listed.
    pub(crate) fn activate_code_lens(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(language) = &self.language else {
            return;
        };
        let Some(placed) = language.code_lenses.lenses.get(index) else {
            return;
        };
        let LensAction::ShowLocations(locations) = &placed.lens.action else {
            return;
        };
        let offset = placed.offset;
        match locations.as_slice() {
            [] => {}
            [location] => {
                let location = location.clone();
                self.go_to_location(location, cx);
            }
            _ => {
                let locations = locations.clone();
                self.show_locations(offset, locations, cx);
            }
        }
    }

    /// Goes to `location`: in this file the cursor moves, another file is
    /// reported for the owner to open.
    pub(crate) fn go_to_location(&mut self, location: Location, cx: &mut Context<Self>) {
        if self.path() == Some(location.path.as_path()) {
            let offset = self.document.buffer().point_to_offset(location.range.start);
            self.go_to(offset, cx);
        } else {
            cx.emit(crate::editor::EditorEvent::OpenLocation(location));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use language::ResolveToken;
    use text::Point;

    use super::*;

    fn lens(row: usize, title: Option<&str>, action: LensAction) -> CodeLens {
        CodeLens {
            range: Point::new(row, 0)..Point::new(row, 2),
            title: title.map(str::to_owned),
            action,
            resolve: title
                .is_none()
                .then(|| ResolveToken(std::sync::Arc::new(()))),
        }
    }

    fn shows() -> LensAction {
        LensAction::ShowLocations(vec![Location {
            path: PathBuf::from("/a.rs"),
            range: Point::new(0, 0)..Point::new(0, 1),
        }])
    }

    #[test]
    fn only_titled_lenses_the_ide_can_act_on_are_shown() {
        let buffer = Buffer::from_text("fn a() {}\nfn b() {}\n");
        let placed = place(
            vec![
                lens(1, Some("Run"), LensAction::Unsupported),
                lens(0, Some("1 implementation"), shows()),
                lens(1, None, LensAction::Unsupported),
            ],
            &buffer,
        );
        let titles: Vec<Option<&str>> = placed.iter().map(PlacedLens::shown_title).collect();
        assert_eq!(titles, vec![Some("1 implementation"), None, None]);
        let state = CodeLenses {
            lenses: placed,
            ..CodeLenses::default()
        };
        assert_eq!(state.unresolved_in(&(1..2), &buffer), vec![2]);
        assert!(state.unresolved_in(&(0..1), &buffer).is_empty());
    }

    #[test]
    fn lenses_follow_edits_and_forget_pending_resolves() {
        let mut document = text::Document::from_text("fn a() {}\nfn b() {}\n");
        let mut state = CodeLenses {
            lenses: place(
                vec![lens(0, Some("x"), shows()), lens(1, Some("y"), shows())],
                document.buffer(),
            ),
            fresh: true,
            ..CodeLenses::default()
        };
        state.lenses[1].resolve_asked = true;
        let generation = state.generation;
        document.track_changes();
        document.replace(0..0, "\n");
        state.follow(&document.take_changes(), document.buffer());
        let offsets: Vec<usize> = state.lenses.iter().map(|placed| placed.offset).collect();
        assert_eq!(offsets, vec![1, 11]);
        assert!(!state.fresh, "asks again after an edit");
        assert_ne!(state.generation, generation);
        assert!(state.lenses.iter().all(|placed| !placed.resolve_asked));
    }
}
