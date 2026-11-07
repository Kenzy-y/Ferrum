//! One terminal: a session drawn by [`TerminalElement`], with keyboard,
//! text input (IME), mouse selection, scrolling and the clipboard.

use std::ops::Range;
use std::time::{Duration, Instant};

use futures::StreamExt as _;
use futures::channel::mpsc::UnboundedReceiver;
use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, EntityInputHandler, EventEmitter,
    FocusHandle, Focusable, InteractiveElement as _, IntoElement, KeyDownEvent, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement as _, Pixels, Point, Render,
    ScrollEvent, Styled as _, Subscription, Task, UTF16Selection, Window, div, px,
};
use terminal::keys::{self, KeyModes};
use terminal::{
    ScrollTarget, SelectionKind, ShellProfile, Snapshot, TerminalEvent, TerminalSession,
    TerminalSettings,
};

use super::element::{CellLayout, TerminalElement};

/// Key context of a focused terminal in `keymap.toml`.
const CONTEXT: &str = "terminal";

/// What a terminal tells its panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TerminalViewEvent {
    /// The shell exited.
    Exited,
    /// The terminal got the keyboard focus.
    Focused,
}

/// Cursor blinking: the phase and the timer flipping it.
struct Blink {
    visible: bool,
    epoch: u64,
    _timer: Option<Task<()>>,
}

/// A terminal of the panel.
pub(crate) struct TerminalView {
    pub(crate) session: TerminalSession,
    pub(crate) profile: ShellProfile,
    pub(crate) focus_handle: FocusHandle,
    pub(crate) focused: bool,
    settings: TerminalSettings,
    /// Cell geometry of the last frame, for the mouse and the IME.
    pub(crate) layout: Option<CellLayout>,
    blink: Blink,
    /// A mouse drag is selecting.
    selecting: bool,
    /// Wheel movement not yet worth a whole line.
    scroll_remainder: f32,
    /// Text being composed by an input method, drawn at the cursor.
    pub(crate) marked: Option<String>,
    /// The shell exited (the terminal stays open without
    /// `[terminal] close_on_exit`).
    pub(crate) exited: bool,
    _events: Task<()>,
    _subscriptions: Vec<Subscription>,
    /// The last frame with the cursor shown, and when it was taken.
    stable: Option<(Snapshot, Instant)>,
    /// Redraws once the grace for a hidden cursor is over.
    grace_timer: Option<Task<()>>,
}

/// How long a frame with the cursor hidden by the program is replaced by
/// the last frame with the cursor shown. Shells (and `ConPTY` around every
/// redraw) hide the cursor while they rewrite the line; drawing those
/// half-written frames makes the line and the cursor flicker while typing.
/// Programs that keep the cursor hidden (full-screen ones) are drawn live
/// after this delay.
const HIDDEN_CURSOR_GRACE: Duration = Duration::from_millis(60);

impl EventEmitter<TerminalViewEvent> for TerminalView {}

impl Focusable for TerminalView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl TerminalView {
    pub(crate) fn new(
        session: TerminalSession,
        mut events: UnboundedReceiver<TerminalEvent>,
        profile: ShellProfile,
        settings: TerminalSettings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        let subscriptions = vec![
            cx.on_focus(&focus_handle, window, |view, window, cx| {
                view.set_focused(window.is_window_active(), cx);
                cx.emit(TerminalViewEvent::Focused);
            }),
            cx.on_blur(&focus_handle, window, |view, _, cx| {
                view.set_focused(false, cx);
            }),
            cx.observe_window_activation(window, |view, window, cx| {
                let focused = window.is_window_active() && view.focus_handle.is_focused(window);
                view.set_focused(focused, cx);
            }),
        ];
        let events = cx.spawn(async move |view, cx| {
            while let Some(event) = events.next().await {
                if view
                    .update(cx, |view, cx| view.session_event(event, cx))
                    .is_err()
                {
                    break;
                }
            }
        });
        Self {
            session,
            profile,
            focus_handle,
            focused: false,
            settings,
            layout: None,
            blink: Blink {
                visible: true,
                epoch: 0,
                _timer: None,
            },
            selecting: false,
            scroll_remainder: 0.0,
            marked: None,
            exited: false,
            _events: events,
            _subscriptions: subscriptions,
            stable: None,
            grace_timer: None,
        }
    }

    fn session_event(&mut self, event: TerminalEvent, cx: &mut Context<Self>) {
        match event {
            TerminalEvent::Wakeup => cx.notify(),
            TerminalEvent::CopyToClipboard(text) => {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
            }
            TerminalEvent::Exited(code) => {
                self.exited = true;
                tracing::debug!(?code, shell = %self.profile.name, "shell exited");
                cx.emit(TerminalViewEvent::Exited);
                cx.notify();
            }
            TerminalEvent::Title(_) | TerminalEvent::ResetTitle | TerminalEvent::Bell => {}
        }
    }

