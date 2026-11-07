//! Walking a folder through `Fs`: the files to search, in path order,
//! without hidden paths, paths ignored by `.gitignore` files (and
//! `.git/info/exclude`) and files outside the include globs.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use fs::{DirEntry, Fs};
use ignore::Match;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use ignore::overrides::Override;

/// Name of the per-folder ignore file.
const IGNORE_FILE_NAME: &str = ".gitignore";

/// The repository's own excludes, relative to the root.
const REPOSITORY_EXCLUDE: &str = ".git/info/exclude";

/// A file to search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WalkedFile {
    pub path: PathBuf,
    pub len: u64,
}

/// A folder still to list, with the ignore files that apply to it,
/// outermost first.
struct PendingDir {
    path: PathBuf,
    ignores: Arc<Vec<Gitignore>>,
}

/// Yields the files to search one by one, listing folders as it goes.
pub(crate) struct Walk {
    fs: Arc<dyn Fs>,
    hidden: Gitignore,
    include: Override,
    dirs: Vec<PendingDir>,
    files: std::vec::IntoIter<WalkedFile>,
    root_pending: Option<PathBuf>,
}

impl Walk {
    /// Walks `root`; `hidden` are gitignore-style patterns of paths never
    /// searched (`.git`).
    pub(crate) fn new(fs: Arc<dyn Fs>, root: &Path, hidden: &[String], include: Override) -> Self {
        let hidden = build(root, hidden.iter().map(String::as_str));
        Self {
            fs,
            hidden,
            include,
            dirs: Vec::new(),
            files: Vec::new().into_iter(),
            root_pending: Some(root.to_path_buf()),
        }
    }

    /// The next file, or `None` when the walk is done.
    pub(crate) async fn next(&mut self) -> Option<WalkedFile> {
        if let Some(root) = self.root_pending.take() {
            let mut ignores = Vec::new();
            if let Ok(text) = self.fs.read_to_string(&root.join(REPOSITORY_EXCLUDE)).await {
                ignores.push(build(&root, text.lines()));
            }
            self.dirs.push(PendingDir {
                path: root,
                ignores: Arc::new(ignores),
            });
        }
        loop {
            if let Some(file) = self.files.next() {
                return Some(file);
            }
            let dir = self.dirs.pop()?;
            self.list(dir).await;
        }
    }

    /// Lists `dir`: queues its files and pushes its folders.
    async fn list(&mut self, dir: PendingDir) {
        let mut entries = match self.fs.read_dir(&dir.path).await {
            Ok(entries) => entries,
            Err(error) => {
                tracing::debug!(%error, path = %dir.path.display(), "cannot list folder");
                return;
            }
        };
        entries.sort_by(|left, right| left.name.cmp(&right.name));
        let mut ignores = dir.ignores;
        if entries
            .iter()
            .any(|entry| entry.name == IGNORE_FILE_NAME && entry.metadata.is_file())
            && let Ok(text) = self
                .fs
                .read_to_string(&dir.path.join(IGNORE_FILE_NAME))
                .await
        {
            let mut own = Vec::clone(&ignores);
            own.push(build(&dir.path, text.lines()));
            ignores = Arc::new(own);
        }
        let mut files = Vec::new();
        let mut dirs = Vec::new();
        for entry in entries {
            let is_dir = entry.metadata.is_dir();
            if !self.visible(&entry, is_dir, &ignores) {
                continue;
            }
            if is_dir {
                // Linked folders are not followed: they can form cycles.
                if !entry.metadata.is_symlink {
                    dirs.push(entry.path);
                }
            } else if entry.metadata.is_file() && self.included(&entry.path) {
                files.push(WalkedFile {
                    path: entry.path,
                    len: entry.metadata.len,
                });
            }
        }
        // Popped from the end: push in reverse so folders come in name order.
        self.dirs
            .extend(dirs.into_iter().rev().map(|path| PendingDir {
                path,
                ignores: ignores.clone(),
            }));
        self.files = files.into_iter();
    }

    fn visible(&self, entry: &DirEntry, is_dir: bool, ignores: &[Gitignore]) -> bool {
        if self.hidden.matched(&entry.path, is_dir).is_ignore() {
            return false;
        }
        // The innermost ignore file that has an opinion decides, as in git.
        for ignore in ignores.iter().rev() {
            match ignore.matched(&entry.path, is_dir) {
                Match::Ignore(_) => return false,
                Match::Whitelist(_) => return true,
                Match::None => {}
            }
        }
        true
    }

    fn included(&self, path: &Path) -> bool {
        !self.include.matched(path, false).is_ignore()
    }
}

/// A matcher rooted at `root` from gitignore-style `lines`; invalid lines
/// are skipped.
fn build<'a>(root: &Path, lines: impl IntoIterator<Item = &'a str>) -> Gitignore {
    let mut builder = GitignoreBuilder::new(root);
    for line in lines {
        if let Err(error) = builder.add_line(None, line) {
            tracing::debug!(%error, "skipping invalid ignore rule");
        }
    }
    builder.build().unwrap_or_else(|error| {
        tracing::debug!(%error, "ignore rules dropped");
        Gitignore::empty()
    })
}
