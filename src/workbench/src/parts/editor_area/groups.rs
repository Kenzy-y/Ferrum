//! Two editor groups side by side (Split Editor). The active group's tabs
//! are `Workbench::tabs` and `Workbench::active_tab`, so everything that
//! works on "the tabs" works on the active group; the other group waits in
//! [`Split`]. Focusing it swaps the two. A tab lives in one group only: a
//! split moves the active tab into the new group instead of opening the
//! file twice (one document per file).

use gpui::{Context, Window};

use crate::Workbench;
use crate::parts::editor_area::tabs::{Tab, TabContent};

/// The tabs of a group that is not active.
#[derive(Default)]
pub(crate) struct EditorGroup {
    pub(crate) tabs: Vec<Tab>,
    pub(crate) active_tab: Option<usize>,
    /// Stays with the group's tabs across swaps.
    pub(crate) id: u64,
}

/// The second group of a split editor area.
pub(crate) struct Split {
    pub(crate) other: EditorGroup,
    /// The other group is drawn to the right of the active one.
    pub(crate) other_on_right: bool,
}

impl Workbench {
    /// Splits the editor area, or with a split already, moves the active
    /// tab to the other group. The group that gets the tab becomes active.
    pub(crate) fn split_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let moved = self.take_active_tab_for_move();
        match &mut self.split {
            None => {
                let old = EditorGroup {
                    tabs: std::mem::take(&mut self.tabs),
                    active_tab: self.active_tab.take(),
                    id: self.active_group,
                };
                self.active_group += 1;
                self.split = Some(Split {
                    other: old,
                    other_on_right: false,
                });
                if let Some(tab) = moved {
                    self.tabs.push(tab);
                    self.active_tab = Some(0);
                }
            }
            Some(split) => {
                if let Some(tab) = moved {
                    split.other.tabs.push(tab);
                    split.other.active_tab = Some(split.other.tabs.len() - 1);
                }
                self.swap_groups();
            }
        }
        self.after_group_change(window, cx);
    }

    /// Makes group `id` active (a click or a tab action in it).
    pub(crate) fn focus_group(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        if id != self.active_group {
            self.focus_other_group(window, cx);
        }
    }

    /// Makes the other group active.
    pub(crate) fn focus_other_group(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.split.is_some() {
            self.swap_groups();
            self.after_group_change(window, cx);
        }
    }

    /// Whether an editor of `path`-less content sits in the other group;
    /// used to find a tab before activating it.
    pub(crate) fn other_group_index(&self, matches: impl Fn(&Tab) -> bool) -> Option<usize> {
        self.split
            .as_ref()
            .and_then(|split| split.other.tabs.iter().position(matches))
    }

    /// A group became empty: the split ends and the remaining group is the
    /// only one.
    pub(crate) fn collapse_empty_group(&mut self) {
        let Some(split) = &mut self.split else {
            return;
        };
        if self.tabs.is_empty() {
            let other = std::mem::take(&mut split.other);
            self.tabs = other.tabs;
            self.active_tab = other.active_tab;
            self.active_group = other.id;
            self.split = None;
        } else if split.other.tabs.is_empty() {
            self.split = None;
        }
    }

    /// The active tab, out of its group, when it can move: the group keeps
    /// at least one tab, unless the other group exists to take over.
    fn take_active_tab_for_move(&mut self) -> Option<Tab> {
        let index = self.active_tab?;
        if self.tabs.len() < 2 && self.split.is_none() {
            return None;
        }
        let tab = self.tabs.remove(index);
        self.active_tab = if self.tabs.is_empty() {
            None
        } else {
            Some(index.min(self.tabs.len() - 1))
        };
        Some(tab)
    }

    fn swap_groups(&mut self) {
        if let Some(split) = &mut self.split {
            std::mem::swap(&mut self.tabs, &mut split.other.tabs);
            std::mem::swap(&mut self.active_tab, &mut split.other.active_tab);
            std::mem::swap(&mut self.active_group, &mut split.other.id);
            split.other_on_right = !split.other_on_right;
        }
    }

    fn after_group_change(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.collapse_empty_group();
        if let Some(active) = self.active_tab {
            self.activate_tab(active, window, cx);
        } else {
            self.sync_outline(cx);
            self.retarget_find(window, cx);
        }
        cx.notify();
    }

    /// Every tab of both groups.
    pub(crate) fn all_tabs(&self) -> impl Iterator<Item = &Tab> {
        self.tabs
            .iter()
            .chain(self.split.iter().flat_map(|split| split.other.tabs.iter()))
    }
}