    pub(crate) fn set_settings(&mut self, settings: TerminalSettings, cx: &mut Context<Self>) {
        self.session.set_scrollback_lines(settings.scrollback_lines);
        self.settings = settings;
        self.restart_blink(cx);
    }

    /// Whether the cursor is in the shown phase of blinking.
    /// The frame to draw: the live screen, or the last frame with the
    /// cursor shown while the program hides the cursor for a moment.
    pub(crate) fn frame(&mut self, cx: &mut Context<Self>) -> Snapshot {
        self.session.redrawn();
        let live = self.session.snapshot();
        let now = Instant::now();
        let hidden = live.cursor.as_ref().is_some_and(|cursor| cursor.hidden);
        if !hidden {
            self.stable = Some((live.clone(), now));
            self.grace_timer = None;
            return live;
        }
        let Some((stable, shown_at)) = &self.stable else {
            return live;
        };
        let same_geometry = stable.columns == live.columns
            && stable.rows.len() == live.rows.len()
            && stable.display_offset == live.display_offset;
        let elapsed = now.saturating_duration_since(*shown_at);
        if !same_geometry || elapsed >= HIDDEN_CURSOR_GRACE {
            return live;
        }
        if self.grace_timer.is_none() {
            let wait = HIDDEN_CURSOR_GRACE.saturating_sub(elapsed);
            self.grace_timer = Some(cx.spawn(async move |view, cx| {
                cx.background_executor().timer(wait).await;
                let _ = view.update(cx, |view, cx| {
                    view.grace_timer = None;
                    cx.notify();
                });
            }));
        }
        stable.clone()
    }

    pub(crate) fn cursor_visible(&self) -> bool {
        !self.focused || self.blink.visible
    }

    fn set_focused(&mut self, focused: bool, cx: &mut Context<Self>) {
        if self.focused != focused {
            self.focused = focused;
            self.session.set_focused(focused);
            self.restart_blink(cx);
            cx.notify();
        }
    }

    /// Shows the cursor and blinks from the start; no timer runs while the
    /// terminal is unfocused or blinking is off.
    fn restart_blink(&mut self, cx: &mut Context<Self>) {
        let epoch = self.blink.epoch + 1;
        let timer = (self.focused && self.settings.cursor_blink).then(|| {
            let interval = Duration::from_millis(self.settings.cursor_blink_interval_ms.max(1));
            cx.spawn(async move |view, cx| {
                loop {
                    cx.background_executor().timer(interval).await;
                    let flipped = view.update(cx, |view, cx| {
                        if view.blink.epoch != epoch {
                            return false;
                        }
                        view.blink.visible = !view.blink.visible;
                        cx.notify();
                        true
                    });
                    if !matches!(flipped, Ok(true)) {
                        break;
                    }
                }
            })
        });
        self.blink = Blink {
            visible: true,
            epoch,
            _timer: timer,
        };
    }

    /// Sends typed bytes to the shell.
    fn type_bytes(&mut self, bytes: Vec<u8>, cx: &mut Context<Self>) {
        if self.exited {
            return;
        }
        self.session.write(bytes);
        self.restart_blink(cx);
        cx.notify();
    }

    pub(crate) fn copy(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(text) = self.session.selected_text() else {
            return false;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        true
    }

    pub(crate) fn paste(&mut self, cx: &mut Context<Self>) {
        if self.exited {
            return;
        }
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.session.paste(&text);
            self.restart_blink(cx);
            cx.notify();
        }
    }

    pub(crate) fn select_all(&mut self, cx: &mut Context<Self>) {
        self.session.select_all();
        cx.notify();
    }

    pub(crate) fn clear(&mut self, cx: &mut Context<Self>) {
        self.session.clear();
        cx.notify();
    }

    pub(crate) fn scroll(&mut self, target: ScrollTarget, cx: &mut Context<Self>) {
        self.session.scroll(target);
        cx.notify();
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let modifiers = keys::Modifiers {
            control: keystroke.modifiers.control,
            alt: keystroke.modifiers.alt,
            shift: keystroke.modifiers.shift,
            platform: keystroke.modifiers.platform,
        };
        let key = keystroke.key.to_lowercase();
        // Ctrl C with the keys of another layout is still Ctrl C.
        let key = if modifiers.control || modifiers.alt {
            us_key(&key, cx).map_or(key, str::to_owned)
        } else {
            key
        };
        // Ctrl C copies a selection instead of interrupting, as in other
        // Windows terminals; without a selection it is Ctrl C.
        if modifiers.control && !modifiers.alt && !modifiers.shift && key == "c" && self.copy(cx) {
            self.session.clear_selection();
            cx.stop_propagation();
            cx.notify();
            return;
        }
        let modes = KeyModes {
            app_cursor: self.session.modes().app_cursor,
        };
        if let Some(bytes) = keys::encode(&key, modifiers, modes) {
            cx.stop_propagation();
            self.type_bytes(bytes, cx);
        }
    }

