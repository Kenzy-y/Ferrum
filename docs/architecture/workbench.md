# Workbench

Crate `workbench` (UI layer): the layout of a main window as in
`docs/design/mockups`.

**Layout of the crate.** `workbench.rs` is the `Workbench` entity (state
shared with the parts, `Render`); `layout.rs` the sidebar view and panel
state; `overlays.rs` quick open and the path dialog with focus restore;
`command_ids.rs` and `command_handlers.rs` the commands. `parts/` are the
regions of the window (one file each, `editor_area/` with tabs, closing
and the message view). `contrib/` are features, one folder each:
`explorer/` (with the file tree and file icons), `outline/`,
`quick_open/`, `welcome/`. `dialogs/` holds modal dialogs. A new region
goes to `parts/`, a new feature to its own `contrib/<name>/` folder.

**Parts.** `title_bar` (logo, menus, search, actions: Run project,
disabled until stages 4 and 8, and the sidebar toggle; `window_controls`),
`activity_bar`, `sidebar`, `editor_area`, `bottom_panel`, `status_bar`. All
sizes come from `theme` tokens (`titlebar.*`, `activity_bar.*`,
`sidebar.width`, `bottom_panel.height`, `layout.gap.top`, `panel.gap`).

**State.** `Workbench` holds layout state: the `SidebarView`, whether
the sidebar and the bottom panel are visible, the active `PanelKind`, and
the tabs of the editor area.

**Tabs** (`tabs.rs`). A tab shows an `editor::Editor` or another view (the
component gallery, a `MessageView` explaining why a file cannot be
opened). `open_editor` reuses the tab of a file that is already open;
`workbench.closeTab` asks about unsaved changes (Save / Don't save /
Cancel), `workbench.nextTab` / `previousTab` cycle. Editor commands
(`editor.*`) go to the active editor. The editor area draws the tab strip
(`ui::EditorTab` with the file icon, the unsaved dot and a close button),
the breadcrumbs of the file from the workspace root, and the tab. The
status bar shows the active editor's position, indentation, encoding and
line endings (LF, CRLF or Mixed).

**Closing** (`closing.rs`). `workbench.closeWindow`, the platform's
close (`Workbench::should_close`, registered by the application), our
Linux close button and `app.quit` ask first when files are unsaved:
Save all / Don't save / Cancel. The window closes only once every
chosen save worked. `save_editor` asks before overwriting a file that
changed on disk. Closing a tab with Save waits for the save too. Above
an editor whose file changed or was deleted on disk, a `ui::NoticeBar`
offers its commands (ADR 0004).

**Commands.** Every action is a command (`command_ids`):
`workbench.toggleSidebar`, `workbench.togglePanel`, `workbench.show*`.
`register_commands` adds them to the registry. `CommandHandlers`
(`command_handlers.rs`) maps ids to what runs: one table per window,
filled when the window opens and read-only afterwards.
`Workbench::register_handlers` registers the workbench commands and
routes every editor command to the active editor; the shell registers
its own. Buttons call the `Dispatch` callback given by the application,
which looks the id up in the table.

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
control marks, tooltips for link cycles and unreadable folders; activating a
file emits `OpenFile`, which the application opens in an editor. It watches
the workspace and emits `WorkspaceSettingsChanged` when
`.ferrum/settings.toml` changes. File icons come from the `FileIcons`
registry (by name, then extension; generic icon otherwise), filled by
contributors at startup. `path_prompt.rs` is the dialog for typing
a folder path. The application passes the open folder (`set_workspace`), the
recent list (`set_recent`) and an `OpenPath` callback.

**Outline** (`outline.rs`, `outline_view.rs`). The Outline section sits at
the bottom of the Explorer, as in `editor.svg`: a divider, a collapsible
header, and a virtualized list (`ui::ListRow::code`) of the active
editor's `OutlineItem`s, indented by depth. Each item has a kind glyph
(`symbol_icons.rs`). The list grows with its items up to
`outline.max_height`. A click goes to the item, Enter also focuses the
editor. The commands are `workbench.focusOutline` and
`workbench.toggleOutline`. The breadcrumbs end with the items around the
cursor (`ƒ main`).

**Menus** (`contrib/menu_bar/`, ADR 0008). `MENUS` lists File, Edit,
View and Go as rows of existing command ids and separators. Rows whose
command is not registered are left out, as are separators that would
lead, trail or double. `MenuView` shows the titles and first shortcuts,
styled like the palette. A click on a title bar menu button toggles it, and
`workbench.openMenuBar` (F10) opens File with its first row selected. Up
and Down skip separators and wrap, Left and Right switch menus
(`MenuEvent::Switch`), Enter or Space runs, Escape closes. While a menu is
open, hovering another menu button opens that one. A backdrop below the
title bar closes it on a click. Running a row closes the menu, gives the
focus back, then dispatches the command (`overlays.rs`, `OpenMenu`). A new
menu entry is one more row in `MENUS`.

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
