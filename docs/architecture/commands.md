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
  rejected whole. `diagnostics(&registry)` finds unknown commands.
- `KeymapMatcher` — feeds keystrokes with the active contexts (innermost
  first, `global` implied last) and returns `Command`, `Pending` or `NoMatch`.

**Format.**
```toml
[global]
"mod-shift-p" = "workbench.showCommandPalette"
[editor]
"ctrl-k ctrl-s" = ""   # unbind
```

**Extending.** New command: register a descriptor, add a handler and a title
message in every locale. New context: just use its name in keymaps and pass it
to the matcher from the view that owns focus. A timeout for ambiguous
sequences is not implemented yet.
