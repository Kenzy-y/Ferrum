# Commands and keymaps

Crate `commands` (core layer, no GPUI).

**Purpose.** Stable command ids, the command registry and keybindings.

**Key types.**
- `CommandId` — validated `area.action` id (`editor.save`).
- `CommandRegistry` / `CommandDescriptor` — known commands with the i18n id of
  their title. Running commands is the UI layer's job: handlers live in
  `workbench::CommandHandlers` (see `workbench.md`).
- `Keystroke`, `KeySequence`, `Platform` — `ctrl-shift-p`, `ctrl-k ctrl-s`;
  `mod` is Cmd on macOS and Ctrl elsewhere.
- `Keymap` — bindings per context. `Keymap::defaults(platform)` loads
  `src/platform/commands/keymaps/default-<platform>.toml`; `apply(text)` layers a
  user `keymap.toml` on top (an empty command unbinds). Invalid files are
  rejected whole. Bindings keep file order (the first one is shown in
  hints). `diagnostics(&registry)` finds unknown commands.
- `KeymapMatcher` — feeds keystrokes with the active contexts (innermost
  first, `global` implied last) and returns `Command`, `Pending` or `NoMatch`.
  `pending_command` gives the command bound to exactly the pending keys:
  when keys are both a shortcut and a prefix (`Ctrl K` / `Ctrl K, Ctrl G`)
  the application runs it after `[keyboard] chord_timeout_ms`
  (`KeyboardSettings`, default 1000).
- `Keystroke::label` / `KeySequence::label` — display form for hints and the
  palette (`Ctrl Shift P`, `Cmd K`, `Ctrl K, Ctrl G`).
- `fuzzy_match` / `filter` — case-insensitive subsequence matching with
  word-start and consecutive-run bonuses; returns byte positions for
  highlighting. Used by quick open.

**Format.**
```toml
[global]
"mod-shift-p" = "workbench.showCommandPalette"
[editor]
"ctrl-k ctrl-s" = ""   # unbind
```

- `LayoutKeys` — shortcuts by physical key. Bindings use US keys; the
  application asks the platform which key of the active layout sits on
  each of `US_KEYS` and translates incoming keys back, so Ctrl+S on a
  Russian layout is still `ctrl-s`. Rebuilt when the layout changes.

**Contexts from focus.** Views name their context with GPUI's
`key_context` (`editor`, `file_tree`); the shell passes the contexts of the
focused view and its parents, innermost first, then `workbench`.

**Extending.** New command: register a descriptor, register its handler
next to the code that implements it (`Workbench::register_handlers`,
`Shell::register_handlers`), and add a title message in every locale. A
test fails if a registered command has no handler. New context: set `key_context` on the view and use
the name in keymaps.
