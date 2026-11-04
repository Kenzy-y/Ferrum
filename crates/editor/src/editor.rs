//! The editor entity: a document with its view state, and the commands.

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    ClipboardEntry, ClipboardItem, Context, CursorStyle, EventEmitter, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, MouseButton, ParentElement as _, Render, Styled as _,
    Subscription, Task, Window, div,
};
use serde::{Deserialize, Serialize};
use text::{Document, LineEndings, Selection, Selections};

use crate::actions::{self, Action};
use crate::element::EditorElement;
use crate::file::FileState;
use crate::layout::LayoutSnapshot;
use crate::mouse::Drag;
use crate::settings::EditorSettings;

/// What an editor reports to its owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorEvent {
    /// The text changed; whether it is saved may have changed too.
    Edited,
    /// Cursors or selections moved.
    SelectionsChanged,
    /// The file was saved.
    Saved,
    /// Saving failed, with the reason.
    SaveFailed(String),
    /// The file changed on disk and was read again.
    Reloaded,
}

/// What the status bar shows about an editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorStatus {
    /// Line of the primary cursor, from 1.
    pub line: usize,
    /// Column of the primary cursor in characters, from 1.
    pub column: usize,
    /// Number of cursors and selections.
    pub selections: usize,
    /// Columns per indentation level.
    pub tab_size: usize,
    /// Whether Tab inserts spaces.
    pub insert_spaces: bool,
    /// Line endings of the text.
    pub line_endings: LineEndings,
}

/// Clipboard metadata: the text is whole lines copied without a selection.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct ClipboardMetadata {
    whole_lines: bool,
}

/// Cursor blinking: shown or hidden, and the timer that flips it.
#[derive(Default)]
pub(crate) struct Blink {
    pub(crate) visible: bool,
    /// Changes whenever the timer restarts; an old timer sees a different
    /// epoch and stops.
    epoch: u64,
    _timer: Option<Task<()>>,
}

/// A text editor.
pub struct Editor {
    pub(crate) document: Document,
    pub(crate) settings: EditorSettings,
    pub(crate) focus_handle: FocusHandle,
    /// Whether the editor has focus in an active window.
    pub(crate) focused: bool,
    /// Scroll position in pixels from the top left of the text.
    pub(crate) scroll_x: f32,
    pub(crate) scroll_y: f32,
    /// The next frame scrolls to the primary cursor.
    pub(crate) autoscroll: bool,
    pub(crate) blink: Blink,
    /// Text being composed by an input method (byte range).
    pub(crate) marked: Option<Range<usize>>,
    pub(crate) drag: Option<Drag>,
    /// Geometry of the last frame, for mouse and input method positions.
    pub(crate) layout: Option<Rc<LayoutSnapshot>>,
    pub(crate) file: Option<FileState>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<EditorEvent> for Editor {}

impl Focusable for Editor {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Editor {
    /// An editor of `document`. With `file`, saving writes there and the
    /// text is read again when the file changes on disk while there are no
    /// unsaved edits.
    pub fn new(
        document: Document,
        file: Option<(Arc<dyn fs::Fs>, PathBuf)>,
        settings: EditorSettings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        let subscriptions = vec![
            cx.on_focus(&focus_handle, window, |editor, window, cx| {
                editor.set_focused(window.is_window_active(), cx);
            }),
            cx.on_blur(&focus_handle, window, |editor, _window, cx| {
                editor.set_focused(false, cx);
            }),
            cx.observe_window_activation(window, |editor, window, cx| {
                let focused = window.is_window_active() && editor.focus_handle.is_focused(window);
                editor.set_focused(focused, cx);
            }),
        ];
        let version = document.version();
        let mut editor = Self {
            document,
            settings,
            focus_handle,
            focused: false,
            scroll_x: 0.0,
            scroll_y: 0.0,
            autoscroll: false,
            blink: Blink {
                visible: true,
                ..Blink::default()
            },
            marked: None,
            drag: None,
            layout: None,
            file: None,
            _subscriptions: subscriptions,
        };
        if let Some((fs, path)) = file {
            editor.file = Some(FileState::new(fs, path, version, cx));
        }
        editor
    }

    /// The document.
    pub fn document(&self) -> &Document {
        &self.document
    }

    /// The file, if the text belongs to one.
    pub fn path(&self) -> Option<&Path> {
        self.file.as_ref().map(FileState::path)
    }

    /// Whether there are edits not saved to the file.
    pub fn is_dirty(&self) -> bool {
        self.file
            .as_ref()
            .is_some_and(|file| file.saved_version() != self.document.version())
    }

    /// Moves keyboard focus to the editor.
    pub fn focus(&self, window: &mut Window, cx: &mut gpui::App) {
        self.focus_handle.focus(window, cx);
    }

    /// Position and format of the text for the status bar.
    pub fn status(&self) -> EditorStatus {
        let buffer = self.document.buffer();
        let head = self.document.selections().primary().head;
        let point = buffer.offset_to_point(head);
        EditorStatus {
            line: point.row + 1,
            column: buffer.char_column(head) + 1,
            selections: self.document.selections().len(),
            tab_size: self.settings.tab_size,
            insert_spaces: self.settings.insert_spaces,
            line_endings: buffer.line_endings(),
        }
    }

    /// Applies changed settings.
    pub fn set_settings(&mut self, settings: EditorSettings, cx: &mut Context<Self>) {
        if settings != self.settings {
            self.settings = settings;
            self.restart_blink(cx);
            cx.notify();
        }
    }

