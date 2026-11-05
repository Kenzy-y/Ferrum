# Editor

Crate `editor` (UI layer): the view that shows and edits a `text::Document`.

**Layout.** `editor.rs` (the entity), `actions.rs` (commands), `file.rs`
(saving, disk state), `settings.rs`; `render/` draws the visible rows
(layout, shaping, display lines, scene, element, viewport arithmetic,
text runs); `input/` takes keyboard text and the mouse; `decorations/`
holds the minimap and the scrollbar; `highlighting/` keeps the syntax
tree in step with edits (`patch.rs` maps highlight ranges across edits).

**Key types.**
- `Editor` — the entity: document, `EditorSettings`, scroll position,
  cursor blink, input method composition (`marked`), the geometry of the
  last frame (`LayoutSnapshot`) and the file (`FileState`). Emits
  `EditorEvent` (`Edited`, `SelectionsChanged`, `Saved`, `SaveFailed`,
  `Reloaded`); `status()` gives the status bar line, column, selections,
  indentation and line endings.
- `EditorElement` (`render/element.rs`) — the GPUI element. Prepaint shapes only
  the visible rows; of a line longer than `LONG_LINE_CHARS` (1024) only the
  visible columns plus a margin (`viewport::column_window`), so a line of
  megabytes costs the same as a short one. Tabs are expanded per row
  (`display::DisplayLine` maps offsets both ways). `render/scene.rs` builds the
  layers around the text: current line highlights, selections, indent
  guides (`viewport::guide_levels`, blank lines take the level of their
  neighbours), cursors, line numbers. Then the scrollbar thumb.
- `render/viewport.rs` — pure arithmetic: visible rows, long-line windows,
  following the cursor with a margin (`follow_row`, `follow_x`), limits.
