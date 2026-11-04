//! The IDE application binary: sets up logging and runs the shell. All
//! logic lives in the library crates, starting with `shell`.

// Release builds on Windows are GUI applications without a console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod logging;

fn main() -> anyhow::Result<()> {
    logging::init();
    tracing::info!(
        product = app_identity::PRODUCT_NAME,
        version = env!("CARGO_PKG_VERSION"),
        "starting"
    );
    shell::run()
}
