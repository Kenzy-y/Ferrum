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
- `shell/problems.rs`: the window's `workbench::ProblemsView`. Every
  service the window starts gets a task that passes its diagnostics, for
  all files, to the panel; opening another folder drops the tasks and
  clears the panel. `OpenProblem` opens the file at the position.
- `shell/search.rs`: the window's `workbench::SearchView`. A
  `SearchRequest` drops the running search (its `AtomicBool` cancels it)
  and starts `search::run` on the background executor with `[search]`,
  `[files] hidden` and the text of unsaved editors (ropes cloned on the UI
  thread, turned into strings in the background); files reach the view in
  batches tagged with the request's generation. `OpenMatch` opens the file
  with the match selected (`open_file` takes a range of points). Opening a
  folder re-runs the query there.
- `crash_report.rs`: `run` installs a panic hook first. A panic in a
  window callback aborts the process (`0xc0000409` on Windows) and the
  message would be lost without a console. The hook logs it with a
  backtrace and writes it to `last-crash.txt` in the data directory.
- `shell/keyboard.rs`: a held shortcut runs once, except commands made to
  repeat (`command_repeats`: paste, undo and redo, deletes, moves and
  selections, `terminal.paste`); never in the middle of a key sequence.
- `shell/language_status.rs` (ADR 0011): gives the workbench the language
  names, finds each pack's `Toolchain::version` in the background (again
  for every folder), passes every started service's status to the status
  bar (the tasks go with the problem tasks), and finds the
  `file_detail` of every opened file. `file_icons()` in `shell/mod.rs`
  registers the packs' `FileIconSpec`s.
- `shell/terminals.rs`: the window's `workbench::TerminalPanel` with the
  real process runner; opening a folder sets where terminals start,
  settings and keyboard layout changes reach every terminal. Keys of a
  focused terminal go through the keymap in the capture phase
  (`keyboard::terminal_focused`, `goes_to_terminal_program`).
- `shell/window_sizes.rs`: saves the panel sizes after every drag to
  `window-state.toml` in the data directory; `config.rs` reads them at
  startup.
- `shell/config_files.rs`: `preferences.openSettings` (the settings
  button, Ctrl+,) and `preferences.openKeymap` open the user
  `settings.toml` and `keymap.toml` in an editor. A missing file is first
  created, in the background, with a commented header from i18n; saving it
  applies it through `live_config`.
- `gallery/`: the component gallery, debug builds only.

**Commands.** `Shell::new` builds the window's `workbench::CommandHandlers`
from `Shell::register_handlers` and `Workbench::register_handlers`.
`run_command` defers the handler to the end of the current update,
because some handlers update the shell itself.

**Extending.** A new application command needs its id and title in
`app_commands.rs` and its handler in `Shell::register_handlers`. A
command of another crate is registered by that crate's own
`register_handlers`, called from `command_handlers` in `shell/mod.rs`.
