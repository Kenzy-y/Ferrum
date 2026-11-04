//! Overlays above the layout: quick open and the path dialog. Each
//! remembers the focus to restore when it closes.

use std::path::PathBuf;

use gpui::{AppContext as _, Context, Entity, FocusHandle, SharedString, Subscription, Window};

use crate::Workbench;
use crate::contrib::quick_open::{Dismiss, QuickOpen, QuickOpenMode};
use crate::dialogs::path_prompt::{PathPrompt, PathPromptEvent, PathPromptTexts};

impl Workbench {
    /// Asks for a folder path, prefilled with `initial`, explaining `error`
    /// if a previous attempt failed. Confirming calls the [`OpenPath`]
    /// callback.
    pub fn show_path_prompt(
        &mut self,
        initial: Option<String>,
        error: Option<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let previous_focus = match self.path_prompt.take() {
            Some(open) => open.previous_focus,
            None => window.focused(cx),
        };
        let texts = PathPromptTexts {
            title: self.text("path-prompt-title"),
            placeholder: self.text("path-prompt-placeholder"),
            open: self.text("path-prompt-open"),
            cancel: self.text("path-prompt-cancel"),
        };
        let view = cx.new(|cx| PathPrompt::new(texts, initial, error, window, cx));
        let subscription = cx.subscribe_in(
            &view,
            window,
            |this, _, event: &PathPromptEvent, window, cx| match event {
                PathPromptEvent::Confirm(path) => {
                    let path = PathBuf::from(path);
                    this.close_path_prompt(window, cx);
                    let open_path = this.open_path.clone();
                    window.defer(cx, move |window, cx| open_path(path, window, cx));
                }
                PathPromptEvent::Dismiss => this.close_path_prompt(window, cx),
            },
        );
        self.path_prompt = Some(OpenPathPrompt {
            view,
            previous_focus,
            _subscription: subscription,
        });
        cx.notify();
    }

    pub(crate) fn close_path_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(open) = self.path_prompt.take() {
            if let Some(focus) = open.previous_focus {
                focus.focus(window, cx);
            }
            cx.notify();
        }
    }

    pub(crate) fn open_quick_open(
        &mut self,
        mode: QuickOpenMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let previous_focus = match self.quick_open.take() {
            Some(open) => open.previous_focus,
            None => window.focused(cx),
        };
        let context = self.quick_open_context.clone();
        let view = cx.new(|cx| QuickOpen::new(mode, &context, window, cx));
        let subscription = cx.subscribe_in(&view, window, |this, _, _: &Dismiss, window, cx| {
            this.close_quick_open(window, cx);
        });
        self.quick_open = Some(OpenQuickOpen {
            view,
            previous_focus,
            _subscription: subscription,
        });
        cx.notify();
    }

    pub(crate) fn close_quick_open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(open) = self.quick_open.take() {
            if let Some(focus) = open.previous_focus {
                focus.focus(window, cx);
            }
            cx.notify();
        }
    }

    /// The open quick open view, drawn by the title bar.
    pub(crate) fn quick_open_view(&self) -> Option<Entity<QuickOpen>> {
        self.quick_open.as_ref().map(|open| open.view.clone())
    }
}

/// An open quick open overlay and the focus to restore when it closes.
pub(crate) struct OpenQuickOpen {
    pub(crate) view: Entity<QuickOpen>,
    pub(crate) previous_focus: Option<FocusHandle>,
    pub(crate) _subscription: Subscription,
}

/// An open path dialog and the focus to restore when it closes.
pub(crate) struct OpenPathPrompt {
    pub(crate) view: Entity<PathPrompt>,
    pub(crate) previous_focus: Option<FocusHandle>,
    pub(crate) _subscription: Subscription,
}
