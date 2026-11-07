//! The terminal in the workbench: its commands, toggling and maximizing
//! the panel, and the header buttons of the Terminal tab.

use gpui::{
    AnyElement, App, Context, Entity, Focusable as _, InteractiveElement as _, IntoElement,
    MouseButton, ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _,
    WeakEntity, Window, anchored, deferred, div, px,
};
use terminal::ScrollTarget;
use theme::ActiveTheme as _;
use ui::{Icon, IconButton, IconName};

use super::panel::{Launch, TerminalPanel, TerminalPanelEvent};
use crate::command_handlers::CommandHandlers;
use crate::layout::PanelKind;
use crate::{Workbench, command_ids};

impl Workbench {
    /// Shows `panel` in the Terminal tab of the bottom panel.
    pub fn set_terminal(&mut self, panel: Entity<TerminalPanel>, cx: &mut Context<Self>) {
        self.terminal_watch = Some(cx.subscribe(
            &panel,
            |workbench, _, event: &TerminalPanelEvent, cx| match event {
                TerminalPanelEvent::AllClosed => {
                    // Killing the last terminal hides the panel, as closing
                    // the last editor shows Welcome.
                    if workbench.panel == PanelKind::Terminal {
                        workbench.panel_visible = false;
                        workbench.panel_maximized = false;
                        cx.notify();
                    }
                }
            },
        ));
        self.terminal_observe = Some(cx.observe(&panel, |_, _, cx| cx.notify()));
        self.terminal = Some(panel);
        cx.notify();
    }