    /// The cell under `position`, and whether it is the cell's right half.
    fn cell_at(&self, position: Point<Pixels>) -> Option<(usize, usize, bool)> {
        let layout = self.layout?;
        let x = (position.x - layout.origin.x).as_f32().max(0.0);
        let y = (position.y - layout.origin.y).as_f32().max(0.0);
        let column = x / layout.cell_width;
        let row = y / layout.line_height;
        // Positions are small and non-negative; truncation is the floor.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let (row, cell) = (row as usize, column as usize);
        Some((row, cell, column.fract() >= 0.5))
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle, cx);
        let Some((row, column, right)) = self.cell_at(event.position) else {
            return;
        };
        let kind = match event.click_count {
            0 | 1 => SelectionKind::Simple,
            2 => SelectionKind::Word,
            _ => SelectionKind::Line,
        };
        let point = self.session.point_at(row, column, right);
        if event.modifiers.shift && kind == SelectionKind::Simple {
            self.session.extend_selection(point);
        } else {
            self.session.start_selection(point, kind);
        }
        self.selecting = true;
        cx.stop_propagation();
        cx.notify();
    }

    fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.selecting || event.pressed_button != Some(MouseButton::Left) {
            return;
        }
        if let Some((row, column, right)) = self.cell_at(event.position) {
            let point = self.session.point_at(row, column, right);
            self.session.extend_selection(point);
            cx.notify();
        }
    }

    fn on_mouse_up(&mut self, _event: &MouseUpEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.selecting) && self.settings.copy_on_select {
            self.copy(cx);
        }
    }

    fn on_scroll(&mut self, event: &ScrollEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(layout) = self.layout else {
            return;
        };
        let delta = event.delta.pixel_delta(px(layout.line_height));
        let lines = delta.y.as_f32() * self.settings.scroll_sensitivity / layout.line_height
            + self.scroll_remainder;
        // Whole lines only; the rest waits for the next event.
        #[allow(clippy::cast_possible_truncation)]
        let whole = lines.trunc() as i32;
        self.scroll_remainder = lines.fract();
        if whole == 0 {
            return;
        }
        let modes = self.session.modes();
        if modes.alt_screen && modes.alternate_scroll {
            // Full-screen programs (less, vim) scroll with arrow keys.
            let key = if whole > 0 { "up" } else { "down" };
            let arrow = KeyModes {
                app_cursor: modes.app_cursor,
            };
            if let Some(bytes) = keys::encode(key, keys::Modifiers::default(), arrow) {
                let bytes = bytes.repeat(whole.unsigned_abs() as usize);
                self.type_bytes(bytes, cx);
            }
        } else {
            self.session.scroll(ScrollTarget::Lines(whole));
            cx.notify();
        }
        cx.stop_propagation();
    }
}

impl Render for TerminalView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("terminal")
            .key_context(CONTEXT)
            .track_focus(&self.focus_handle)
            .size_full()
            .cursor(CursorStyle::IBeam)
            .on_key_down(cx.listener(Self::on_key_down))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_scroll(cx.listener(Self::on_scroll))
            .child(TerminalElement::new(cx.entity()))
    }
}

/// The US key on the same physical key as `key` in the active layout
/// (`с` on a Russian layout is `c`), so that Ctrl C is Ctrl C in any
/// layout. Asks the platform at key time, so a layout switch is never
/// missed. ASCII keys are returned as they are.
fn us_key(key: &str, cx: &App) -> Option<&'static str> {
    if key.is_ascii() {
        return None;
    }
    let mapper = cx.keyboard_mapper();
    commands::US_KEYS.iter().copied().find(|us| {
        let keystroke = gpui::Keystroke {
            modifiers: gpui::Modifiers::default(),
            key: (*us).to_owned(),
            key_char: None,
        };
        mapper.map_key_equivalent(keystroke, true).inner().key == key
    })
}

/// Text typed through the platform input (plain characters, IME) goes
/// straight to the shell; composed text is shown at the cursor until it
/// is committed.
impl EntityInputHandler for TerminalView {
    fn text_for_range(
        &mut self,
        _range: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        *adjusted_range = Some(0..0);
        Some(String::new())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: 0..0,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked
            .as_ref()
            .map(|marked| 0..marked.encode_utf16().count())
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.marked.take().is_some() {
            cx.notify();
        }
    }

    fn replace_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.marked = None;
        // With Ctrl held (and not AltGr, which is Ctrl Alt on Windows) the
        // key was a shortcut: some layouts still send its letter as text.
        let modifiers = window.modifiers();
        if modifiers.control && !modifiers.alt {
            return;
        }
        self.type_bytes(text.as_bytes().to_vec(), cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.marked = (!new_text.is_empty()).then(|| new_text.to_owned());
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        _element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        self.layout.map(|layout| layout.cursor_bounds)
    }

    fn character_index_for_point(
        &mut self,
        _point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}