- `actions.rs` — every `editor.*` command: id, title, action. The keymap
  binds them in the `editor` context (the editor's `key_context`); the
  workbench registers a handler for each id in `command_ids()` that runs
  it on the active editor with `Editor::run_command`.
- `input/mod.rs` — `EntityInputHandler`: typed text arrives here, not as key
  bindings; input methods compose with marked text (underlined). Offsets
  convert to UTF-16 through `text::Buffer`.
- `input/mouse.rs` — click places a cursor, Shift+click extends, double click
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
  **Disk state** (`DiskState`, `EditorEvent::DiskStateChanged`): a change
  on disk while there are unsaved edits is `Changed`. Then
  `editor.reloadFromDisk` replaces the text as one undoable edit
  (`Document::replace_all`) and `editor.keepLocalChanges` keeps the edits
  (the file's text becomes the saved text; nothing in the history matches
  it). A gone file that was not renamed is `Deleted`. Saving resolves
  both; `save` returns a task that tells whether it worked.
  **Unsaved edits** are `Document::state_id` differing from the saved
  state, so undo back to the saved text clears the dot. `mark_saved`
  ends the undo group, so typing after a save is a new step.

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
`editor.active_line.*`, `cursor.*`, `scrollbar.*`, `minimap.*`; line
numbers are at least four digits wide (`MIN_LINE_NUMBER_DIGITS`).
`render/shaping.rs` shapes the visible rows.

**Scrollbar** (`decorations/scrollbar.rs`, pure geometry). Dragging the thumb keeps it
where it was grabbed. A click on the track jumps there, holding the thumb
by its middle. Hovered or dragged, it uses `bg.scrollbar.thumb.hover`. The
strip around it shows the arrow cursor.

**Minimap** (`decorations/minimap.rs`, `[editor] minimap`, on by default; ADR 0004).
It is drawn right of the text: one bar per run of a syntax color (spaces
included), dimmed by `opacity.minimap`, and the visible part in
`bg.minimap.viewport`. While the whole text fits, it does not scroll;
otherwise it scrolls with the view, so its marked part moves like a
thumb. A click or drag scrolls there. Only the lines it shows are read,
at most the columns that fit, with one highlight query for all of them.
The bars are cached until the text, the highlights, the shown lines or
the colors change. It is hidden when the editor is too narrow.

**Benchmarks** (`cargo bench -p editor --bench frame`; a whole prepaint and
paint on GPUI's test platform, which does not rasterize glyphs; release,
Windows 11, i5-13600K, 2026-11-05; budget for a frame: 8 ms):

| Frame | 100 000 lines | one 4 MB line |
|---|---|---|
| repaint, nothing changed | 1.78 ms | 33 µs |
| after typing a character | 1.75 ms | 55 µs |
| after Page Down / Page Up | 1.79 ms | 33 µs |

About 1.1 ms of a 100 000-line frame is painting the minimap's bars, on
every repaint including cursor blinks. ROADMAP stage 10 tracks drawing
the minimap once per change.

**Highlighting and outline** (`highlighting/`, see
`docs/architecture/syntax.md`). `Editor::set_grammar` gives the editor a
grammar. The tree follows every edit, parsing runs in the background, and
`render/runs.rs` turns the highlights of a shaped row into text runs (colors from
the theme's `[syntax]` tokens, the input method underline on top).
`outline()`, `symbols_at_cursor()` and `go_to()` serve the Outline section
and the breadcrumbs. `language_id()` selects `[lang.<id>]` settings.

**Language support** (`language/`, ADR 0006, 0007). The owner connects a
`language::LanguageService` with `Editor::set_language_service`; only
editors of files can be connected. The document is opened at version 1,
every edit goes out with `change_document` (from `follow_text_changes`),
saves with `save_document`, a rename reopens it under the new name, and
dropping the state closes it. Only what `capabilities()` allows is offered.
- `diagnostics.rs` places the service's diagnostics as byte ranges, which
  follow edits until new ones arrive. An empty range marks one character.
  `problem_counts()` gives errors and warnings, and
  `EditorEvent::DiagnosticsChanged` tells when they change.
  `render/marks.rs` draws waves (`diagnostic.error`, `.warning`, `.info`)
  and dots under hints, for visible rows only.
- `hover.rs`: the pointer resting on an identifier or a diagnostic for
  `[editor] hover_delay_ms` (or `editor.showHover`, Ctrl K Ctrl I) shows
  the diagnostics there and the service's hover. The popup hides once the
  pointer has left both its word and the popup for the same delay, and on
  an edit, a scroll, a click or Escape.
- `completion.rs`: the list opens with `editor.triggerSuggest` (Ctrl
  Space), after a trigger character, or when a word starts
  (`[editor] suggest_on_type`). Typing filters it with
  `commands::fuzzy_match`, and an incomplete list is asked for again.
  While it is open, `Editor::popup_takes` routes Up, Down, Page Up and
  Page Down to it, Enter and Tab accept, Escape closes. The keymap has no
  "list open" context, so the editor decides. Accepting replaces the
  item's range (or the typed word) and applies its additional edits.
  Without them, resolving the item fills them in afterwards (imports).
- `navigation.rs`: `editor.goToDefinition` (F12), or a click with the
  modifier that does not add cursors (Ctrl/Cmd, or Alt when
  `multi_cursor_modifier = "ctrl"`). A definition in the same file moves
  the cursor; one elsewhere emits `EditorEvent::OpenLocation`.
- `markdown.rs` reduces hover and documentation Markdown to paragraphs,
  code blocks and rules. Code in the editor's language is highlighted with
  its grammar in the background. `popup.rs` styles popups like the palette
  and anchors them below the line, or above it in the lower part of the
  view (`deferred` + `anchored`, which also switches sides at the window
  edge).

**Extending.** Decorations (inlay hints) become further
layers in `render/scene.rs`. New commands: a row in `actions::COMMANDS`, a title
in every locale, a binding in the default keymaps.
