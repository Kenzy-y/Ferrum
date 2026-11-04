//! The status bar. Items will be contributed by services (git, language
//! servers); for now it shows the folder and the active editor's position
//! and text format.

use editor::EditorStatus;
use gpui::{Context, IntoElement, ParentElement as _, SharedString, Styled as _, div, px};
use i18n::FluentArgs;
use text::{LineEnding, LineEndings};
use theme::ActiveTheme as _;

use crate::Workbench;

pub(crate) fn render(
    workbench: &Workbench,
    cx: &mut Context<Workbench>,
) -> impl IntoElement + use<> {
    let status = workbench
        .active_editor()
        .map(|editor| editor.read(cx).status());
    let items = status.map(|status| editor_items(workbench, &status));
    let theme = cx.theme();
    let metrics = &theme.metrics;
    let mut right = div().flex().items_center().gap(px(metrics.space.xxl));
    for item in items.into_iter().flatten() {
        right = right.child(item);
    }
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_between()
        .h(px(metrics.size.statusbar_height))
        .px(px(metrics.space.xl))
        .gap(px(metrics.space.xl))
        .text_size(px(metrics.font_size.sm))
        .text_color(theme.colors.text_secondary)
        .child(div().truncate().child(match &workbench.workspace {
            Some(workspace) => workspace.path.clone(),
            None => workbench.text("status-no-folder"),
        }))
        .child(right)
}

/// Position, indentation, encoding and line endings.
fn editor_items(workbench: &Workbench, status: &EditorStatus) -> Vec<SharedString> {
    let i18n = &workbench.i18n;
    let mut position = FluentArgs::new();
    position.set("line", status.line);
    position.set("column", status.column);
    let mut items = vec![i18n.text_with("status-position", &position).into()];
    if status.selections > 1 {
        let mut count = FluentArgs::new();
        count.set("count", status.selections);
        items.push(i18n.text_with("status-selections", &count).into());
    }
    let mut indent = FluentArgs::new();
    indent.set("size", status.tab_size);
    let indent_id = if status.insert_spaces {
        "status-indent-spaces"
    } else {
        "status-indent-tabs"
    };
    items.push(i18n.text_with(indent_id, &indent).into());
    items.push(workbench.text("status-encoding"));
    let ending = match status.line_endings {
        LineEndings::Uniform(LineEnding::Lf) => "status-line-ending-lf",
        LineEndings::Uniform(LineEnding::CrLf) => "status-line-ending-crlf",
        LineEndings::Mixed { .. } => "status-line-ending-mixed",
    };
    items.push(workbench.text(ending));
    items
}
