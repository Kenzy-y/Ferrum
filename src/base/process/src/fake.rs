//! [`ProcessRunner`] with canned results, for tests.

use std::collections::HashMap;
use std::ffi::OsString;
use std::sync::{Mutex, PoisonError};

use crate::{BoxFuture, CommandSpec, Output, ProcessError, ProcessRunner};

/// Answers each program with a prepared [`Output`] and records every
/// command it was asked to run. Unknown programs are "not found".
#[derive(Debug, Default)]
pub struct FakeProcessRunner {
    outputs: Mutex<HashMap<OsString, Output>>,
    runs: Mutex<Vec<CommandSpec>>,
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

    /// Commands run so far, in order.
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
}
