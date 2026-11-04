//! Minimize, maximize and close buttons on Windows and Linux. macOS uses
//! the system traffic lights instead.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Decorations, Hsla, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, StatefulInteractiveElement as _, Styled as _, Window, WindowControlArea,
    div, px,
};
use theme::ActiveTheme as _;
use ui::{Icon, IconName};

use commands::CommandId;

use crate::{Dispatch, command_ids};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Control {
    Minimize,
    Maximize,
    Close,
}

/// The buttons; Close runs `close` (which asks about unsaved files).
pub(crate) fn render(window: &mut Window, close: &Dispatch, cx: &mut App) -> AnyElement {
    // The window manager draws its own buttons with server-side decorations.
    let server_decorated =
        cfg!(target_os = "linux") && matches!(window.window_decorations(), Decorations::Server);
    if cfg!(target_os = "macos") || server_decorated {
        return div().into_any_element();
    }
    let maximized = window.is_maximized();
    div()
        .flex()
        .h_full()
        .children(
            [Control::Minimize, Control::Maximize, Control::Close]
                .into_iter()
                .map(|control| button(control, maximized, close.clone(), cx)),
        )
        .into_any_element()
}

fn button(
    control: Control,
    maximized: bool,
    close: Dispatch,
    cx: &App,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let colors = &theme.colors;
    let (id, icon, area) = match control {
        Control::Minimize => ("window-minimize", IconName::Minus, WindowControlArea::Min),
        Control::Maximize if maximized => {
            ("window-restore", IconName::Copy, WindowControlArea::Max)
        }
        Control::Maximize => ("window-maximize", IconName::Square, WindowControlArea::Max),
        Control::Close => ("window-close", IconName::X, WindowControlArea::Close),
    };
    let hover: Hsla = if control == Control::Close {
        colors.bg_window_close_hover.into()
    } else {
        colors.bg_ghost_hover.into()
    };
    let pressed: Hsla = if control == Control::Close {
        colors.bg_window_close_hover.into()
    } else {
        colors.bg_ghost_pressed.into()
    };
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .w(px(theme.metrics.size.window_control_width))
        .h(px(theme.metrics.size.titlebar_height))
        .hover(move |style| style.bg(hover))
        .active(move |style| style.bg(pressed))
        .when(cfg!(target_os = "windows"), |element| {
            element.window_control_area(area)
        })
        .when(cfg!(target_os = "linux"), |element| {
            element
                .on_mouse_down(MouseButton::Left, |_, window, cx| {
                    window.prevent_default();
                    cx.stop_propagation();
                })
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    match control {
                        Control::Minimize => window.minimize_window(),
                        Control::Maximize => window.zoom_window(),
                        Control::Close => {
                            if let Ok(id) = CommandId::new(command_ids::CLOSE_WINDOW) {
                                close(&id, window, cx);
                            }
                        }
                    }
                })
        })
        .child(
            Icon::new(icon)
                .size(theme.metrics.size.icon_tiny)
                .color(colors.icon_strong),
        )
}
