//! [`ProcessRunner`] for the local machine.

use crate::{BoxFuture, CommandSpec, Output, ProcessError, ProcessRunner};

/// Windows: start console programs without flashing a console window.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

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
            let mut process = std::process::Command::new(&command.program);
            process
                .args(&command.args)
                .stdin(std::process::Stdio::null());
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
                process.creation_flags(CREATE_NO_WINDOW);
            }
            let output = process.output().map_err(|source| {
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
            })?;
            Ok(Output {
                code: output.status.code(),
                stdout: output.stdout,
                stderr: output.stderr,
            })
        })
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
