//! The title bar: logo, menus, search, actions and window controls.

use commands::CommandId;
use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Context, Div, FontWeight, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, Styled as _, TitlebarOptions, Window, WindowControlArea, deferred, div,
    point, px,
};
use theme::ActiveTheme as _;
use ui::{Button, ButtonVariant, Icon, IconButton, IconName, SearchField};

use crate::{Dispatch, Workbench, command_ids, window_controls};

/// Space reserved on macOS for the traffic light buttons, which the system
/// draws over the left edge of the title bar.
const MACOS_TRAFFIC_LIGHTS_WIDTH: f32 = 72.0;

/// Where macOS places the traffic lights: vertically centered in the
/// title bar of the mockups.
const MACOS_TRAFFIC_LIGHTS_POSITION: (f32, f32) = (20.0, 26.0);

/// Command of the "Open folder" button; implemented in stage 2.
const OPEN_FOLDER_COMMAND: &str = "workspace.openFolder";

/// Menu buttons: message ids of their labels.
const MENUS: [&str; 4] = ["menu-file", "menu-edit", "menu-view", "menu-go"];

/// Title bar options for a window whose title bar the workbench draws.
pub(crate) fn titlebar_options() -> TitlebarOptions {
    let (x, y) = MACOS_TRAFFIC_LIGHTS_POSITION;
    TitlebarOptions {
        title: None,
        appears_transparent: true,
        traffic_light_position: Some(point(px(x), px(y))),
    }
}

/// A click handler that runs `command` through `dispatch`.
fn run(dispatch: &Dispatch, command: &'static str) -> impl Fn(&mut Window, &mut App) + 'static {
    let dispatch = dispatch.clone();
    move |window, cx| {
        if let Ok(id) = CommandId::new(command) {
            dispatch(&id, window, cx);
        }
    }
}

/// Wraps an interactive element of the title bar so that it covers the
/// window drag area under it. Without this the platform hit test (Windows
/// `HTCAPTION`) sees the drag area through the element and treats a click on
/// it as the start of a window move, so the element never gets the click.
fn clickable(element: impl IntoElement) -> Div {
    div().flex().flex_none().occlude().child(element)
}

/// Logo, wordmark and menus.
fn leading(workbench: &Workbench, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let colors = &theme.colors;
    let metrics = &theme.metrics;
    let size = &metrics.size;
    let logo = div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(size.titlebar_logo_size))
        .rounded(px(size.titlebar_logo_radius))
        .bg(colors.accent)
        .child(Icon::new(IconName::Terminal).color(colors.text_on_accent));
    let wordmark = div()
        .ml(px(metrics.space.md))
        .text_size(px(metrics.font_size.brand))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(colors.text_primary)
        .child(workbench.text("app-wordmark"));
    let menus = div()
        .flex()
        .flex_none()
        .items_center()
        .ml(px(size.titlebar_menus_offset - metrics.space.md))
        .children(MENUS.iter().map(|id| {
            clickable(Button::new(*id, workbench.text(id)).variant(ButtonVariant::Text))
        }));
    div()
        .flex()
        .flex_none()
        .items_center()
        .child(logo)
        .child(wordmark)
        .child(menus)
        .into_any_element()
}

/// The search field: takes the free space between the side groups, up to
/// its width in the mockups, centered in it.
fn search(workbench: &Workbench, cx: &App) -> AnyElement {
    let theme = cx.theme();
    div()
        .flex()
        .flex_1()
        .min_w_0()
        .justify_center()
        .px(px(theme.metrics.space.xl))
        .child(
            div()
                .flex()
                .w_full()
                .min_w_0()
                .max_w(px(theme.metrics.size.titlebar_search_width))
                .relative()
                // Covers the drag area, see `clickable`.
                .occlude()
                .child({
                    let field = SearchField::new(
                        "titlebar-search",
                        workbench.text("titlebar-search-placeholder"),
                    )
                    .on_activate(run(&workbench.dispatch, command_ids::QUICK_OPEN));
                    match workbench.shortcut_label(command_ids::QUICK_OPEN) {
                        Some(keys) => field.shortcut(keys),
                        None => field,
                    }
                })
                // Quick open opens exactly over the field; `deferred` paints
                // it after the panels so its list is not covered by them.
                .children(workbench.quick_open_view().map(|view| {
                    deferred(div().absolute().top_0().left_0().w_full().child(view))
                        .with_priority(1)
                })),
        )
        .into_any_element()
}

/// Actions on the right, before the window controls.
fn actions(workbench: &Workbench, cx: &App) -> AnyElement {
    let gap = cx.theme().metrics.space.lg;
    let open_folder = Button::new("open-folder", workbench.text("titlebar-open-folder"))
        .icon(IconName::Folder)
        .on_activate(run(&workbench.dispatch, OPEN_FOLDER_COMMAND));
    let toggle_sidebar = IconButton::new("toggle-sidebar", IconName::Columns)
        .on_activate(run(&workbench.dispatch, command_ids::TOGGLE_SIDEBAR));
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(gap))
        .child(clickable(open_folder))
        .child(clickable(toggle_sidebar))
        .into_any_element()
}

pub(crate) fn render(
    workbench: &Workbench,
    window: &mut Window,
    cx: &mut Context<Workbench>,
) -> impl IntoElement + use<> {
    let leading = leading(workbench, cx);
    let search = search(workbench, cx);
    let actions = actions(workbench, cx);
    let controls = window_controls::render(window, cx);

    let theme = cx.theme();
    let size = &theme.metrics.size;
    let left_inset = if cfg!(target_os = "macos") {
        MACOS_TRAFFIC_LIGHTS_WIDTH + size.titlebar_inset
    } else {
        size.titlebar_inset
    };
    div()
        .id("title-bar")
        .flex()
        .flex_none()
        .items_center()
        .h(px(size.titlebar_height))
        .pl(px(left_inset))
        .bg(theme.colors.bg_app)
        .when(cfg!(target_os = "windows"), |bar| {
            bar.window_control_area(WindowControlArea::Drag)
        })
        .when(!cfg!(target_os = "windows"), |bar| {
            bar.on_mouse_down(MouseButton::Left, |event, window, _| {
                match event.click_count {
                    1 => window.start_window_move(),
                    2 if cfg!(target_os = "macos") => window.titlebar_double_click(),
                    2 => window.zoom_window(),
                    _ => {}
                }
            })
        })
        .child(leading)
        .child(search)
        .child(actions)
        .child(div().ml(px(theme.metrics.space.lg)).child(controls))
}
