# Strings of the application shell.
# The product name is provided by the `i18n` crate as the `-product-name` term.

app-title = { -product-name }

## Commands

command-app-quit = Quit { -product-name }

## Workbench

app-wordmark = { -product-wordmark }
menu-file = File
menu-edit = Edit
menu-view = View
menu-go = Go
titlebar-search-placeholder = Search commands and settings
titlebar-open-folder = Open folder
titlebar-run-project = Run project
view-explorer = Explorer
view-search = Search
view-source-control = Source control
view-run-and-debug = Run and debug
view-extensions = Extensions
panel-terminal = Terminal
panel-problems = Problems
panel-output = Output
panel-debug-console = Debug console
status-no-folder = No folder open
command-workbench-toggle-sidebar = Toggle sidebar
command-workbench-toggle-panel = Toggle panel
command-workbench-show-explorer = Show Explorer
command-workbench-show-search = Show Search
command-workbench-show-source-control = Show Source control
command-workbench-show-run-and-debug = Show Run and debug
command-workbench-show-extensions = Show Extensions
command-workbench-quick-open = Search files, symbols and commands
command-workbench-show-command-palette = Show all commands
quick-open-commands-placeholder = Type the name of a command
quick-open-no-results = Nothing found
command-workbench-toggle-component-gallery = Toggle component gallery

## Workspace

window-title-workspace = { $name } — { -product-name }
command-workspace-open-folder = Open folder
command-workspace-open-folder-by-path = Open folder by path
command-preferences-open-settings = Open Settings
command-preferences-open-keymap = Open Keyboard Shortcuts
config-settings-header =
    Settings. They apply when the file is saved.
    Example:
    {"["}editor]
    tab_size = 4
config-keymap-header =
    Keyboard shortcuts. They apply when the file is saved.
    Example:
    {"["}editor]
    "ctrl-d" = "editor.addCursorBelow"
path-prompt-title = Open folder
path-prompt-placeholder = Folder path
path-prompt-open = Open
path-prompt-cancel = Cancel
open-folder-picker-unavailable = The system folder picker is not available ({ $error }). Type the folder path instead.
open-folder-not-found = The folder { $path } does not exist.
open-folder-not-a-folder = { $path } is not a folder.
open-folder-failed = Cannot open { $path }: { $error }
explorer-empty-title = No folder opened
explorer-empty-text = Open a folder to explore your project files.
explorer-files = Files
explorer-outline = Outline
outline-empty = No symbols in the active editor
file-tree-cycle = Links to one of its parent folders; not expanded
file-tree-failed = Cannot read this folder

## Welcome

welcome-tab = Welcome
welcome-title = A little focus. A lot of Rust.
welcome-subtitle = Your next great project starts here.
welcome-new-project = New Rust project
welcome-clone = Clone repository
welcome-later = Coming in a later version
welcome-recent = Recent projects
welcome-recent-empty-title = A clean slate
welcome-recent-empty-text = Your recently opened projects will appear here.
welcome-get-started = Get started
welcome-start-palette = Command palette
welcome-start-find-file = Find a file
welcome-start-settings = Open settings
welcome-start-terminal = Toggle terminal
welcome-workflow-title = Built for your workflow
welcome-workflow-text = Rust-analyzer, Cargo and Git. Ready when you are.
welcome-docs-link = Read the documentation

## Component gallery (temporary, stage 1)

