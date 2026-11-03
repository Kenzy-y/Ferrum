# Settings

Crate `settings` (core layer, no GPUI).

**Purpose.** Schema-first, layered settings; the only code that reads
configuration files.

**Key types.**
- `Settings` — trait for a settings struct (`serde` + `schemars` + `Default`)
  with a `NAMESPACE`. Field doc comments become UI descriptions.
- `SettingsRegistry` — namespaces with schemas and serialized defaults; built
  once at startup via `SettingsRegistry::builder().register::<T>()`.
- `SettingsLayer` — one parsed file: namespaced tables plus `[lang.<id>.<ns>]`
  overrides.
- `SettingsStore` — layers by `LayerKind` (`User`, `Workspace`); `get::<T>(language)`
  deep-merges defaults → user → workspace → user `lang` → workspace `lang`.
  Invalid values fall back to defaults; `check::<T>()` and
  `unknown_namespaces()` report problems. `set_layer`/`remove_layer` return a
  `SettingsChange` with affected namespaces and bump `revision()`.
- `load::{load_layer, read_config_file}` — read files through `fs::Fs`;
  a missing file is `None`, not an error.
- `paths::ConfigPaths` — per-user config directory and file names.

**File locations.** Names come from `app_identity` (`APPLICATION`,
`LOWERCASE_NAME`, `WORKSPACE_DIR_NAME`); base directories from `directories`.

| Platform | User config directory |
|---|---|
| Windows | `%APPDATA%\Ferrum\config` (`C:\Users\<user>\AppData\Roaming\Ferrum\config`) |
| macOS | `~/Library/Application Support/Ferrum` |
| Linux | `$XDG_CONFIG_HOME/ferrum`, by default `~/.config/ferrum` |

The directory holds `settings.toml` and `keymap.toml`. Workspace settings are
`<workspace>/.ferrum/settings.toml` on every platform.

**Extending.** New crate settings: define a struct, implement `Settings`,
register it in the application's registry (`ferrum::config`). New layer:
add a `LayerKind` variant at its place in the order. Live reload: the UI layer
re-reads a file, calls `set_layer` and notifies observers of the returned
`SettingsChange`.
