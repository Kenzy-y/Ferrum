//! Commands and keybindings.
//!
//! Every user action is a command with a stable [`CommandId`] of the form
//! `area.action`. The palette, keybindings, menus and plugins refer to
//! actions only by id. The [`CommandRegistry`] holds what is known about
//! each command; running a command is the job of the UI layer, which maps
//! ids to handlers.
//!
//! Keybindings come from `keymap.toml` files whose tables are contexts
//! (`global`, `editor`, `terminal`, ...). See [`Keymap`].

mod id;
mod keymap;
mod keystroke;
mod matcher;
mod registry;

pub use id::{CommandId, InvalidCommandId};
pub use keymap::{Binding, ContextName, GLOBAL_CONTEXT, Keymap, KeymapDiagnostic, KeymapError};
pub use keystroke::{KeySequence, Keystroke, KeystrokeError, Modifiers, Platform};
pub use matcher::{KeymapMatcher, MatchResult};
pub use registry::{CommandDescriptor, CommandRegistry, RegistryError};