/// Whether `tab` shows `editor`.
pub(crate) fn shows_editor(tab: &Tab, editor: &gpui::Entity<editor::Editor>) -> bool {
    matches!(&tab.content, TabContent::Editor(open) if open == editor)
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;
    use std::sync::Arc;

    use commands::{CommandRegistry, Keymap};
    use editor::{Editor, EditorSettings};
    use fs::{FakeFs, Fs};
    use gpui::{AppContext as _, TestAppContext, WindowHandle};
    use i18n::I18n;
    use text::Document;

    use crate::Workbench;

    fn workbench_with_files(cx: &mut TestAppContext, names: &[&str]) -> WindowHandle<Workbench> {
        cx.update(|cx| {
            let theme =
                theme::Theme::built_in(theme::DEFAULT_THEME, theme::Density::default()).unwrap();
            theme::set_theme(theme, cx);
        });
        let fake = Arc::new(FakeFs::new());
        let fs: Arc<dyn Fs> = fake.clone();
        let window = cx.add_window(|_, _| {
            Workbench::new(
                Arc::new(I18n::new("en").unwrap()),
                Rc::new(|_, _, _| {}),
                Arc::new(CommandRegistry::new()),
                Arc::new(Keymap::default()),
                Rc::new(|_, _, _| {}),
            )
        });
        for name in names {
            let path = format!("/project/{name}");
            fake.insert_file(&path, "fn a() {}\n");
            let fs = fs.clone();
            window
                .update(cx, |workbench, window, cx| {
                    let editor = cx.new(|cx| {
                        Editor::new(
                            Document::from_text("fn a() {}\n"),
                            Some((fs, path.into())),
                            EditorSettings::default(),
                            window,
                            cx,
                        )
                    });
                    workbench.open_editor(editor, window, cx);
                })
                .unwrap();
        }
        window
    }

    fn titles(workbench: &Workbench) -> (Vec<String>, Vec<String>) {
        let active = workbench
            .tabs
            .iter()
            .map(|tab| tab.title.to_string())
            .collect();
        let other = workbench
            .split
            .as_ref()
            .map(|split| {
                split
                    .other
                    .tabs
                    .iter()
                    .map(|tab| tab.title.to_string())
                    .collect()
            })
            .unwrap_or_default();
        (active, other)
    }

    #[gpui::test]
    fn split_moves_the_active_tab_and_closing_a_group_ends_the_split(cx: &mut TestAppContext) {
        let window = workbench_with_files(cx, &["a.rs", "b.rs"]);
        window
            .update(cx, |workbench, window, cx| {
                workbench.split_editor(window, cx);
            })
            .unwrap();
        let (active, other) = window
            .update(cx, |workbench, _, _| titles(workbench))
            .unwrap();
        assert_eq!(active, ["b.rs"]);
        assert_eq!(other, ["a.rs"]);
        assert_eq!(
            window
                .update(cx, |workbench, _, _| workbench.editors().count())
                .unwrap(),
            2,
            "both groups' editors are listed"
        );

        window
            .update(cx, |workbench, window, cx| {
                workbench.focus_other_group(window, cx);
            })
            .unwrap();
        let (active, _) = window
            .update(cx, |workbench, _, _| titles(workbench))
            .unwrap();
        assert_eq!(active, ["a.rs"]);

        // Closing the last tab of the active group leaves the other one.
        window
            .update(cx, |workbench, window, cx| {
                workbench.close_tab(0, window, cx);
            })
            .unwrap();
        let (active, other) = window
            .update(cx, |workbench, _, _| titles(workbench))
            .unwrap();
        assert_eq!(active, ["b.rs"]);
        assert!(other.is_empty());
        assert!(
            window
                .update(cx, |workbench, _, _| workbench.split.is_none())
                .unwrap()
        );
    }

    #[gpui::test]
    fn close_other_tabs_keeps_the_active_one(cx: &mut TestAppContext) {
        let window = workbench_with_files(cx, &["a.rs", "b.rs", "c.rs"]);
        window
            .update(cx, |workbench, window, cx| {
                workbench.activate_tab(1, window, cx);
                let active = workbench.active_tab;
                workbench.close_tabs_where(move |index, _, _| Some(index) != active, window, cx);
            })
            .unwrap();
        let (active, _) = window
            .update(cx, |workbench, _, _| titles(workbench))
            .unwrap();
        assert_eq!(active, ["b.rs"]);
    }
}
