//! The editor area: a tab strip and the active content. Real editors come
//! in stage 3; until then it can host any view (the component gallery).

use gpui::{Context, IntoElement, ParentElement as _, Styled as _, div, px};
use theme::ActiveTheme as _;
use ui::{EditorTab, Panel, PanelSurface};

use crate::Workbench;

pub(crate) fn render(
    workbench: &Workbench,
    cx: &mut Context<Workbench>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let size = &theme.metrics.size;
    let space = &theme.metrics.space;
    let mut panel = Panel::new(PanelSurface::Editor).flex_1().min_h_0();
    if let Some(content) = &workbench.editor {
        let tab_inset = (size.tab_strip_height - size.tab_height) / 2.0;
        let strip = div()
            .flex()
            .flex_none()
            .items_center()
            .h(px(size.tab_strip_height))
            .px(px(tab_inset))
            .gap(px(space.sm))
            .bg(theme.colors.bg_editor_tabs)
            .child(EditorTab::new("editor-tab-0", content.title.clone()).active(true));
        panel = panel
            .child(strip)
            .child(div().flex_1().min_h_0().child(content.view.clone()));
    }
    panel
}
