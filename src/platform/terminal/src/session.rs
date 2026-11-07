//! A shell running in a pseudo-terminal, emulated by `alacritty_terminal`.

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use alacritty_terminal::Term;
use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::event_loop::{EventLoop, EventLoopSender, Msg};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::{Config, TermMode};
use alacritty_terminal::vte::ansi::{ClearMode, Handler as _};
use futures::channel::mpsc;
use process::{CommandSpec, ProcessError, ProcessRunner, PtySize};

use crate::palette::Palette;
use crate::profiles::ShellProfile;
use crate::snapshot::{self, Snapshot};

/// What a session tells the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalEvent {
    /// The screen changed. Sent once until [`TerminalSession::redrawn`].
    Wakeup,
    /// The program set the title.
    Title(String),
    /// The program reset the title.
    ResetTitle,
    /// The bell rang.
    Bell,
    /// The program asked to put text on the clipboard (OSC 52).
    CopyToClipboard(String),
    /// The shell exited, with its exit code if it has one.
    Exited(Option<i32>),
}

/// Errors when starting a session.
#[derive(Debug, thiserror::Error)]
pub enum TerminalError {
    /// The shell could not be started.
    #[error(transparent)]
    Process(#[from] ProcessError),
    /// The emulator thread could not be started.
    #[error("cannot start the terminal emulator: {0}")]
    Emulator(#[source] std::io::Error),
}

/// How to start a session.
#[derive(Debug, Clone)]
pub struct LaunchOptions {
    /// The shell.
    pub profile: ShellProfile,
    /// Where it starts.
    pub working_directory: Option<PathBuf>,
    /// Initial size; the UI resizes once it knows its bounds.
    pub size: PtySize,
    /// Lines kept for scrolling back.
    pub scrollback_lines: usize,
    /// Colors, also answered to programs that ask for them.
    pub palette: Palette,
}

/// Terminal modes the UI needs to know.
// Each flag mirrors one terminal mode set by the program.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TerminalModes {
    /// Application cursor keys.
    pub app_cursor: bool,
    /// Pasted text is wrapped in bracketed paste markers.
    pub bracketed_paste: bool,
    /// The alternate screen (full-screen programs) is shown.
    pub alt_screen: bool,
    /// The wheel sends arrow keys on the alternate screen.
    pub alternate_scroll: bool,
    /// The program wants focus in and out reports.
    pub focus_reports: bool,
}

/// Where to scroll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollTarget {
    /// Lines up (positive) or down (negative).
    Lines(i32),
    /// A screen up.
    PageUp,
    /// A screen down.
    PageDown,
    /// The oldest line.
    Top,
    /// The live screen.
    Bottom,
}

/// How a selection grows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionKind {
    /// Character by character (drag).
    Simple,
    /// Word by word (double click).
    Word,
    /// Line by line (triple click).
    Line,
}

/// A cell of the grid, in scrollback coordinates; made by
/// [`TerminalSession::point_at`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellPoint {
    point: Point,
    side: Side,
}

/// Size of the grid, for `alacritty_terminal`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GridSize {
    pub(crate) columns: usize,
    pub(crate) lines: usize,
}

impl Dimensions for GridSize {
    fn total_lines(&self) -> usize {
        self.lines
    }

    fn screen_lines(&self) -> usize {
        self.lines
    }

    fn columns(&self) -> usize {
        self.columns
    }
}

impl From<PtySize> for GridSize {
    fn from(size: PtySize) -> Self {
        Self {
            columns: usize::from(size.columns.max(1)),
            lines: usize::from(size.rows.max(1)),
        }
    }
}

/// State shared by the emulator thread (through [`Listener`]) and the UI.
struct Shared {
    events: mpsc::UnboundedSender<TerminalEvent>,
    /// A wakeup was sent and the UI has not redrawn yet; further wakeups
    /// are dropped, so a flood of output costs one event per frame.
    wakeup_pending: AtomicBool,
    exited: AtomicBool,
    /// Writes answers (cursor position, colors) back to the program.
    sender: OnceLock<EventLoopSender>,
    palette: Mutex<Palette>,
    size: Mutex<WindowSize>,
}

