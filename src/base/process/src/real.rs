//! [`ProcessRunner`] for the local machine.

use std::process::{Child, Stdio};
use std::sync::{Arc, Mutex, PoisonError};

use crate::{
    BoxFuture, ChildProcess, CommandSpec, Output, ProcessControl, ProcessError, ProcessRunner, Pty,
    PtySize, SpawnOptions,
};

/// Windows: start console programs without flashing a console window.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Windows: the priority class below normal.
#[cfg(windows)]
const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;

/// Unix: the niceness of processes started with lowered priority.
#[cfg(unix)]
const LOWERED_NICENESS: &str = "10";

/// Runs programs on this machine.
#[derive(Debug, Default, Clone, Copy)]
pub struct RealProcessRunner;

impl ProcessRunner for RealProcessRunner {
    fn run<'a>(&'a self, command: &'a CommandSpec) -> BoxFuture<'a, Result<Output, ProcessError>> {
        Box::pin(async move {
            let _span = tracing::debug_span!(
                "process.run",
                program = %command.program.to_string_lossy()
            )
            .entered();
            let mut process = std_command(command, SpawnOptions::default());
            process.stdin(Stdio::null());
            let output = process
                .output()
                .map_err(|source| start_error(command, source))?;
            Ok(Output {
                code: output.status.code(),
                stdout: output.stdout,
                stderr: output.stderr,
            })
        })
    }

    fn spawn(
        &self,
        command: &CommandSpec,
        options: SpawnOptions,
    ) -> Result<ChildProcess, ProcessError> {
        let _span = tracing::debug_span!(
            "process.spawn",
            program = %command.program.to_string_lossy()
        )
        .entered();
        let mut process = std_command(command, options);
        process
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = process
            .spawn()
            .map_err(|source| start_error(command, source))?;
        let missing = || ProcessError::Io {
            program: command.program.clone(),
            source: std::io::Error::other("standard streams were not piped"),
        };
        let stdin = child.stdin.take().ok_or_else(missing)?;
        let stdout = child.stdout.take().ok_or_else(missing)?;
        let stderr = child.stderr.take().ok_or_else(missing)?;
        Ok(ChildProcess {
            stdin: Box::new(stdin),
            stdout: Box::new(stdout),
            stderr: Box::new(stderr),
            control: Arc::new(RealControl(Mutex::new(child))),
        })
    }

    fn spawn_pty(&self, command: &CommandSpec, size: PtySize) -> Result<Pty, ProcessError> {
        crate::pty::spawn(command, size)
    }
}

/// The `std` command for `command`. With lowered priority on Unix the
/// program runs under `nice`, resolved first so that a missing program is
/// still reported as not found.
fn std_command(command: &CommandSpec, options: SpawnOptions) -> std::process::Command {
    #[cfg(unix)]
    let mut process = {
        let nice = options
            .lowered_priority
            .then(|| find_in_path("nice".as_ref()))
            .flatten();
        let program = find_in_path(&command.program);
        match (nice, program) {
            (Some(nice), Some(program)) => {
                let mut process = std::process::Command::new(nice);
                process.args(["-n", LOWERED_NICENESS]).arg(program);
                process
            }
            _ => std::process::Command::new(&command.program),
        }
    };
    #[cfg(not(unix))]
    let mut process = std::process::Command::new(&command.program);
    process.args(&command.args);
    if let Some(directory) = &command.current_dir {
        process.current_dir(directory);
    }
    for (key, value) in &command.env {
        match value {
            Some(value) => process.env(key, value),
            None => process.env_remove(key),
        };
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        let priority = if options.lowered_priority {
            BELOW_NORMAL_PRIORITY_CLASS
        } else {
            0
        };
        process.creation_flags(CREATE_NO_WINDOW | priority);
    }
    #[cfg(not(any(unix, windows)))]
    let _ = options;
    process
}

