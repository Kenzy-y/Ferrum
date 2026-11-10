//! Overlays above the layout: quick open and the path dialog. Each
//! remembers the focus to restore when it closes.

use std::path::PathBuf;
use std::rc::Rc;

use gpui::{AppContext as _, Context, Entity, FocusHandle, SharedString, Subscription, Window};

use crate::Workbench;
use crate::contrib::menu_bar::MENUS;
use crate::contrib::menu_bar::view::{MenuEvent, MenuView};
use crate::contrib::quick_open::{Dismiss, QuickOpen, QuickOpenMode};
use crate::dialogs::path_prompt::{PathPrompt, PathPromptEvent, PathPromptTexts};
use ui::IconName;

/// What confirming the path dialog does with the typed text.
pub(crate) type PromptConfirm =
    Rc<dyn Fn(&mut Workbench, String, &mut Window, &mut Context<Workbench>)>;

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
        let texts = PathPromptTexts {
            title: self.text("path-prompt-title"),
            placeholder: self.text("path-prompt-placeholder"),
            open: self.text("path-prompt-open"),
            cancel: self.text("path-prompt-cancel"),
            icon: IconName::Folder,
        };
        self.open_prompt(
            texts,
            initial,
            error,
            Rc::new(|this, path, window, cx| {
                let path = PathBuf::from(path);
                this.close_path_prompt(window, cx);
                let open_path = this.open_path.clone();
                window.defer(cx, move |window, cx| open_path(path, window, cx));
            }),
            window,
            cx,
        );
    }

    /// Shows the path dialog; `confirm` gets the typed text.
    pub(crate) fn open_prompt(
        &mut self,
        texts: PathPromptTexts,
        initial: Option<String>,
        error: Option<SharedString>,
        confirm: PromptConfirm,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let previous_focus = match self.path_prompt.take() {
            Some(open) => open.previous_focus,
            None => window.focused(cx),
        };
        let view = cx.new(|cx| PathPrompt::new(texts, initial, error, window, cx));
        let subscription = cx.subscribe_in(
            &view,
            window,
            move |this, _, event: &PathPromptEvent, window, cx| match event {
                PathPromptEvent::Confirm(text) => confirm(this, text.clone(), window, cx),
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

/// An open title bar menu and the focus to restore when it closes.
pub(crate) struct OpenMenu {
    /// Which of [`MENUS`].
    pub(crate) index: usize,
    pub(crate) view: Entity<MenuView>,
    pub(crate) previous_focus: Option<FocusHandle>,
    pub(crate) _subscription: Subscription,
}

impl Workbench {
    /// Opens menu `index`; from the keyboard its first row is selected.
    /// Opening the menu that is open closes it.
    pub(crate) fn toggle_menu(
        &mut self,
        index: usize,
        from_keyboard: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.menu.as_ref().is_some_and(|open| open.index == index) {
            self.close_menu(window, cx);
        } else {
            self.open_menu(index, from_keyboard, window, cx);
        }
    }

    /// The pointer moved onto menu button `index`: while a menu is open,
    /// that one opens instead.
    pub(crate) fn hover_menu(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.menu.as_ref().is_some_and(|open| open.index != index) {
            self.open_menu(index, false, window, cx);
        }
    }

    fn open_menu(
        &mut self,
        index: usize,
        from_keyboard: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let index = index % MENUS.len();
        let previous_focus = match self.menu.take() {
            Some(open) => open.previous_focus,
            None => window.focused(cx),
        };
        let context = self.quick_open_context.clone();
        let view =
            cx.new(|cx| MenuView::new(MENUS[index].items, from_keyboard, &context, window, cx));
        let subscription =
            cx.subscribe_in(
                &view,
                window,
                move |this, _, event, window, cx| match *event {
                    MenuEvent::Dismiss => this.close_menu(window, cx),
                    MenuEvent::Switch(offset) => {
                        let count = MENUS.len().cast_signed();
                        let next = (index.cast_signed() + offset)
                            .rem_euclid(count)
                            .cast_unsigned();
                        this.open_menu(next, true, window, cx);
                    }
                },
            );
        self.menu = Some(OpenMenu {
            index,
            view,
            previous_focus,
            _subscription: subscription,
        });
        cx.notify();
    }

    pub(crate) fn close_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(open) = self.menu.take() {
            if let Some(focus) = open.previous_focus {
                focus.focus(window, cx);
            }
            cx.notify();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::Arc;

    use commands::{CommandId, CommandRegistry, Keymap};
    use gpui::{TestAppContext, VisualTestContext};
    use i18n::I18n;

    use super::*;
    use crate::{Dispatch, command_ids};

    fn workbench(
        cx: &mut TestAppContext,
    ) -> (
        Entity<Workbench>,
        VisualTestContext,
        Rc<RefCell<Vec<String>>>,
    ) {
        cx.update(|cx| {
            let theme =
                theme::Theme::built_in(theme::DEFAULT_THEME, theme::Density::default()).unwrap();
            theme::set_theme(theme, cx);
        });
        let ran = Rc::new(RefCell::new(Vec::new()));
        let recorded = ran.clone();
        let dispatch: Dispatch = Rc::new(move |id: &CommandId, _, _| {
            recorded.borrow_mut().push(id.as_str().to_owned());
        });
        let mut registry = CommandRegistry::new();
        crate::register_commands(&mut registry).unwrap();
        let window = cx.add_window(move |_, _| {
            Workbench::new(
                Arc::new(I18n::new("en").unwrap()),
                dispatch,
                Arc::new(registry),
                Arc::new(Keymap::default()),
                Rc::new(|_, _, _| {}),
            )
        });
        let entity = window.root(cx).unwrap();
        let visual = VisualTestContext::from_window(window.into(), cx);
        (entity, visual, ran)
    }

    fn open_index(workbench: &Entity<Workbench>, cx: &mut VisualTestContext) -> Option<usize> {
        workbench.read_with(cx, |workbench, _| {
            workbench.menu.as_ref().map(|open| open.index)
        })
    }

    #[gpui::test]
    fn a_menu_opens_below_its_button(cx: &mut TestAppContext) {
        let (workbench, mut cx, _) = workbench(cx);
        workbench.update_in(&mut cx, |workbench, window, cx| {
            workbench.toggle_menu(1, false, window, cx);
        });
        cx.run_until_parked();
        let button = cx.debug_bounds("menu-button-1").unwrap();
        let menu = cx.debug_bounds("menu").unwrap();
        assert!(menu.top() >= button.bottom(), "{menu:?} covers {button:?}");
        assert_eq!(menu.left(), button.left());
    }

    #[gpui::test]
    fn the_keyboard_opens_switches_runs_and_closes_menus(cx: &mut TestAppContext) {
        let (workbench, mut cx, ran) = workbench(cx);
        workbench.update_in(&mut cx, |workbench, window, cx| {
            workbench.toggle_menu(0, true, window, cx);
        });
        assert_eq!(open_index(&workbench, &mut cx), Some(0));

        cx.simulate_keystrokes("right");
        assert_eq!(open_index(&workbench, &mut cx), Some(1), "Right opens Edit");
        cx.simulate_keystrokes("left left");
        assert_eq!(open_index(&workbench, &mut cx), Some(3), "Left wraps to Go");
        cx.simulate_keystrokes("right");

        // File, as registered here (the shell's own rows are left out):
        // New File, Save, Close Tab, Close Window. Down picks the third.
        cx.simulate_keystrokes("down down enter");
        cx.run_until_parked();
        assert_eq!(
            open_index(&workbench, &mut cx),
            None,
            "running closes the menu"
        );
        assert_eq!(*ran.borrow(), [command_ids::CLOSE_TAB]);

        workbench.update_in(&mut cx, |workbench, window, cx| {
            workbench.toggle_menu(2, false, window, cx);
            workbench.toggle_menu(2, false, window, cx);
        });
        assert_eq!(
            open_index(&workbench, &mut cx),
            None,
            "a second click closes it"
        );
    }
}
