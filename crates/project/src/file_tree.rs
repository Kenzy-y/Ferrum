//! The file tree of a workspace, as a model without UI.
//!
//! Folders are read lazily, when expanded, by [`load_dir`] on a background
//! executor; the result is applied with [`FileTree::apply`]. [`FileTree::rows`]
//! flattens the expanded part into the rows a virtualized list shows.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use fs::{EntryKind, Fs, FsError, FsEvent, FsEventKind};

use crate::ignore_rules::{IGNORE_FILE_NAME, IgnoreRules, Visibility};

/// One file or folder in a listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
    /// Full path.
    pub path: PathBuf,
    /// File name.
    pub name: String,
    /// File, folder or other.
    pub kind: EntryKind,
    /// Whether the entry is a symbolic link.
    pub is_symlink: bool,
    /// Ignored entries are shown dimmed.
    pub visibility: Visibility,
}

impl TreeEntry {
    fn is_dir(&self) -> bool {
        self.kind == EntryKind::Dir
    }
}

/// The contents of one folder, as read by [`load_dir`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirListing {
    /// The folder.
    pub directory: PathBuf,
    /// The folder with links resolved, to detect link cycles.
    pub canonical: Option<PathBuf>,
    /// Visible and ignored entries; hidden ones are left out.
    pub entries: Vec<TreeEntry>,
}

/// Reads `directory`: its `.gitignore` (updating `rules`) and its entries,
/// folders first, then by name ignoring case.
///
/// # Errors
///
/// Fails if the folder cannot be listed.
pub async fn load_dir(
    fs: &dyn Fs,
    directory: &Path,
    rules: &IgnoreRules,
) -> Result<DirListing, FsError> {
    let ignore_file = fs
        .read_to_string(&directory.join(IGNORE_FILE_NAME))
        .await
        .ok();
    rules.set_ignore_file(directory, ignore_file.as_deref());
    let mut entries: Vec<TreeEntry> = fs
        .read_dir(directory)
        .await?
        .into_iter()
        .filter_map(|entry| {
            let is_dir = entry.metadata.is_dir();
            let visibility = rules.visibility(&entry.path, is_dir);
            if visibility == Visibility::Hidden {
                return None;
            }
            Some(TreeEntry {
                path: entry.path,
                name: entry.name,
                kind: entry.metadata.kind,
                is_symlink: entry.metadata.is_symlink,
                visibility,
            })
        })
        .collect();
    entries.sort_by(|left, right| {
        right
            .is_dir()
            .cmp(&left.is_dir())
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
            .then_with(|| left.name.cmp(&right.name))
    });
    let canonical = fs.canonicalize(directory).await.ok();
    Ok(DirListing {
        directory: directory.to_path_buf(),
        canonical,
        entries,
    })
}

/// State of a folder in the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
enum DirState {
    Loading,
    Loaded {
        canonical: Option<PathBuf>,
        entries: Vec<TreeEntry>,
    },
    Failed(String),
    /// A link to one of its own parents: never expanded.
    Cycle,
}

/// What a folder row shows besides its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowState {
    /// Nothing special.
    Normal,
    /// The folder is being read.
    Loading,
    /// The folder could not be read.
    Failed,
    /// The folder is a link to one of its parents.
    Cycle,
}

/// One visible row of the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeRow {
    /// The entry.
    pub entry: TreeEntry,
    /// Nesting level; children of the root are at 0.
    pub depth: usize,
    /// For folders, whether they are expanded.
    pub expanded: Option<bool>,
    /// Loading, error or cycle state of a folder.
    pub state: RowState,
}

/// The tree of one workspace.
#[derive(Debug)]
pub struct FileTree {
    root: PathBuf,
    rules: Arc<IgnoreRules>,
    dirs: HashMap<PathBuf, DirState>,
    expanded: HashSet<PathBuf>,
}

impl FileTree {
    /// A tree of `root` with the root waiting to be loaded.
    pub fn new(root: PathBuf) -> Self {
        let rules = Arc::new(IgnoreRules::new(root.clone()));
        let mut dirs = HashMap::new();
        dirs.insert(root.clone(), DirState::Loading);
        let mut expanded = HashSet::new();
        expanded.insert(root.clone());
        Self {
            root,
            rules,
            dirs,
            expanded,
        }
    }

    /// The workspace root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Ignore rules, shared with the watcher filter.
    pub fn rules(&self) -> Arc<IgnoreRules> {
        self.rules.clone()
    }

    /// Applies a finished listing. Folders that disappeared from it are
    /// forgotten with everything below them. A folder that resolves to one
    /// of its own parents is marked as a cycle and not expanded.
    pub fn apply(&mut self, listing: DirListing) {
        let directory = listing.directory;
        if self.is_cycle(&directory, listing.canonical.as_deref()) {
            self.dirs.insert(directory.clone(), DirState::Cycle);
            self.expanded.remove(&directory);
            return;
        }
        let present: HashSet<&Path> = listing
            .entries
            .iter()
            .map(|entry| entry.path.as_path())
            .collect();
        let gone: Vec<PathBuf> = self
            .dirs
            .keys()
            .filter(|path| {
                path.parent() == Some(directory.as_path()) && !present.contains(path.as_path())
            })
            .cloned()
            .collect();
        for path in gone {
            self.forget(&path);
        }
        self.dirs.insert(
            directory,
            DirState::Loaded {
                canonical: listing.canonical,
                entries: listing.entries,
            },
        );
    }

