//! Moving messages to and from a server process: one thread writes, one
//! reads and dispatches, one forwards standard error to the log. Each
//! blocks on its stream, so an idle server costs no CPU.

mod framing;
mod message;

use std::io::{BufRead as _, BufReader, Read, Write};
use std::sync::mpsc;

pub use message::ResponseError;
pub(crate) use message::{
    CONTENT_MODIFIED, Incoming, METHOD_NOT_FOUND, REQUEST_CANCELLED, SERVER_CANCELLED,
    notification, request, response,
};

/// Something for the writer thread to send.
pub(crate) enum WriterJob {
    /// A ready message body.
    Body(Vec<u8>),
    /// A body built on the writer thread, in order with the others:
    /// document changes are converted there, off the UI thread. `None`
    /// sends nothing.
    Build(Box<dyn FnOnce() -> Option<Vec<u8>> + Send>),
}

/// The streams of a server.
pub(crate) struct Streams {
    pub(crate) stdin: Box<dyn Write + Send>,
    pub(crate) stdout: Box<dyn Read + Send>,
    pub(crate) stderr: Box<dyn Read + Send>,
}

/// Starts the threads of a server named `name`. `on_message` runs on the
/// reader thread for every message; `on_closed` once the server's output
/// ends. Dropping every sender of the returned channel closes the
/// server's input.
///
/// # Errors
///
/// Fails if a thread cannot be started.
pub(crate) fn start(
    name: &str,
    streams: Streams,
    mut on_message: impl FnMut(Incoming) + Send + 'static,
    on_closed: impl FnOnce() + Send + 'static,
) -> std::io::Result<mpsc::Sender<WriterJob>> {
    let Streams {
        mut stdin,
        stdout,
        stderr,
    } = streams;
    let (sender, receiver) = mpsc::channel::<WriterJob>();

    let server = name.to_owned();
    std::thread::Builder::new()
        .name(format!("lsp-write-{name}"))
        .spawn(move || {
            for job in receiver {
                let body = match job {
                    WriterJob::Body(body) => body,
                    WriterJob::Build(build) => match build() {
                        Some(body) => body,
                        None => continue,
                    },
                };
                if let Err(error) = framing::write_frame(&mut stdin, &body) {
                    tracing::debug!(%server, %error, "language server input closed");
                    break;
                }
            }
        })?;

    let server = name.to_owned();
    std::thread::Builder::new()
        .name(format!("lsp-read-{name}"))
        .spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                match framing::read_frame(&mut reader) {
                    Ok(Some(body)) => match Incoming::parse(&body) {
                        Ok(message) => on_message(message),
                        Err(error) => tracing::warn!(%server, %error, "bad message from server"),
                    },
                    Ok(None) => break,
                    Err(error) => {
                        tracing::warn!(%server, %error, "cannot read from language server");
                        break;
                    }
                }
            }
            on_closed();
        })?;

    let server = name.to_owned();
    std::thread::Builder::new()
        .name(format!("lsp-stderr-{name}"))
        .spawn(move || {
            for line in BufReader::new(stderr).lines() {
                let Ok(line) = line else { break };
                tracing::debug!(target: "lsp::stderr", %server, "{line}");
            }
        })?;

    Ok(sender)
}
