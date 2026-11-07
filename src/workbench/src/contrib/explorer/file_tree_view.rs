//! The file tree of the Explorer: a virtualized list over
//! `project::FileTree`, kept up to date by watching the workspace.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use fs::{Fs, WatchFilter, WatchMode};
use futures::StreamExt as _;
use gpui::{
    App, Context, EventEmitter, FocusHandle, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, Render, ScrollStrategy, SharedString, Styled as _, Task,
    UniformListScrollHandle, Window, div, px, uniform_list,
};
use project::{
    FileTree, GitEnvironment, RowState, TreeRow, Visibility, load_dir, read_repository_excludes,
};
use theme::ActiveTheme as _;
use ui::{Icon, IconName, ListRow};

use crate::contrib::explorer::file_icons::{self, FileIcons};
use crate::contrib::explorer::new_file::{self, NewFileError};

/// Emitted when `.ferrum/settings.toml` of the workspace changed on disk.
pub struct WorkspaceSettingsChanged;

/// Emitted when a file is activated (clicked, or Enter).
pub struct OpenFile(pub PathBuf);

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
    /// A just created file to select once its folder is listed again.
    reveal: Option<PathBuf>,
    /// Latest load request per folder; older results are dropped.
    generations: HashMap<PathBuf, u64>,
    next_generation: u64,
    icons: Arc<FileIcons>,
    texts: FileTreeTexts,
    focus_handle: FocusHandle,
    scroll: UniformListScrollHandle,
    /// Watch of the whole workspace (ignored paths dropped).
    _watch: Task<()>,
    /// One non-recursive watch per open ignored folder.
    ignored_watches: HashMap<PathBuf, Task<()>>,
}

impl EventEmitter<WorkspaceSettingsChanged> for FileTreeView {}
impl EventEmitter<OpenFile> for FileTreeView {}

impl FileTreeView {
    /// Shows the tree of `root` and watches it. `settings_file` is the
    /// workspace settings file, reported with [`WorkspaceSettingsChanged`];
    /// `hidden` are the `[files] hidden` patterns.
    pub fn new(
        fs: Arc<dyn Fs>,
        root: PathBuf,
        settings_file: PathBuf,
        hidden: &[String],
        icons: Arc<FileIcons>,
        texts: FileTreeTexts,
        cx: &mut Context<Self>,
    ) -> Self {
        let tree = FileTree::new(root.clone());
        let rules = tree.rules();
        rules.set_hidden(hidden);
        let workspace_rules = rules.clone();
        let filter: Arc<dyn WatchFilter> =
            Arc::new(move |path: &Path| workspace_rules.is_ignored_for_watch(path));
        let watch = Self::watch(
            fs.clone(),
            root.clone(),
            WatchMode::Recursive,
            filter,
            Some(settings_file),
            cx,
        );
        let view = Self {
            fs: fs.clone(),
            tree,
            rows: Rc::new(Vec::new()),
            selected: None,
            reveal: None,
            generations: HashMap::new(),
            next_generation: 0,
            icons,
            texts,
            focus_handle: cx.focus_handle(),
            scroll: UniformListScrollHandle::new(),
            _watch: watch,
            ignored_watches: HashMap::new(),
        };
        // The repository-wide exclude files apply from the first listing on.
        cx.spawn(async move |view, cx| {
            let excludes_root = root.clone();
            let excludes = cx
                .background_executor()
                .spawn(async move {
                    read_repository_excludes(
                        fs.as_ref(),
                        &excludes_root,
                        &GitEnvironment::from_platform(),
                    )
                    .await
                })
                .await;
            rules.set_repository_excludes(
                excludes.info_exclude.as_deref(),
                excludes.global.as_deref(),
            );
            let _ = view.update(cx, |view, cx| view.load(root, cx));
        })
        .detach();
        view
    }

    /// Replaces the hidden patterns and lists every loaded folder again.
    pub fn set_hidden(&mut self, patterns: &[String], cx: &mut Context<Self>) {
        self.tree.rules().set_hidden(patterns);
        for directory in self.tree.loaded_dirs() {
            self.load(directory, cx);
        }
    }

