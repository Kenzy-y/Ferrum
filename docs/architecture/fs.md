# File system

Crate `fs` (core layer, no GPUI).

**Purpose.** The only place that touches the file system (see ADR 0002).

**Key types.** `Fs` trait (object-safe, methods return `BoxFuture`),
`FsError` (`NotFound`, `InvalidUtf8`, `Io`), `RealFs` (local, blocking inside
the future: poll on a background executor).

**Extending.** Add operations to `Fs` and implement them in `RealFs`; a fake
in-memory implementation is planned for stage 2.
