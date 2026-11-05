# Language

Crate `language` (core layer, no GPUI): what a language declares and what
the UI asks of language support. ADR 0006.

**Key types.**
- `LanguagePack`: id, name, file extensions, exact file names, an optional
  `syntax::GrammarSpec` and an optional `ServerFactory`.
  `LanguagePack::new(grammar)` takes the identity from the grammar,
  `without_grammar` exists for languages a server serves as plain text,
  `with_server` adds the server. `with_pairs` replaces the auto-closing pairs
  (`text::DEFAULT_PAIRS` by default; Rust drops `'`), and the editor gets them
  from `LanguageEntry::pairs`.
- `LanguageRegistry`: packs by id and by path (exact name wins over the
  extension, case ignored). It registers each pack's grammar in its
  `GrammarRegistry` (`grammars()`), the pack's identity overriding the
  grammar's. It is filled at startup in `src/app/shell/src/languages.rs`
  and read-only afterwards.
- `ServerFactory::start(ServerContext)` returns a `LanguageService` at
  once. The server starts in the background, and a missing program
  becomes `ServerStatus::NotFound`, not an error. `ServerContext` holds
  the workspace root, the `ProcessRunner` and the settings at start.
- `LanguageService`: object-safe, with `BoxFuture<T>` (`'static`, so the
  UI can hold it in a task). Document notifications (`open_document`,
  `change_document` with `text::TextChanges` and the new `text::Buffer`,
  `save_document`, `close_document`) return at once. Requests (`hover`,
  `completions`, `resolve_completion`, `definition`, `inlay_hints`,
  `code_lenses`, `resolve_code_lens`) are futures, and dropping one
  cancels it. `events()` gives a new stream of `ServiceEvent`s
  (`Diagnostics`, `Status`, `CapabilitiesChanged`, `InlayHintsChanged`,
  `CodeLensesChanged`), starting with the current status and the
  diagnostics already published. `capabilities()` tells what to show.
  `configure` passes changed settings on.
- Types are the IDE's own, not the protocol's. Positions are
  `text::Point` (row, byte column), documents are paths. `Diagnostic`
  (`Severity`), `Hover` (markdown), `Completion` (`CompletionEdit`,
  additional `TextReplacement`s, an opaque `ResolveToken`), `Location`,
  `InlayHint`, `CodeLens` (`LensAction`), `ServerStatus`.

**Extending.** A new language is a crate with `pack()` (see `lang_toml`)
and one line in `shell/src/languages.rs`. A server is a `ServerFactory`.
LSP servers use `lsp::start` with an `lsp::LspAdapter`
(`docs/architecture/lsp.md`). A plugin may implement `LanguageService`
any other way.