    /// Watches `path` and reloads the folders events touch. With
    /// `settings_file`, changes of that file are reported too.
    fn watch(
        fs: Arc<dyn Fs>,
        path: PathBuf,
        mode: WatchMode,
        filter: Arc<dyn WatchFilter>,
        settings_file: Option<PathBuf>,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        cx.spawn(async move |view, cx| {
            let watcher = cx
                .background_executor()
                .spawn(async move { fs.watch(&path, mode, filter) })
                .await;
            let mut watcher = match watcher {
                Ok(watcher) => watcher,
                Err(error) => {
                    tracing::warn!(%error, "file tree will not follow changes on disk");
                    return;
                }
            };
            while let Some(batch) = watcher.events().next().await {
                let settings_changed = settings_file
                    .as_ref()
                    .is_some_and(|file| batch.iter().any(|event| &event.path == file));
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

    /// Keeps one non-recursive watch per open ignored folder: the workspace
    /// watch drops their events, so without it an expanded `target` would go
    /// stale. Dropping a task stops its watch.
    fn sync_ignored_watches(&mut self, cx: &mut Context<Self>) {
        let open: std::collections::HashSet<PathBuf> =
            self.tree.open_ignored_dirs().into_iter().collect();
        self.ignored_watches.retain(|path, _| open.contains(path));
        for directory in open {
            if self.ignored_watches.contains_key(&directory) {
                continue;
            }
            let rules = self.tree.rules();
            // Ignored entries are wanted here; hidden ones still are not.
            let filter: Arc<dyn WatchFilter> = Arc::new(move |path: &Path| rules.is_hidden(path));
            let watch = Self::watch(
                self.fs.clone(),
                directory.clone(),
                WatchMode::NonRecursive,
                filter,
                None,
                cx,
            );
            self.ignored_watches.insert(directory, watch);
        }
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
        self.sync_ignored_watches(cx);
        if let Some(reveal) = &self.reveal
            && let Some(index) = self.rows.iter().position(|row| &row.entry.path == reveal)
        {
            self.reveal = None;
            self.select_index(index, cx);
        }
        if let Some(selected) = &self.selected
            && !self.rows.iter().any(|row| &row.entry.path == selected)
        {
            self.selected = None;
        }
        cx.notify();
    }

    /// The folder a new file goes to: the selected folder, the folder of
    /// the selected file, or the root.
    pub(crate) fn target_folder(&self) -> PathBuf {
        let Some(row) = self.selected_index().and_then(|index| self.rows.get(index)) else {
            return self.tree.root().to_path_buf();
        };
        if row.expanded.is_some() {
            row.entry.path.clone()
        } else {
            row.entry
                .path
                .parent()
                .map_or_else(|| self.tree.root().to_path_buf(), Path::to_path_buf)
        }
    }

    /// Creates the empty file `name` in [`Self::target_folder`] in the
    /// background, then selects it and emits [`OpenFile`] for it.
    pub(crate) fn create_file(
        &mut self,
        name: &str,
        cx: &mut Context<Self>,
    ) -> Task<Result<PathBuf, NewFileError>> {
        let folder = self.target_folder();
        let path = match new_file::target_path(&folder, name) {
            Ok(path) => path,
            Err(error) => return Task::ready(Err(error)),
        };
        let collapsed = self
            .rows
            .iter()
            .any(|row| row.entry.path == folder && row.expanded == Some(false));
        if collapsed {
            self.toggle(&folder, cx);
        }
        let fs = self.fs.clone();
        cx.spawn(async move |view, cx| {
            let created = path.clone();
            cx.background_executor()
                .spawn(async move { new_file::create(fs.as_ref(), &created).await })
                .await?;
            let _ = view.update(cx, |view, cx| {
                view.reveal = Some(path.clone());
                view.load(folder, cx);
                cx.emit(OpenFile(path.clone()));
            });
            Ok(path)
        })
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
        } else {
            cx.emit(OpenFile(row.entry.path.clone()));
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
                Some(row) => cx.emit(OpenFile(row.entry.path.clone())),
                None => return,
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
            .key_context("file_tree")
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
