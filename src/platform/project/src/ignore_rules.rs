//! Which paths of a workspace are ignored or hidden.
//!
//! - Hidden (not shown, not watched): the `[files] hidden` patterns, `.git`
//!   by default.
//! - Ignored (shown dimmed): `.gitignore` files of loaded folders, then
//!   `.git/info/exclude`, then git's global excludes file (`core.excludesFile`),
//!   in that order of precedence, as in git.
//!
//! The rules are shared between the file tree and the file watcher (whose
//! filter runs on the watcher thread), hence the locks. All rule files are
//! read through `fs::Fs` by the callers; this module only matches.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{PoisonError, RwLock};

use ignore::gitignore::{Gitignore, GitignoreBuilder};

/// Name of the per-folder ignore file.
pub const IGNORE_FILE_NAME: &str = ".gitignore";

/// Default of `[files] hidden`: version control internals.
pub const DEFAULT_HIDDEN: &[&str] = &[".git"];

/// Whether a path is ignored, hidden or neither.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    /// Shown normally.
    Visible,
    /// Shown dimmed: matched by an ignore rule (build output and the like).
    Ignored,
    /// Not shown at all.
    Hidden,
}

/// Ignore rules of a workspace.
#[derive(Debug)]
pub struct IgnoreRules {
    root: PathBuf,
    hidden: RwLock<Gitignore>,
    /// `.git/info/exclude`.
    repository: RwLock<Option<Gitignore>>,
    /// The global excludes file of git.
    global: RwLock<Option<Gitignore>>,
    /// `.gitignore` of each loaded folder.
    matchers: RwLock<BTreeMap<PathBuf, Gitignore>>,
}

/// Builds a matcher rooted at `root` from gitignore-style `lines`.
/// Invalid lines are skipped.
fn build(
    root: &Path,
    source: &Path,
    lines: impl IntoIterator<Item = impl AsRef<str>>,
) -> Option<Gitignore> {
    let mut builder = GitignoreBuilder::new(root);
    for line in lines {
        if let Err(error) = builder.add_line(Some(source.to_path_buf()), line.as_ref()) {
            tracing::debug!(%error, "skipping invalid ignore rule");
        }
    }
    match builder.build() {
        Ok(matcher) => Some(matcher),
        Err(error) => {
            tracing::warn!(%error, path = %source.display(), "invalid ignore rules");
            None
        }
    }
}

fn write<T>(lock: &RwLock<T>) -> std::sync::RwLockWriteGuard<'_, T> {
    lock.write().unwrap_or_else(PoisonError::into_inner)
}

fn read<T>(lock: &RwLock<T>) -> std::sync::RwLockReadGuard<'_, T> {
    lock.read().unwrap_or_else(PoisonError::into_inner)
}

impl IgnoreRules {
    /// Rules for the workspace at `root`: the default hidden patterns, no
    /// ignore files yet.
    pub fn new(root: PathBuf) -> Self {
        let hidden = build(&root, &root, DEFAULT_HIDDEN).unwrap_or_else(Gitignore::empty);
        Self {
            root,
            hidden: RwLock::new(hidden),
            repository: RwLock::new(None),
            global: RwLock::new(None),
            matchers: RwLock::new(BTreeMap::new()),
        }
    }

    /// Replaces the hidden patterns (gitignore syntax: `.git` hides every
    /// `.git`, `/build` only the one at the root).
    pub fn set_hidden(&self, patterns: &[String]) {
        *write(&self.hidden) =
            build(&self.root, &self.root, patterns).unwrap_or_else(Gitignore::empty);
    }

    /// Sets the repository-wide excludes (`.git/info/exclude`) and the
    /// global excludes file of git; `None` means the file does not exist.
    pub fn set_repository_excludes(&self, info_exclude: Option<&str>, global: Option<&str>) {
        let source = self.root.join(".git").join("info").join("exclude");
        *write(&self.repository) =
            info_exclude.and_then(|text| build(&self.root, &source, text.lines()));
        *write(&self.global) = global.and_then(|text| build(&self.root, &self.root, text.lines()));
    }

    /// Sets the rules of `.gitignore` in `directory` from its `text`
    /// (`None` removes them).
    pub fn set_ignore_file(&self, directory: &Path, text: Option<&str>) {
        let mut matchers = write(&self.matchers);
        let Some(text) = text else {
            matchers.remove(directory);
            return;
        };
        let source = directory.join(IGNORE_FILE_NAME);
        if let Some(matcher) = build(directory, &source, text.lines()) {
            matchers.insert(directory.to_path_buf(), matcher);
        }
    }

