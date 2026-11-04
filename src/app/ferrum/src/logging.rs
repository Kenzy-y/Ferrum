//! Logging setup.

use tracing_subscriber::EnvFilter;

/// Log filter used when `RUST_LOG` is not set.
const DEFAULT_FILTER: &str = "info";

/// Installs the global `tracing` subscriber. The filter comes from the
/// `RUST_LOG` environment variable (for example `RUST_LOG=debug` or
/// `RUST_LOG=settings=trace`).
pub(crate) fn init() {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .init();
}