    /// Shows the Terminal tab and focuses its terminal, starting one if
    /// none runs.
    pub(crate) fn show_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(panel) = self.terminal.clone() else {
            return;
        };
        self.panel_visible = true;
        self.panel = PanelKind::Terminal;
        panel.update(cx, |panel, cx| {
            if !panel.focus_active(window, cx) {
                panel.launch(
                    Launch {
                        profile: None,
                        split: false,
                        focus: true,
                    },
                    window,
                    cx,
                );
            }
        });
        cx.notify();
    }

    /// `workbench.toggleTerminal`: shows and focuses the terminal, or
    /// hides the panel when the terminal has the focus.
    fn toggle_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let focused = self
            .terminal
            .as_ref()
            .is_some_and(|panel| panel.read(cx).contains_focus(window, cx));
        if self.panel_visible && self.panel == PanelKind::Terminal && focused {
            self.panel_visible = false;
            self.panel_maximized = false;
            if let Some(editor) = self.active_editor() {
                let focus = editor.read(cx).focus_handle(cx);
                window.focus(&focus, cx);
            }
            cx.notify();
        } else {
            self.show_terminal(window, cx);
        }
    }

    /// Starts a terminal (`split`: next to the active one) and shows it.
    fn new_terminal(&mut self, split: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(panel) = self.terminal.clone() else {
            return;
        };
        self.panel_visible = true;
        self.panel = PanelKind::Terminal;
        let split = split && !panel.read(cx).is_empty();
        panel.update(cx, |panel, cx| {
            panel.launch(
                Launch {
                    profile: None,
                    split,
                    focus: true,
                },
                window,
                cx,
            );
        });
        cx.notify();
    }

    fn toggle_maximized_panel(&mut self, cx: &mut Context<Self>) {
        if self.panel_visible {
            self.panel_maximized = !self.panel_maximized;
        } else {
            self.panel_visible = true;
            self.panel_maximized = true;
        }
        cx.notify();
    }

    /// Catches clicks outside the open shell picker and closes it.
    pub(crate) fn shell_picker_backdrop(&self, cx: &App) -> Option<impl IntoElement + use<>> {
        let panel = self
            .terminal
            .as_ref()
            .filter(|panel| panel.read(cx).picker().is_some())?
            .downgrade();
        Some(
            div()
                .id("shell-picker-backdrop")
                .absolute()
                .inset_0()
                .occlude()
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    let _ = panel.update(cx, |panel, cx| panel.close_picker(window, cx));
                }),
        )
    }

    /// Runs `action` on the terminal panel, if there is one.
    fn on_terminal(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        action: impl FnOnce(&mut TerminalPanel, &mut Window, &mut Context<TerminalPanel>),
    ) {
        if let Some(panel) = self.terminal.clone() {
            panel.update(cx, |panel, cx| action(panel, window, cx));
        }
    }

    /// Registers the terminal commands.
    pub(crate) fn register_terminal_handlers(
        handlers: &mut CommandHandlers,
        this: &WeakEntity<Self>,
    ) -> Result<(), commands::RegistryError> {
        handlers.register_entity(command_ids::TOGGLE_TERMINAL, this, Self::toggle_terminal)?;
        handlers.register_entity(command_ids::TOGGLE_MAXIMIZED_PANEL, this, |this, _, cx| {
            this.toggle_maximized_panel(cx);
        })?;
        handlers.register_entity(command_ids::TERMINAL_NEW, this, |this, window, cx| {
            this.new_terminal(false, window, cx);
        })?;
        handlers.register_entity(command_ids::TERMINAL_SPLIT, this, |this, window, cx| {
            this.new_terminal(true, window, cx);
        })?;
        handlers.register_entity(command_ids::TERMINAL_KILL, this, |this, window, cx| {
            this.on_terminal(window, cx, TerminalPanel::kill_active);
        })?;
        handlers.register_entity(
            command_ids::TERMINAL_SELECT_SHELL,
            this,
            |this, window, cx| {
                this.panel_visible = true;
                this.panel = PanelKind::Terminal;
                this.on_terminal(window, cx, TerminalPanel::toggle_picker);
                cx.notify();
            },
        )?;
        handlers.register_entity(
            command_ids::TERMINAL_FOCUS_NEXT,
            this,
            |this, window, cx| {
                this.on_terminal(window, cx, |panel, window, cx| {
                    panel.cycle(true, window, cx);
                });
            },
        )?;
        handlers.register_entity(
            command_ids::TERMINAL_FOCUS_PREVIOUS,
            this,
            |this, window, cx| {
                this.on_terminal(window, cx, |panel, window, cx| {
                    panel.cycle(false, window, cx);
                });
            },
        )?;
        handlers.register_entity(command_ids::TERMINAL_COPY, this, |this, window, cx| {
            this.on_terminal(window, cx, |panel, _, cx| {
                panel.with_active(cx, |view, cx| {
                    view.copy(cx);
                });
            });
        })?;
        handlers.register_entity(command_ids::TERMINAL_PASTE, this, |this, window, cx| {
            this.on_terminal(window, cx, |panel, _, cx| {
                panel.with_active(cx, super::view::TerminalView::paste);
            });
        })?;
        handlers.register_entity(
            command_ids::TERMINAL_SELECT_ALL,
            this,
            |this, window, cx| {
                this.on_terminal(window, cx, |panel, _, cx| {
                    panel.with_active(cx, super::view::TerminalView::select_all);
                });
            },
        )?;
        handlers.register_entity(command_ids::TERMINAL_CLEAR, this, |this, window, cx| {
            this.on_terminal(window, cx, |panel, _, cx| {
                panel.with_active(cx, super::view::TerminalView::clear);
            });
        })?;
        let scrolls = [
            (command_ids::TERMINAL_SCROLL_PAGE_UP, ScrollTarget::PageUp),
            (
                command_ids::TERMINAL_SCROLL_PAGE_DOWN,
                ScrollTarget::PageDown,
            ),
            (command_ids::TERMINAL_SCROLL_TO_TOP, ScrollTarget::Top),
            (command_ids::TERMINAL_SCROLL_TO_BOTTOM, ScrollTarget::Bottom),
        ];
        for (id, target) in scrolls {
            handlers.register_entity(id, this, move |this, window, cx| {
                this.on_terminal(window, cx, |panel, _, cx| panel.scroll(target, cx));
            })?;
        }
        Ok(())
    }
}

