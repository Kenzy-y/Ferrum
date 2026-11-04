# Text

Crate `text` (core layer, no GPUI): what the editor edits.

**Key types.**
- `Buffer` — a `ropey` rope plus the UTF-8 BOM. The text is kept exactly as
  in the file, line breaks included, so lines nobody edits are saved with
  their own ending even in files that mix `
` and `
`. Counts of both
  kinds are updated by every edit from the edited text only (plus one byte of
  context on each side, for pairs split or joined at the edges).
  `line_ending()` is the dominant kind, which new lines and pasted text get
  (`with_line_ending`); `line_endings()` is `Uniform(..)` or `Mixed { .. }`
  (the status bar shows "Mixed"). Positions are byte offsets; `Point` is a
  row and a byte column; `char_column` / `offset_at_char_column` serve
  vertical movement.
- `Selection` / `Selections` — anchor and head (a cursor is an empty
  selection), sorted, merged when they overlap (touching selections stay
  apart, a cursor at or inside a selection merges), never empty, with a
  primary one.
- `Document` — buffer, selections and `History`. Every operation works at
  all cursors at once: `insert`, `paste` (one line per cursor when the
  counts match), `newline` (keeps indentation), `delete_backward/forward`,
  `delete_word_*`, `delete_selections`, `move_cursors(Movement, extend)`,
  `add_cursor_vertically`, `select_all`, `set_line_endings` (one undo step,
  cursors keep their places). `version()` changes with the text.
- `History` — each transaction has an id (`state_id`: the text right
  after it, back to an earlier value after undo); each transaction stores
  its edits and the selections before
  and after, so undo and redo put the cursors back where they were.
  Consecutive typing, or consecutive deleting, is one undo step until the
  cursors move, typing pauses for `UNDO_GROUP_PAUSE` (1 s) or a new word
  starts (a non-space typed after a space: "hello world" undoes as
  "world", then "hello "). Anything else is its own step. The time comes
  from the document's `Clock`, replaceable in tests.
- `next_grapheme` / `previous_grapheme` — boundaries of grapheme clusters
  (`unicode-segmentation`) over rope chunks: an emoji of several code
  points, a letter with combining marks or a flag is one step for the
  cursor and for deletion. `next_word_end` / `previous_word_start` split
  words, punctuation runs and whitespace.
- `TextChanges` — with `Document::track_changes` (off by default), every
  replacement is logged as a `TextEdit` with offsets and points, in the
  order it was applied. `take_changes` drains the log. Syntax trees follow
  the text with it. A step of more than 1024 edits is logged as one
  spanning edit. A reload, or a log of 10 000 undrained edits, reports
  `replaced`. Cost: typing 1.7 → 3.1 µs per character with tracking;
  1000 and 5000 cursors unchanged (2.0 and 9.3 ms).
- `load` / `save` — through `fs::Fs`; UTF-8 only for now: files with NUL
  bytes in the first 8 KB are `TextError::Binary`, other invalid UTF-8 is
  `TextError::NotUtf8`.

**Costs.** Keystroke operations are logarithmic in the document size (rope
lookups and edits); only `Buffer::from_text` and `to_text` are linear and
run in the background.

**Benchmarks** (`cargo bench -p text`; release, Windows 11, Intel Core i5-13600K,
2026-11-04; budget for keystroke work: under 100 µs):

| Operation | 100 000 lines (4.4 MB) | one 4 MB line |
|---|---|---|
| open (`from_text`, background) | 4.8 ms | 5.1 ms |
| save (`to_text`, background) | 0.89 ms | 1.1 ms |
| type a character | 0.97 µs | 0.97 µs |
| backspace | 6.2 µs | 8.9 µs |
| type + undo + redo | 49 µs | 52 µs |
| page down + up | 3.6 µs | 0.94 µs |
| right + left | 0.45 µs | 0.44 µs |
| offset ↔ point | 1.0 µs | 0.71 µs |

Many cursors (100 000 lines, cursors spread evenly):

| Operation | 1000 cursors | 5000 cursors | growth |
|---|---|---|---|
| type a character | 2.7 ms | 13.0 ms | ×4.9 |
| backspace | 1.8 ms | 5.1 ms | ×2.9 |

Growth is linear (×5 cursors, at most ×5 time). Typing at 5000 cursors takes
more than one 8 ms frame; per cursor it is about 2.6 µs, spent in the rope
edits and the undo record.

CI runs every benchmark once as a smoke test (`cargo test --benches`);
timings are only meaningful on a quiet machine.

**Extending.** Encodings other than UTF-8 plug into `load` / `save`
(`encoding_rs`). LSP positions (UTF-16) get a conversion next to
`char_column`. The editor (stage 3, part 2) maps its commands onto
`Document` operations.
