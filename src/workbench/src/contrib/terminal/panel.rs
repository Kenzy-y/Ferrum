//! The Terminal tab of the bottom panel: terminal tabs, each with one or
//! more terminals side by side, the shells of the machine and starting
//! new sessions in the background.

use std::path::PathBuf;
use std::sync::Arc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AppContext as _, Context, Entity, EventEmitter, FocusHandle, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _,
    Styled as _, Subscription, Window, div, px,
};
use i18n::{FluentArgs, I18n};
use process::{ProcessRunner, PtySize};
use terminal::profiles::{self, ShellEnvironment};
use terminal::{LaunchOptions, ScrollTarget, ShellProfile, TerminalSession, TerminalSettings};
use theme::ActiveTheme as _;
use ui::{Icon, IconName, ListRow};

use super::palette::palette;
use super::picker::{PickerEvent, ShellPicker};
use super::view::{TerminalView, TerminalViewEvent};

/// Size a session starts with, before its first frame resizes it.
const INITIAL_SIZE: PtySize = PtySize {
    columns: 80,
    rows: 24,
    cell_width: 8,
    cell_height: 16,
};

/// What the panel tells the workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalPanelEvent {
    /// The last terminal was closed.
    AllClosed,
}

/// A request for a new terminal.
#[derive(Debug, Clone)]
pub(crate) struct Launch {
    /// The shell; `None` for the one `[terminal] shell` names.
    pub(crate) profile: Option<ShellProfile>,
    /// Next to the active terminal instead of in a new tab.
    pub(crate) split: bool,
    /// Moves the focus to it once it runs.
    pub(crate) focus: bool,
}

/// A terminal tab: terminals shown side by side.
struct TerminalTab {
    panes: Vec<Entity<TerminalView>>,
    active: usize,
}

/// The open shell picker and the focus to give back.
struct OpenPicker {
    view: Entity<ShellPicker>,
    previous_focus: Option<FocusHandle>,
    _subscription: Subscription,
}

/// The terminals of a window.
pub struct TerminalPanel {
    runner: Arc<dyn ProcessRunner>,
    i18n: Arc<I18n>,
    settings: TerminalSettings,
    working_directory: Option<PathBuf>,
    /// Shells of the machine; `None` while they are being found.
    profiles: Option<Vec<ShellProfile>>,
    /// Requests made before the shells were known.
    pending: Vec<Launch>,
    tabs: Vec<TerminalTab>,
    active: usize,
    picker: Option<OpenPicker>,
    /// Why the last terminal could not start.
    error: Option<SharedString>,
    subscriptions: Vec<(gpui::EntityId, Subscription)>,
}

impl EventEmitter<TerminalPanelEvent> for TerminalPanel {}

impl TerminalPanel {
    /// A panel without terminals; finds the shells of the machine through
    /// `fs` in the background.
    pub fn new(
        runner: Arc<dyn ProcessRunner>,
        fs: Arc<dyn fs::Fs>,
        i18n: Arc<I18n>,
        settings: TerminalSettings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.spawn_in(window, async move |panel, cx| {
            let found = cx
                .background_spawn(async move {
                    profiles::detect(fs.as_ref(), &ShellEnvironment::current()).await
                })
                .await;
            let _ = panel.update_in(cx, |panel, window, cx| {
                panel.profiles = Some(found);
                for launch in std::mem::take(&mut panel.pending) {
                    panel.launch(launch, window, cx);
                }
                cx.notify();
            });
        })
        .detach();
        Self {
            runner,
            i18n,
            settings,
            working_directory: None,
            profiles: None,
            pending: Vec::new(),
            tabs: Vec::new(),
            active: 0,
            picker: None,
            error: None,
            subscriptions: Vec::new(),
        }
    }

