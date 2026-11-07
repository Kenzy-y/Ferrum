//! Documentation of the selected completion, beside the list. Items
//! without documentation are resolved first when the service can; the
//! resolved item replaces the listed one, so accepting it later already
//! has its edits elsewhere (imports). Markdown is parsed and its code
//! highlighted in the background, as for hover.

use gpui::{
    AnyElement, Context, IntoElement as _, ParentElement as _, SharedString, Styled as _, Task,
    div, px,
};
use theme::Theme;

use crate::editor::Editor;
use crate::language::markdown::{self, Block};
use crate::language::popup;

/// Documentation of one listed item.
struct Shown {
    /// Index of the item and its label, so a changed list does not show
    /// the documentation of another item.
    index: usize,
    label: String,
    detail: Option<String>,
    blocks: Vec<Block>,
}

/// The documentation beside the completion list.
#[derive(Default)]
pub(crate) struct CompletionDocs {
    shown: Option<Shown>,
    /// The item last asked about.
    asked: Option<(usize, String)>,
    /// Resolving or parsing in flight; dropping it cancels.
    task: Option<Task<()>>,
}

impl Editor {
    /// After the selection of the completion list changed: shows the
    /// documentation of the selected item, resolving it first if needed.
    pub(crate) fn sync_completion_docs(&mut self, cx: &mut Context<Self>) {
        let grammar = self.loaded_grammar();
        let language_id = self.language_id().map(str::to_owned);
        let Some(language) = &mut self.language else {
            return;
        };
        let Some((index, item)) = language
            .completion
            .as_ref()
            .and_then(|menu| menu.selected_item())
        else {
            language.completion_docs = CompletionDocs::default();
            return;
        };
        let key = (index, item.label.clone());
        if language.completion_docs.asked.as_ref() == Some(&key) {
            return;
        }
        let resolve = item.documentation.is_none()
            && item.resolve.is_some()
            && language
                .capabilities
                .completion
                .as_ref()
                .is_some_and(|completion| completion.resolve.resolve);
        let request = resolve.then(|| language.service.resolve_completion(item.clone()));
        let item = item.clone();
        let docs = &mut language.completion_docs;
        docs.asked = Some(key);
        docs.task = Some(cx.spawn(async move |editor, cx| {
            let resolved = match request {
                Some(request) => request.await.ok(),
                None => None,
            };
            let current = resolved.clone().unwrap_or(item);
            let detail = current.detail.clone();
            let documentation = current.documentation.clone();
            let blocks = match documentation {
                Some(documentation) => {
                    cx.background_executor()
                        .spawn(async move {
                            let mut blocks = markdown::parse(&documentation);
                            if let (Some(grammar), Some(id)) = (grammar, language_id) {
                                markdown::highlight_code(&mut blocks, &grammar, &id);
                            }
                            blocks
                        })
                        .await
                }
                None => Vec::new(),
            };
            let _ = editor.update(cx, |editor, cx| {
                let Some(language) = &mut editor.language else {
                    return;
                };
                let Some(menu) = &mut language.completion else {
                    return;
                };
                if !menu.has_item(index, &current.label) {
                    return;
                }
                if let Some(resolved) = resolved {
                    menu.replace_item(index, resolved);
                }
                language.completion_docs.shown = Some(Shown {
                    index,
                    label: current.label,
                    detail,
                    blocks,
                });
                cx.notify();
            });
        }));
    }

    /// The documentation panel for the selected item, if it has any.
    pub(crate) fn render_completion_docs(&self, theme: &Theme) -> Option<AnyElement> {
        let language = self.language.as_ref()?;
        let (index, item) = language.completion.as_ref()?.selected_item()?;
        let shown = language.completion_docs.shown.as_ref()?;
        if shown.index != index || shown.label != item.label {
            return None;
        }
        if shown.detail.is_none() && shown.blocks.is_empty() {
            return None;
        }
        let metrics = &theme.metrics;
        let sizes = &metrics.size;
        let colors = self.syntax_colors(theme);
        let detail = shown.detail.as_ref().map(|detail| {
            div()
                .px(px(metrics.space.md))
                .py(px(metrics.space.xs))
                .font_family(SharedString::from(theme.fonts.mono.clone()))
                .text_color(theme.colors.text_primary)
                .child(detail.clone())
        });
        let rule = (shown.detail.is_some() && !shown.blocks.is_empty()).then(|| {
            div()
                .my(px(metrics.space.xs))
                .h(px(1.0))
                .bg(theme.colors.border_divider)
        });
        let content = popup::scrolling("completion-docs-content", sizes.hover_max_height)
            .children(detail)
            .children(rule)
            .children(popup::blocks(&shown.blocks, &colors, theme));
        Some(
            popup::container(theme)
                .max_w(px(sizes.hover_max_width))
                .py(px(metrics.space.xs))
                .child(content)
                .into_any_element(),
        )
    }
}
