//! Exclude files of git outside the working tree: `.git/info/exclude` and
//! the global excludes file (`core.excludesFile`). Read through `fs::Fs`,
//! so they work with a remote file system too.

use std::path::{Path, PathBuf};

use fs::Fs;

/// Where git looks for user configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GitEnvironment {
    /// The home directory (`~`).
    pub home: Option<PathBuf>,
    /// `$XDG_CONFIG_HOME`, if set to an absolute path.
    pub xdg_config_home: Option<PathBuf>,
}

impl GitEnvironment {
    /// The environment of this machine.
    pub fn from_platform() -> Self {
        Self {
            home: directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_path_buf()),
            xdg_config_home: std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .filter(|path| path.is_absolute()),
        }
    }

    /// `$XDG_CONFIG_HOME`, or `~/.config` as git defaults to.
    fn config_home(&self) -> Option<PathBuf> {
        self.xdg_config_home
            .clone()
            .or_else(|| self.home.as_ref().map(|home| home.join(".config")))
    }

    /// Expands a leading `~/` like git does.
    fn expand(&self, path: &str) -> Option<PathBuf> {
        match path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\")) {
            Some(rest) => self.home.as_ref().map(|home| home.join(rest)),
            None => Some(PathBuf::from(path)),
        }
    }
}

/// Contents of the exclude files; `None` when a file does not exist.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepositoryExcludes {
    /// `.git/info/exclude` of the workspace.
    pub info_exclude: Option<String>,
    /// The global excludes file.
    pub global: Option<String>,
}

/// Reads the exclude files that apply to the workspace at `root`.
pub async fn read_repository_excludes(
    fs: &dyn Fs,
    root: &Path,
    environment: &GitEnvironment,
) -> RepositoryExcludes {
    let info_exclude = fs
        .read_to_string(&root.join(".git").join("info").join("exclude"))
        .await
        .ok();
    let global = match global_excludes_path(fs, environment).await {
        Some(path) => fs.read_to_string(&path).await.ok(),
        None => None,
    };
    RepositoryExcludes {
        info_exclude,
        global,
    }
}

/// The global excludes file: `core.excludesFile` from the user
/// configuration (`~/.gitconfig` wins over `$XDG_CONFIG_HOME/git/config`),
/// or git's default `$XDG_CONFIG_HOME/git/ignore`.
async fn global_excludes_path(fs: &dyn Fs, environment: &GitEnvironment) -> Option<PathBuf> {
    let config_home = environment.config_home();
    let configs = [
        config_home
            .as_ref()
            .map(|home| home.join("git").join("config")),
        environment
            .home
            .as_ref()
            .map(|home| home.join(".gitconfig")),
    ];
    let mut configured = None;
    for config in configs.into_iter().flatten() {
        if let Ok(text) = fs.read_to_string(&config).await
            && let Some(value) = excludes_file_setting(&text)
        {
            configured = Some(value);
        }
    }
    match configured {
        Some(value) => environment.expand(&value),
        None => config_home.map(|home| home.join("git").join("ignore")),
    }
}

/// The value of `core.excludesFile` in a git config file, if set (the last
/// one wins, as in git). Section and key names are case-insensitive.
fn excludes_file_setting(config: &str) -> Option<String> {
    let mut in_core = false;
    let mut value = None;
    for line in config.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.starts_with(';') || line.is_empty() {
            continue;
        }
        if let Some(section) = line.strip_prefix('[') {
            let name = section.trim_end_matches(']').trim();
            in_core = name.eq_ignore_ascii_case("core");
            continue;
        }
        if !in_core {
            continue;
        }
        if let Some((key, raw)) = line.split_once('=')
            && key.trim().eq_ignore_ascii_case("excludesfile")
        {
            let raw = raw.trim();
            let unquoted = raw
                .strip_prefix('"')
                .and_then(|rest| rest.strip_suffix('"'))
                .unwrap_or(raw);
            value = Some(unquoted.to_owned());
        }
    }
    value
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    use fs::FakeFs;

    use super::*;

    fn ready<T>(future: impl Future<Output = T>) -> T {
        match pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("FakeFs futures complete immediately"),
        }
    }

    fn environment() -> GitEnvironment {
        GitEnvironment {
            home: Some(PathBuf::from("/home/me")),
            xdg_config_home: None,
        }
    }

    #[test]
    fn parses_core_excludes_file() {
        let config = "[user]\n\tname = Me\n[Core]\n\texcludesFile = \"~/.globalignore\"\n[alias]\n\texcludesfile = no\n";
        assert_eq!(
            excludes_file_setting(config),
            Some("~/.globalignore".to_owned())
        );
        assert_eq!(excludes_file_setting("[core]\n# excludesfile = x\n"), None);
    }

    #[test]
    fn reads_info_exclude_and_configured_global_file() {
        let fs = FakeFs::new();
        fs.insert_file("/work/forge/.git/info/exclude", "*.local\n");
        fs.insert_file(
            "/home/me/.config/git/config",
            "[core]\nexcludesfile = ~/.xdgignore\n",
        );
        fs.insert_file(
            "/home/me/.gitconfig",
            "[core]\nexcludesfile = ~/.globalignore\n",
        );
        fs.insert_file("/home/me/.globalignore", "*.swp\n");

        let excludes = ready(read_repository_excludes(
            &fs,
            Path::new("/work/forge"),
            &environment(),
        ));
        assert_eq!(excludes.info_exclude.as_deref(), Some("*.local\n"));
        assert_eq!(
            excludes.global.as_deref(),
            Some("*.swp\n"),
            "~/.gitconfig wins"
        );
    }

    #[test]
    fn falls_back_to_the_default_global_file() {
        let fs = FakeFs::new();
        fs.insert_file("/home/me/.config/git/ignore", ".DS_Store\n");
        let excludes = ready(read_repository_excludes(
            &fs,
            Path::new("/work/forge"),
            &environment(),
        ));
        assert_eq!(excludes.info_exclude, None);
        assert_eq!(excludes.global.as_deref(), Some(".DS_Store\n"));
    }
}
