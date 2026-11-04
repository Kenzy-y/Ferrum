//! The file tree of the Explorer: a virtualized list over
//! `project::FileTree`, kept up to date by watching the workspace.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use fs::{Fs, WatchFilter};
use futures::StreamExt as _;
use gpui::{
    App, Context, EventEmitter, FocusHandle, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, Render, ScrollStrategy, SharedString, Styled as _, Task,
    UniformListScrollHandle, Window, div, px, uniform_list,
};
use project::{FileTree, RowState, TreeRow, Visibility, load_dir};
use theme::ActiveTheme as _;
use ui::{Icon, IconName, ListRow};

use crate::file_icons::{self, FileIcons};

/// Emitted when `.ferrum/settings.toml` of the workspace changed on disk.
pub struct WorkspaceSettingsChanged;

/// Texts of the tree, already localized.
#[derive(Clone)]
pub struct FileTreeTexts {
    /// Tooltip of a folder that links to one of its parents.
    pub cycle: SharedString,
    /// Tooltip of a folder that cannot be read.
    pub failed: SharedString,
}

/// The file tree view.
pub struct FileTreeView {
    fs: Arc<dyn Fs>,
    tree: FileTree,
    rows: Rc<Vec<TreeRow>>,
    selected: Option<PathBuf>,
    /// Latest load request per folder; older results are dropped.
    generations: HashMap<PathBuf, u64>,
    next_generation: u64,
    icons: Arc<FileIcons>,
    texts: FileTreeTexts,
    focus_handle: FocusHandle,
    scroll: UniformListScrollHandle,
    _watch: Task<()>,
}

impl EventEmitter<WorkspaceSettingsChanged> for FileTreeView {}

impl FileTreeView {
    /// Shows the tree of `root` and watches it. `settings_file` is the
    /// workspace settings file, reported with [`WorkspaceSettingsChanged`].
    pub fn new(
        fs: Arc<dyn Fs>,
        root: PathBuf,
        settings_file: PathBuf,
        icons: Arc<FileIcons>,
        texts: FileTreeTexts,
        cx: &mut Context<Self>,
    ) -> Self {
        let tree = FileTree::new(root.clone());
        let watch = Self::watch(fs.clone(), root.clone(), settings_file, &tree, cx);
        let mut view = Self {
            fs,
            tree,
            rows: Rc::new(Vec::new()),
            selected: None,
            generations: HashMap::new(),
            next_generation: 0,
            icons,
            texts,
            focus_handle: cx.focus_handle(),
            scroll: UniformListScrollHandle::new(),
            _watch: watch,
        };
        view.load(root, cx);
        view
    }

    /// Watches the workspace; events under ignored folders are dropped on
    /// the watcher thread.
    fn watch(
        fs: Arc<dyn Fs>,
        root: PathBuf,
        settings_file: PathBuf,
        tree: &FileTree,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        let rules = tree.rules();
        let filter: Arc<dyn WatchFilter> =
            Arc::new(move |path: &Path| rules.is_ignored_for_watch(path));
        cx.spawn(async move |view, cx| {
            let watcher = cx
                .background_executor()
                .spawn(async move { fs.watch(&root, filter) })
                .await;
            let mut watcher = match watcher {
                Ok(watcher) => watcher,
                Err(error) => {
                    tracing::warn!(%error, "file tree will not follow changes on disk");
                    return;
                }
            };
            while let Some(batch) = watcher.events().next().await {
                let settings_changed = batch.iter().any(|event| event.path == settings_file);
                let updated = view.update(cx, |view, cx| {
                    if settings_changed {
                        cx.emit(WorkspaceSettingsChanged);
                    }
                    for directory in view.tree.dirs_to_reload(&batch) {
                        view.load(directory, cx);
                    }
                });
                if updated.is_err() {
                    return;
                }
            }
        })
    }

    /// Reads `directory` in the background and applies the result.
    fn load(&mut self, directory: PathBuf, cx: &mut Context<Self>) {
        self.next_generation += 1;
        let generation = self.next_generation;
        self.generations.insert(directory.clone(), generation);
        let fs = self.fs.clone();
        let rules = self.tree.rules();
        cx.spawn(async move |view, cx| {
            let listed_directory = directory.clone();
            let result = cx
                .background_executor()
                .spawn(async move { load_dir(fs.as_ref(), &listed_directory, &rules).await })
                .await;
            let _ = view.update(cx, |view, cx| {
                if view.generations.get(&directory) != Some(&generation) {
                    return;
                }
                view.generations.remove(&directory);
                match result {
                    Ok(listing) => view.tree.apply(listing),
                    Err(error) => {
                        tracing::warn!(%error, "cannot read folder");
                        view.tree.fail(&directory, &error);
                    }
                }
                view.refresh_rows(cx);
            });
        })
        .detach();
    }

    fn refresh_rows(&mut self, cx: &mut Context<Self>) {
        self.rows = Rc::new(self.tree.rows());
        if let Some(selected) = &self.selected
            && !self.rows.iter().any(|row| &row.entry.path == selected)
        {
            self.selected = None;
        }
        cx.notify();
    }

