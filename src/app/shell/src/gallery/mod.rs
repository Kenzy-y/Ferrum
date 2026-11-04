//! Temporary component gallery (stage 1): every `ui` component in every
//! state, for comparison with the mockups. To be removed or hidden behind a
//! command after stage 1 (see ROADMAP).

mod samples;
mod sections;

use std::sync::Arc;

use gpui::{
    AnyElement, App, AppContext as _, Context, ElementId, Entity, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _,
    Styled as _, Window, div, px,
};
use gpui_component::input::{InputState, TextareaState};
use i18n::I18n;
use theme::ActiveTheme as _;
use ui::{InteractionState, Label};

/// Rows of the commit message text area.
const COMMIT_ROWS: usize = 3;

pub(crate) struct Gallery {
    i18n: Arc<I18n>,
    /// One editing state per cell of the field rows (forced states + live):
    /// an input state must not be rendered twice in a frame.
    fields: Vec<Entity<InputState>>,
    commits: Vec<Entity<TextareaState>>,
    live_tab: usize,
    live_row: usize,
}

impl Gallery {
    pub(crate) fn new(i18n: Arc<I18n>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let placeholder = i18n.text("gallery-sample-field-placeholder");
        let commit_placeholder = i18n.text("gallery-sample-commit-placeholder");
        let cells = InteractionState::ALL.len() + 1;
        let fields = (0..cells)
            .map(|_| cx.new(|cx| InputState::new(window, cx).placeholder(placeholder.clone())))
            .collect();
        let commits = (0..cells)
            .map(|_| {
                cx.new(|cx| {
                    TextareaState::new(window, cx)
                        .rows(COMMIT_ROWS)
                        .placeholder(commit_placeholder.clone())
                })
            })
            .collect();
        Self {
            i18n,
            fields,
            commits,
            live_tab: 0,
            live_row: 0,
        }
    }

    fn text(&self, id: &str) -> SharedString {
        self.i18n.text(id).into()
    }

    fn state_label(&self, state: Option<InteractionState>) -> SharedString {
        let id = match state {
            None => "gallery-state-live",
            Some(InteractionState::Normal) => "gallery-state-normal",
            Some(InteractionState::Hover) => "gallery-state-hover",
            Some(InteractionState::Pressed) => "gallery-state-pressed",
            Some(InteractionState::Focused) => "gallery-state-focused",
            Some(InteractionState::Disabled) => "gallery-state-disabled",
        };
        self.text(id)
    }

    /// A row with one cell per forced state plus a live cell. `build` gets
    /// the forced state (`None` for the live cell) and a unique element id.
    fn states_row(
        &self,
        name: &str,
        cx: &App,
        build: impl Fn(Option<InteractionState>, ElementId) -> AnyElement,
    ) -> AnyElement {
        let theme = cx.theme();
        let caption_size = theme.metrics.font_size.xs;
        let caption_color = theme.colors.text_muted;
        let gap = theme.metrics.space.lg;
        let cells = InteractionState::ALL
            .into_iter()
            .map(Some)
            .chain(std::iter::once(None))
            .enumerate()
            .map(|(index, state)| {
                let id = ElementId::Name(format!("{name}-{index}").into());
                div()
                    .flex()
                    .flex_col()
                    .gap(px(theme.metrics.space.xs))
                    .child(
                        Label::new(self.state_label(state))
                            .size(caption_size)
                            .color(caption_color),
                    )
                    .child(build(state, id))
            });
        div()
            .flex()
            .flex_wrap()
            .items_end()
            .gap(px(gap))
            .children(cells)
            .into_any_element()
    }

    fn select_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        self.live_tab = index;
        cx.notify();
    }

    fn select_row(&mut self, index: usize, cx: &mut Context<Self>) {
        self.live_row = index;
        cx.notify();
    }
}

/// A titled block of the gallery.
fn section(title: SharedString, content: Vec<AnyElement>, cx: &App) -> AnyElement {
    let theme = cx.theme();
    div()
        .flex()
        .flex_col()
        .gap(px(theme.metrics.space.lg))
        .child(
            Label::new(title)
                .size(theme.metrics.font_size.h3)
                .semibold()
                .color(theme.colors.text_primary),
        )
        .children(content)
        .into_any_element()
}

impl Render for Gallery {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sections = vec![
            sections::colors(self, cx),
            sections::syntax(self, cx),
            sections::typography(self, cx),
            sections::buttons(self, cx),
            sections::badges(self, cx),
            sections::tabs(self, cx),
            sections::panels(self, cx),
            sections::lists(self, cx),
            sections::inputs(self, window, cx),
            sections::icons(self, cx),
        ];
        let theme = cx.theme();
        let padding = theme.metrics.space.xxl;
        let header = div()
            .flex()
            .flex_col()
            .gap(px(theme.metrics.space.sm))
            .child(
                Label::new(self.text("gallery-title"))
                    .size(theme.metrics.font_size.h1)
                    .semibold()
                    .color(theme.colors.text_primary),
            )
            .child(
                Label::new(self.text("gallery-note"))
                    .size(theme.metrics.font_size.base)
                    .color(theme.colors.text_secondary),
            );
        div()
            .id("gallery")
            .size_full()
            .overflow_y_scroll()
            .bg(theme.colors.bg_editor)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(padding))
                    .p(px(padding))
                    .child(header)
                    .children(sections),
            )
    }
}
