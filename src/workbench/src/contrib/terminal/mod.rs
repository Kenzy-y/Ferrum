//! The integrated terminal (ADR 0010): terminal tabs in the bottom panel,
//! splits, the shell picker and the terminal commands. Sessions come from
//! the `terminal` crate; the application gives the panel its process
//! runner and settings.

mod element;
mod palette;
mod panel;
mod picker;
mod view;
mod workbench;

pub use panel::{TerminalPanel, TerminalPanelEvent};
pub(crate) use workbench::header_actions;
