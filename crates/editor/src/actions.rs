//! Editor commands: their ids, localized titles and what they do.

use commands::{CommandDescriptor, CommandId, CommandRegistry, RegistryError};
use text::{LineEnding, Movement};

/// Keybinding context of a focused editor.
pub const CONTEXT: &str = "editor";

/// Keys the editor's own keymap uses that the component library binds to
/// GPUI actions at the window root (Tab moves the focus there). GPUI runs
/// action bindings before key listeners, so these keys never reached the
/// keymap inside the editor.
const KEYS_TAKEN_BY_THE_ROOT: [&str; 2] = ["tab", "shift-tab"];

/// Turns off the root's action bindings for keys the editor handles itself,
/// so Tab indents instead of moving the focus. Call once at startup, after
/// the component library is initialized.
pub fn init(cx: &mut gpui::App) {
    cx.bind_keys(
        KEYS_TAKEN_BY_THE_ROOT.map(|key| gpui::KeyBinding::new(key, gpui::NoAction, Some(CONTEXT))),
    );
}

/// Saves the file of the active editor.
pub const SAVE: &str = "editor.save";
/// Replaces unsaved edits with the file changed on disk (undoable).
pub const RELOAD_FROM_DISK: &str = "editor.reloadFromDisk";
/// Keeps unsaved edits over a file changed on disk.
pub const KEEP_LOCAL_CHANGES: &str = "editor.keepLocalChanges";

/// What an editor command does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    /// Moves the cursors; `true` extends the selections.
    Move(Motion, bool),
    AddCursorAbove,
    AddCursorBelow,
    /// Escape: back to the primary cursor, selection collapsed.
    SingleCursor,
    SelectAll,
    Undo,
    Redo,
    Copy,
    Cut,
    Paste,
    DeleteLeft,
    DeleteRight,
    DeleteWordLeft,
    DeleteWordRight,
    Newline,
    Indent,
    Outdent,
    Save,
    ReloadFromDisk,
    KeepLocalChanges,
    LineEndings(LineEnding),
}

/// A movement without its page size, which depends on the view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Motion {
    Left,
    Right,
    WordLeft,
    WordRight,
    Up,
    Down,
    PageUp,
    PageDown,
    LineStart,
    LineEnd,
    DocumentStart,
    DocumentEnd,
}

impl Motion {
    /// The text movement; pages are `page_rows` lines.
    pub(crate) fn movement(self, page_rows: usize) -> Movement {
        match self {
            Self::Left => Movement::Left,
            Self::Right => Movement::Right,
            Self::WordLeft => Movement::WordLeft,
            Self::WordRight => Movement::WordRight,
            Self::Up => Movement::Up,
            Self::Down => Movement::Down,
            Self::PageUp => Movement::PageUp(page_rows),
            Self::PageDown => Movement::PageDown(page_rows),
            Self::LineStart => Movement::LineStart,
            Self::LineEnd => Movement::LineEnd,
            Self::DocumentStart => Movement::DocumentStart,
            Self::DocumentEnd => Movement::DocumentEnd,
        }
    }
}

