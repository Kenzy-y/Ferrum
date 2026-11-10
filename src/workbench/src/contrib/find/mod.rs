//! The find widget over the active editor (Ctrl F, Ctrl H): the query with
//! Aa / ab / .* toggles, the match count, previous / next, and a replace
//! row. The editor finds and replaces ([`editor::Editor::set_find_query`]);
//! the widget only edits the query and calls it.

use editor::{Editor, EditorEvent, MAX_MATCHES};
use gpui::{
    AppContext as _, Context, Entity, EventEmitter, Focusable as _, InteractiveElement as _,
    IntoElement, KeyDownEvent, MouseButton, ParentElement as _, Render, SharedString, Styled as _,
    Subscription, WeakEntity, Window, div, px,
};
use gpui_component::input::{InputEvent, InputState};
use i18n::{FluentArgs, I18n};
use search::SearchQuery;
use std::sync::Arc;
use theme::ActiveTheme as _;
use ui::{IconButton, IconName, TextField, ToggleButton};

/// Emitted when the widget closes itself (Escape, the close button).
pub(crate) struct CloseFind;

/// Width of the widget, in space tokens `xl`.
const WIDTH_STEPS: f32 = 22.0;

// The three find options and the replace row are independent toggles.
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct FindWidget {
    query: Entity<InputState>,
    replace: Entity<InputState>,
    case_sensitive: bool,
    whole_word: bool,
    regex: bool,
    pub(crate) show_replace: bool,
    editor: WeakEntity<Editor>,
    i18n: Arc<I18n>,
    editor_events: Option<Subscription>,
    _inputs: [Subscription; 2],
}

impl EventEmitter<CloseFind> for FindWidget {}

impl FindWidget {
    pub(crate) fn new(
        editor: &Entity<Editor>,
        initial: Option<&str>,
        show_replace: bool,
        i18n: Arc<I18n>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let query_placeholder = i18n.text("find-placeholder");
        let replace_placeholder = i18n.text("find-replace-placeholder");
        let query = cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder(query_placeholder);
            if let Some(initial) = initial {
                state = state.default_value(initial.to_owned());
            }
            state
        });
        let replace = cx.new(|cx| InputState::new(window, cx).placeholder(replace_placeholder));
        let query_events = cx.subscribe(&query, |widget, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                widget.push_query(cx);
            }
        });
        let replace_events = cx.subscribe(&replace, |_, _, _: &InputEvent, _| {});
        let mut widget = Self {
            query,
            replace,
            case_sensitive: false,
            whole_word: false,
            regex: false,
            show_replace,
            editor: editor.downgrade(),
            i18n,
            editor_events: None,
            _inputs: [query_events, replace_events],
        };
        widget.attach(editor, cx);
        widget.focus_query(window, cx);
        widget
    }

    /// Follows another editor (the active tab changed): the old one loses
    /// its highlights, the new one finds the same query.
    pub(crate) fn attach(&mut self, editor: &Entity<Editor>, cx: &mut Context<Self>) {
        if let Some(old) = self.editor.upgrade()
            && old != *editor
        {
            old.update(cx, |old, cx| old.set_find_query(None, cx));
        }
        self.editor = editor.downgrade();
        self.editor_events = Some(cx.subscribe(editor, |_, _, event: &EditorEvent, cx| {
            if matches!(
                event,
                EditorEvent::FindChanged | EditorEvent::SelectionsChanged
            ) {
                cx.notify();
            }
        }));
        self.push_query(cx);
    }

    pub(crate) fn focus_query(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.query.update(cx, |query, cx| query.focus(window, cx));
    }

    pub(crate) fn set_query_text(
        &mut self,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.query
            .update(cx, |query, cx| query.set_value(text, window, cx));
        self.push_query(cx);
    }

    /// The editor stops finding (the widget is closing).
    pub(crate) fn detach(&mut self, cx: &mut Context<Self>) {
        if let Some(editor) = self.editor.upgrade() {
            editor.update(cx, |editor, cx| editor.set_find_query(None, cx));
        }
    }

    fn search_query(&self, cx: &gpui::App) -> SearchQuery {
        SearchQuery {
            pattern: self.query.read(cx).value().to_string(),
            case_sensitive: self.case_sensitive,
            whole_word: self.whole_word,
            regex: self.regex,
            include: Vec::new(),
        }
    }

    fn push_query(&mut self, cx: &mut Context<Self>) {
        let query = self.search_query(cx);
        if let Some(editor) = self.editor.upgrade() {
            editor.update(cx, |editor, cx| editor.set_find_query(Some(query), cx));
        }
        cx.notify();
    }

    pub(crate) fn next(&mut self, forward: bool, cx: &mut Context<Self>) {
        if let Some(editor) = self.editor.upgrade() {
            editor.update(cx, |editor, cx| editor.find_next(forward, cx));
        }
    }

    fn replace_next(&mut self, cx: &mut Context<Self>) {
        let replacement = self.replace.read(cx).value().to_string();
        if let Some(editor) = self.editor.upgrade() {
            editor.update(cx, |editor, cx| editor.replace_next(&replacement, cx));
        }
    }

    fn replace_all(&mut self, cx: &mut Context<Self>) {
        let replacement = self.replace.read(cx).value().to_string();
        if let Some(editor) = self.editor.upgrade() {
            editor.update(cx, |editor, cx| {
                editor.replace_all(&replacement, cx);
            });
        }
    }

    fn toggle(&mut self, option: FindOption, cx: &mut Context<Self>) {
        let flag = match option {
            FindOption::Case => &mut self.case_sensitive,
            FindOption::Word => &mut self.whole_word,
            FindOption::Regex => &mut self.regex,
        };
        *flag = !*flag;
        self.push_query(cx);
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let modifiers = keystroke.modifiers;
        let in_replace = self.replace.read(cx).focus_handle(cx).is_focused(window);
        match keystroke.key.as_str() {
            "escape" => cx.emit(CloseFind),
            "enter" if in_replace && (modifiers.control || modifiers.platform) && modifiers.alt => {
                self.replace_all(cx);
            }
            "enter" if in_replace => self.replace_next(cx),
            "enter" => self.next(!modifiers.shift, cx),
            _ => return,
        }
        cx.stop_propagation();
    }

    fn count_text(&self, cx: &gpui::App) -> SharedString {
        let Some(editor) = self.editor.upgrade() else {
            return SharedString::default();
        };
        if self.query.read(cx).value().is_empty() {
            return SharedString::default();
        }
        let summary = editor.read(cx).find_summary();
        if summary.invalid {
            return self.i18n.text("find-invalid").into();
        }
        if summary.total == 0 {
            return self.i18n.text("find-no-results").into();
        }
        let mut args = FluentArgs::new();
        args.set("current", summary.current.map_or(0, |current| current + 1));
        args.set("total", summary.total);
        let id = if summary.total >= MAX_MATCHES {
            "find-count-limited"
        } else {
            "find-count"
        };
        self.i18n.text_with(id, &args).into()
    }
}

