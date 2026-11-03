//! Gallery sections for tokens: colors, syntax, typography.

use gpui::{
    AnyElement, App, Context, Hsla, IntoElement, ParentElement as _, SharedString, Styled as _,
    div, px,
};
use theme::{ActiveTheme as _, Color};
use ui::Label;

use super::super::samples::{self, Syntax};
use super::super::{Gallery, section};

/// Width of one color swatch cell.
const SWATCH_CELL_WIDTH: f32 = 220.0;

/// Width of the code sample.
const CODE_SAMPLE_WIDTH: f32 = 760.0;

/// Width of a line number column in the code sample.
const LINE_NUMBER_WIDTH: f32 = 32.0;

fn swatch(key: &'static str, color: Color, cx: &App) -> AnyElement {
    let theme = cx.theme();
    div()
        .flex()
        .items_center()
        .gap(px(theme.metrics.space.sm))
        .w(px(SWATCH_CELL_WIDTH))
        .child(
            div()
                .size(px(theme.metrics.size.badge_size + theme.metrics.space.xs))
                .flex_none()
                .rounded(px(theme.metrics.radius.xs))
                .border_1()
                .border_color(theme.colors.border_input)
                .bg(color),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .child(
                    Label::new(key)
                        .size(theme.metrics.font_size.sm)
                        .color(theme.colors.text_secondary),
                )
                .child(
                    Label::new(color.to_string())
                        .mono()
                        .size(theme.metrics.font_size.xs)
                        .color(theme.colors.text_muted),
                ),
        )
        .into_any_element()
}

fn swatches(entries: Vec<(&'static str, Color)>, cx: &App) -> AnyElement {
    let gap = cx.theme().metrics.space.md;
    div()
        .flex()
        .flex_wrap()
        .gap(px(gap))
        .children(
            entries
                .into_iter()
                .map(|(key, color)| swatch(key, color, cx)),
        )
        .into_any_element()
}

pub(in crate::gallery) fn colors(gallery: &Gallery, cx: &mut Context<Gallery>) -> AnyElement {
    let entries = cx.theme().colors.entries();
    section(
        gallery.text("gallery-section-colors"),
        vec![swatches(entries, cx)],
        cx,
    )
}

fn syntax_color(kind: Syntax, cx: &App) -> Hsla {
    let syntax = &cx.theme().syntax;
    let color = match kind {
        Syntax::Plain => syntax.plain,
        Syntax::Punctuation => syntax.punctuation,
        Syntax::Keyword => syntax.keyword,
        Syntax::Type => syntax.type_name,
        Syntax::Function => syntax.function,
        Syntax::Macro => syntax.macro_name,
        Syntax::Attribute => syntax.attribute,
        Syntax::String => syntax.string,
        Syntax::Number => syntax.number,
        Syntax::Constant => syntax.constant,
        Syntax::Lifetime => syntax.lifetime,
        Syntax::Comment => syntax.comment,
        Syntax::DocComment => syntax.doc_comment,
        Syntax::InlayHint => syntax.inlay_hint,
    };
    color.into()
}

fn code_sample(cx: &App) -> AnyElement {
    let theme = cx.theme();
    let metrics = &theme.metrics;
    let lines = samples::CODE_SAMPLE
        .iter()
        .enumerate()
        .map(|(index, spans)| {
            let active = index == samples::ACTIVE_LINE;
            let number_color = if active {
                theme.colors.text_line_number_active
            } else {
                theme.colors.text_line_number
            };
            let mut line = div()
                .flex()
                .items_center()
                .h(px(metrics.size.code_line_height))
                .rounded(px(metrics.radius.sm))
                .child(
                    div()
                        .w(px(LINE_NUMBER_WIDTH))
                        .flex_none()
                        .text_size(px(metrics.font_size.code_small))
                        .text_color(number_color)
                        .child(SharedString::from((index + 1).to_string())),
                )
                .children(spans.iter().map(|(kind, text)| {
                    let size = if *kind == Syntax::InlayHint {
                        metrics.font_size.xs
                    } else {
                        metrics.font_size.code
                    };
                    div()
                        .whitespace_nowrap()
                        .text_size(px(size))
                        .text_color(syntax_color(*kind, cx))
                        .child(SharedString::from(*text))
                }));
            if active {
                line = line.bg(theme.colors.bg_line_active);
            }
            line
        });
    div()
        .w(px(CODE_SAMPLE_WIDTH))
        .p(px(metrics.space.lg))
        .rounded(px(metrics.radius.xl))
        .border_1()
        .border_color(theme.colors.border_panel)
        .bg(theme.colors.bg_editor)
        .font_family(SharedString::from(theme.fonts.mono.clone()))
        .children(lines)
        .into_any_element()
}

pub(in crate::gallery) fn syntax(gallery: &Gallery, cx: &mut Context<Gallery>) -> AnyElement {
    let entries = cx.theme().syntax.entries();
    section(
        gallery.text("gallery-section-syntax"),
        vec![code_sample(cx), swatches(entries, cx)],
        cx,
    )
}

pub(in crate::gallery) fn typography(gallery: &Gallery, cx: &mut Context<Gallery>) -> AnyElement {
    let theme = cx.theme();
    let sizes = &theme.metrics.font_size;
    let colors = &theme.colors;
    let rows = vec![
        Label::new(gallery.text("gallery-type-display"))
            .size(sizes.display)
            .semibold()
            .color(colors.text_primary),
        Label::new(gallery.text("gallery-type-heading"))
            .size(sizes.h2)
            .semibold()
            .color(colors.text_primary),
        Label::new(gallery.text("gallery-type-title"))
            .size(sizes.h3)
            .semibold()
            .color(colors.text_primary),
        Label::new(gallery.text("gallery-type-body"))
            .size(sizes.xl)
            .color(colors.text_secondary),
        Label::new(gallery.text("gallery-type-secondary"))
            .size(sizes.base)
            .color(colors.text_secondary),
        Label::new(gallery.text("gallery-type-caption"))
            .size(sizes.md)
            .color(colors.text_subtle),
        Label::new(gallery.text("gallery-section-colors").to_uppercase())
            .size(sizes.sm)
            .semibold()
            .color(colors.text_subtle),
        Label::new(gallery.text("gallery-type-mono"))
            .mono()
            .size(sizes.code)
            .color(colors.text_primary),
    ];
    let rows = rows
        .into_iter()
        .map(IntoElement::into_any_element)
        .collect();
    section(gallery.text("gallery-section-typography"), rows, cx)
}