    /// Visibility of `path` (`is_dir` tells whether it is a folder; rules
    /// such as `target/` only match folders).
    pub fn visibility(&self, path: &Path, is_dir: bool) -> Visibility {
        if !path.starts_with(&self.root) || path == self.root {
            return Visibility::Visible;
        }
        if read(&self.hidden)
            .matched_path_or_any_parents(path, is_dir)
            .is_ignore()
        {
            return Visibility::Hidden;
        }
        if let Some(decision) = self.folder_decision(path, is_dir) {
            return if decision {
                Visibility::Ignored
            } else {
                Visibility::Visible
            };
        }
        for source in [&self.repository, &self.global] {
            if let Some(matcher) = read(source).as_ref() {
                let matched = matcher.matched_path_or_any_parents(path, is_dir);
                if matched.is_ignore() {
                    return Visibility::Ignored;
                }
                if matched.is_whitelist() {
                    return Visibility::Visible;
                }
            }
        }
        Visibility::Visible
    }

    /// The decision of the `.gitignore` files for `path`: `Some(true)`
    /// ignored, `Some(false)` explicitly whitelisted, `None` no rule. A path
    /// is ignored when it, or any of its parent folders, is matched by the
    /// closest rule that applies to it.
    fn folder_decision(&self, path: &Path, is_dir: bool) -> Option<bool> {
        let matchers = read(&self.matchers);
        let mut current = Some(path);
        let mut current_is_dir = is_dir;
        while let Some(candidate) = current {
            if candidate == self.root {
                return None;
            }
            if let Some(decision) =
                closest_decision(&matchers, &self.root, candidate, current_is_dir)
            {
                return Some(decision);
            }
            current = candidate.parent();
            current_is_dir = true;
        }
        None
    }

    /// Whether events for `path` are worth reporting (for the watcher).
    pub fn is_ignored_for_watch(&self, path: &Path) -> bool {
        // A path that disappeared cannot be checked for being a folder;
        // folder-only rules then also apply, which is the cheap choice.
        self.visibility(path, true) != Visibility::Visible
    }

    /// Whether `path` is hidden (for watches of ignored folders, which want
    /// ignored entries but never hidden ones).
    pub fn is_hidden(&self, path: &Path) -> bool {
        self.visibility(path, true) == Visibility::Hidden
    }
}

/// The decision of the nearest `.gitignore` above `path` that has an opinion.
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
    fn version_control_folders_are_hidden_by_default() {
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
            rules.visibility(Path::new("/work/forge/vendor/lib/.git"), true),
            Visibility::Hidden
        );
        assert_eq!(
            rules.visibility(Path::new("/work/forge/.github"), true),
            Visibility::Visible
        );
    }

    #[test]
    fn hidden_patterns_are_configurable() {
        let rules = rules();
        rules.set_hidden(&["/dist".to_owned(), "*.tmp".to_owned()]);
        assert_eq!(
            rules.visibility(Path::new("/work/forge/.git"), true),
            Visibility::Visible
        );
        assert_eq!(
            rules.visibility(Path::new("/work/forge/dist"), true),
            Visibility::Hidden
        );
        assert_eq!(
            rules.visibility(Path::new("/work/forge/web/dist"), true),
            Visibility::Visible,
            "anchored to the root"
        );
        assert_eq!(
            rules.visibility(Path::new("/work/forge/src/a.tmp"), false),
            Visibility::Hidden
        );
    }

    #[test]
    fn repository_and_global_excludes_have_lower_precedence() {
        let rules = rules();
        rules.set_repository_excludes(Some("*.local\nsecret/\n"), Some("*.swp\nkeep.log\n"));
        let visibility = |path: &str, is_dir| rules.visibility(Path::new(path), is_dir);
        assert_eq!(
            visibility("/work/forge/config.local", false),
            Visibility::Ignored
        );
        assert_eq!(
            visibility("/work/forge/secret/key", false),
            Visibility::Ignored
        );
        assert_eq!(
            visibility("/work/forge/src/main.rs.swp", false),
            Visibility::Ignored
        );
        assert_eq!(
            visibility("/work/forge/keep.log", false),
            Visibility::Visible,
            ".gitignore whitelist wins"
        );

        rules.set_repository_excludes(None, None);
        assert_eq!(
            visibility("/work/forge/config.local", false),
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
        assert!(rules.is_hidden(Path::new("/work/forge/.git/index")));
        assert!(!rules.is_hidden(Path::new("/work/forge/target/debug")));
    }
}
