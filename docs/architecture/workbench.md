# Workbench

Crate `workbench` (UI layer): the layout of a main window as in
`docs/design/mockups`.

**Parts.** `title_bar` (logo, menus, search, actions, `window_controls`),
`activity_bar`, `sidebar`, `editor_area`, `bottom_panel`, `status_bar`. All
sizes come from `theme` tokens (`titlebar.*`, `activity_bar.*`,
`sidebar.width`, `bottom_panel.height`, `layout.gap.top`, `panel.gap`).

**State.** `Workbench` holds layout state only: the `SidebarView`, whether
the sidebar and the bottom panel are visible, the active `PanelKind`, and an
optional `EditorContent` (any view with a tab title).

**Commands.** Every action is a command (`command_ids`):
`workbench.toggleSidebar`, `workbench.togglePanel`, `workbench.show*`.
`register_commands` adds them to the registry. Buttons call the `Dispatch`
callback given by the application, which routes ids to their owners;
`Workbench::run_command` handles the workbench's own ids.

**Quick open.** `quick_open::QuickOpen` is the overlay behind the title bar
search (`workbench.quickOpen`, Ctrl K) and the command palette
(`workbench.showCommandPalette`, Ctrl Shift P / F1; it starts with the `>`
prefix). It opens exactly over the title bar field (same position and width,
drawn with `deferred` above the panels; a full-window backdrop closes it on
a click outside). It lists every registered command with its localized title and
first shortcut, filters with `commands::filter`, highlights matches, and is
driven by the keyboard (Up / Down / Enter / Escape) or the mouse. Closing
restores the previous focus. Commands are the only source for now; files,
symbols and settings become further sources in their stages.

**Welcome and Explorer.** Without editor content the editor area shows the
Welcome page (`welcome.rs`, from `welcome.svg`): logo, title, Open folder
plus two disabled buttons with a "later" tooltip, recent projects (or the
empty "clean slate" card), Get started with live shortcut labels, and a
documentation link (`app_identity::REPOSITORY_URL`). The Explorer view
(`explorer.rs`) shows an empty state without a folder and the project card
with the Files section with one. `file_tree_view.rs` is the tree: a `uniform_list` over
`project::FileTree` rows (only visible rows are built), one tab stop with
arrow keys (Up/Down/Home/End select, Right expands or enters, Left collapses
or goes to the parent, Enter/Space toggle), a reserved slot for version
control marks, tooltips for link cycles and unreadable folders. It watches
the workspace and emits `WorkspaceSettingsChanged` when
`.ferrum/settings.toml` changes. File icons come from the `FileIcons`
registry (by name, then extension; generic icon otherwise), filled by
contributors at startup. `path_prompt.rs` is the dialog for typing
a folder path. The application passes the open folder (`set_workspace`), the
recent list (`set_recent`) and an `OpenPath` callback.

**Window chrome.** `Workbench::window_options()` hides the platform title
bar. Windows: drag and min/max/close use native hit-test areas
(`window_control_area`), so Snap layouts work. Interactive elements in the
title bar must cover the drag area with `occlude()` (see `clickable` in
`title_bar.rs`), otherwise Windows treats a click on them as a window move. Linux: client-side
decorations, our buttons, `start_window_move`, and `window_border` from
gpui-component for the resize frame; with server-side decorations our
buttons are hidden. macOS: system traffic lights over the left inset.

**Extending.** New sidebar view: add a `SidebarView` variant, its icon,
title, command id and keybinding. View contents, status bar items and panel
contents will be contributed by their crates as the stages arrive.
