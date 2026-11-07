//! Resizing the sidebar and the bottom panel by dragging the gaps next to
//! them, as in VS Code. A double click on a gap gives the default size
//! back. Sizes are clamped so the editor area keeps its least size, and
//! the application saves them ([`PanelSizesChanged`]).

use gpui::{
    App, Context, CursorStyle, EventEmitter, InteractiveElement as _, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, Pixels, Point, Styled as _, Window, div, px,
};
use project::WindowState;
use theme::ActiveTheme as _;

use crate::Workbench;

/// The user dragged a panel to a new size; the application saves it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelSizesChanged(pub WindowState);

impl EventEmitter<PanelSizesChanged> for Workbench {}

/// Which gap is dragged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResizeEdge {
    /// Between the sidebar and the editor area: the sidebar width.
    Sidebar,
    /// Between the editor area and the bottom panel: the panel height.
    BottomPanel,
}

/// A drag in progress.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResizeDrag {
    edge: ResizeEdge,
    start: Point<Pixels>,
    start_size: f32,
}

/// Allowed range of a size; the maximum never drops below the minimum.
fn clamp(size: f32, min: f32, max: f32) -> f32 {
    size.clamp(min, max.max(min))
}

impl Workbench {
    /// Sizes the user chose, as restored from the last run.
    pub fn set_panel_sizes(&mut self, sizes: WindowState, cx: &mut Context<Self>) {
        self.sizes = sizes;
        cx.notify();
    }

    /// Width of the sidebar in this window.
    pub(crate) fn sidebar_width(&self, window: &Window, cx: &App) -> f32 {
        let size = &cx.theme().metrics.size;
        let viewport = window.viewport_size().width.as_f32();
        let max = viewport - size.activity_bar_width - size.editor_min_width - size.panel_gap * 2.0;
        clamp(
            self.sizes.sidebar_width.unwrap_or(size.sidebar_width),
            size.sidebar_min_width,
            max,
        )
    }

    /// Height of the bottom panel (when not maximized) in this window.
    pub(crate) fn panel_height(&self, window: &Window, cx: &App) -> f32 {
        let size = &cx.theme().metrics.size;
        let viewport = window.viewport_size().height.as_f32();
        let max = viewport
            - size.titlebar_height
            - size.statusbar_height
            - size.layout_gap_top
            - size.editor_min_height
            - size.panel_gap;
        clamp(
            self.sizes.panel_height.unwrap_or(size.bottom_panel_height),
            size.bottom_panel_min_height,
            max,
        )
    }