/// Where `program` is: itself if it names a path, otherwise the first
/// executable file of that name in `PATH`.
#[cfg(unix)]
fn find_in_path(program: &std::ffi::OsStr) -> Option<std::path::PathBuf> {
    use std::os::unix::fs::PermissionsExt as _;
    let path = std::path::Path::new(program);
    if path.components().count() > 1 {
        return Some(path.to_path_buf());
    }
    let executable = |candidate: &std::path::Path| {
        std::fs::metadata(candidate)
            .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
    };
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|directory| directory.join(program))
        .find(|candidate| executable(candidate))
}

fn start_error(command: &CommandSpec, source: std::io::Error) -> ProcessError {
    if source.kind() == std::io::ErrorKind::NotFound {
        ProcessError::NotFound {
            program: command.program.clone(),
        }
    } else {
        ProcessError::Io {
            program: command.program.clone(),
            source,
        }
    }
}

/// A child of this process.
struct RealControl(Mutex<Child>);

impl ProcessControl for RealControl {
    fn kill(&self) -> std::io::Result<()> {
        let mut child = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if child.try_wait()?.is_some() {
            return Ok(());
        }
        child.kill()?;
        // Reap it, so it does not linger as a zombie.
        child.wait().map(|_| ())
    }

    fn try_wait(&self) -> std::io::Result<Option<Option<i32>>> {
        let mut child = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        Ok(child.try_wait()?.map(|status| status.code()))
    }
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    use super::*;

    fn ready<T>(future: impl Future<Output = T>) -> T {
        match pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("RealProcessRunner futures complete on first poll"),
        }
    }

    /// A shell command that prints `text` and exits with `code`.
    fn shell(text: &str, code: i32) -> CommandSpec {
        if cfg!(windows) {
            CommandSpec::new("cmd").args(["/C", &format!("echo {text}& exit /b {code}")])
        } else {
            CommandSpec::new("sh").args(["-c", &format!("echo {text}; exit {code}")])
        }
    }

    #[test]
    fn collects_output_and_exit_code() {
        let output = ready(RealProcessRunner.run(&shell("hello", 0))).unwrap();
        assert!(output.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "hello");

        let failed = ready(RealProcessRunner.run(&shell("oops", 3))).unwrap();
        assert_eq!(failed.code, Some(3));
        assert!(!failed.success());
    }

    #[test]
    fn missing_program_is_not_found() {
        let command = CommandSpec::new("definitely-not-a-program-ferrum-test");
        assert!(matches!(
            ready(RealProcessRunner.run(&command)),
            Err(ProcessError::NotFound { .. })
        ));
    }

    #[test]
    fn spawned_processes_stream_through_pipes() {
        use std::io::{Read as _, Write as _};
        // Programs that copy standard input to standard output.
        let command = if cfg!(windows) {
            CommandSpec::new("findstr").arg("^")
        } else {
            CommandSpec::new("cat")
        };
        let options = SpawnOptions {
            lowered_priority: true,
        };
        let mut child = RealProcessRunner.spawn(&command, options).unwrap();
        child.stdin.write_all(b"b\na\n").unwrap();
        drop(child.stdin);
        let mut echoed = String::new();
        child.stdout.read_to_string(&mut echoed).unwrap();
        assert_eq!(echoed.lines().collect::<Vec<_>>(), ["b", "a"]);
        child.control.kill().unwrap();
        assert!(child.control.try_wait().unwrap().is_some());

        let missing = CommandSpec::new("definitely-not-a-program-ferrum-test");
        assert!(matches!(
            RealProcessRunner.spawn(&missing, options),
            Err(ProcessError::NotFound { .. })
        ));
    }

    #[test]
    fn passes_environment_and_directory() {
        let directory = std::env::temp_dir();
        let command = if cfg!(windows) {
            CommandSpec::new("cmd").args(["/C", "echo %PROCESS_TEST%& cd"])
        } else {
            CommandSpec::new("sh").args(["-c", "echo $PROCESS_TEST; pwd"])
        }
        .env("PROCESS_TEST", "value")
        .current_dir(&directory);
        let output = ready(RealProcessRunner.run(&command)).unwrap();
        let text = String::from_utf8_lossy(&output.stdout);
        assert!(text.contains("value"), "{text}");
    }
}
