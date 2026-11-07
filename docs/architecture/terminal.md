# Terminal

Crate `terminal` (services layer, no GPUI) and the view in
`workbench/src/contrib/terminal` (ADR 0010).

**Session.** `TerminalSession::spawn(runner, LaunchOptions)` starts the
shell through `ProcessRunner::spawn_pty` (the only place a PTY is made)
and the `alacritty_terminal` event loop on its own thread, which reads
the PTY and feeds the emulator. Events (`TerminalEvent`: `Wakeup`,
`Title`, `Bell`, `CopyToClipboard`, `Exited`) arrive on an unbounded
channel; only one `Wakeup` is sent until the UI calls `redrawn()`, so a
flood of output costs one redraw per frame and a hidden terminal costs
nothing. Answers to programs (cursor position, colors, size) are written
back on the emulator thread. `write`, `paste` (bracketed paste when
asked), `resize`, `scroll`, selections (`point_at`, `start_selection`,
`extend_selection`, `selected_text`, `select_all`) and `clear` lock the
emulator briefly. Dropping the session ends the shell.

**Snapshot.** `snapshot()` returns the visible rows as `Span`s (cells of
one style, resolved `Rgb` colors from the theme `Palette` and OSC
overrides, wide characters as spans of their own, blank cells left out),
the selected columns per row and the cursor.

**Keys.** `keys::encode` turns a GPUI-named key with modifiers into xterm
bytes (arrows follow application cursor mode); plain text is left to the
platform input (IME). `keys::belongs_to_program` decides which keys skip
global bindings: Ctrl or Alt with a character.

**Profiles.** `profiles::detect` finds shells through `Fs`: on Windows
PowerShell 7, Windows PowerShell, cmd, Git Bash, WSL; on Unix `$SHELL`
then `/etc/shells` (macOS starts them as login shells). `choose` applies
`[terminal] shell` (a profile name or a program path).

**View.** `TerminalPanel` (entity, owned by the shell, shown by the
workbench) holds terminal tabs, each with one or more panes side by side,
the detected profiles, the shell picker and the start error. Sessions
start on a background executor. `TerminalView` is one terminal (key
context `terminal`): key handling, `EntityInputHandler` for text and IME,
mouse selection (double click words, triple lines), wheel scrolling
(arrow keys on the alternate screen), cursor blink only while focused.
`TerminalElement` sizes the grid to its bounds, resizes the session and
paints backgrounds, selection, text and the cursor.

**Flicker.** Shells (and `ConPTY` around every redraw) hide the cursor
while they rewrite a line. `Cursor::hidden` marks it; `TerminalView::frame`
keeps drawing the last frame with the cursor shown for
`HIDDEN_CURSOR_GRACE` (60 ms), then draws the live screen, so typing does
not flicker and full-screen programs that keep the cursor hidden are only
delayed by that much.

**Layouts.** Ctrl or Alt with a letter of another layout (`с` on a Russian
one) is translated to the US key on the same physical key by asking
`cx.keyboard_mapper()` at key time; text that the platform still sends
while Ctrl (without AltGr) is held is dropped.

**Folders.** `TerminalPanel::set_working_directory` with another folder
ends the running shells and starts one terminal in the new folder.

**Keyboard routing.** The shell's capture-phase handler runs the keymap
for a focused terminal before the view sees the key, except keys that
belong to the program and are not bound in `[terminal]`.

**Extending.** Mouse reporting to programs and links are next (ROADMAP).
A new terminal command is an id in `workbench::command_ids`, a handler in
`contrib/terminal/workbench.rs` and a title in i18n.
