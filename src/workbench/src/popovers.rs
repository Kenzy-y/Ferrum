//! Small windows that hang from a button: the notification center (the
//! bell in the status bar), the recent projects (the arrow of the Explorer
//! card) and the tab menu ("…" right of the tabs). One is open at a time;
//! a click outside or Escape closes it.

use std::path::PathBuf;

use gpui::{
    Anchor, AnyElement, AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _,
    IntoElement, KeyDownEvent, MouseButton, ParentElement as _, SharedString, Styled as _,
    Subscription, Window, anchored, deferred, div, px,
};
use theme::ActiveTheme as _;
use ui::{Button, ButtonVariant, Icon, IconButton, IconName, ListRow};

use crate::Workbench;
use crate::contrib::menu_bar::TAB_MENU;
use crate::contrib::menu_bar::view::{MenuEvent, MenuView};

/// How important a notification is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationKind {
    /// Something finished or is worth knowing.
    Info,
    /// Something may need attention.
    Warning,
    /// Something failed.
    Error,
}

/// A message in the notification center, already localized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    /// How important it is.
    pub kind: NotificationKind,
    /// The message.
    pub message: SharedString,
}

/// Notifications shown at most; older ones are dropped.
const MAX_NOTIFICATIONS: usize = 50;
/// Width of the notification center and the project list, in space tokens
/// `xl`.
const POPOVER_WIDTH_STEPS: f32 = 16.0;

/// The notifications and whether some arrived since the center was last
/// opened.
#[derive(Default)]
pub(crate) struct Notifications {
    pub(crate) items: Vec<Notification>,
    pub(crate) unread: bool,
}

/// Which popover is open.
pub(crate) enum Popover {
    Notifications,
    Projects,
    TabMenu {
        view: Entity<MenuView>,
        _subscription: Subscription,
    },
}

/// The open popover and the focus to give back.
pub(crate) struct OpenPopover {
    pub(crate) popover: Popover,
    previous_focus: Option<FocusHandle>,
}

impl Workbench {
    /// Adds a notification; the bell shows that there is something new.
    pub fn notify(&mut self, notification: Notification, cx: &mut Context<Self>) {
        let notifications = &mut self.notifications;
        notifications.items.insert(0, notification);
        notifications.items.truncate(MAX_NOTIFICATIONS);
        notifications.unread = true;
        cx.notify();
    }

    pub(crate) fn clear_notifications(&mut self, cx: &mut Context<Self>) {
        self.notifications.items.clear();
        self.notifications.unread = false;
        cx.notify();
    }

    fn dismiss_notification(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.notifications.items.len() {
            self.notifications.items.remove(index);
            cx.notify();
        }
    }

    pub(crate) fn toggle_notifications(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(
            self.popover.as_ref().map(|open| &open.popover),
            Some(Popover::Notifications)
        ) {
            self.close_popover(window, cx);
        } else {
            self.notifications.unread = false;
            self.open_popover(Popover::Notifications, window, cx);
        }
    }

    pub(crate) fn toggle_projects(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(
            self.popover.as_ref().map(|open| &open.popover),
            Some(Popover::Projects)
        ) {
            self.close_popover(window, cx);
        } else {
            self.open_popover(Popover::Projects, window, cx);
        }
    }