    /// The gap next to a resizable panel; dragging it resizes the panel.
    pub(crate) fn resize_handle(
        edge: ResizeEdge,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let gap = cx.theme().metrics.size.panel_gap;
        let (id, cursor) = match edge {
            ResizeEdge::Sidebar => ("resize-sidebar", CursorStyle::ResizeLeftRight),
            ResizeEdge::BottomPanel => ("resize-panel", CursorStyle::ResizeUpDown),
        };
        let handle = div()
            .id(id)
            .debug_selector(move || id.to_owned())
            .flex_none()
            .cursor(cursor)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    if event.click_count >= 2 {
                        this.reset_size(edge, cx);
                    } else {
                        this.start_resize(edge, event.position, window, cx);
                    }
                }),
            );
        match edge {
            ResizeEdge::Sidebar => handle.w(px(gap)).h_full(),
            ResizeEdge::BottomPanel => handle.h(px(gap)).w_full(),
        }
    }

    fn start_resize(
        &mut self,
        edge: ResizeEdge,
        position: Point<Pixels>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let start_size = match edge {
            ResizeEdge::Sidebar => self.sidebar_width(window, cx),
            ResizeEdge::BottomPanel => self.panel_height(window, cx),
        };
        self.resize = Some(ResizeDrag {
            edge,
            start: position,
            start_size,
        });
        cx.notify();
    }

    /// Follows the pointer while a gap is dragged.
    pub(crate) fn drag_resize(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(drag) = self.resize else {
            return;
        };
        if event.pressed_button != Some(MouseButton::Left) {
            self.end_resize(cx);
            return;
        }
        match drag.edge {
            ResizeEdge::Sidebar => {
                let delta = (event.position.x - drag.start.x).as_f32();
                self.sizes.sidebar_width = Some(drag.start_size + delta);
                let clamped = self.sidebar_width(window, cx);
                self.sizes.sidebar_width = Some(clamped);
            }
            ResizeEdge::BottomPanel => {
                // The panel grows when the gap moves up.
                let delta = (drag.start.y - event.position.y).as_f32();
                self.sizes.panel_height = Some(drag.start_size + delta);
                let clamped = self.panel_height(window, cx);
                self.sizes.panel_height = Some(clamped);
            }
        }
        cx.notify();
    }

    /// Ends a drag and reports the new sizes.
    pub(crate) fn end_resize(&mut self, cx: &mut Context<Self>) {
        if self.resize.take().is_some() {
            cx.emit(PanelSizesChanged(self.sizes));
            cx.notify();
        }
    }

    fn reset_size(&mut self, edge: ResizeEdge, cx: &mut Context<Self>) {
        match edge {
            ResizeEdge::Sidebar => self.sizes.sidebar_width = None,
            ResizeEdge::BottomPanel => self.sizes.panel_height = None,
        }
        self.resize = None;
        cx.emit(PanelSizesChanged(self.sizes));
        cx.notify();
    }

    /// Cursor over the whole window while a gap is dragged.
    pub(crate) fn resize_cursor(&self) -> Option<CursorStyle> {
        self.resize.map(|drag| match drag.edge {
            ResizeEdge::Sidebar => CursorStyle::ResizeLeftRight,
            ResizeEdge::BottomPanel => CursorStyle::ResizeUpDown,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::Arc;

    use commands::{CommandRegistry, Keymap};
    use gpui::{Modifiers, TestAppContext, VisualTestContext, point, size};
    use i18n::I18n;

    use super::*;

    fn moved(x: f32, y: f32) -> MouseMoveEvent {
        MouseMoveEvent {
            position: point(px(x), px(y)),
            pressed_button: Some(MouseButton::Left),
            modifiers: Modifiers::default(),
        }
    }

    #[gpui::test]
    fn dragging_resizes_within_limits_and_reports_the_sizes(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let theme =
                theme::Theme::built_in(theme::DEFAULT_THEME, theme::Density::default()).unwrap();
            theme::set_theme(theme, cx);
        });
        let window = cx.add_window(|_, _| {
            Workbench::new(
                Arc::new(I18n::new("en").unwrap()),
                Rc::new(|_, _, _| {}),
                Arc::new(CommandRegistry::new()),
                Arc::new(Keymap::default()),
                Rc::new(|_, _, _| {}),
            )
        });
        let workbench = window.root(cx).unwrap();
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        cx.simulate_resize(size(px(1600.0), px(1000.0)));
        let reported = Rc::new(RefCell::new(Vec::new()));
        let record = reported.clone();
        cx.update(|_, cx| {
            cx.subscribe(&workbench, move |_, event: &PanelSizesChanged, _| {
                record.borrow_mut().push(event.0);
            })
            .detach();
        });
        workbench.update_in(&mut cx, |workbench, window, cx| {
            let default = workbench.sidebar_width(window, cx);
            workbench.start_resize(ResizeEdge::Sidebar, point(px(300.0), px(500.0)), window, cx);
            workbench.drag_resize(&moved(360.0, 500.0), window, cx);
            assert!((workbench.sidebar_width(window, cx) - (default + 60.0)).abs() < 0.5);
            // Far to the left: stops at the least width.
            workbench.drag_resize(&moved(-2000.0, 500.0), window, cx);
            let min = cx.theme().metrics.size.sidebar_min_width;
            assert!((workbench.sidebar_width(window, cx) - min).abs() < 0.5);
            workbench.end_resize(cx);

            workbench.panel_visible = true;
            let panel = workbench.panel_height(window, cx);
            workbench.start_resize(
                ResizeEdge::BottomPanel,
                point(px(800.0), px(700.0)),
                window,
                cx,
            );
            workbench.drag_resize(&moved(800.0, 650.0), window, cx);
            assert!((workbench.panel_height(window, cx) - (panel + 50.0)).abs() < 0.5);
            workbench.end_resize(cx);
        });
        cx.run_until_parked();
        let reported = reported.borrow();
        assert_eq!(reported.len(), 2);
        assert!(reported[1].panel_height.is_some());
        assert!(reported[1].sidebar_width.is_some());
    }

    #[test]
    fn clamping_keeps_the_minimum_when_the_window_is_too_small() {
        assert!((clamp(50.0, 100.0, 400.0) - 100.0).abs() < f32::EPSILON);
        assert!((clamp(500.0, 100.0, 400.0) - 400.0).abs() < f32::EPSILON);
        assert!((clamp(250.0, 100.0, 40.0) - 100.0).abs() < f32::EPSILON);
    }
}
