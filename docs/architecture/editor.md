# Editor

Crate `editor` (UI layer): the view that shows and edits a `text::Document`.

**Key types.**
- `Editor` — the entity: document, `EditorSettings`, scroll position,
  cursor blink, input method composition (`marked`), the geometry of the
  last frame (`LayoutSnapshot`) and the file (`FileState`). Emits
  `EditorEvent` (`Edited`, `SelectionsChanged`, `Saved`, `SaveFailed`,
  `Reloaded`); `status()` gives the status bar line, column, selections,
  indentation and line endings.
- `EditorElement` (`element.rs`) — the GPUI element. Prepaint shapes only
  the visible rows; of a line longer than `LONG_LINE_CHARS` (1024) only the
  visible columns plus a margin (`viewport::column_window`), so a line of
  megabytes costs the same as a short one. Tabs are expanded per row
  (`display::DisplayLine` maps offsets both ways). `scene.rs` builds the
  layers around the text: current line highlights, selections, indent
  guides (`viewport::guide_levels`, blank lines take the level of their
  neighbours), cursors, line numbers. Then the scrollbar thumb.
- `viewport.rs` — pure arithmetic: visible rows, long-line windows,
  following the cursor with a margin (`follow_row`, `follow_x`), limits.
- `actions.rs` — every `editor.*` command: id, title, action. The keymap
  binds them in the `editor` context (the editor's `key_context`); the
  application runs them on the active editor with `Editor::run_command`.
- `input.rs` — `EntityInputHandler`: typed text arrives here, not as key
  bindings; input methods compose with marked text (underlined). Offsets
  convert to UTF-16 through `text::Buffer`.
- `mouse.rs` — click places a cursor, Shift+click extends, double click
  selects a word, triple a line, dragging extends by the unit of the click;
  `[editor] multi_cursor_modifier` (`alt` default, or `ctrl` / Cmd for
  window managers that take Alt+click) adds a cursor. The wheel scrolls
  (Shift: horizontally).
- `file.rs` — `save` writes in the background (the buffer is a cheap rope
  clone). The file's folder is watched non-recursively; when the file
  changes and there are no unsaved edits, it is read and compared in the
  background and swapped in only if it differs, so own saves change
  nothing. If the file is gone and exactly one other file of the folder
  has its saved text (`FileState::saved`, a rope clone), it was renamed:
  the editor saves there from now on, unsaved edits included, and emits
  `EditorEvent::Renamed`. Renames are atomic, so the new name exists
  when the old one is reported gone. Moves to another folder are not
  followed.

**Cursor blink.** A timer flips the cursor every
`cursor_blink_interval_ms`; it runs only while the editor is focused in an
active window, restarts on every edit or move, and is off with
`cursor_blink = false`. Without focus nothing ticks.

**Tab** indents. The component library binds Tab to moving the focus at
the window root, and GPUI runs action bindings before key listeners, so
`editor::init` binds Tab and Shift+Tab to `NoAction` in the `editor`
context.

**Clipboard.** Copy without a selection copies the cursor's line; such text
carries metadata, and pasting it at a single cursor inserts it above the
line. With as many lines as cursors, each cursor gets one.

**Geometry** comes from theme tokens taken from `editor.svg`: line height
`code.line_height`, `editor.inset`, `editor.gutter_gap`, `editor.top`,
`editor.active_line.*`, `cursor.*`, `scrollbar.*`; line numbers are at
least four digits wide (`MIN_LINE_NUMBER_DIGITS`).

**Highlighting and outline** (`syntax_state.rs`, see
`docs/architecture/syntax.md`). `Editor::set_grammar` gives the editor a
grammar. The tree follows every edit, parsing runs in the background, and
`runs.rs` turns the highlights of a shaped row into text runs (colors from
the theme's `[syntax]` tokens, the input method underline on top).
`outline()`, `symbols_at_cursor()` and `go_to()` serve the Outline section
and the breadcrumbs. `language_id()` selects `[lang.<id>]` settings.

**Extending.** Decorations (diagnostics, inlay hints) become further
layers in `scene.rs`. New commands: a row in `actions::COMMANDS`, a title
in every locale, a binding in the default keymaps.
