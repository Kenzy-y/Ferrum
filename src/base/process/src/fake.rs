//! [`ProcessRunner`] with canned results, for tests.

use std::collections::HashMap;
use std::ffi::OsString;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use crate::pipe::{PipeCloser, pipe};
use crate::{
    BoxFuture, ChildProcess, CommandSpec, Output, ProcessControl, ProcessError, ProcessRunner, Pty,
    PtySize, SpawnOptions,
};

/// A scripted long-running program: called on its own thread with the
/// program's side of the pipes. The process ends when it returns.
pub type FakeServer = Arc<dyn Fn(ServerSide) + Send + Sync>;

/// The program's side of a fake process.
pub struct ServerSide {
    /// The command it was started with.
    pub command: CommandSpec,
    /// What the IDE writes to the process.
    pub stdin: Box<dyn Read + Send>,
    /// What the IDE reads from the process.
    pub stdout: Box<dyn Write + Send>,
    /// The process's error stream.
    pub stderr: Box<dyn Write + Send>,
}

/// Answers each program with a prepared [`Output`] and records every
/// command it was asked to run. Unknown programs are "not found".
/// Programs registered with [`FakeProcessRunner::serve`] can be spawned.
#[derive(Default)]
pub struct FakeProcessRunner {
    outputs: Mutex<HashMap<OsString, Output>>,
    servers: Mutex<HashMap<OsString, FakeServer>>,
    runs: Mutex<Vec<CommandSpec>>,
}

impl std::fmt::Debug for FakeProcessRunner {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FakeProcessRunner")
            .finish_non_exhaustive()
    }
}

impl FakeProcessRunner {
    /// Creates a runner that knows no programs.
    pub fn new() -> Self {
        Self::default()
    }

    /// Makes every run of `program` return `output`.
    pub fn respond(&self, program: impl Into<OsString>, output: Output) {
        self.outputs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(program.into(), output);
    }

    /// Makes every spawn of `program` start `server` on a new thread.
    pub fn serve(&self, program: impl Into<OsString>, server: FakeServer) {
        self.servers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(program.into(), server);
    }

    /// Commands run or spawned so far, in order.
    pub fn runs(&self) -> Vec<CommandSpec> {
        self.runs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl ProcessRunner for FakeProcessRunner {
    fn run<'a>(&'a self, command: &'a CommandSpec) -> BoxFuture<'a, Result<Output, ProcessError>> {
        self.runs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(command.clone());
        let result = self
            .outputs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(command.program())
            .cloned()
            .ok_or_else(|| ProcessError::NotFound {
                program: command.program().to_owned(),
            });
        Box::pin(async move { result })
    }

    fn spawn(
        &self,
        command: &CommandSpec,
        _options: SpawnOptions,
    ) -> Result<ChildProcess, ProcessError> {
        self.runs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(command.clone());
        let server = self
            .servers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(command.program())
            .cloned()
            .ok_or_else(|| ProcessError::NotFound {
                program: command.program().to_owned(),
            })?;
        let (stdin_writer, stdin_reader, stdin_closer) = pipe();
        let (stdout_writer, stdout_reader, stdout_closer) = pipe();
        let (stderr_writer, stderr_reader, stderr_closer) = pipe();
        let control = Arc::new(FakeControl {
            pipes: [stdin_closer, stdout_closer, stderr_closer],
            finished: Arc::new(AtomicBool::new(false)),
        });
        let side = ServerSide {
            command: command.clone(),
            stdin: Box::new(stdin_reader),
            stdout: Box::new(stdout_writer),
            stderr: Box::new(stderr_writer),
        };
        let finished = control.finished.clone();
        std::thread::Builder::new()
            .name("fake-process".to_owned())
            .spawn(move || {
                server(side);
                finished.store(true, Ordering::Release);
            })
            .map_err(|source| ProcessError::Io {
                program: command.program().to_owned(),
                source,
            })?;
        Ok(ChildProcess {
            stdin: Box::new(stdin_writer),
            stdout: Box::new(stdout_reader),
            stderr: Box::new(stderr_reader),
            control,
        })
    }

    /// Records the command; fake runners have no pseudo-terminals, so the
    /// terminal is tested by feeding its emulator directly.
    fn spawn_pty(&self, command: &CommandSpec, _size: PtySize) -> Result<Pty, ProcessError> {
        self.runs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(command.clone());
        Err(ProcessError::Io {
            program: command.program().to_owned(),
            source: std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "fake runners have no pseudo-terminals",
            ),
        })
    }
}

/// Kills a fake process by closing its pipes.
struct FakeControl {
    pipes: [PipeCloser; 3],
    finished: Arc<AtomicBool>,
}

impl ProcessControl for FakeControl {
    fn kill(&self) -> std::io::Result<()> {
        for pipe in &self.pipes {
            pipe.close();
        }
        Ok(())
    }

    fn try_wait(&self) -> std::io::Result<Option<Option<i32>>> {
        Ok(self.finished.load(Ordering::Acquire).then_some(Some(0)))
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
            Poll::Pending => panic!("fake futures complete immediately"),
        }
    }

    #[test]
    fn answers_known_programs_and_records_runs() {
        let runner = FakeProcessRunner::new();
        runner.respond(
            "cargo",
            Output {
                code: Some(0),
                stdout: b"cargo 1.98.1".to_vec(),
                stderr: Vec::new(),
            },
        );
        let version = CommandSpec::new("cargo").arg("--version");
        assert!(ready(runner.run(&version)).unwrap().success());
        assert!(ready(runner.run(&CommandSpec::new("git"))).is_err());
        assert_eq!(runner.runs().len(), 2);
        assert_eq!(runner.runs()[0], version);
    }

    #[test]
    fn spawned_servers_talk_through_pipes() {
        let runner = FakeProcessRunner::new();
        runner.serve(
            "echo-server",
            Arc::new(|mut side: ServerSide| {
                let mut line = [0; 4];
                side.stdin.read_exact(&mut line).unwrap();
                side.stdout.write_all(&line).unwrap();
            }),
        );
        let mut child = runner
            .spawn(&CommandSpec::new("echo-server"), SpawnOptions::default())
            .unwrap();
        child.stdin.write_all(b"ping").unwrap();
        let mut answer = String::new();
        child.stdout.read_to_string(&mut answer).unwrap();
        assert_eq!(answer, "ping");
        assert!(
            runner
                .spawn(&CommandSpec::new("missing"), SpawnOptions::default())
                .is_err()
        );
    }

    #[test]
    fn killing_ends_the_streams() {
        let runner = FakeProcessRunner::new();
        runner.serve(
            "sleeper",
            Arc::new(|mut side: ServerSide| {
                let mut rest = Vec::new();
                let _ = side.stdin.read_to_end(&mut rest);
            }),
        );
        let mut child = runner
            .spawn(&CommandSpec::new("sleeper"), SpawnOptions::default())
            .unwrap();
        child.control.kill().unwrap();
        let mut rest = Vec::new();
        assert_eq!(child.stdout.read_to_end(&mut rest).unwrap(), 0);
    }
}