impl Shared {
    fn send(&self, event: TerminalEvent) {
        let _ = self.events.unbounded_send(event);
    }

    fn write(&self, text: String) {
        if let Some(sender) = self.sender.get() {
            let _ = sender.send(Msg::Input(Cow::Owned(text.into_bytes())));
        }
    }

    fn exited(&self, code: Option<i32>) {
        if !self.exited.swap(true, Ordering::AcqRel) {
            self.send(TerminalEvent::Exited(code));
        }
    }
}

/// Receives the emulator's events on its thread.
#[derive(Clone)]
struct Listener(Arc<Shared>);

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        let shared = &self.0;
        match event {
            Event::Wakeup => {
                if !shared.wakeup_pending.swap(true, Ordering::AcqRel) {
                    shared.send(TerminalEvent::Wakeup);
                }
            }
            Event::Title(title) => shared.send(TerminalEvent::Title(title)),
            Event::ResetTitle => shared.send(TerminalEvent::ResetTitle),
            Event::Bell => shared.send(TerminalEvent::Bell),
            Event::ClipboardStore(_, text) => shared.send(TerminalEvent::CopyToClipboard(text)),
            Event::PtyWrite(text) => shared.write(text),
            Event::ColorRequest(index, format) => {
                let color = shared
                    .palette
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .slot(index);
                shared.write(format(color.into()));
            }
            Event::TextAreaSizeRequest(format) => {
                let size = *shared.size.lock().unwrap_or_else(PoisonError::into_inner);
                shared.write(format(size));
            }
            Event::ChildExit(status) => shared.exited(status.code()),
            Event::Exit => shared.exited(None),
            // Programs never read the clipboard: that would hand them
            // whatever the user copied elsewhere.
            Event::ClipboardLoad(..) | Event::MouseCursorDirty | Event::CursorBlinkingChange => {}
        }
    }
}

/// A shell in a pseudo-terminal. Reading the shell's output and emulating
/// it happen on a thread of `alacritty_terminal`; the UI locks the terminal
/// only to take a [`Snapshot`] or to change the selection. Dropping the
/// session ends the shell.
pub struct TerminalSession {
    term: Arc<FairMutex<Term<Listener>>>,
    shared: Arc<Shared>,
    size: PtySize,
}

impl std::fmt::Debug for TerminalSession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TerminalSession")
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

/// Environment of every shell: what terminal it talks to.
const TERMINAL_ENVIRONMENT: &[(&str, &str)] =
    &[("TERM", "xterm-256color"), ("COLORTERM", "truecolor")];

impl TerminalSession {
    /// Starts the shell of `options` and the emulator thread. Spawns a
    /// process: call it off the UI thread.
    ///
    /// # Errors
    ///
    /// Fails if the shell or the emulator thread cannot be started.
    pub fn spawn(
        runner: &dyn ProcessRunner,
        options: LaunchOptions,
    ) -> Result<(Self, mpsc::UnboundedReceiver<TerminalEvent>), TerminalError> {
        let _span = tracing::info_span!("terminal.spawn", shell = %options.profile.name).entered();
        let mut command = CommandSpec::new(&options.profile.program).args(&options.profile.args);
        if let Some(directory) = &options.working_directory {
            command = command.current_dir(directory);
        }
        for (key, value) in TERMINAL_ENVIRONMENT {
            command = command.env(key, value);
        }
        let pty = runner.spawn_pty(&command, options.size)?;

        let (events, receiver) = mpsc::unbounded();
        let shared = Arc::new(Shared {
            events,
            wakeup_pending: AtomicBool::new(false),
            exited: AtomicBool::new(false),
            sender: OnceLock::new(),
            palette: Mutex::new(options.palette),
            size: Mutex::new(options.size.window_size()),
        });
        let config = Config {
            scrolling_history: options.scrollback_lines,
            ..Config::default()
        };
        let term = Term::new(
            config,
            &GridSize::from(options.size),
            Listener(shared.clone()),
        );
        let term = Arc::new(FairMutex::new(term));
        let event_loop = EventLoop::new(
            term.clone(),
            Listener(shared.clone()),
            pty.into_inner(),
            true,
            false,
        )
        .map_err(TerminalError::Emulator)?;
        let _ = shared.sender.set(event_loop.channel());
        // The thread ends when the shell exits or the session is dropped.
        drop(event_loop.spawn());
        Ok((
            Self {
                term,
                shared,
                size: options.size,
            },
            receiver,
        ))
    }

