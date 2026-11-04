//! Recently opened projects, kept in the data directory.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use fs::Fs;
use serde::{Deserialize, Serialize};

use crate::workspace::display_path;
use crate::{ProjectError, Workspace};

/// File name of the list in the data directory.
pub const RECENT_PROJECTS_FILE_NAME: &str = "recent-projects.toml";

/// One remembered folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentProject {
    /// The folder.
    pub path: PathBuf,
    /// When it was last opened, in seconds since the Unix epoch.
    pub opened: u64,
}

impl RecentProject {
    /// Name shown to the user: the folder name.
    pub fn name(&self) -> String {
        Workspace::from_root(self.path.clone()).name().to_owned()
    }

    /// The folder for display.
    pub fn display_path(&self) -> String {
        display_path(&self.path)
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct RecentFile {
    #[serde(default, rename = "project")]
    projects: Vec<RecentProject>,
}

/// Recently opened folders, most recent first, at most `limit` of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentProjects {
    projects: Vec<RecentProject>,
    limit: usize,
}

impl RecentProjects {
    /// An empty list.
    pub fn new(limit: usize) -> Self {
        Self {
            projects: Vec::new(),
            limit,
        }
    }

    /// Reads the list from `file`. A missing or damaged file gives an empty
    /// list (logged): losing history must never stop the IDE.
    pub async fn load(fs: &dyn Fs, file: &Path, limit: usize) -> Self {
        let mut recent = Self::new(limit);
        match fs.read_to_string(file).await {
            Ok(text) => match toml::from_str::<RecentFile>(&text) {
                Ok(parsed) => {
                    recent.projects = parsed.projects;
                    recent
                        .projects
                        .sort_by_key(|project| std::cmp::Reverse(project.opened));
                    recent.projects.truncate(limit);
                }
                Err(error) => {
                    tracing::warn!(path = %file.display(), %error, "recent projects file is damaged");
                }
            },
            Err(error) if error.is_not_found() => {}
            Err(error) => tracing::warn!(%error, "cannot read recent projects"),
        }
        recent
    }

    /// Writes the list to `file`, creating the directory if needed.
    ///
    /// # Errors
    ///
    /// Fails if the file cannot be written.
    pub async fn save(&self, fs: &dyn Fs, file: &Path) -> Result<(), ProjectError> {
        let text = toml::to_string(&RecentFile {
            projects: self.projects.clone(),
        })
        .map_err(|error| ProjectError::Serialize(error.to_string()))?;
        if let Some(directory) = file.parent() {
            fs.create_dir_all(directory).await?;
        }
        fs.write(file, text.as_bytes()).await?;
        Ok(())
    }

    /// Moves `root` to the front (or adds it), stamped with `now`.
    pub fn record(&mut self, root: &Path, now: SystemTime) {
        self.remove(root);
        let opened = now
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_secs());
        self.projects.insert(
            0,
            RecentProject {
                path: root.to_path_buf(),
                opened,
            },
        );
        self.projects.truncate(self.limit);
    }

    /// Forgets `root`.
    pub fn remove(&mut self, root: &Path) {
        self.projects.retain(|project| project.path != root);
    }

    /// The folders, most recent first.
    pub fn projects(&self) -> &[RecentProject] {
        &self.projects
    }
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};
    use std::time::Duration;

    use fs::FakeFs;

    use super::*;

    fn ready<T>(future: impl Future<Output = T>) -> T {
        match pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("FakeFs futures complete immediately"),
        }
    }

    fn at(seconds: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(seconds)
    }

    #[test]
    fn keeps_most_recent_first_within_the_limit() {
        let mut recent = RecentProjects::new(2);
        recent.record(Path::new("/a"), at(1));
        recent.record(Path::new("/b"), at(2));
        recent.record(Path::new("/a"), at(3));
        recent.record(Path::new("/c"), at(4));

        let paths: Vec<_> = recent
            .projects()
            .iter()
            .map(|project| project.path.clone())
            .collect();
        assert_eq!(paths, [PathBuf::from("/c"), PathBuf::from("/a")]);
        assert_eq!(recent.projects()[1].opened, 3);
    }

    #[test]
    fn saves_and_loads() {
        let fs = FakeFs::new();
        let file = Path::new("/data/recent-projects.toml");
        let mut recent = RecentProjects::new(10);
        recent.record(Path::new("/work/forge"), at(10));
        recent.record(Path::new("/work/anvil"), at(20));
        ready(recent.save(&fs, file)).unwrap();

        let loaded = ready(RecentProjects::load(&fs, file, 10));
        assert_eq!(loaded, recent);
        assert_eq!(loaded.projects()[0].name(), "anvil");

        let limited = ready(RecentProjects::load(&fs, file, 1));
        assert_eq!(limited.projects().len(), 1);
    }

    #[test]
    fn damaged_or_missing_files_give_an_empty_list() {
        let fs = FakeFs::new();
        let file = Path::new("/data/recent-projects.toml");
        assert!(
            ready(RecentProjects::load(&fs, file, 10))
                .projects()
                .is_empty()
        );

        fs.insert_file(file, "[[project]\npath = 1");
        assert!(
            ready(RecentProjects::load(&fs, file, 10))
                .projects()
                .is_empty()
        );
    }
}
