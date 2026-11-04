# Commands and keymaps

Crate `commands` (core layer, no GPUI).

**Purpose.** Stable command ids, the command registry and keybindings.

**Key types.**
- `CommandId` — validated `area.action` id (`editor.save`).
- `CommandRegistry` / `CommandDescriptor` — known commands with the i18n id of
  their title. Running commands is the UI layer's job: it maps ids to
  handlers (for now `ferrum::app_commands`).
- `Keystroke`, `KeySequence`, `Platform` — `ctrl-shift-p`, `ctrl-k ctrl-s`;
  `mod` is Cmd on macOS and Ctrl elsewhere.
- `Keymap` — bindings per context. `Keymap::defaults(platform)` loads
  `crates/commands/keymaps/default-<platform>.toml`; `apply(text)` layers a
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

**Extending.** New command: register a descriptor, add a handler and a title
message in every locale. New context: just use its name in keymaps and pass it
to the matcher from the view that owns focus. Contexts are not yet
derived from focus: the shell passes `workbench` for the whole window.
