//! The Outline section of the Explorer: which items it shows (those of the
//! active editor), collapsing it, and going to an item.

pub(crate) mod symbol_icons;
pub(crate) mod view;

use gpui::{
    AnyElement, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement as _,
    ParentElement as _, StatefulInteractiveElement as _, Styled as _, Subscription, Window, div,
    px,
};
use theme::ActiveTheme as _;
use ui::SectionHeader;

use crate::Workbench;
use crate::contrib::outline::view::{GoToSymbol, OutlineView};

/// State of the section.
pub(crate) struct OutlineSection {
    view: Option<Entity<OutlineView>>,
    expanded: bool,
    /// Goes to the items the view activates.
    subscription: Option<Subscription>,
}

impl Default for OutlineSection {
    fn default() -> Self {
        Self {
            view: None,
            expanded: true,
            subscription: None,
        }
    }
}

impl Workbench {
    /// Creates the outline view the first time a frame needs it.
    pub(crate) fn ensure_outline(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.outline.view.is_some() {
            return;
        }
        let empty = self.text("outline-empty");
        let view = cx.new(|cx| OutlineView::new(empty, cx));
        let subscription = cx.subscribe_in(
            &view,
            window,
            |workbench, _, event: &GoToSymbol, window, cx| {
                if let Some(editor) = workbench.active_editor() {
                    editor.update(cx, |editor, cx| {
                        editor.go_to(event.offset, cx);
                        if event.focus_editor {
                            editor.focus(window, cx);
                        }
                    });
                }
            },
        );
        self.outline.view = Some(view);
        self.outline.subscription = Some(subscription);
        self.sync_outline(cx);
    }

    /// Shows the outline of the active editor (nothing without one).
    pub(crate) fn sync_outline(&mut self, cx: &mut Context<Self>) {
        let Some(view) = self.outline.view.clone() else {
            return;
        };
        let items = self.active_editor().map_or_else(
            || std::sync::Arc::from([]),
            |editor| editor.read(cx).outline(),
        );
        let empty = self.text("outline-empty");
        view.update(cx, |view, cx| {
            view.set_empty_text(empty, cx);
            view.set_items(items, cx);
        });
    }

    /// Expands or collapses the section.
    pub(crate) fn toggle_outline(&mut self, cx: &mut Context<Self>) {
        self.outline.expanded = !self.outline.expanded;
        cx.notify();
    }

    /// Shows the Explorer with the section expanded and focuses the list.
    pub(crate) fn focus_outline(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sidebar_visible = true;
        self.view = crate::SidebarView::Explorer;
        self.outline.expanded = true;
        self.ensure_outline(window, cx);
        if let Some(view) = &self.outline.view {
            view.update(cx, |view, cx| view.focus(window, cx));
        }
        cx.notify();
    }

    /// The section: a divider, the header (click to collapse) and the list.
    pub(crate) fn render_outline(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let metrics = &theme.metrics;
        let header = div()
            .id("outline-header")
            .cursor_pointer()
            .on_click(cx.listener(|workbench, _, _, cx| workbench.toggle_outline(cx)))
            .child(SectionHeader::group(
                self.text("explorer-outline"),
                Some(self.outline.expanded),
            ));
        div()
            .flex()
            .flex_col()
            .flex_none()
            .gap(px(metrics.space.md))
            .child(
                div()
                    .mx(px(metrics.space.lg))
                    .border_t_1()
                    .border_color(theme.colors.border_panel),
            )
            .child(header)
            .children(
                self.outline
                    .expanded
                    .then(|| self.outline.view.clone())
                    .flatten(),
            )
            .into_any_element()
    }
}
