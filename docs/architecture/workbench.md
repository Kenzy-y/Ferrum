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

**Window chrome.** `Workbench::window_options()` hides the platform title
bar. Windows: drag and min/max/close use native hit-test areas
(`window_control_area`), so Snap layouts work. Linux: client-side
decorations, our buttons, `start_window_move`, and `window_border` from
gpui-component for the resize frame; with server-side decorations our
buttons are hidden. macOS: system traffic lights over the left inset.

**Extending.** New sidebar view: add a `SidebarView` variant, its icon,
title, command id and keybinding. View contents, status bar items and panel
contents will be contributed by their crates as the stages arrive.