    pub(crate) fn show_tab_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(
            self.popover.as_ref().map(|open| &open.popover),
            Some(Popover::TabMenu { .. })
        ) {
            self.close_popover(window, cx);
            return;
        }
        let context = self.quick_open_context.clone();
        let view = cx.new(|cx| MenuView::new(TAB_MENU, true, &context, window, cx));
        let subscription = cx.subscribe_in(&view, window, |this, _, event, window, cx| {
            if *event == MenuEvent::Dismiss {
                this.close_popover(window, cx);
            }
        });
        self.open_popover(
            Popover::TabMenu {
                view,
                _subscription: subscription,
            },
            window,
            cx,
        );
    }

    fn open_popover(&mut self, popover: Popover, window: &mut Window, cx: &mut Context<Self>) {
        let previous_focus = match self.popover.take() {
            Some(open) => open.previous_focus,
            None => window.focused(cx),
        };
        // The tab menu focuses itself; the others take keys (Escape)
        // through their frame.
        if !matches!(popover, Popover::TabMenu { .. }) {
            let focus = self
                .popover_focus
                .get_or_insert_with(|| cx.focus_handle())
                .clone();
            focus.focus(window, cx);
        }
        self.popover = Some(OpenPopover {
            popover,
            previous_focus,
        });
        cx.notify();
    }

    pub(crate) fn close_popover(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(open) = self.popover.take() {
            if let Some(focus) = open.previous_focus {
                focus.focus(window, cx);
            }
            cx.notify();
        }
    }

    /// Catches clicks outside the open popover.
    pub(crate) fn popover_backdrop(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.popover.as_ref()?;
        Some(
            div()
                .id("popover-backdrop")
                .absolute()
                .inset_0()
                .occlude()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| this.close_popover(window, cx)),
                )
                .into_any_element(),
        )
    }

    /// The popover hanging from the button it belongs to: `which` names
    /// that button's popover, `corner` the corner it hangs from.
    pub(crate) fn popover_at(
        &self,
        which: PopoverSlot,
        corner: Anchor,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let open = self.popover.as_ref()?;
        let content = match (&open.popover, which) {
            (Popover::Notifications, PopoverSlot::Bell) => self.notification_center(cx),
            (Popover::Projects, PopoverSlot::ProjectCard) => self.project_list(cx),
            (Popover::TabMenu { view, .. }, PopoverSlot::TabMenu) => {
                view.clone().into_any_element()
            }
            _ => return None,
        };
        let space = cx.theme().metrics.space.sm;
        let padded = match corner {
            Anchor::BottomLeft | Anchor::BottomRight | Anchor::BottomCenter => div().pb(px(space)),
            _ => div().pt(px(space)),
        };
        Some(
            deferred(anchored().anchor(corner).child(padded.child(content)))
                .with_priority(1)
                .into_any_element(),
        )
    }

    pub(crate) fn on_popover_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.key == "escape" && self.popover.is_some() {
            self.close_popover(window, cx);
            cx.stop_propagation();
        }
    }

    fn popover_frame(&self, cx: &Context<Self>) -> gpui::Stateful<gpui::Div> {
        let mut frame = div().id("popover");
        if let Some(focus) = &self.popover_focus {
            frame = frame
                .track_focus(focus)
                .on_key_down(cx.listener(Self::on_popover_key));
        }
        let theme = cx.theme();
        let metrics = &theme.metrics;
        frame
            .flex()
            .flex_col()
            .w(px(metrics.space.xl * POPOVER_WIDTH_STEPS))
            .p(px(metrics.space.xs))
            .rounded(px(metrics.radius.md))
            .border_1()
            .border_color(theme.colors.border_panel)
            .bg(theme.colors.bg_overlay)
            .occlude()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
    }

    fn notification_center(&self, cx: &mut Context<Self>) -> AnyElement {
        let entity = cx.entity().downgrade();
        let theme = cx.theme();
        let metrics = &theme.metrics;
        let colors = &theme.colors;
        let clear = {
            let entity = entity.clone();
            Button::new("notifications-clear", self.text("notifications-clear"))
                .variant(ButtonVariant::Ghost)
                .disabled(self.notifications.items.is_empty())
                .on_activate(move |_, cx| {
                    let _ = entity.update(cx, Workbench::clear_notifications);
                })
        };
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .px(px(metrics.space.md))
            .py(px(metrics.space.xs))
            .text_size(px(metrics.font_size.sm))
            .text_color(colors.text_subtle)
            .child(self.text("notifications-title"))
            .child(clear);
        let mut frame = self.popover_frame(cx).child(header);
        if self.notifications.items.is_empty() {
            frame = frame.child(
                div()
                    .px(px(metrics.space.md))
                    .py(px(metrics.space.lg))
                    .text_size(px(metrics.font_size.base))
                    .text_color(colors.text_muted)
                    .child(self.text("notifications-empty")),
            );
        }
        for (index, notification) in self.notifications.items.iter().enumerate() {
            let (icon, color) = match notification.kind {
                NotificationKind::Info => (IconName::Info, colors.text_accent),
                NotificationKind::Warning => (IconName::TriangleAlert, colors.diagnostic_warning),
                NotificationKind::Error => (IconName::CircleX, colors.diagnostic_error),
            };
            let entity = entity.clone();
            frame = frame.child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(metrics.space.sm))
                    .px(px(metrics.space.md))
                    .py(px(metrics.space.sm))
                    .rounded(px(metrics.radius.sm))
                    .bg(colors.bg_card)
                    .mt(px(metrics.space.xs))
                    .child(Icon::new(icon).color(color))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(metrics.font_size.base))
                            .text_color(colors.text_primary)
                            .child(notification.message.clone()),
                    )
                    .child(
                        IconButton::new(("notification-dismiss", index), IconName::X)
                            .tooltip(self.text("notifications-dismiss"))
                            .on_activate(move |_, cx| {
                                let _ = entity.update(cx, |workbench, cx| {
                                    workbench.dismiss_notification(index, cx);
                                });
                            }),
                    ),
            );
        }
        frame.into_any_element()
    }

    fn project_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let entity = cx.entity().downgrade();
        // Display paths: the stored root may differ in form (case,
        // canonical prefix) from the open one.
        let current = self
            .workspace
            .as_ref()
            .map(|workspace| workspace.path.clone());
        let theme = cx.theme();
        let metrics = &theme.metrics;
        let colors = &theme.colors;
        let mut frame = self.popover_frame(cx).child(
            div()
                .px(px(metrics.space.md))
                .py(px(metrics.space.xs))
                .text_size(px(metrics.font_size.sm))
                .text_color(colors.text_subtle)
                .child(self.text("project-picker-title")),
        );
        let recent: Vec<_> = self
            .recent
            .iter()
            .filter(|entry| Some(&entry.path) != current.as_ref())
            .collect();
        if recent.is_empty() {
            frame = frame.child(
                div()
                    .px(px(metrics.space.md))
                    .py(px(metrics.space.sm))
                    .text_size(px(metrics.font_size.base))
                    .text_color(colors.text_muted)
                    .child(self.text("project-picker-empty")),
            );
        }
        for (index, entry) in recent.into_iter().enumerate() {
            let entity = entity.clone();
            let root: PathBuf = entry.root.clone();
            frame = frame.child(
                ListRow::new(("project-row", index), entry.name.clone())
                    .leading(Icon::new(IconName::Folder).color(colors.icon))
                    .trailing(
                        div()
                            .truncate()
                            .max_w(px(metrics.space.xl * POPOVER_WIDTH_STEPS / 2.0))
                            .text_size(px(metrics.font_size.xs))
                            .text_color(colors.text_subtle)
                            .child(entry.path.clone()),
                    )
                    .on_activate(move |window, cx| {
                        let root = root.clone();
                        let _ = entity.update(cx, |workbench, cx| {
                            workbench.close_popover(window, cx);
                            let open_path = workbench.open_path.clone();
                            window.defer(cx, move |window, cx| open_path(root, window, cx));
                        });
                    }),
            );
        }
        let open_entity = entity.clone();
        frame = frame.child(
            div().mt(px(metrics.space.xs)).child(
                ListRow::new(
                    "project-open-folder",
                    self.text("project-picker-open-folder"),
                )
                .leading(Icon::new(IconName::FolderOpen).color(colors.icon))
                .on_activate(move |window, cx| {
                    let _ = open_entity.update(cx, |workbench, cx| {
                        workbench.close_popover(window, cx);
                        if let Ok(id) = commands::CommandId::new(crate::command_ids::OPEN_FOLDER) {
                            let dispatch = workbench.dispatch.clone();
                            window.defer(cx, move |window, cx| dispatch(&id, window, cx));
                        }
                    });
                }),
            ),
        );
        frame.into_any_element()
    }
}

/// The buttons a popover hangs from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PopoverSlot {
    Bell,
    ProjectCard,
    TabMenu,
}
