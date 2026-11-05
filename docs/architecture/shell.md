# Shell

Crate `shell` (application layer, `src/app/shell`): starts the
application and hosts the root view of each main window. The `ferrum`
binary only sets up logging and calls `shell::run()`.

**Modules.**
- `lib.rs`: `run()` starts GPUI, registers fonts and the theme, loads
  the configuration in the background and opens the main window.
- `config.rs` loads `settings.toml` and `keymap.toml` and builds the
  settings registry. `live_config.rs` watches both files and hands
  changes to `Shell::apply_config`.
- `app_commands.rs` holds the application's own commands (`app.quit`,
  `workspace.openFolder*`, the debug-only gallery) and the registry of
  every command. `languages.rs` registers the built-in language packs.
- `shell/mod.rs`: `Shell`, the window root. It owns the workbench, the
  open folder, the settings store and the command handler table, and
  draws the window frame.
- `shell/keyboard.rs`: `KeyboardDispatcher`. Key presses go through the
  keymap with the focus contexts and the active keyboard layout. It
  holds pending sequences and the chord timeout.
- `shell/settings_sync.rs` applies changed settings (language, editor,
  file tree) and keymaps without a restart.
- `shell/workspace.rs` and `shell/files.rs` open folders (picker, typed
  path, recent projects) and files.
- `language_servers.rs`: `LanguageServers`, the running services keyed
  by server id and folder. `shell/files.rs` connects every opened editor
  to the service of its language in the open folder, and opens the file
  and position of `EditorEvent::OpenLocation`. Opening another folder
  stops the previous folder's servers in the background. Settings changes
  go to every service with `configure`. The table belongs to the window
  for now; it moves to the application when there are several windows
  (ADR 0006: one server per folder).
- `gallery/`: the component gallery, debug builds only.

**Commands.** `Shell::new` builds the window's `workbench::CommandHandlers`
from `Shell::register_handlers` and `Workbench::register_handlers`.
`run_command` defers the handler to the end of the current update,
because some handlers update the shell itself.

**Extending.** A new application command needs its id and title in
`app_commands.rs` and its handler in `Shell::register_handlers`. A
command of another crate is registered by that crate's own
`register_handlers`, called from `command_handlers` in `shell/mod.rs`.
