# Processes

Crate `process` (core layer, no GPUI). The only place that starts programs.

**Key types.** `CommandSpec` (program, arguments, working directory,
environment), `Output` (exit code, stdout, stderr), `ProcessError`
(`NotFound`, `Io`), the object-safe `ProcessRunner` trait with `run` and
`spawn`. `spawn` starts a long-running process (a language server) with
piped streams: `ChildProcess` has `stdin`, `stdout`, `stderr` (blocking,
one thread each) and a shared `ProcessControl` (`kill`, `try_wait`).
`SpawnOptions::lowered_priority` runs it below normal priority: on Windows
the `BELOW_NORMAL_PRIORITY_CLASS` flag, on Unix `nice -n 10` (the program
is resolved in `PATH` first, so a missing one is still `NotFound`).

**Implementations.** `RealProcessRunner` (blocking inside the future: poll on
a background executor; on Windows no console window is shown) and
`FakeProcessRunner` (canned outputs, records commands) for tests;
`FakeProcessRunner::serve` registers a scripted program that `spawn` runs
on a thread, talking through in-memory pipes (`pipe.rs`).

**Extending.** A pseudo-terminal for the terminal (stage 6) is a further
trait method.
