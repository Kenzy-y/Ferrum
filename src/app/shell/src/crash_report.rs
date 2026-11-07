//! Keeps the reason of a crash. A panic inside a window callback aborts
//! the process at once (Windows reports `0xc0000409`) and its message is
//! lost without a console, so a hook logs it with a backtrace and writes
//! it to `last-crash.txt` in the data directory.

use std::backtrace::Backtrace;
use std::path::PathBuf;

use fs::{Fs as _, RealFs};

/// File in the data directory with the last panic.
pub(crate) const CRASH_FILE_NAME: &str = "last-crash.txt";

/// Installs the panic hook; the default hook still runs after it.
pub(crate) fn install(data_dir: Option<PathBuf>) {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        let report = format!(
            "{product} {version} panicked on thread `{thread}`:\n{info}\n\n{backtrace}\n",
            product = app_identity::PRODUCT_NAME,
            version = env!("CARGO_PKG_VERSION"),
            thread = thread.name().unwrap_or("unnamed"),
            backtrace = Backtrace::force_capture(),
        );
        tracing::error!(target: "panic", "{report}");
        if let Some(data_dir) = &data_dir {
            let path = data_dir.join(CRASH_FILE_NAME);
            // The process may be about to abort: write synchronously.
            let written = futures::executor::block_on(async {
                RealFs.create_dir_all(data_dir).await?;
                RealFs.write(&path, report.as_bytes()).await
            });
            if let Err(error) = written {
                tracing::error!(%error, path = %path.display(), "cannot save the crash report");
            }
        }
        default_hook(info);
    }));
}
