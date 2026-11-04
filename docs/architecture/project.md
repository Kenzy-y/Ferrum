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
  (on a background executor), folders first then by name.
- `IgnoreRules` — decide `Visibility` of a path:
  - `Hidden` (not shown, not watched): `[files] hidden` patterns
    (`FilesSettings`, gitignore syntax, default `[".git"]`; changes apply
    live);
  - `Ignored` (shown dimmed): the `.gitignore` of each loaded folder (the
    closest rule wins, `!` whitelists), then `.git/info/exclude`, then git's
    global excludes file, as in git;
  - otherwise `Visible`.
- `read_repository_excludes` — reads `.git/info/exclude` and the global
  excludes file through `Fs`: `core.excludesFile` from
  `$XDG_CONFIG_HOME/git/config` then `~/.gitconfig` (the later wins), else
  git's default `$XDG_CONFIG_HOME/git/ignore` (`~/.config/git/ignore`).
  The `ignore` crate can read these itself, but with `std::fs`, which would
  bypass `Fs`; we only use it for matching.
- In the tree, a folder whose
  canonical path equals that of one of its parents is a link cycle
  (`RowState::Cycle`) and never expanded. `rows()` flattens the expanded
  part for a virtualized list; `dirs_to_reload(events)` picks the folders to
  read again after watcher events (parents of created/removed entries, the
  folder of a changed `.gitignore`, everything on a rescan).
- **Watching.** The workspace is watched recursively with `IgnoreRules` as
  the filter, so events under ignored or hidden paths (`target/` during
  `cargo build`, `.git`) are dropped on the watcher thread.
  `FileTree::open_ignored_dirs` lists the ignored folders that are expanded
  and visible; the view keeps one non-recursive watch (direct entries only,
  hidden paths still dropped) for each and drops it when the folder is
  collapsed or hidden by a collapsed parent. An open `target` therefore stays
  current without watching the whole build output.

**Opening a folder (application).** `workspace.openFolder` (Ctrl K, Ctrl O)
asks the platform for a folder. If the platform has no picker (Linux without
xdg-desktop-portal) or opening fails, the workbench path dialog explains why
and lets the user type the path (`workspace.openFolderByPath`). On success
the shell sets the workspace settings layer, records the folder in the
recent list (saved in the background), updates the title, the explorer and
the status bar. A recent folder that no longer exists is removed.

**Extending.** Multi-root workspaces add roots to `Workspace` and one
`FileTree` per root; search builds on `Workspace::root` and `IgnoreRules`.

