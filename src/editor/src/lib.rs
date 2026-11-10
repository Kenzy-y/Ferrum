//! The text editor view: draws only the visible part of a
//! [`text::Document`], takes keyboard, mouse and input method input, and
//! runs the `editor.*` commands.
//!
//! - [`Editor`] — the view entity: document, scroll, cursor blink, file.
//! - [`EditorSettings`] — the `[editor]` settings.
//! - [`Editor::set_grammar`] — highlighting and the outline from a
//!   `syntax` grammar; the tree follows every edit in the background.
//! - [`register_commands`] — the editor commands for the registry;
//!   [`command_ids`] lists them so the workbench can route each one to
//!   the active editor with [`Editor::run_command`].

mod actions;
mod decorations;
mod editor;
mod file;
mod find;
mod highlighting;
mod input;
mod language;
mod render;
mod settings;

pub use actions::{
    CONTEXT, KEEP_LOCAL_CHANGES, RELOAD_FROM_DISK, SAVE, command_ids, init, is_editor_command,
    register_commands,
};
pub use editor::{Editor, EditorEvent, EditorStatus};
pub use file::DiskState;
pub use find::{FindSummary, MAX_MATCHES};
pub use settings::{EditorSettings, MultiCursorModifier};
