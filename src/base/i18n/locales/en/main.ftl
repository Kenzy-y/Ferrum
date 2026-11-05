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
welcome-start-search = Search files and commands
welcome-start-open-folder = Open a folder
welcome-start-panel = Toggle panel
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