    /// Where terminals start: the open folder. Opening another folder
    /// ends the shells of the previous one and starts a terminal in the
    /// new folder in their place, so the terminal always works in the
    /// open project.
    pub fn set_working_directory(
        &mut self,
        directory: Option<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if directory == self.working_directory {
            return;
        }
        self.working_directory = directory;
        if self.tabs.is_empty() {
            return;
        }
        let had_focus = self.contains_focus(window, cx);
        self.tabs.clear();
        self.subscriptions.clear();
        self.active = 0;
        self.launch(
            Launch {
                profile: None,
                split: false,
                focus: had_focus,
            },
            window,
            cx,
        );
        cx.notify();
    }

    /// Applies changed `[terminal]` settings to every terminal.
    pub fn set_settings(&mut self, settings: TerminalSettings, cx: &mut Context<Self>) {
        if settings == self.settings {
            return;
        }
        for view in self.views() {
            view.update(cx, |view, cx| view.set_settings(settings.clone(), cx));
        }
        self.settings = settings;
    }

    pub(crate) fn set_i18n(&mut self, i18n: Arc<I18n>, cx: &mut Context<Self>) {
        self.i18n = i18n;
        cx.notify();
    }

    fn views(&self) -> impl Iterator<Item = Entity<TerminalView>> + '_ {
        self.tabs.iter().flat_map(|tab| tab.panes.iter().cloned())
    }

    /// Why the last terminal could not start, if it could not.
    #[cfg(test)]
    pub(crate) fn error_message(&self) -> Option<&SharedString> {
        self.error.as_ref()
    }

    /// Whether any terminal is open.
    pub(crate) fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    /// The terminal with the keyboard focus in the active tab.
    pub(crate) fn active_view(&self) -> Option<Entity<TerminalView>> {
        let tab = self.tabs.get(self.active)?;
        tab.panes.get(tab.active).cloned()
    }

    /// Whether a terminal of the panel has the keyboard focus.
    pub(crate) fn contains_focus(&self, window: &Window, cx: &gpui::App) -> bool {
        self.views()
            .any(|view| view.read(cx).focus_handle.is_focused(window))
    }

    /// Name of the active terminal's shell, or of the shell a new
    /// terminal gets, for the shell button.
    pub(crate) fn shell_label(&self, cx: &gpui::App) -> Option<SharedString> {
        if let Some(view) = self.active_view() {
            return Some(view.read(cx).profile.name.clone().into());
        }
        let profiles = self.profiles.as_ref()?;
        profiles::choose(profiles, &self.settings.shell, &self.settings.shell_args)
            .map(|profile| profile.name.into())
    }

    /// Focuses the active terminal; `false` if there is none.
    pub(crate) fn focus_active(&self, window: &mut Window, cx: &mut gpui::App) -> bool {
        let Some(view) = self.active_view() else {
            return false;
        };
        let focus = view.read(cx).focus_handle.clone();
        window.focus(&focus, cx);
        true
    }

    /// Starts a terminal. Before the shells are known the request waits.
    pub(crate) fn launch(&mut self, launch: Launch, window: &mut Window, cx: &mut Context<Self>) {
        let Some(profiles) = &self.profiles else {
            self.pending.push(launch);
            return;
        };
        let profile = launch.profile.clone().or_else(|| {
            profiles::choose(profiles, &self.settings.shell, &self.settings.shell_args)
        });
        let Some(profile) = profile else {
            self.error = Some(self.i18n.text("terminal-no-shell").into());
            cx.notify();
            return;
        };
        let options = LaunchOptions {
            profile: profile.clone(),
            working_directory: self.working_directory.clone(),
            size: INITIAL_SIZE,
            scrollback_lines: self.settings.scrollback_lines,
            palette: palette(cx.theme()),
        };
        let runner = self.runner.clone();
        cx.spawn_in(window, async move |panel, cx| {
            let started = cx
                .background_spawn(async move { TerminalSession::spawn(runner.as_ref(), options) })
                .await;
            let _ = panel.update_in(cx, |panel, window, cx| match started {
                Ok((session, events)) => {
                    panel.attach(session, events, profile, &launch, window, cx);
                }
                Err(error) => {
                    tracing::warn!(shell = %profile.name, "cannot start the terminal: {error}");
                    let mut args = FluentArgs::new();
                    args.set("shell", profile.name.clone());
                    args.set("error", error.to_string());
                    panel.error = Some(panel.i18n.text_with("terminal-start-failed", &args).into());
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn attach(
        &mut self,
        session: TerminalSession,
        events: futures::channel::mpsc::UnboundedReceiver<terminal::TerminalEvent>,
        profile: ShellProfile,
        launch: &Launch,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let settings = self.settings.clone();
        let view = cx.new(|cx| TerminalView::new(session, events, profile, settings, window, cx));
        let subscription =
            cx.subscribe_in(
                &view,
                window,
                |panel, view, event, window, cx| match event {
                    TerminalViewEvent::Exited => {
                        if panel.settings.close_on_exit {
                            panel.close(view, window, cx);
                        }
                    }
                    TerminalViewEvent::Focused => panel.activate(view, cx),
                },
            );
        self.subscriptions.push((view.entity_id(), subscription));
        self.error = None;
        match self.tabs.get_mut(self.active) {
            Some(tab) if launch.split => {
                tab.active += 1;
                tab.panes.insert(tab.active, view.clone());
            }
            _ => {
                self.tabs.push(TerminalTab {
                    panes: vec![view.clone()],
                    active: 0,
                });
                self.active = self.tabs.len() - 1;
            }
        }
        if launch.focus {
            let focus = view.read(cx).focus_handle.clone();
            window.focus(&focus, cx);
        }
        cx.notify();
    }

    /// Makes `view` the active terminal.
    fn activate(&mut self, view: &Entity<TerminalView>, cx: &mut Context<Self>) {
        for (index, tab) in self.tabs.iter_mut().enumerate() {
            if let Some(pane) = tab.panes.iter().position(|pane| pane == view) {
                tab.active = pane;
                if self.active != index {
                    self.active = index;
                    cx.notify();
                }
                return;
            }
        }
    }

    /// Shows terminal tab `index` and focuses its terminal.
    fn select_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index < self.tabs.len() {
            self.active = index;
            self.focus_active(window, cx);
            cx.notify();
        }
    }

    /// Shows the next (or previous) terminal tab.
    pub(crate) fn cycle(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let count = self.tabs.len();
        if count > 1 {
            let next = if forward {
                (self.active + 1) % count
            } else {
                (self.active + count - 1) % count
            };
            self.select_tab(next, window, cx);
        }
    }

    /// Ends the shell of `view` and closes it.
    fn close(&mut self, view: &Entity<TerminalView>, window: &mut Window, cx: &mut Context<Self>) {
        let had_focus = view.read(cx).focus_handle.is_focused(window);
        let Some(tab_index) = self.tabs.iter().position(|tab| tab.panes.contains(view)) else {
            return;
        };
        let tab = &mut self.tabs[tab_index];
        if let Some(pane) = tab.panes.iter().position(|pane| pane == view) {
            tab.panes.remove(pane);
            tab.active = tab.active.min(tab.panes.len().saturating_sub(1));
        }
        if tab.panes.is_empty() {
            self.tabs.remove(tab_index);
            if self.active >= tab_index && self.active > 0 {
                self.active -= 1;
            }
        }
        self.subscriptions.retain(|(id, _)| *id != view.entity_id());
        if self.tabs.is_empty() {
            cx.emit(TerminalPanelEvent::AllClosed);
        } else if had_focus {
            self.focus_active(window, cx);
        }
        cx.notify();
    }

    /// Ends and closes the active terminal.
    pub(crate) fn kill_active(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(view) = self.active_view() {
            self.close(&view, window, cx);
        }
    }

    /// Runs `action` on the active terminal.
    pub(crate) fn with_active(
        &self,
        cx: &mut gpui::App,
        action: impl FnOnce(&mut TerminalView, &mut Context<TerminalView>),
    ) {
        if let Some(view) = self.active_view() {
            view.update(cx, action);
        }
    }

    pub(crate) fn scroll(&self, target: ScrollTarget, cx: &mut gpui::App) {
        self.with_active(cx, |view, cx| view.scroll(target, cx));
    }

    /// Whether the shell picker is open.
    pub(crate) fn picker(&self) -> Option<Entity<ShellPicker>> {
        self.picker.as_ref().map(|open| open.view.clone())
    }

    /// Opens or closes the shell picker.
    pub(crate) fn toggle_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.picker.is_some() {
            self.close_picker(window, cx);
            return;
        }
        let profiles = self.profiles.clone().unwrap_or_default();
        let names = profiles
            .iter()
            .map(|profile| SharedString::from(profile.name.clone()))
            .collect();
        let current = self.shell_label(cx);
        let selected = profiles
            .iter()
            .position(|profile| current.as_deref() == Some(profile.name.as_str()))
            .unwrap_or(0);
        let empty: SharedString = if self.profiles.is_some() {
            self.i18n.text("terminal-no-shell").into()
        } else {
            self.i18n.text("terminal-finding-shells").into()
        };
        let previous_focus = window.focused(cx);
        let view = cx.new(|cx| ShellPicker::new(names, empty, selected, window, cx));
        let subscription =
            cx.subscribe_in(
                &view,
                window,
                move |panel, _, event, window, cx| match *event {
                    PickerEvent::Picked(index) => {
                        let profile = profiles.get(index).cloned();
                        panel.picker = None;
                        cx.notify();
                        panel.launch(
                            Launch {
                                profile,
                                split: false,
                                focus: true,
                            },
                            window,
                            cx,
                        );
                    }
                    PickerEvent::Dismiss => panel.close_picker(window, cx),
                },
            );
        self.picker = Some(OpenPicker {
            view,
            previous_focus,
            _subscription: subscription,
        });
        cx.notify();
    }

    pub(crate) fn close_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(open) = self.picker.take() {
            if let Some(focus) = open.previous_focus {
                window.focus(&focus, cx);
            }
            cx.notify();
        }
    }
}

impl Render for TerminalPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let metrics = &theme.metrics;
        let inset = metrics.size.section_header_inset + metrics.space.xs;
        let content = match self.tabs.get(self.active) {
            Some(tab) => {
                let panes = tab.panes.iter().enumerate().map(|(index, pane)| {
                    div()
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .when(index > 0, |pane| {
                            pane.border_l_1()
                                .border_color(theme.colors.border_panel)
                                .pl(px(metrics.space.md))
                        })
                        .child(pane.clone())
                });
                div().flex().flex_1().min_w_0().h_full().children(panes)
            }
            None => div().flex_1().children(self.error.clone().map(|error| {
                div()
                    .text_size(px(metrics.font_size.sm))
                    .text_color(theme.colors.text_muted)
                    .child(error)
            })),
        };
        let list = (self.tabs.len() > 1).then(|| {
            let entity = cx.entity().downgrade();
            let rows = self.tabs.iter().enumerate().map(|(index, tab)| {
                let names: Vec<String> = tab
                    .panes
                    .iter()
                    .map(|pane| pane.read(cx).profile.name.clone())
                    .collect();
                let entity = entity.clone();
                ListRow::new(("terminal-tab", index), names.join(", "))
                    .flat()
                    .leading(Icon::new(IconName::Terminal).size(metrics.size.icon_small))
                    .selected(index == self.active)
                    .on_activate(move |window, cx| {
                        let _ = entity.update(cx, |panel, cx| panel.select_tab(index, window, cx));
                    })
            });
            div()
                .id("terminal-tabs")
                .flex()
                .flex_col()
                .flex_none()
                .w(px(metrics.size.terminal_tabs_width))
                .h_full()
                .overflow_y_scroll()
                .ml(px(metrics.space.md))
                .pl(px(metrics.space.xs))
                .border_l_1()
                .border_color(theme.colors.border_panel)
                .children(rows)
        });
        div()
            .flex()
            .size_full()
            .pl(px(inset))
            .pr(px(inset))
            .pt(px(metrics.size.terminal_top))
            .child(content)
            .children(list)
    }
}