    fn selected_index(&self) -> Option<usize> {
        let selected = self.selected.as_ref()?;
        self.rows.iter().position(|row| &row.entry.path == selected)
    }

    fn select_index(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(row) = self.rows.get(index) {
            self.selected = Some(row.entry.path.clone());
            self.scroll.scroll_to_item(index, ScrollStrategy::Nearest);
            cx.notify();
        }
    }

    fn toggle(&mut self, path: &Path, cx: &mut Context<Self>) {
        if let Some(directory) = self.tree.toggle(path) {
            self.load(directory, cx);
        }
        self.refresh_rows(cx);
    }

    /// Click: select the row; folders also open or close.
    fn activate(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_handle.focus(window, cx);
        let Some(row) = self.rows.get(index).cloned() else {
            return;
        };
        self.selected = Some(row.entry.path.clone());
        if row.expanded.is_some() {
            self.toggle(&row.entry.path, cx);
        }
        cx.notify();
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let modifiers = event.keystroke.modifiers;
        if modifiers.control || modifiers.alt || modifiers.platform || modifiers.shift {
            return;
        }
        let count = self.rows.len();
        if count == 0 {
            return;
        }
        let current = self.selected_index();
        let row = current.and_then(|index| self.rows.get(index).cloned());
        match event.keystroke.key.as_str() {
            "down" => self.select_index(current.map_or(0, |index| (index + 1).min(count - 1)), cx),
            "up" => self.select_index(current.map_or(0, |index| index.saturating_sub(1)), cx),
            "home" => self.select_index(0, cx),
            "end" => self.select_index(count - 1, cx),
            "right" => match (current, row) {
                (Some(index), Some(row)) if row.expanded == Some(true) => {
                    self.select_index(index + 1, cx);
                }
                (_, Some(row)) if row.expanded == Some(false) => self.toggle(&row.entry.path, cx),
                _ => return,
            },
            "left" => match row {
                Some(row) if row.expanded == Some(true) => self.toggle(&row.entry.path, cx),
                Some(row) => {
                    let parent = row.entry.path.parent().map(Path::to_path_buf);
                    if let Some(index) = parent.and_then(|parent| {
                        self.rows.iter().position(|row| row.entry.path == parent)
                    }) {
                        self.select_index(index, cx);
                    }
                }
                None => return,
            },
            "enter" | "space" => match row {
                Some(row) if row.expanded.is_some() => self.toggle(&row.entry.path, cx),
                _ => return,
            },
            _ => return,
        }
        cx.stop_propagation();
    }
}

fn row_icon(row: &TreeRow, icons: &FileIcons, cx: &App) -> gpui::AnyElement {
    match row.expanded {
        Some(true) => Icon::new(IconName::FolderOpen).into_any_element(),
        Some(false) => Icon::new(IconName::Folder).into_any_element(),
        None => file_icons::render(&icons.for_file(&row.entry.name), cx),
    }
}

impl Render for FileTreeView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = self.rows.clone();
        let selected = self.selected.clone();
        let icons = self.icons.clone();
        let texts = self.texts.clone();
        let entity = cx.entity().downgrade();
        let status_slot = cx.theme().metrics.size.icon_small;
        let list = uniform_list("file-tree", rows.len(), move |range, _window, cx| {
            range
                .map(|index| {
                    let row = &rows[index];
                    let entity = entity.clone();
                    let mut item = ListRow::new(("file-tree-row", index), row.entry.name.clone())
                        .depth(row.depth)
                        .leading(row_icon(row, &icons, cx))
                        .ignored(
                            row.entry.visibility == Visibility::Ignored
                                || row.state == RowState::Cycle,
                        )
                        .selected(selected.as_ref() == Some(&row.entry.path))
                        .tab_stop(false)
                        // Reserved for version control marks (stage 7).
                        .trailing(div().w(px(status_slot)))
                        .on_activate(move |window, cx| {
                            let _ = entity.update(cx, |view, cx| view.activate(index, window, cx));
                        });
                    if let Some(expanded) = row.expanded {
                        item = item.expanded(expanded);
                    }
                    match row.state {
                        RowState::Cycle => item = item.tooltip(texts.cycle.clone()),
                        RowState::Failed => item = item.tooltip(texts.failed.clone()),
                        RowState::Normal | RowState::Loading => {}
                    }
                    item
                })
                .collect()
        })
        .track_scroll(&self.scroll)
        .size_full();

        let theme = cx.theme();
        let ring = theme.colors.border_focus;
        div()
            .id("file-tree-container")
            .track_focus(&self.focus_handle.clone().tab_stop(true))
            .on_key_down(cx.listener(Self::on_key_down))
            .flex()
            .flex_1()
            .min_h_0()
            .px(px(theme.metrics.space.sm))
            .border_1()
            .border_color(gpui::transparent_black())
            .rounded(px(theme.metrics.radius.md))
            .focus_visible(move |style| style.border_color(ring))
            .child(list)
    }
}