gallery-title = Component gallery
gallery-note = Temporary screen for comparing components with the mockups. Hover, press and Tab through the "Live" column.
gallery-section-colors = Color tokens
gallery-section-syntax = Syntax tokens
gallery-section-typography = Typography
gallery-section-buttons = Buttons
gallery-section-badges = Badges
gallery-section-tabs = Tabs
gallery-section-panels = Panels and section headers
gallery-section-lists = Lists and activity bar
gallery-section-inputs = Inputs
gallery-section-icons = Icons
gallery-state-normal = Normal
gallery-state-hover = Hover
gallery-state-pressed = Pressed
gallery-state-focused = Focus
gallery-state-disabled = Disabled
gallery-state-live = Live
gallery-variant-primary = Primary
gallery-variant-secondary = Secondary
gallery-variant-ghost = Ghost
gallery-variant-icon = Icon button
gallery-variant-medium = Primary, large
gallery-type-display = A little focus. A lot of Rust.
gallery-type-heading = Language intelligence for Rust
gallery-type-title = Make your workspace yours
gallery-type-body = Your next great project starts here.
gallery-type-secondary = Open a folder to explore your project files.
gallery-type-caption = Your recently opened projects will appear here.
gallery-type-mono = let config = config::load(&cli.config)?;
gallery-sample-run-project = Run project
gallery-sample-open-folder = Open folder
gallery-sample-new-project = New Rust project
gallery-sample-explorer = Explorer
gallery-sample-files = Files
gallery-sample-outline = Outline
gallery-sample-terminal = Terminal
gallery-sample-problems = Problems
gallery-sample-output = Output
gallery-sample-installed = Installed
gallery-sample-marketplace = Marketplace
gallery-sample-search-placeholder = Search files, symbols and commands
gallery-sample-field-placeholder = Search extensions
gallery-sample-commit-placeholder = Commit message
gallery-sample-card-title = A clean slate
gallery-sample-card-text = Your recently opened projects will appear here.
gallery-sample-row-selected = Selected
gallery-sample-row-ignored = Ignored
gallery-sample-row-nested = Nested
gallery-sample-tab-active = Active
gallery-sample-tab-dirty = Unsaved

## Editor

