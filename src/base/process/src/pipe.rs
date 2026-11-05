//! An in-memory pipe: what one side writes, the other reads. Reads block
//! until there is data or the writer is gone. Used by
//! [`crate::FakeProcessRunner`] for scripted servers.

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::sync::{Arc, Condvar, Mutex, PoisonError};

#[derive(Default)]
struct State {
    bytes: VecDeque<u8>,
    closed: bool,
}

#[derive(Default)]
struct Shared {
    state: Mutex<State>,
    ready: Condvar,
}

impl Shared {
    fn close(&self) {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .closed = true;
        self.ready.notify_all();
    }
}

/// The writing end; dropping it ends the stream for the reader.
pub(crate) struct PipeWriter(Arc<Shared>);

/// The reading end; dropping it makes writes fail with `BrokenPipe`.
pub(crate) struct PipeReader(Arc<Shared>);

/// Closes both ends of a pipe from outside, as if the process died.
#[derive(Clone)]
pub(crate) struct PipeCloser(Arc<Shared>);

impl PipeCloser {
    pub(crate) fn close(&self) {
        self.0.close();
    }
}

/// A new pipe.
pub(crate) fn pipe() -> (PipeWriter, PipeReader, PipeCloser) {
    let shared = Arc::new(Shared::default());
    (
        PipeWriter(shared.clone()),
        PipeReader(shared.clone()),
        PipeCloser(shared),
    )
}

impl Write for PipeWriter {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        let mut state = self.0.state.lock().unwrap_or_else(PoisonError::into_inner);
        if state.closed {
            return Err(std::io::ErrorKind::BrokenPipe.into());
        }
        state.bytes.extend(buffer);
        self.0.ready.notify_all();
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Drop for PipeWriter {
    fn drop(&mut self) {
        self.0.close();
    }
}

impl Read for PipeReader {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let mut state = self.0.state.lock().unwrap_or_else(PoisonError::into_inner);
        while state.bytes.is_empty() && !state.closed {
            state = self
                .0
                .ready
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
        let count = buffer.len().min(state.bytes.len());
        for (slot, byte) in buffer.iter_mut().zip(state.bytes.drain(..count)) {
            *slot = byte;
        }
        Ok(count)
    }
}

impl Drop for PipeReader {
    fn drop(&mut self) {
        self.0.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_what_was_written_then_ends() {
        let (mut writer, mut reader, _) = pipe();
        writer.write_all(b"hello").unwrap();
        drop(writer);
        let mut text = String::new();
        reader.read_to_string(&mut text).unwrap();
        assert_eq!(text, "hello");
    }

    #[test]
    fn a_read_waits_for_another_thread() {
        let (mut writer, mut reader, _) = pipe();
        let thread = std::thread::spawn(move || {
            let mut buffer = [0; 3];
            reader.read_exact(&mut buffer).unwrap();
            buffer
        });
        writer.write_all(b"abc").unwrap();
        assert_eq!(&thread.join().unwrap(), b"abc");
    }

    #[test]
    fn closing_fails_writes() {
        let (mut writer, _reader, closer) = pipe();
        closer.close();
        assert!(writer.write(b"x").is_err());
    }
}