    /// Runs editor command `id`; `false` if it is not one.
    pub fn run_command(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(action) = actions::action_for(id) else {
            return false;
        };
        self.perform(action, window, cx);
        true
    }

    fn perform(&mut self, action: Action, _window: &mut Window, cx: &mut Context<Self>) {
        let indent = self.settings.indent();
        let page_rows = self.page_rows();
        match action {
            Action::Move(motion, extend) => self.change(cx, |document| {
                document.move_cursors(motion.movement(page_rows), extend);
            }),
            Action::AddCursorAbove => {
                self.change(cx, |document| document.add_cursor_vertically(true));
            }
            Action::AddCursorBelow => {
                self.change(cx, |document| document.add_cursor_vertically(false));
            }
            Action::SingleCursor => self.change(cx, |document| {
                let primary = document.selections().primary();
                let single = if document.selections().len() > 1 {
                    primary
                } else {
                    Selection::cursor(primary.head)
                };
                document.set_selections(&Selections::single(single));
            }),
            Action::SelectAll => self.change(cx, Document::select_all),
            Action::Undo => self.change(cx, |document| {
                document.undo();
            }),
            Action::Redo => self.change(cx, |document| {
                document.redo();
            }),
            Action::Copy => write_clipboard(&self.document.copy_text(), cx),
            Action::Cut => {
                let mut copied = None;
                self.change(cx, |document| copied = Some(document.cut()));
                if let Some(copied) = copied {
                    write_clipboard(&copied, cx);
                }
            }
            Action::Paste => self.paste(cx),
            Action::DeleteLeft => self.change(cx, Document::delete_backward),
            Action::DeleteRight => self.change(cx, Document::delete_forward),
            Action::DeleteWordLeft => self.change(cx, Document::delete_word_backward),
            Action::DeleteWordRight => self.change(cx, Document::delete_word_forward),
            Action::Newline => self.change(cx, Document::newline),
            Action::Indent => self.change(cx, |document| document.indent(indent)),
            Action::Outdent => self.change(cx, |document| document.outdent(indent)),
            Action::Save => self.save(cx),
            Action::LineEndings(ending) => {
                self.change(cx, |document| document.set_line_endings(ending));
            }
        }
    }

    fn paste(&mut self, cx: &mut Context<Self>) {
        let Some(item) = cx.read_from_clipboard() else {
            return;
        };
        let Some(text) = item.text() else {
            return;
        };
        let whole_lines = item.entries().iter().any(|entry| match entry {
            ClipboardEntry::String(string) => string
                .metadata_json::<ClipboardMetadata>()
                .is_some_and(|metadata| metadata.whole_lines),
            _ => false,
        });
        let cursors_only = self.document.selections().iter().all(Selection::is_empty);
        self.change(cx, |document| {
            if whole_lines && cursors_only && document.selections().len() == 1 {
                document.paste_lines(&text);
            } else {
                document.paste(&text);
            }
        });
    }

    /// Runs `change` on the document and reports what it changed: the
    /// view follows the cursor, the cursor shows, owners hear about it.
    pub(crate) fn change(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut Document)) {
        let version = self.document.version();
        let selections = self.document.selections().clone();
        change(&mut self.document);
        let edited = self.document.version() != version;
        if edited {
            self.marked = None;
            cx.emit(EditorEvent::Edited);
        }
        if edited || *self.document.selections() != selections {
            cx.emit(EditorEvent::SelectionsChanged);
        }
        self.autoscroll = true;
        self.restart_blink(cx);
        cx.notify();
    }

    /// Lines moved by Page Up and Page Down: the visible rows less one.
    fn page_rows(&self) -> usize {
        self.layout
            .as_ref()
            .map_or(1, |layout| layout.page_rows())
            .max(1)
    }

    fn set_focused(&mut self, focused: bool, cx: &mut Context<Self>) {
        if self.focused != focused {
            self.focused = focused;
            self.restart_blink(cx);
            cx.notify();
        }
    }

    /// Shows the cursor and starts blinking again from the beginning; no
    /// timer runs while the editor is unfocused or blinking is off.
    pub(crate) fn restart_blink(&mut self, cx: &mut Context<Self>) {
        let epoch = self.blink.epoch + 1;
        let timer = (self.focused && self.settings.cursor_blink).then(|| {
            let interval = Duration::from_millis(self.settings.cursor_blink_interval_ms.max(1));
            cx.spawn(async move |editor, cx| {
                loop {
                    cx.background_executor().timer(interval).await;
                    let flipped = editor.update(cx, |editor, cx| {
                        if editor.blink.epoch != epoch {
                            return false;
                        }
                        editor.blink.visible = !editor.blink.visible;
                        cx.notify();
                        true
                    });
                    if !matches!(flipped, Ok(true)) {
                        break;
                    }
                }
            })
        });
        // Replacing the blink drops the old timer.
        self.blink = Blink {
            visible: true,
            epoch,
            _timer: timer,
        };
    }
}

fn write_clipboard(copied: &text::CopiedText, cx: &mut Context<Editor>) {
    let item = if copied.whole_lines {
        ClipboardItem::new_string_with_json_metadata(
            copied.text.clone(),
            ClipboardMetadata { whole_lines: true },
        )
    } else {
        ClipboardItem::new_string(copied.text.clone())
    };
    cx.write_to_clipboard(item);
}

impl Render for Editor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("editor")
            .key_context(actions::CONTEXT)
            .track_focus(&self.focus_handle)
            .size_full()
            .cursor(CursorStyle::IBeam)
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_scroll(cx.listener(Self::on_scroll))
            .child(EditorElement::new(cx.entity()))
    }
}