    /// Records that `directory` could not be read.
    pub fn fail(&mut self, directory: &Path, error: &FsError) {
        if directory == self.root && error.is_not_found() {
            // The workspace itself is gone; keep it empty.
            self.dirs
                .insert(directory.to_path_buf(), DirState::Failed(error.to_string()));
            return;
        }
        if error.is_not_found() {
            self.forget(directory);
        } else {
            self.dirs
                .insert(directory.to_path_buf(), DirState::Failed(error.to_string()));
        }
    }

    fn forget(&mut self, path: &Path) {
        self.dirs.retain(|key, _| !key.starts_with(path));
        self.expanded.retain(|key| !key.starts_with(path));
    }

    fn is_cycle(&self, directory: &Path, canonical: Option<&Path>) -> bool {
        let Some(canonical) = canonical else {
            return false;
        };
        directory.ancestors().skip(1).any(|ancestor| {
            matches!(
                self.dirs.get(ancestor),
                Some(DirState::Loaded { canonical: Some(parent), .. }) if parent == canonical
            )
        })
    }

    /// Whether `directory` is expanded.
    pub fn is_expanded(&self, directory: &Path) -> bool {
        self.expanded.contains(directory)
    }

    /// Expands `directory`; returns it if it must be loaded first.
    pub fn expand(&mut self, directory: &Path) -> Option<PathBuf> {
        if matches!(self.dirs.get(directory), Some(DirState::Cycle)) {
            return None;
        }
        self.expanded.insert(directory.to_path_buf());
        match self.dirs.get(directory) {
            None | Some(DirState::Failed(_)) => {
                self.dirs.insert(directory.to_path_buf(), DirState::Loading);
                Some(directory.to_path_buf())
            }
            Some(_) => None,
        }
    }

    /// Collapses `directory` (the root cannot be collapsed).
    pub fn collapse(&mut self, directory: &Path) {
        if directory != self.root {
            self.expanded.remove(directory);
        }
    }

    /// Expands a collapsed folder or collapses an expanded one; returns the
    /// folder to load, if any.
    pub fn toggle(&mut self, directory: &Path) -> Option<PathBuf> {
        if self.is_expanded(directory) {
            self.collapse(directory);
            None
        } else {
            self.expand(directory)
        }
    }

    /// Folders that are loaded (or loading) and therefore kept up to date.
    pub fn loaded_dirs(&self) -> Vec<PathBuf> {
        self.dirs
            .iter()
            .filter(|(_, state)| matches!(state, DirState::Loaded { .. } | DirState::Loading))
            .map(|(path, _)| path.clone())
            .collect()
    }

    /// Folders to read again after a batch of watcher events.
    pub fn dirs_to_reload(&self, events: &[FsEvent]) -> BTreeSet<PathBuf> {
        let mut reload = BTreeSet::new();
        for event in events {
            if event.kind == FsEventKind::Rescan {
                return self.loaded_dirs().into_iter().collect();
            }
            // The listing of the parent changes when an entry appears,
            // disappears or (for `.gitignore`) changes the rules.
            if let Some(parent) = event.path.parent()
                && matches!(self.dirs.get(parent), Some(DirState::Loaded { .. }))
                && (event.kind != FsEventKind::Changed
                    || event
                        .path
                        .file_name()
                        .is_some_and(|name| name == IGNORE_FILE_NAME))
            {
                reload.insert(parent.to_path_buf());
            }
        }
        reload
    }

    /// Ignored folders that are expanded and visible. The workspace watch
    /// skips ignored paths, so each of these gets its own non-recursive
    /// watch while it stays open.
    pub fn open_ignored_dirs(&self) -> Vec<PathBuf> {
        self.rows()
            .into_iter()
            .filter(|row| row.expanded == Some(true) && row.entry.visibility == Visibility::Ignored)
            .map(|row| row.entry.path)
            .collect()
    }

    /// The visible rows: children of expanded folders, depth first.
    pub fn rows(&self) -> Vec<TreeRow> {
        let mut rows = Vec::new();
        self.push_children(&self.root, 0, &mut rows);
        rows
    }

    fn push_children(&self, directory: &Path, depth: usize, rows: &mut Vec<TreeRow>) {
        let Some(DirState::Loaded { entries, .. }) = self.dirs.get(directory) else {
            return;
        };
        for entry in entries {
            if !entry.is_dir() {
                rows.push(TreeRow {
                    entry: entry.clone(),
                    depth,
                    expanded: None,
                    state: RowState::Normal,
                });
                continue;
            }
            let expanded = self.is_expanded(&entry.path);
            let state = match self.dirs.get(&entry.path) {
                Some(DirState::Loading) if expanded => RowState::Loading,
                Some(DirState::Failed(_)) => RowState::Failed,
                Some(DirState::Cycle) => RowState::Cycle,
                _ => RowState::Normal,
            };
            rows.push(TreeRow {
                entry: entry.clone(),
                depth,
                expanded: Some(expanded),
                state,
            });
            if expanded {
                self.push_children(&entry.path, depth + 1, rows);
            }
        }
    }
}

#[cfg(test)]
mod tests;
