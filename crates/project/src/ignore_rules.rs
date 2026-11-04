//! Which paths of a workspace are ignored: `.gitignore` files of loaded
//! folders plus always-hidden names (`.git`).
//!
//! The rules are shared between the file tree and the file watcher (whose
//! filter runs on the watcher thread), hence the lock.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{PoisonError, RwLock};

use ignore::gitignore::{Gitignore, GitignoreBuilder};

/// Name of the per-folder ignore file.
pub const IGNORE_FILE_NAME: &str = ".gitignore";

/// Folder names never shown or watched: version control internals.
pub const HIDDEN_NAMES: &[&str] = &[".git"];

/// Whether a path is ignored, hidden or neither.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    /// Shown normally.
    Visible,
    /// Shown dimmed: matched by a `.gitignore` (build output and the like).
    Ignored,
    /// Not shown at all.
    Hidden,
}

/// Ignore rules of the folders loaded so far.
#[derive(Debug)]
pub struct IgnoreRules {
    root: PathBuf,
    matchers: RwLock<BTreeMap<PathBuf, Gitignore>>,
}

impl IgnoreRules {
    /// Rules for the workspace at `root`, initially empty.
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            matchers: RwLock::new(BTreeMap::new()),
        }
    }

    /// Sets the rules of `.gitignore` in `directory` from its `text`
    /// (`None` removes them). Invalid lines are skipped.
    pub fn set_ignore_file(&self, directory: &Path, text: Option<&str>) {
        let mut matchers = self
            .matchers
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        let Some(text) = text else {
            matchers.remove(directory);
            return;
        };
        let mut builder = GitignoreBuilder::new(directory);
        let source = directory.join(IGNORE_FILE_NAME);
        for line in text.lines() {
            if let Err(error) = builder.add_line(Some(source.clone()), line) {
                tracing::debug!(%error, "skipping invalid ignore rule");
            }
        }
        match builder.build() {
            Ok(matcher) => {
                matchers.insert(directory.to_path_buf(), matcher);
            }
            Err(error) => tracing::warn!(%error, path = %source.display(), "invalid ignore file"),
        }
    }

    /// Visibility of `path` (`is_dir` tells whether it is a folder; rules
    /// such as `target/` only match folders).
    pub fn visibility(&self, path: &Path, is_dir: bool) -> Visibility {
        let Ok(relative) = path.strip_prefix(&self.root) else {
            return Visibility::Visible;
        };
        if relative.components().any(|component| {
            HIDDEN_NAMES
                .iter()
                .any(|name| component.as_os_str() == *name)
        }) {
            return Visibility::Hidden;
        }
        let matchers = self.matchers.read().unwrap_or_else(PoisonError::into_inner);
        // A path is ignored when it, or any of its parent folders, is matched
        // by the closest rule that applies to it.
        let mut current = Some(path);
        let mut current_is_dir = is_dir;
        while let Some(candidate) = current {
            if candidate == self.root {
                break;
            }
            if let Some(decision) =
                closest_decision(&matchers, &self.root, candidate, current_is_dir)
            {
                if decision {
                    return Visibility::Ignored;
                }
                // Whitelisted explicitly: parents cannot ignore it back.
                return Visibility::Visible;
            }
            current = candidate.parent();
            current_is_dir = true;
        }
        Visibility::Visible
    }

    /// Whether events for `path` are worth reporting (for the watcher).
    pub fn is_ignored_for_watch(&self, path: &Path) -> bool {
        // A path that disappeared cannot be checked for being a folder;
        // folder-only rules then also apply, which is the cheap choice.
        self.visibility(path, true) != Visibility::Visible
    }
}

/// The decision of the nearest `.gitignore` above `path` that has an opinion:
/// `Some(true)` ignored, `Some(false)` whitelisted, `None` no rule matched.
fn closest_decision(
    matchers: &BTreeMap<PathBuf, Gitignore>,
    root: &Path,
    path: &Path,
    is_dir: bool,
) -> Option<bool> {
    let mut directory = path.parent();
    while let Some(folder) = directory {
        if let Some(matcher) = matchers.get(folder) {
            let matched = matcher.matched(path, is_dir);
            if matched.is_ignore() {
                return Some(true);
            }
            if matched.is_whitelist() {
                return Some(false);
            }
        }
        if folder == root {
            break;
        }
        directory = folder.parent();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> IgnoreRules {
        let rules = IgnoreRules::new(PathBuf::from("/work/forge"));
        rules.set_ignore_file(
            Path::new("/work/forge"),
            Some("target/\n*.log\n!keep.log\n"),
        );
        rules.set_ignore_file(Path::new("/work/forge/web"), Some("node_modules\n"));
        rules
    }

    #[test]
    fn applies_rules_of_the_folder_and_its_parents() {
        let rules = rules();
        let visibility = |path: &str, is_dir| rules.visibility(Path::new(path), is_dir);
        assert_eq!(visibility("/work/forge/target", true), Visibility::Ignored);
        assert_eq!(
            visibility("/work/forge/target/debug/app", false),
            Visibility::Ignored
        );
        assert_eq!(
            visibility("/work/forge/build.log", false),
            Visibility::Ignored
        );
        assert_eq!(
            visibility("/work/forge/keep.log", false),
            Visibility::Visible
        );
        assert_eq!(
            visibility("/work/forge/web/node_modules", true),
            Visibility::Ignored
        );
        assert_eq!(
            visibility("/work/forge/src/main.rs", false),
            Visibility::Visible
        );
        assert_eq!(
            visibility("/work/forge/target", false),
            Visibility::Visible,
            "target/ matches folders only"
        );
    }

    #[test]
    fn version_control_folders_are_hidden() {
        let rules = rules();
        assert_eq!(
            rules.visibility(Path::new("/work/forge/.git"), true),
            Visibility::Hidden
        );
        assert_eq!(
            rules.visibility(Path::new("/work/forge/.git/HEAD"), false),
            Visibility::Hidden
        );
        assert_eq!(
            rules.visibility(Path::new("/work/forge/.github"), true),
            Visibility::Visible
        );
    }

    #[test]
    fn removing_an_ignore_file_drops_its_rules() {
        let rules = rules();
        rules.set_ignore_file(Path::new("/work/forge"), None);
        assert_eq!(
            rules.visibility(Path::new("/work/forge/target"), true),
            Visibility::Visible
        );
    }

    #[test]
    fn watcher_drops_ignored_and_hidden_paths() {
        let rules = rules();
        assert!(rules.is_ignored_for_watch(Path::new("/work/forge/target/debug/deps/x.o")));
        assert!(rules.is_ignored_for_watch(Path::new("/work/forge/.git/index")));
        assert!(!rules.is_ignored_for_watch(Path::new("/work/forge/src/main.rs")));
    }
}