/// Every editor command: id, i18n id of its title, action.
pub(crate) const COMMANDS: &[(&str, &str, Action)] = &[
    (
        "editor.moveLeft",
        "command-editor-move-left",
        Action::Move(Motion::Left, false),
    ),
    (
        "editor.moveRight",
        "command-editor-move-right",
        Action::Move(Motion::Right, false),
    ),
    (
        "editor.moveUp",
        "command-editor-move-up",
        Action::Move(Motion::Up, false),
    ),
    (
        "editor.moveDown",
        "command-editor-move-down",
        Action::Move(Motion::Down, false),
    ),
    (
        "editor.moveWordLeft",
        "command-editor-move-word-left",
        Action::Move(Motion::WordLeft, false),
    ),
    (
        "editor.moveWordRight",
        "command-editor-move-word-right",
        Action::Move(Motion::WordRight, false),
    ),
    (
        "editor.moveToLineStart",
        "command-editor-move-to-line-start",
        Action::Move(Motion::LineStart, false),
    ),
    (
        "editor.moveToLineEnd",
        "command-editor-move-to-line-end",
        Action::Move(Motion::LineEnd, false),
    ),
    (
        "editor.moveToDocumentStart",
        "command-editor-move-to-document-start",
        Action::Move(Motion::DocumentStart, false),
    ),
    (
        "editor.moveToDocumentEnd",
        "command-editor-move-to-document-end",
        Action::Move(Motion::DocumentEnd, false),
    ),
    (
        "editor.pageUp",
        "command-editor-page-up",
        Action::Move(Motion::PageUp, false),
    ),
    (
        "editor.pageDown",
        "command-editor-page-down",
        Action::Move(Motion::PageDown, false),
    ),
    (
        "editor.selectLeft",
        "command-editor-select-left",
        Action::Move(Motion::Left, true),
    ),
    (
        "editor.selectRight",
        "command-editor-select-right",
        Action::Move(Motion::Right, true),
    ),
    (
        "editor.selectUp",
        "command-editor-select-up",
        Action::Move(Motion::Up, true),
    ),
    (
        "editor.selectDown",
        "command-editor-select-down",
        Action::Move(Motion::Down, true),
    ),
    (
        "editor.selectWordLeft",
        "command-editor-select-word-left",
        Action::Move(Motion::WordLeft, true),
    ),
    (
        "editor.selectWordRight",
        "command-editor-select-word-right",
        Action::Move(Motion::WordRight, true),
    ),
    (
        "editor.selectToLineStart",
        "command-editor-select-to-line-start",
        Action::Move(Motion::LineStart, true),
    ),
    (
        "editor.selectToLineEnd",
        "command-editor-select-to-line-end",
        Action::Move(Motion::LineEnd, true),
    ),
    (
        "editor.selectToDocumentStart",
        "command-editor-select-to-document-start",
        Action::Move(Motion::DocumentStart, true),
    ),
    (
        "editor.selectToDocumentEnd",
        "command-editor-select-to-document-end",
        Action::Move(Motion::DocumentEnd, true),
    ),
    (
        "editor.selectPageUp",
        "command-editor-select-page-up",
        Action::Move(Motion::PageUp, true),
    ),
    (
        "editor.selectPageDown",
        "command-editor-select-page-down",
        Action::Move(Motion::PageDown, true),
    ),
    (
        "editor.addCursorAbove",
        "command-editor-add-cursor-above",
        Action::AddCursorAbove,
    ),
    (
        "editor.addCursorBelow",
        "command-editor-add-cursor-below",
        Action::AddCursorBelow,
    ),
    (
        "editor.singleCursor",
        "command-editor-single-cursor",
        Action::SingleCursor,
    ),
    (
        "editor.selectAll",
        "command-editor-select-all",
        Action::SelectAll,
    ),
    ("editor.undo", "command-editor-undo", Action::Undo),
    ("editor.redo", "command-editor-redo", Action::Redo),
    ("editor.copy", "command-editor-copy", Action::Copy),
    ("editor.cut", "command-editor-cut", Action::Cut),
    ("editor.paste", "command-editor-paste", Action::Paste),
    (
        "editor.deleteLeft",
        "command-editor-delete-left",
        Action::DeleteLeft,
    ),
    (
        "editor.deleteRight",
        "command-editor-delete-right",
        Action::DeleteRight,
    ),
    (
        "editor.deleteWordLeft",
        "command-editor-delete-word-left",
        Action::DeleteWordLeft,
    ),
    (
        "editor.deleteWordRight",
        "command-editor-delete-word-right",
        Action::DeleteWordRight,
    ),
    ("editor.newline", "command-editor-newline", Action::Newline),
    ("editor.indent", "command-editor-indent", Action::Indent),
    ("editor.outdent", "command-editor-outdent", Action::Outdent),
    (SAVE, "command-editor-save", Action::Save),
    (
        RELOAD_FROM_DISK,
        "command-editor-reload-from-disk",
        Action::ReloadFromDisk,
    ),
    (
        KEEP_LOCAL_CHANGES,
        "command-editor-keep-local-changes",
        Action::KeepLocalChanges,
    ),
    (
        "editor.convertLineEndingsToLf",
        "command-editor-line-endings-lf",
        Action::LineEndings(LineEnding::Lf),
    ),
    (
        "editor.convertLineEndingsToCrlf",
        "command-editor-line-endings-crlf",
        Action::LineEndings(LineEnding::CrLf),
    ),
];

/// The action of command `id`, if it is an editor command.
pub(crate) fn action_for(id: &str) -> Option<Action> {
    COMMANDS
        .iter()
        .find(|(command, _, _)| *command == id)
        .map(|(_, _, action)| *action)
}

/// Whether `id` is an editor command.
pub fn is_editor_command(id: &str) -> bool {
    action_for(id).is_some()
}

/// Registers the editor commands.
///
/// # Errors
///
/// Fails if a command id is already taken.
pub fn register_commands(registry: &mut CommandRegistry) -> Result<(), RegistryError> {
    for (id, title, _) in COMMANDS {
        if let Ok(id) = CommandId::new(id) {
            registry.register(CommandDescriptor::new(id, *title))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_valid_and_unique() {
        let mut registry = CommandRegistry::new();
        register_commands(&mut registry).unwrap();
        assert_eq!(registry.iter().count(), COMMANDS.len());
        assert!(is_editor_command(SAVE));
        assert!(!is_editor_command("workbench.toggleSidebar"));
    }
}
