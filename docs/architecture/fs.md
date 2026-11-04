# File system

Crate `fs` (core layer, no GPUI). The only place that touches the file
system; everything else uses `Arc<dyn Fs>`.

**Operations.** `read_to_string`, `read`, `write`, `create_dir_all`,
`metadata`, `read_dir`, `canonicalize` return `BoxFuture` (object-safe, ready
for a remote implementation); `watch` returns a `Watcher`. Errors are
`FsError` (`NotFound`, `InvalidUtf8`, `WrongKind`, `Watch`, `Io`).

**Writes are atomic.** Contents go to a temporary file in the target's
directory, which is synced, given the target's permissions and renamed over
it. If the path is a symbolic link (or a chain), the final target is
written and the link stays. Leftover temporary files are removed on error.

**Metadata.** `Metadata.kind` describes the target of a link,
`is_symlink` the path itself; a dangling link is `EntryKind::Other`.
`canonicalize` resolves links and is how tree walkers detect link cycles.

**Watching.** `watch(root, mode, filter)` watches with `notify`, either
recursively or only the direct entries of `root` (`WatchMode`). The
`WatchFilter` runs on the watcher thread, so events under ignored paths
(build output) are dropped before they are queued. A debounce thread
collects events for `WATCH_DEBOUNCE` (50 ms) and sends deduplicated batches
of `FsEvent` (`Created`, `Changed`, `Removed`, `Rescan`) on a `futures`
stream. Dropping the `Watcher` stops everything.

**Implementations.** `RealFs` (blocking I/O inside the futures: poll on a
background executor) and `FakeFs` (in memory, with symlinks, read-only files
and immediate watch events) for tests.