#[derive(Clone, Copy)]
enum FindOption {
    Case,
    Word,
    Regex,
}

impl Render for FindWidget {
    // One frame of rows and buttons; splitting it would only pass the same
    // closures around.
    #[allow(clippy::too_many_lines)]
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let count = self.count_text(cx);
        let widget = cx.entity().downgrade();
        let on = |handler: fn(&mut Self, &mut Context<Self>)| {
            let widget = widget.clone();
            move |_: &mut Window, cx: &mut gpui::App| {
                let _ = widget.update(cx, handler);
            }
        };
        let toggle = |id: &'static str,
                      label: &'static str,
                      active: bool,
                      option: FindOption,
                      tip: SharedString| {
            let widget = widget.clone();
            ToggleButton::new(id, label)
                .active(active)
                .tooltip(tip)
                .on_activate(move |_, cx| {
                    let _ = widget.update(cx, |widget, cx| widget.toggle(option, cx));
                })
        };
        let theme = cx.theme();
        let metrics = &theme.metrics;
        let colors = &theme.colors;
        let query_row = div()
            .flex()
            .items_center()
            .gap(px(metrics.space.xs))
            .child(
                div().flex_1().child(
                    TextField::new(&self.query)
                        .trailing(toggle(
                            "find-case",
                            "Aa",
                            self.case_sensitive,
                            FindOption::Case,
                            self.i18n.text("find-match-case").into(),
                        ))
                        .trailing(toggle(
                            "find-word",
                            "ab",
                            self.whole_word,
                            FindOption::Word,
                            self.i18n.text("find-whole-word").into(),
                        ))
                        .trailing(toggle(
                            "find-regex",
                            ".*",
                            self.regex,
                            FindOption::Regex,
                            self.i18n.text("find-regex").into(),
                        )),
                ),
            )
            .child(
                div()
                    .min_w(px(metrics.space.xl * 3.0))
                    .text_size(px(metrics.font_size.sm))
                    .text_color(colors.text_subtle)
                    .child(count),
            )
            .child(
                IconButton::new("find-previous", IconName::ArrowUp)
                    .tooltip(self.i18n.text("find-previous"))
                    .on_activate(on(|widget, cx| widget.next(false, cx))),
            )
            .child(
                IconButton::new("find-next", IconName::ArrowDown)
                    .tooltip(self.i18n.text("find-next"))
                    .on_activate(on(|widget, cx| widget.next(true, cx))),
            )
            .child(
                IconButton::new("find-close", IconName::X)
                    .tooltip(self.i18n.text("find-close"))
                    .on_activate(on(|_, cx| cx.emit(CloseFind))),
            );
        let replace_row = self.show_replace.then(|| {
            div()
                .flex()
                .items_center()
                .gap(px(metrics.space.xs))
                .child(div().flex_1().child(TextField::new(&self.replace)))
                .child(
                    IconButton::new("find-replace-one", IconName::ArrowRight)
                        .tooltip(self.i18n.text("find-replace"))
                        .on_activate(on(Self::replace_next)),
                )
                .child(
                    IconButton::new("find-replace-all", IconName::Check)
                        .tooltip(self.i18n.text("find-replace-all"))
                        .on_activate(on(Self::replace_all)),
                )
        });
        let expand = IconButton::new(
            "find-toggle-replace",
            if self.show_replace {
                IconName::ChevronDown
            } else {
                IconName::ChevronRight
            },
        )
        .tooltip(self.i18n.text("find-toggle-replace"))
        .on_activate(on(|widget, cx| {
            widget.show_replace = !widget.show_replace;
            cx.notify();
        }));
        div()
            .id("find-widget")
            .flex()
            .gap(px(metrics.space.xs))
            .w(px(metrics.space.xl * WIDTH_STEPS))
            .p(px(metrics.space.xs))
            .rounded(px(metrics.radius.md))
            .border_1()
            .border_color(colors.border_panel)
            .bg(colors.bg_overlay)
            .occlude()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .capture_key_down(cx.listener(Self::on_key_down))
            .child(expand)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .gap(px(metrics.space.xs))
                    .child(query_row)
                    .children(replace_row),
            )
    }
}

