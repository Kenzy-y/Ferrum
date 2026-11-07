//! Project-wide search.
//!
//! [`run`] walks a folder through [`fs::Fs`] (honouring `.gitignore` files,
//! hidden patterns and "files to include" globs) and matches every text
//! file line by line with `grep-searcher`. Matches arrive per file through
//! a callback, so the caller can show them while the walk goes on. The
//! search is cancelled through an [`AtomicBool`](std::sync::atomic::AtomicBool)
//! checked before every file. Files are read and matched on a few worker
//! threads of the search's own; results still arrive in path order.
//!
//! The walk blocks the polling thread on folder reads: run the search on a
//! background executor.

mod error;
mod preview;
mod query;
mod run;
mod settings;
mod walk;
mod workers;

pub use error::SearchError;
pub use preview::Preview;
pub use query::SearchQuery;
pub use run::{FileMatches, LineMatch, SearchOptions, SearchSummary, run};
pub use settings::SearchSettings;
