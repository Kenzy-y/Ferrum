# Text

Crate `text` (core layer, no GPUI): what the editor edits.

**Key types.**
- `Buffer` — a `ropey` rope plus the file's line ending (`LineEnding`) and
  UTF-8 BOM. The rope always holds `
`; `to_text()` restores the original
  ending (the dominant one for mixed files) and the BOM. Positions are byte
  offsets; `Point` is a row and a byte column; `char_column` /
  `offset_at_char_column` serve vertical movement.
- `Selection` / `Selections` — anchor and head (a cursor is an empty
  selection), sorted, merged when they overlap (touching selections stay
  apart, a cursor at or inside a selection merges), never empty, with a
  primary one.
- `Document` — buffer, selections and `History`. Every operation works at
  all cursors at once: `insert`, `paste` (one line per cursor when the
  counts match), `newline` (keeps indentation), `delete_backward/forward`,
  `delete_word_*`, `delete_selections`, `move_cursors(Movement, extend)`,
  `add_cursor_vertically`, `select_all`. `version()` changes with the text.
- `History` — each transaction stores its edits and the selections before
  and after, so undo and redo put the cursors back where they were.
  Consecutive typing, or consecutive deleting, with cursors that did not
  move in between is one undo step; anything else is its own step.
- `next_grapheme` / `previous_grapheme` — boundaries of grapheme clusters
  (`unicode-segmentation`) over rope chunks: an emoji of several code
  points, a letter with combining marks or a flag is one step for the
  cursor and for deletion. `next_word_end` / `previous_word_start` split
  words, punctuation runs and whitespace.
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

CI runs every benchmark once as a smoke test (`cargo test --benches`);
timings are only meaningful on a quiet machine.

**Extending.** Encodings other than UTF-8 plug into `load` / `save`
(`encoding_rs`). LSP positions (UTF-16) get a conversion next to
`char_column`. The editor (stage 3, part 2) maps its commands onto
`Document` operations.