/// The open widget and the subscription to its close event.
pub(crate) struct OpenFind {
    pub(crate) view: Entity<FindWidget>,
    _close: Subscription,
}

impl crate::Workbench {
    /// Opens the find widget over the active editor (with the replace row
    /// for Ctrl H), filled with the selected text when it is one line.
    pub(crate) fn open_find(&mut self, replace: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = self.active_editor() else {
            return;
        };
        let selected = editor.read(cx).document().selected_text();
        let initial = (!selected.is_empty() && !selected.contains('\n')).then_some(selected);
        if let Some(open) = &self.find {
            let view = open.view.clone();
            view.update(cx, |widget, cx| {
                widget.attach(&editor, cx);
                widget.show_replace = replace || widget.show_replace;
                if let Some(initial) = initial {
                    widget.set_query_text(initial, window, cx);
                }
                widget.focus_query(window, cx);
            });
        } else {
            let i18n = self.i18n.clone();
            let view = cx
                .new(|cx| FindWidget::new(&editor, initial.as_deref(), replace, i18n, window, cx));
            let close = cx.subscribe_in(&view, window, |this, _, _: &CloseFind, window, cx| {
                this.close_find(window, cx);
            });
            self.find = Some(OpenFind {
                view,
                _close: close,
            });
        }
        cx.notify();
    }

    /// Goes to the next (or previous) match; opens the widget if needed.
    pub(crate) fn find_next(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        match &self.find {
            Some(open) => open.view.update(cx, |widget, cx| widget.next(forward, cx)),
            None => self.open_find(false, window, cx),
        }
    }

    pub(crate) fn close_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(open) = self.find.take() {
            open.view.update(cx, FindWidget::detach);
            if let Some(editor) = self.active_editor() {
                let focus = editor.read(cx).focus_handle(cx);
                focus.focus(window, cx);
            }
            cx.notify();
        }
    }

    /// The active tab changed: the widget follows the editor, or closes
    /// over a view that is not one.
    pub(crate) fn retarget_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(open) = &self.find else {
            return;
        };
        match self.active_editor() {
            Some(editor) => {
                let view = open.view.clone();
                view.update(cx, |widget, cx| widget.attach(&editor, cx));
            }
            None => self.close_find(window, cx),
        }
    }
}
