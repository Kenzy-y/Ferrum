//! Shells a terminal can run, found on this machine.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use fs::Fs;

/// A shell the user can pick for a new terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellProfile {
    /// Name shown in the shell picker ("PowerShell", "bash").
    pub name: String,
    /// The program.
    pub program: PathBuf,
    /// Arguments it starts with.
    pub args: Vec<String>,
}

impl ShellProfile {
    /// A profile named after the file name of `program`.
    pub fn from_program(program: impl Into<PathBuf>, args: Vec<String>) -> Self {
        let program = program.into();
        let name = program
            .file_stem()
            .map_or_else(|| program.to_string_lossy(), |stem| stem.to_string_lossy())
            .into_owned();
        Self {
            name,
            program,
            args,
        }
    }
}

/// The environment variables shell detection reads; [`Self::current`] for
/// this process, anything for tests.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShellEnvironment {
    /// `PATH`.
    pub path: Option<OsString>,
    /// `SHELL` (Unix): the user's login shell.
    pub shell: Option<OsString>,
    /// `SystemRoot` (Windows).
    pub system_root: Option<OsString>,
    /// `ProgramFiles` (Windows).
    pub program_files: Option<OsString>,
    /// `ComSpec` (Windows): the command interpreter.
    pub comspec: Option<OsString>,
}

impl ShellEnvironment {
    /// The variables of this process.
    pub fn current() -> Self {
        Self {
            path: std::env::var_os("PATH"),
            shell: std::env::var_os("SHELL"),
            system_root: std::env::var_os("SystemRoot"),
            program_files: std::env::var_os("ProgramFiles"),
            comspec: std::env::var_os("ComSpec"),
        }
    }
}

/// Which family of shells to look for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellPlatform {
    /// PowerShell, cmd, Git Bash, WSL.
    Windows,
    /// `$SHELL` and `/etc/shells`, started as login shells.
    MacOs,
    /// `$SHELL` and `/etc/shells`.
    Linux,
}

impl ShellPlatform {
    /// The platform this build runs on.
    pub fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::Linux
        }
    }
}

/// Programs listed in `/etc/shells` that are not interactive shells.
const NOT_SHELLS: &[&str] = &["nologin", "false", "true", "git-shell"];

/// Shells installed on this machine, the default one first. Reads the file
/// system through `fs`: poll on a background executor.
pub async fn detect(fs: &dyn Fs, environment: &ShellEnvironment) -> Vec<ShellProfile> {
    detect_for(fs, environment, ShellPlatform::current()).await
}

/// [`detect`] for a given platform.
pub async fn detect_for(
    fs: &dyn Fs,
    environment: &ShellEnvironment,
    platform: ShellPlatform,
) -> Vec<ShellProfile> {
    let profiles = match platform {
        ShellPlatform::Windows => windows_shells(fs, environment).await,
        ShellPlatform::MacOs | ShellPlatform::Linux => {
            unix_shells(fs, environment, platform == ShellPlatform::MacOs).await
        }
    };
    tracing::debug!(count = profiles.len(), "detected shells");
    profiles
}

/// The profile for `settings.shell`: a detected profile of that name
/// (ignoring case), or else the program at that path with `args`. Empty:
/// the first detected profile.
pub fn choose(profiles: &[ShellProfile], shell: &str, args: &[String]) -> Option<ShellProfile> {
    let shell = shell.trim();
    if shell.is_empty() {
        return profiles.first().cloned();
    }
    profiles
        .iter()
        .find(|profile| profile.name.eq_ignore_ascii_case(shell))
        .cloned()
        .or_else(|| Some(ShellProfile::from_program(shell, args.to_vec())))
}

async fn is_file(fs: &dyn Fs, path: &Path) -> bool {
    matches!(fs.metadata(path).await, Ok(Some(metadata)) if metadata.is_file())
}

/// The first file named `name` in the directories of `PATH`.
async fn find_in_path(fs: &dyn Fs, environment: &ShellEnvironment, name: &str) -> Option<PathBuf> {
    let path = environment.path.as_ref()?;
    for directory in std::env::split_paths(path) {
        let candidate = directory.join(name);
        if is_file(fs, &candidate).await {
            return Some(candidate);
        }
    }
    None
}

async fn windows_shells(fs: &dyn Fs, environment: &ShellEnvironment) -> Vec<ShellProfile> {
    let system_root = environment
        .system_root
        .as_ref()
        .map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
    let system32 = system_root.join("System32");
    let program_files = environment
        .program_files
        .as_ref()
        .map_or_else(|| PathBuf::from(r"C:\Program Files"), PathBuf::from);
    let mut profiles = Vec::new();

    // PowerShell 7 from PATH, or from its default install folder.
    let pwsh = if let Some(path) = find_in_path(fs, environment, "pwsh.exe").await {
        Some(path)
    } else {
        let path = program_files.join("PowerShell").join("7").join("pwsh.exe");
        is_file(fs, &path).await.then_some(path)
    };
    let candidates = [
        ("PowerShell", pwsh, Vec::new()),
        (
            "Windows PowerShell",
            Some(
                system32
                    .join("WindowsPowerShell")
                    .join("v1.0")
                    .join("powershell.exe"),
            ),
            Vec::new(),
        ),
        (
            "cmd",
            Some(
                environment
                    .comspec
                    .as_ref()
                    .map_or_else(|| system32.join("cmd.exe"), PathBuf::from),
            ),
            Vec::new(),
        ),
        (
            "Git Bash",
            Some(program_files.join("Git").join("bin").join("bash.exe")),
            vec!["--login".to_owned(), "-i".to_owned()],
        ),
        ("WSL", Some(system32.join("wsl.exe")), Vec::new()),
    ];
    for (name, program, args) in candidates {
        let Some(program) = program else { continue };
        if is_file(fs, &program).await {
            profiles.push(ShellProfile {
                name: name.to_owned(),
                program,
                args,
            });
        }
    }
    profiles
}

