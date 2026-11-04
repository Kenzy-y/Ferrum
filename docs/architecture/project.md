# Project

Crate `project` (services layer, no GPUI): the folder open in a window.

**Key types.**
- `Workspace` — `Workspace::open(fs, path)` checks that the path is a folder
  and canonicalizes it (one folder is one workspace, whatever link led to
  it); `settings_file()` is `.ferrum/settings.toml` (name from
  `app_identity`), `load_settings(fs)` reads it as the workspace settings
  layer (invalid files are logged and ignored); `display_root()` drops the
  Windows `\?\` prefix.
- `RecentProjects` — most recent first, at most `[project]
  recent_projects_limit` (`ProjectSettings`, default 10). Stored as
  `recent-projects.toml` in the data directory (`ConfigPaths::data_dir`);
  a missing or damaged file gives an empty list.
- `ProjectError` — `NotFound`, `NotAFolder`, `Serialize`, `Fs`.

- `FileTree` — the tree as a model: folders are read lazily by `load_dir`
  (on a background executor), folders first then by name. `IgnoreRules`
  hold the `.gitignore` rules of loaded folders: matched entries are
  `Visibility::Ignored` (shown dimmed), `.git` is `Hidden`. A folder whose
  canonical path equals that of one of its parents is a link cycle
  (`RowState::Cycle`) and never expanded. `rows()` flattens the expanded
  part for a virtualized list; `dirs_to_reload(events)` picks the folders to
  read again after watcher events (parents of created/removed entries, the
  folder of a changed `.gitignore`, everything on a rescan).
- The same `IgnoreRules` are the watcher filter, so events under ignored
  folders (`target/` during `cargo build`) are dropped on the watcher thread.

**Opening a folder (application).** `workspace.openFolder` (Ctrl K, Ctrl O)
asks the platform for a folder. If the platform has no picker (Linux without
xdg-desktop-portal) or opening fails, the workbench path dialog explains why
and lets the user type the path (`workspace.openFolderByPath`). On success
the shell sets the workspace settings layer, records the folder in the
recent list (saved in the background), updates the title, the explorer and
the status bar. A recent folder that no longer exists is removed.

**Extending.** Multi-root workspaces add roots to `Workspace` and one
`FileTree` per root; search builds on `Workspace::root` and `IgnoreRules`.
Ignored folders are not watched: their contents refresh when expanded again.