    fn send(&self, message: Msg) {
        if let Some(sender) = self.shared.sender.get() {
            let _ = sender.send(message);
        }
    }

    /// The UI has drawn the latest screen: the next change sends a new
    /// [`TerminalEvent::Wakeup`].
    pub fn redrawn(&self) {
        self.shared.wakeup_pending.store(false, Ordering::Release);
    }

    /// Sends typed bytes to the shell; the view jumps back to the live
    /// screen and the selection is dropped, as typing replaces it.
    pub fn write(&self, bytes: impl Into<Cow<'static, [u8]>>) {
        let bytes = bytes.into();
        if bytes.is_empty() {
            return;
        }
        {
            let mut term = self.term.lock();
            term.scroll_display(Scroll::Bottom);
            term.selection = None;
        }
        self.send(Msg::Input(bytes));
    }

    /// Pastes `text`: wrapped in bracketed paste markers when the program
    /// asked for them, otherwise with line breaks sent as Enter.
    pub fn paste(&self, text: &str) {
        let bracketed = self.term.lock().mode().contains(TermMode::BRACKETED_PASTE);
        let bytes = if bracketed {
            // A pasted end marker would end the paste early.
            let text = text.replace('\x1b', "");
            format!("\x1b[200~{text}\x1b[201~")
        } else {
            text.replace("\r\n", "\r").replace('\n', "\r")
        };
        self.write(bytes.into_bytes());
    }

    /// The current size.
    pub fn size(&self) -> PtySize {
        self.size
    }

    /// Resizes the grid and the pseudo-terminal; nothing happens when the
    /// size is unchanged.
    pub fn resize(&mut self, size: PtySize) {
        if size == self.size {
            return;
        }
        self.size = size;
        *self
            .shared
            .size
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = size.window_size();
        self.term.lock().resize(GridSize::from(size));
        self.send(Msg::Resize(size.window_size()));
    }

    /// Scrolls the view through the scrollback.
    pub fn scroll(&self, target: ScrollTarget) {
        let scroll = match target {
            ScrollTarget::Lines(lines) => Scroll::Delta(lines),
            ScrollTarget::PageUp => Scroll::PageUp,
            ScrollTarget::PageDown => Scroll::PageDown,
            ScrollTarget::Top => Scroll::Top,
            ScrollTarget::Bottom => Scroll::Bottom,
        };
        self.term.lock().scroll_display(scroll);
    }

    /// The visible screen.
    pub fn snapshot(&self) -> Snapshot {
        let palette = self
            .shared
            .palette
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        snapshot::build(&self.term.lock(), &palette)
    }

    /// Replaces the colors, after a theme change.
    pub fn set_palette(&self, palette: Palette) {
        *self
            .shared
            .palette
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = palette;
    }

    /// Changes how many lines are kept for scrolling back.
    pub fn set_scrollback_lines(&self, lines: usize) {
        let mut term = self.term.lock();
        if term.grid().history_size() == lines {
            return;
        }
        term.set_options(Config {
            scrolling_history: lines,
            ..Config::default()
        });
    }

    /// Modes set by the program.
    pub fn modes(&self) -> TerminalModes {
        let term = self.term.lock();
        let mode = term.mode();
        TerminalModes {
            app_cursor: mode.contains(TermMode::APP_CURSOR),
            bracketed_paste: mode.contains(TermMode::BRACKETED_PASTE),
            alt_screen: mode.contains(TermMode::ALT_SCREEN),
            alternate_scroll: mode.contains(TermMode::ALTERNATE_SCROLL),
            focus_reports: mode.contains(TermMode::FOCUS_IN_OUT),
        }
    }

