//! The status bar. Items will be contributed by services (git, language
//! servers); for now it shows the folder, the problems of the project and
//! the active editor's position and text format.

use editor::EditorStatus;
use gpui::{
    AnyElement, App, Context, IntoElement, ParentElement as _, SharedString, Styled as _, div, px,
};
use i18n::FluentArgs;
use text::{LineEnding, LineEndings};
use theme::ActiveTheme as _;
use ui::{Icon, IconName};

use crate::Workbench;

pub(crate) fn render(
    workbench: &Workbench,
    cx: &mut Context<Workbench>,
) -> impl IntoElement + use<> {
    let status = workbench
        .active_editor()
        .map(|editor| editor.read(cx).status());
    let items = status.map(|status| editor_items(workbench, &status));
    let problems = workbench
        .problem_counts(cx)
        .map(|counts| problem_items(workbench, counts, cx));
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
        .child(
            div()
                .flex()
                .min_w_0()
                .items_center()
                .gap(px(metrics.space.xl))
                .child(div().truncate().child(match &workbench.workspace {
                    Some(workspace) => workspace.path.clone(),
                    None => workbench.text("status-no-folder"),
                }))
                .children(problems.into_iter().flatten()),
        )
        .child(right)
}

/// Errors and warnings of the project, as in `editor.svg`: a check before
/// the errors while there are none, otherwise the error icon; the warning
/// icon only while there are warnings.
fn problem_items(
    workbench: &Workbench,
    (errors, warnings): (usize, usize),
    cx: &App,
) -> Vec<AnyElement> {
    let theme = cx.theme();
    let colors = &theme.colors;
    let item = |icon: Option<Icon>, id: &str, count: usize| {
        let mut args = FluentArgs::new();
        args.set("count", count);
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(theme.metrics.space.sm))
            .children(icon)
            .child(workbench.i18n.text_with(id, &args))
            .into_any_element()
    };
    let error_icon = if errors == 0 {
        Icon::new(IconName::Check).color(colors.icon_success)
    } else {
        Icon::new(IconName::CircleX).color(colors.diagnostic_error)
    };
    let warning_icon =
        (warnings > 0).then(|| Icon::new(IconName::TriangleAlert).color(colors.diagnostic_warning));
    vec![
        item(Some(error_icon), "status-errors", errors),
        item(warning_icon, "status-warnings", warnings),
    ]
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
