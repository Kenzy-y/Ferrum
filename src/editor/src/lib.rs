//! The text editor view: draws only the visible part of a
//! [`text::Document`], takes keyboard, mouse and input method input, and
//! runs the `editor.*` commands.
//!
//! - [`Editor`] — the view entity: document, scroll, cursor blink, file.
//! - [`EditorSettings`] — the `[editor]` settings.
//! - [`Editor::set_grammar`] — highlighting and the outline from a
//!   `syntax` grammar; the tree follows every edit in the background.
//! - [`register_commands`] — the editor commands for the registry; the
//!   application routes them to the focused editor with
//!   [`Editor::run_command`].

mod actions;
mod display;
mod editor;
mod element;
mod file;
mod input;
mod layout;
mod minimap;
mod mouse;
mod runs;
mod scene;
mod scrollbar;
mod settings;
mod shaping;
mod syntax_patch;
mod syntax_state;
mod viewport;

pub use actions::{
    CONTEXT, KEEP_LOCAL_CHANGES, RELOAD_FROM_DISK, SAVE, init, is_editor_command, register_commands,
};
pub use editor::{Editor, EditorEvent, EditorStatus};
pub use file::DiskState;
pub use settings::{EditorSettings, MultiCursorModifier};
