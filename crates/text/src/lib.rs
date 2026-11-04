//! Text buffers: a rope with edits, undo and redo, and multiple selections.
//!
//! - [`Buffer`] — the text (a `ropey` rope) with its line ending and BOM.
//!   Positions are byte offsets; [`Point`] is a row and a byte column.
//! - [`Document`] — a buffer with [`Selections`] and a [`History`]: every
//!   edit applies at all cursors at once, is recorded as one transaction and
//!   can be undone and redone with the selections it had.
//! - [`load`] / [`save`] read and write UTF-8 files through `fs::Fs`.
//!
//! Every operation on a keystroke is logarithmic in the document size; only
//! loading and saving touch the whole text (on a background executor).

mod boundaries;
mod buffer;
mod document;
mod history;
mod io;
mod selection;

pub use boundaries::{next_grapheme, next_word_end, previous_grapheme, previous_word_start};
pub use buffer::{Buffer, LineEnding, LineEndings, Point};
pub use document::{Clock, Document, Movement};
pub use history::{EditKind, History, UNDO_GROUP_PAUSE};
pub use io::{TextError, load, save};
pub use selection::{Selection, Selections};

/// The rope type, for consumers that read text in chunks (rendering,
/// parsing) without copying.
pub use ropey;