    /// Tells the program the terminal gained or lost focus, if it asked.
    pub fn set_focused(&self, focused: bool) {
        let reports = {
            let mut term = self.term.lock();
            term.is_focused = focused;
            term.mode().contains(TermMode::FOCUS_IN_OUT)
        };
        if reports {
            let report: &'static [u8] = if focused { b"\x1b[I" } else { b"\x1b[O" };
            self.send(Msg::Input(Cow::Borrowed(report)));
        }
    }

    /// The cell at `row` and `column` of the screen, in its left or right
    /// half.
    pub fn point_at(&self, row: usize, column: usize, right_half: bool) -> CellPoint {
        let term = self.term.lock();
        let offset = term.grid().display_offset();
        let line = snapshot::line_of(row.min(term.screen_lines().saturating_sub(1)), offset);
        let column = column.min(term.columns().saturating_sub(1));
        CellPoint {
            point: Point::new(Line(line), Column(column)),
            side: if right_half { Side::Right } else { Side::Left },
        }
    }

    /// Starts a selection at `at`.
    pub fn start_selection(&self, at: CellPoint, kind: SelectionKind) {
        let kind = match kind {
            SelectionKind::Simple => SelectionType::Simple,
            SelectionKind::Word => SelectionType::Semantic,
            SelectionKind::Line => SelectionType::Lines,
        };
        self.term.lock().selection = Some(Selection::new(kind, at.point, at.side));
    }

    /// Moves the free end of the selection to `to`.
    pub fn extend_selection(&self, to: CellPoint) {
        if let Some(selection) = &mut self.term.lock().selection {
            selection.update(to.point, to.side);
        }
    }

    /// Selects everything, the scrollback included.
    pub fn select_all(&self) {
        let mut term = self.term.lock();
        let top = Point::new(term.topmost_line(), Column(0));
        let bottom = Point::new(term.bottommost_line(), term.last_column());
        let mut selection = Selection::new(SelectionType::Lines, top, Side::Left);
        selection.update(bottom, Side::Right);
        term.selection = Some(selection);
    }

    /// Drops the selection.
    pub fn clear_selection(&self) {
        self.term.lock().selection = None;
    }

    /// The selected text, if anything non-empty is selected.
    pub fn selected_text(&self) -> Option<String> {
        let term = self.term.lock();
        let selection = term.selection.as_ref()?;
        if selection.is_empty() {
            return None;
        }
        term.selection_to_string().filter(|text| !text.is_empty())
    }

    /// Clears the scrollback and asks the shell to redraw its prompt at
    /// the top (Ctrl L).
    pub fn clear(&self) {
        {
            let mut term = self.term.lock();
            term.clear_screen(ClearMode::Saved);
            term.selection = None;
        }
        self.send(Msg::Input(Cow::Borrowed(b"\x0c")));
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        self.send(Msg::Shutdown);
    }
}

#[cfg(test)]
mod tests {
    use process::FakeProcessRunner;

    use super::*;
    use crate::palette::Rgb;

    fn options() -> LaunchOptions {
        LaunchOptions {
            profile: ShellProfile::from_program("sh", Vec::new()),
            working_directory: None,
            size: PtySize {
                columns: 80,
                rows: 24,
                cell_width: 8,
                cell_height: 16,
            },
            scrollback_lines: 100,
            palette: Palette {
                foreground: Rgb::new(0, 0, 0),
                background: Rgb::new(0, 0, 0),
                cursor: Rgb::new(0, 0, 0),
                ansi: [Rgb::new(0, 0, 0); 16],
            },
        }
    }

    #[test]
    fn spawning_goes_through_the_process_runner() {
        let runner = FakeProcessRunner::new();
        let mut options = options();
        options.working_directory = Some(PathBuf::from("/project"));
        let error = TerminalSession::spawn(&runner, options).unwrap_err();
        assert!(matches!(error, TerminalError::Process(_)));
        let runs = runner.runs();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].program(), "sh");
        assert_eq!(
            runs[0].working_directory(),
            Some(std::path::Path::new("/project"))
        );
    }
}