/// Buttons of the Terminal tab header, as in `editor.svg`: the shell
/// picker, new terminal, split and kill.
pub(crate) fn header_actions(
    workbench: &Workbench,
    panel: &Entity<TerminalPanel>,
    cx: &App,
) -> AnyElement {
    let theme = cx.theme();
    let metrics = &theme.metrics;
    let button = |id: &'static str, icon: IconName, command: &'static str| {
        let dispatch = workbench.dispatch.clone();
        IconButton::new(id, icon)
            .tooltip(workbench.command_title(command))
            .on_activate(move |window, cx| {
                if let Ok(id) = commands::CommandId::new(command) {
                    dispatch(&id, window, cx);
                }
            })
    };
    let panel_state = panel.read(cx);
    let label: SharedString = panel_state.shell_label(cx).unwrap_or_default();
    let dispatch = workbench.dispatch.clone();
    let picker = panel_state.picker().map(|view| {
        deferred(anchored().child(div().pt(px(metrics.space.sm)).child(view))).with_priority(1)
    });
    let shell_button = div()
        .id("terminal-shell")
        .debug_selector(|| "terminal-shell".to_owned())
        .flex()
        .items_center()
        .gap(px(metrics.space.md))
        .h(px(metrics.size.icon_small + metrics.space.xs * 4.0))
        .px(px(metrics.space.xs))
        .rounded(px(metrics.radius.sm))
        .cursor_pointer()
        .hover(|style| style.bg(theme.colors.bg_ghost_hover))
        .font_family(SharedString::from(theme.fonts.mono.clone()))
        .text_size(px(metrics.font_size.sm))
        .text_color(theme.colors.text_secondary)
        .on_click(move |_, window, cx| {
            if let Ok(id) = commands::CommandId::new(command_ids::TERMINAL_SELECT_SHELL) {
                dispatch(&id, window, cx);
            }
        })
        .child(label)
        .child(Icon::new(IconName::ChevronDown).color(theme.colors.icon));
    div()
        .flex()
        .items_center()
        .gap(px(metrics.size.panel_actions_gap))
        .child(div().child(shell_button).children(picker))
        .child(button(
            "terminal-new",
            IconName::Plus,
            command_ids::TERMINAL_NEW,
        ))
        .child(button(
            "terminal-split",
            IconName::Columns,
            command_ids::TERMINAL_SPLIT,
        ))
        .child(button(
            "terminal-kill",
            IconName::Trash,
            command_ids::TERMINAL_KILL,
        ))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;
    use std::sync::Arc;

    use commands::{CommandRegistry, Keymap};
    use gpui::{AppContext as _, TestAppContext, VisualTestContext};
    use i18n::I18n;
    use process::FakeProcessRunner;

    use super::*;

    fn workbench(
        cx: &mut TestAppContext,
    ) -> (Entity<Workbench>, Arc<FakeProcessRunner>, VisualTestContext) {
        cx.update(|cx| {
            let theme =
                theme::Theme::built_in(theme::DEFAULT_THEME, theme::Density::default()).unwrap();
            theme::set_theme(theme, cx);
        });
        let mut registry = CommandRegistry::new();
        crate::register_commands(&mut registry).unwrap();
        let runner = Arc::new(FakeProcessRunner::new());
        let panel_runner: Arc<dyn process::ProcessRunner> = runner.clone();
        let window = cx.add_window(move |window, cx| {
            let i18n = Arc::new(I18n::new("en").unwrap());
            let mut workbench = Workbench::new(
                i18n.clone(),
                Rc::new(|_, _, _| {}),
                Arc::new(registry),
                Arc::new(Keymap::default()),
                Rc::new(|_, _, _| {}),
            );
            let settings = terminal::TerminalSettings {
                shell: "/opt/shells/test-shell".to_owned(),
                ..terminal::TerminalSettings::default()
            };
            let panel = cx.new(|cx| {
                let mut panel = TerminalPanel::new(
                    panel_runner,
                    Arc::new(fs::FakeFs::new()),
                    i18n,
                    settings,
                    window,
                    cx,
                );
                panel.set_working_directory(Some("/project".into()), window, cx);
                panel
            });
            workbench.set_terminal(panel, cx);
            workbench
        });
        let entity = window.root(cx).unwrap();
        let visual = VisualTestContext::from_window(window.into(), cx);
        (entity, runner, visual)
    }

    #[gpui::test]
    fn showing_the_terminal_starts_the_configured_shell_in_the_folder(cx: &mut TestAppContext) {
        let (workbench, runner, mut cx) = workbench(cx);
        workbench.update_in(&mut cx, |workbench, window, cx| {
            workbench.toggle_terminal(window, cx);
        });
        cx.run_until_parked();
        let runs = runner.runs();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].program(), "/opt/shells/test-shell");
        assert_eq!(
            runs[0].working_directory(),
            Some(std::path::Path::new("/project"))
        );
        workbench.read_with(&cx, |workbench, cx| {
            assert!(workbench.panel_visible);
            assert_eq!(workbench.panel, PanelKind::Terminal);
            let panel = workbench.terminal.as_ref().unwrap().read(cx);
            // Fake runners have no pseudo-terminals: the panel says so.
            assert!(panel.is_empty());
            assert!(panel.error_message().is_some());
        });
    }

    #[gpui::test]
    fn maximizing_shows_the_panel_and_restoring_keeps_it(cx: &mut TestAppContext) {
        let (workbench, _, mut cx) = workbench(cx);
        workbench.update_in(&mut cx, |workbench, _, cx| {
            workbench.toggle_maximized_panel(cx);
            assert!(workbench.panel_visible && workbench.panel_maximized);
            workbench.toggle_maximized_panel(cx);
            assert!(workbench.panel_visible && !workbench.panel_maximized);
        });
    }
}