async fn unix_shells(
    fs: &dyn Fs,
    environment: &ShellEnvironment,
    login: bool,
) -> Vec<ShellProfile> {
    let args = || {
        if login {
            vec!["-l".to_owned()]
        } else {
            Vec::new()
        }
    };
    let mut programs: Vec<PathBuf> = Vec::new();
    if let Some(shell) = &environment.shell {
        programs.push(PathBuf::from(shell));
    }
    if let Ok(listed) = fs.read_to_string(Path::new("/etc/shells")).await {
        programs.extend(
            listed
                .lines()
                .map(str::trim)
                .filter(|line| line.starts_with('/'))
                .map(PathBuf::from),
        );
    }
    programs.push(PathBuf::from("/bin/sh"));

    let mut profiles: Vec<ShellProfile> = Vec::new();
    for program in programs {
        let profile = ShellProfile::from_program(&program, args());
        // `/bin/bash` and `/usr/bin/bash` are the same shell for the user.
        let duplicate = profiles.iter().any(|known| known.name == profile.name);
        if duplicate || NOT_SHELLS.contains(&profile.name.as_str()) {
            continue;
        }
        if is_file(fs, &program).await {
            profiles.push(profile);
        }
    }
    profiles
}

#[cfg(test)]
mod tests {
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    use fs::FakeFs;

    use super::*;

    fn ready<T>(future: impl Future<Output = T>) -> T {
        match pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("FakeFs futures complete on first poll"),
        }
    }

    fn names(profiles: &[ShellProfile]) -> Vec<&str> {
        profiles
            .iter()
            .map(|profile| profile.name.as_str())
            .collect()
    }

    #[test]
    fn windows_lists_installed_shells_powershell_first() {
        let fs = FakeFs::new();
        let windows = PathBuf::from(r"C:\Windows");
        let system32 = windows.join("System32");
        let program_files = PathBuf::from(r"C:\Program Files");
        fs.insert_file(
            system32
                .join("WindowsPowerShell")
                .join("v1.0")
                .join("powershell.exe"),
            "",
        );
        fs.insert_file(system32.join("cmd.exe"), "");
        fs.insert_file(
            program_files.join("PowerShell").join("7").join("pwsh.exe"),
            "",
        );
        fs.insert_file(program_files.join("Git").join("bin").join("bash.exe"), "");
        let environment = ShellEnvironment {
            system_root: Some(windows.into()),
            program_files: Some(program_files.into()),
            ..ShellEnvironment::default()
        };
        let profiles = ready(detect_for(&fs, &environment, ShellPlatform::Windows));
        assert_eq!(
            names(&profiles),
            ["PowerShell", "Windows PowerShell", "cmd", "Git Bash"]
        );
        assert_eq!(profiles[3].args, ["--login", "-i"]);
    }

    #[test]
    fn unix_puts_the_login_shell_first_and_skips_non_shells() {
        let fs = FakeFs::new();
        fs.insert_file("/etc/shells", "# comment\n/bin/sh\n/bin/bash\n/usr/bin/bash\n/usr/bin/zsh\n/usr/sbin/nologin\n/bin/fish\n");
        fs.insert_file("/bin/sh", "");
        fs.insert_file("/bin/bash", "");
        fs.insert_file("/usr/bin/bash", "");
        fs.insert_file("/usr/bin/zsh", "");
        fs.insert_file("/usr/sbin/nologin", "");
        let environment = ShellEnvironment {
            shell: Some("/usr/bin/zsh".into()),
            ..ShellEnvironment::default()
        };
        let profiles = ready(detect_for(&fs, &environment, ShellPlatform::Linux));
        assert_eq!(names(&profiles), ["zsh", "sh", "bash"]);
        assert!(profiles[0].args.is_empty());
        let mac = ready(detect_for(&fs, &environment, ShellPlatform::MacOs));
        assert_eq!(mac[0].args, ["-l"]);
    }

    #[test]
    fn the_setting_picks_a_profile_by_name_or_a_program() {
        let profiles = vec![
            ShellProfile::from_program("/bin/zsh", Vec::new()),
            ShellProfile::from_program("/bin/bash", Vec::new()),
        ];
        assert_eq!(choose(&profiles, "", &[]).unwrap().name, "zsh");
        assert_eq!(choose(&profiles, "BASH", &[]).unwrap().name, "bash");
        let custom = choose(&profiles, "/opt/nu/bin/nu", &["-l".to_owned()]).unwrap();
        assert_eq!(custom.name, "nu");
        assert_eq!(custom.args, ["-l"]);
        assert_eq!(choose(&[], "", &[]), None);
    }
}
