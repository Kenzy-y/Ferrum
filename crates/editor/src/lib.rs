//! The text editor view: draws only the visible part of a
//! [`text::Document`], takes keyboard, mouse and input method input, and
//! runs the `editor.*` commands.
//!
//! - [`Editor`] — the view entity: document, scroll, cursor blink, file.
//! - [`EditorSettings`] — the `[editor]` settings.
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
mod mouse;
mod scene;
mod settings;
mod viewport;

pub use actions::{CONTEXT, SAVE, is_editor_command, register_commands};
pub use editor::{Editor, EditorEvent, EditorStatus};
pub use settings::{EditorSettings, MultiCursorModifier};
