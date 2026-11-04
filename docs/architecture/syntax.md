# Syntax

Crate `syntax` (core layer, no GPUI): tree-sitter trees of a text,
highlighting and the outline. It knows no language: grammars come from
language packs (`lang_rust`, `lang_toml`; plugins in stage 9).

**Key types.**
- `GrammarSpec`: what a pack declares. It holds the language id and name,
  file extensions and exact file names (`Cargo.lock`), the tree-sitter
  `Language`, the highlight query and the outline query.
- `GrammarRegistry`: grammars by id, file name (wins) and extension.
  Filled at startup in `ferrum/src/languages.rs`, read-only afterwards.
  `GrammarEntry::load` compiles the queries once (about 20 ms for Rust), so
  it runs in the background on first use.
- `SyntaxTree`: `parse` reads the rope chunk by chunk, with no copy. It
  stops on a cancel flag or after `[syntax] parse_timeout_ms`. `edit` moves
  the tree through a `text::TextEdit`. It is cheap (µs) and runs on the UI
  thread, so highlights stay in place until the next parse finishes.
- `highlights(grammar, tree, rope, range)`: sorted, disjoint
  `HighlightSpan`s of a byte range, at most `MAX_HIGHLIGHT_RANGE`.
- `outline(grammar, tree, rope)`: `OutlineItem`s (kind, name, range, name
  offset, depth) in text order. `items_at` returns the items around an
  offset, for the breadcrumbs.

**Highlight queries.** Capture names are keys of the theme's `[syntax]`
tokens: `keyword`, `type`, `function`, `macro`, `attribute`, `string`,
`number`, `constant`, `lifetime`, `comment`, `doc_comment`. A dotted name
falls back to its prefix (`function.method` → `function`). Captures
starting with `_` only serve predicates. Text without a capture is
`plain`. Where captures overlap, **the pattern written earlier wins**. Each
byte keeps the capture of the earliest matching pattern, so a pack lists
comments and attributes first to color them as a whole, strings inside
included. This matches the mockup's `#[command(name = "forge")]`.

**Outline queries.** `@outline.<kind>` captures the whole item. The kinds
are `module`, `struct`, `enum`, `union`, `trait`, `impl`, `function`,
`constant`, `static`, `type`, `macro`, `table`, `key` (`SymbolKind`). `@name`
is the name. `@context` nodes are joined with it in text order (`impl
Display for Cli`). Depth comes from how item ranges contain one another.

**In the editor** (`editor/src/syntax_state.rs`).
- `Editor::set_grammar` turns on `Document::track_changes`. After every
  edit the editor moves the tree through the logged edits and starts a
  background parse, unless one is already running.
- Edits made during a parse are replayed on its result, then the next
  parse starts from that tree, so fast typing batches itself.
- A reloaded file (`TextChanges::replaced`) is parsed from scratch.
- Each frame highlights only the visible rows. A cache covers the visible
  text plus a margin, keyed by tree generation, so cursor blinks and small
  scrolls reuse it.
- Rows longer than `[syntax] max_line_length` (20 000 bytes) are plain:
  tree-sitter walks the siblings of a node linearly, so a 4 MB line costs
  270 ms per query.
- Files over `max_file_size_mb` (16) are not parsed.
- The outline is computed in the background 150 ms after the last parse
  and announced with `EditorEvent::OutlineChanged`.

**Change tracking** (`text::TextChanges`). Every replacement is logged in
the order it was applied, with byte offsets and points. A step of more
than 64 edits (thousands of cursors) is logged as one spanning edit,
otherwise logging would double the cost of the step. A log nobody drains
turns into "replaced" after 10 000 edits.

**Benchmarks** (`cargo bench -p lang_rust --bench syntax`; release, Windows
11, i5-13600K, 2026-11-04, machine under load):

| Operation | 100 000 lines | one 4 MB line |
|---|---|---|
| compile queries (once, background) | 20 ms | — |
| full parse (background) | 600 ms | 1.65 s |
| reparse after typing a character (background) | 66 ms | 270 ms |
| outline (background) | 257 ms | 814 ms |
| move the tree through an edit (UI) | 7 µs | 16 µs |
| highlight 60 visible rows (UI) | 190 µs | 270 ms → not done, see above |

Reparsing a large file costs tens of milliseconds because the top level
has tens of thousands of items. It runs in the background, and the edited
tree keeps the highlights in place meanwhile.

**Adding a language.** Create a crate with a `GrammarSpec`
(`lang_toml` is the example), its `queries/highlights.scm` and
`queries/outline.scm`, add one `register` line to
`ferrum/src/languages.rs`. Nothing in `syntax`, `editor` or `workbench`
changes. Queries are our own: grammar crates' queries may be the starting
point, never other editors' ones.