command-editor-move-left = Move cursor left
command-editor-move-right = Move cursor right
command-editor-move-up = Move cursor up
command-editor-move-down = Move cursor down
command-editor-move-word-left = Move cursor to the previous word
command-editor-move-word-right = Move cursor to the next word
command-editor-move-to-line-start = Move cursor to the line start
command-editor-move-to-line-end = Move cursor to the line end
command-editor-move-to-document-start = Move cursor to the start of the file
command-editor-move-to-document-end = Move cursor to the end of the file
command-editor-page-up = Page up
command-editor-page-down = Page down
command-editor-select-left = Select left
command-editor-select-right = Select right
command-editor-select-up = Select up
command-editor-select-down = Select down
command-editor-select-word-left = Select to the previous word
command-editor-select-word-right = Select to the next word
command-editor-select-to-line-start = Select to the line start
command-editor-select-to-line-end = Select to the line end
command-editor-select-to-document-start = Select to the start of the file
command-editor-select-to-document-end = Select to the end of the file
command-editor-select-page-up = Select a page up
command-editor-select-page-down = Select a page down
command-editor-add-cursor-above = Add cursor above
command-editor-add-cursor-below = Add cursor below
command-editor-single-cursor = Keep only the primary cursor
command-editor-select-all = Select all
command-editor-undo = Undo
command-editor-redo = Redo
command-editor-copy = Copy
command-editor-cut = Cut
command-editor-paste = Paste
command-editor-delete-left = Delete left
command-editor-delete-right = Delete right
command-editor-delete-word-left = Delete the previous word
command-editor-delete-word-right = Delete the next word
command-editor-newline = New line
command-editor-indent = Indent
command-editor-outdent = Outdent
command-editor-save = Save
command-editor-reload-from-disk = Reload from disk
command-editor-keep-local-changes = Keep my changes
command-editor-line-endings-lf = Convert line endings to LF
command-editor-line-endings-crlf = Convert line endings to CRLF
command-editor-trigger-suggest = Trigger Suggest
command-editor-show-hover = Show Hover
command-editor-go-to-definition = Go to Definition
command-workbench-close-tab = Close tab
command-workbench-next-tab = Next tab
command-workbench-previous-tab = Previous tab
command-workbench-close-window = Close window
command-workbench-focus-outline = Focus outline
command-workbench-toggle-outline = Toggle outline
command-workbench-open-menu-bar = Open Menu Bar
command-workbench-toggle-terminal = Toggle terminal
command-workbench-toggle-maximized-panel = Maximize or restore panel
command-terminal-new = New terminal
command-terminal-split = Split terminal
command-terminal-kill = Kill terminal
command-terminal-select-shell = Select shell
command-terminal-focus-next = Next terminal
command-terminal-focus-previous = Previous terminal
command-terminal-copy = Copy from terminal
command-terminal-paste = Paste into terminal
command-terminal-select-all = Select all in terminal
command-terminal-clear = Clear terminal
command-terminal-scroll-page-up = Scroll terminal page up
command-terminal-scroll-page-down = Scroll terminal page down
command-terminal-scroll-to-top = Scroll terminal to top
command-terminal-scroll-to-bottom = Scroll terminal to bottom
editor-close-unsaved = Save changes to { $name }?
editor-close-unsaved-detail = Your changes will be lost if you don't save them.
editor-untitled = Untitled
editor-close-save = Save
editor-close-discard = Don't save
editor-close-cancel = Cancel
editor-overwrite = { $name } changed on disk. Overwrite it with your version?
editor-overwrite-detail = The changes on disk will be lost.
editor-overwrite-confirm = Overwrite
window-close-unsaved = { $count ->
    [one] Save changes to { $count } file?
   *[other] Save changes to { $count } files?
}
window-close-save-all = Save all
editor-disk-changed = The file changed on disk.
editor-disk-deleted = The file was deleted on disk.
editor-disk-reload = Reload from disk
editor-disk-keep = Keep mine
editor-disk-save-again = Save again
editor-disk-close = Close
editor-open-binary = { $name } is a binary file and cannot be opened as text.
editor-open-not-utf8 = { $name } is not UTF-8 text. Other encodings are not supported yet.
editor-open-failed = Cannot open { $name }: { $error }
editor-save-failed = Cannot save { $name }: { $error }
status-position = Ln { $line }, Col { $column }
status-selections = { $count } selections
status-indent-spaces = Spaces: { $size }
status-indent-tabs = Tab size: { $size }
status-encoding = UTF-8
status-line-ending-lf = LF
status-line-ending-crlf = CRLF
status-line-ending-mixed = Mixed
status-errors = { $count ->
    [one] { $count } error
   *[other] { $count } errors
}
status-warnings = { $count ->
    [one] { $count } warning
   *[other] { $count } warnings
}
status-toolchain-ready = { $language } toolchain ready
status-server-starting = { $name }: starting
status-server-indexing = { $name }: { $detail }
status-server-failed = { $name } stopped.
status-server-not-found = { $name } was not found: { $program }
lang-rust-analyzer-not-installed = rust-analyzer is not installed for this toolchain. Run: rustup component add rust-analyzer
lang-rust-edition = { $value } edition
problems-empty = No problems have been detected in the workspace.
problems-position = Ln { $line }, Col { $column }
terminal-no-shell = No shell was found on this computer.
terminal-finding-shells = Looking for shells…
terminal-start-failed = Cannot start { $shell }: { $error }
terminal-hint-toggle = Toggle terminal
terminal-hint-palette = Command palette
command-explorer-new-file = New file
new-file-title = New file
new-file-placeholder = File name, for example src/main.rs
new-file-create = Create
new-file-error-empty = Type a file name.
new-file-error-outside = The file must stay inside the selected folder.
new-file-error-exists = A file or folder with this name already exists.
new-file-error-failed = Cannot create the file: { $error }

## Search view
search-placeholder = Search
search-match-case = Match case
search-whole-word = Match whole word
search-regex = Use regular expression
search-replace = Replace
search-include = Files to include
search-include-placeholder = e.g. src/**/*.rs, !target
search-searching = Searching…
search-no-results = No results found
search-no-folder = Open a folder to search its files.
search-summary = { $results ->
    [one] { $results } result
   *[other] { $results } results
} in { $files ->
    [one] { $files } file
   *[other] { $files } files
}
search-summary-limited = First { $results } results in { $files ->
    [one] { $files } file
   *[other] { $files } files
} (search stopped at the limit)
search-invalid-pattern = Invalid regular expression: { $error }
search-invalid-glob = Invalid file pattern: { $error }
search-in = Search in { $name }

## Run and Debug view
debug-no-configuration = No launch configuration
debug-variables = Variables
debug-variables-empty = Not paused.
debug-watch = Watch
debug-watch-empty = No watch expressions.
debug-call-stack = Call stack
debug-call-stack-empty = No debug session.
debug-breakpoints = Breakpoints
debug-breakpoints-empty = No breakpoints.
debug-all-exceptions = All exceptions
debug-uncaught-exceptions = Uncaught exceptions
