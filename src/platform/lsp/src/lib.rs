//! A generic Language Server Protocol client.
//!
//! [`start`] runs a server and gives a [`language::LanguageService`] for
//! it. What differs between servers (the program, options, their own
//! notifications) is an [`LspAdapter`], implemented by each language pack.
//! Nothing here knows a particular language or server.
//!
//! - `transport`: framing and the reader, writer and stderr threads;
//! - `client`: request ids, answers, timeouts, `$/cancelRequest` when a
//!   request future is dropped;
//! - `sync`: positions in UTF-8 or UTF-16, document changes by range;
//! - `service`: startup, restarts, diagnostics with debounce, progress.

mod adapter;
mod client;
mod convert;
mod service;
mod settings;
mod sync;
mod timer;
mod transport;
mod uri;

pub use adapter::{LocationConverter, LspAdapter};
pub use service::{LspService, start};
pub use settings::LspSettings;
pub use transport::ResponseError;
