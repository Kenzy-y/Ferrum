# Processes

Crate `process` (core layer, no GPUI). The only place that starts programs.

**Key types.** `CommandSpec` (program, arguments, working directory,
environment), `Output` (exit code, stdout, stderr), `ProcessError`
(`NotFound`, `Io`), the object-safe `ProcessRunner` trait with `run`.

**Implementations.** `RealProcessRunner` (blocking inside the future: poll on
a background executor; on Windows no console window is shown) and
`FakeProcessRunner` (canned outputs, records commands) for tests.

**Extending.** Long-running processes with streamed output and lowered
priority (language servers, stage 4; terminal, stage 6) are added as new
trait methods.
