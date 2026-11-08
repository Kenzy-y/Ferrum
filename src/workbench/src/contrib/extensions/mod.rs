//! The Extensions view, as in `extensions.svg`: the search field, the
//! Installed and Marketplace tabs, the installed extensions and the
//! workspace summary; selecting an extension opens its page in the editor
//! area (`page.rs`). The application provides the entries; until the
//! plugin system (stage 9) it lists a sample.

mod page;

use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _, Subscription,
    Window, div, px,
};
use gpui_component::input::{InputEvent, InputState};
use i18n::FluentArgs;
use theme::ActiveTheme as _;
use ui::{IconName, ListCard, SectionHeader, SegmentedTab, TextField};

use crate::Workbench;

pub(crate) use self::page::render as render_page;

/// A title with a line of detail: a feature, a contribution, a toolkit item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionItem {
    /// Short title.
    pub title: SharedString,
    /// What it means.
    pub detail: SharedString,
}

/// The Overview tab of an extension page.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExtensionOverview {
    /// Heading.
    pub title: SharedString,
    /// Text under the heading.
    pub text: SharedString,
    /// Checked features.
    pub features: Vec<ExtensionItem>,
    /// Lines of the code sample; lines starting with `//` or `#` are
    /// comments.
    pub sample: Vec<SharedString>,
    /// Title of the card on the right.
    pub side_title: SharedString,
    /// Items of the card on the right.
    pub side: Vec<ExtensionItem>,
}

/// An installed extension, already localized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionEntry {
    /// Stable id (`publisher.name`).
    pub id: SharedString,
    /// Display name.
    pub name: SharedString,
    /// Who publishes it.
    pub publisher: SharedString,
    /// One line for the list.
    pub summary: SharedString,
    /// One sentence for the page.
    pub description: SharedString,
    /// Icon of its tile.
    pub icon: IconName,
    /// Whether it runs.
    pub enabled: bool,
    /// The Overview tab.
    pub overview: ExtensionOverview,
    /// The Feature contributions tab.
    pub contributions: Vec<ExtensionItem>,
}

/// Tabs of the sidebar view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum ListTab {
    #[default]
    Installed,
    Marketplace,
}

/// Tabs of the extension page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum PageTab {
    #[default]
    Overview,
    Contributions,
}

/// State of the view and of the open page.
#[derive(Default)]
pub(crate) struct ExtensionsState {
    pub(crate) entries: Vec<ExtensionEntry>,
    /// The entry whose page is shown in the editor area.
    pub(crate) page: Option<usize>,
    pub(crate) list_tab: ListTab,
    pub(crate) page_tab: PageTab,
    /// The search field and the subscription that redraws on typing.
    query: Option<(Entity<InputState>, Subscription)>,
}

impl ExtensionsState {
    /// Indices of the entries matching the search field.
    fn visible(&self, cx: &App) -> Vec<usize> {
        let query = self
            .query
            .as_ref()
            .map(|(query, _)| query.read(cx).value().trim().to_lowercase())
            .unwrap_or_default();
        filter(&self.entries, &query)
    }

    fn enabled_count(&self) -> usize {
        self.entries.iter().filter(|entry| entry.enabled).count()
    }
}

/// Indices of `entries` whose name, publisher or summary contains
/// `query` (lower case).
fn filter(entries: &[ExtensionEntry], query: &str) -> Vec<usize> {
    entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| {
            query.is_empty()
                || [&entry.name, &entry.publisher, &entry.summary]
                    .iter()
                    .any(|text| text.to_lowercase().contains(query))
        })
        .map(|(index, _)| index)
        .collect()
}

impl Workbench {
    /// Lists `entries` as the installed extensions.
    pub fn set_extensions(&mut self, entries: Vec<ExtensionEntry>, cx: &mut Context<Self>) {
        self.extensions.entries = entries;
        self.extensions.page = self
            .extensions
            .page
            .filter(|index| *index < self.extensions.entries.len());
        cx.notify();
    }

    /// The listed extensions, with what the user enabled.
    pub fn extensions(&self) -> &[ExtensionEntry] {
        &self.extensions.entries
    }

    /// Creates the search field of the view.
    pub(crate) fn ensure_extensions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.extensions.query.is_some() {
            return;
        }
        let placeholder = self.i18n.text("extensions-search-placeholder");
        let query = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        let watch = cx.subscribe(&query, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        self.extensions.query = Some((query, watch));
    }

    /// Shows the page of entry `index` in the editor area.
    pub(crate) fn open_extension(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.extensions.entries.len() {
            self.extensions.page = Some(index);
            self.extensions.page_tab = PageTab::Overview;
            cx.notify();
        }
    }

    pub(crate) fn close_extension_page(&mut self, cx: &mut Context<Self>) {
        self.extensions.page = None;
        cx.notify();
    }

    /// Enables or disables entry `index` (only in the list until the
    /// plugin system can stop extensions).
    pub(crate) fn toggle_extension(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(entry) = self.extensions.entries.get_mut(index) {
            entry.enabled = !entry.enabled;
            cx.notify();
        }
    }

    /// "Enabled" or "Disabled" and its color.
    pub(crate) fn extension_status(
        &self,
        entry: &ExtensionEntry,
        cx: &App,
    ) -> (SharedString, gpui::Hsla) {
        let colors = &cx.theme().colors;
        if entry.enabled {
            (self.text("extensions-enabled"), colors.text_success.into())
        } else {
            (self.text("extensions-disabled"), colors.text_muted.into())
        }
    }
}

/// A handler for `on_activate` that updates the workbench.
pub(crate) fn on_workbench(
    cx: &Context<Workbench>,
    handler: impl Fn(&mut Workbench, &mut Context<Workbench>) + 'static,
) -> impl Fn(&mut Window, &mut App) + 'static {
    let workbench = cx.entity().downgrade();
    move |_, cx| {
        let _ = workbench.update(cx, |workbench, cx| handler(workbench, cx));
    }
}

fn tabs(workbench: &Workbench, cx: &mut Context<Workbench>) -> AnyElement {
    let theme = cx.theme();
    let current = workbench.extensions.list_tab;
    let tab = |id: &'static str, label: &str, kind: ListTab, cx: &Context<Workbench>| {
        SegmentedTab::new(id, workbench.text(label))
            .active(current == kind)
            .on_activate(on_workbench(cx, move |workbench, cx| {
                workbench.extensions.list_tab = kind;
                cx.notify();
            }))
    };
    let installed = tab(
        "extensions-installed",
        "extensions-installed",
        ListTab::Installed,
        cx,
    )
    .count(workbench.extensions.entries.len().to_string());
    let marketplace = tab(
        "extensions-marketplace",
        "extensions-marketplace",
        ListTab::Marketplace,
        cx,
    );
    div()
        .flex()
        .flex_none()
        .px(px(theme.metrics.space.lg))
        .gap(px(theme.metrics.space.xxl))
        .child(installed)
        .child(marketplace)
        .into_any_element()
}

/// Muted text centered in the list area.
fn empty(text: SharedString, cx: &App) -> AnyElement {
    let theme = cx.theme();
    div()
        .px(px(theme.metrics.size.section_header_inset))
        .pt(px(theme.metrics.space.lg))
        .text_size(px(theme.metrics.font_size.md))
        .text_color(theme.colors.text_muted)
        .child(text)
        .into_any_element()
}

fn installed(workbench: &Workbench, cx: &mut Context<Workbench>) -> AnyElement {
    let visible = workbench.extensions.visible(cx);
    if visible.is_empty() {
        return empty(workbench.text("extensions-none-found"), cx);
    }
    let cards: Vec<AnyElement> = visible
        .into_iter()
        .map(|index| {
            let entry = &workbench.extensions.entries[index];
            let (status, color) = workbench.extension_status(entry, cx);
            ListCard::new(("extension", index), entry.icon, entry.name.clone())
                .subtitle(entry.summary.clone())
                .status(status, color)
                .selected(workbench.extensions.page == Some(index))
                .on_activate(on_workbench(cx, move |workbench, cx| {
                    workbench.open_extension(index, cx);
                }))
                .into_any_element()
        })
        .collect();
    let theme = cx.theme();
    div()
        .flex()
        .flex_col()
        .px(px(theme.metrics.space.sm))
        .gap(px(theme.metrics.space.md))
        .children(cards)
        .into_any_element()
}

fn workspace_summary(workbench: &Workbench, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let colors = &theme.colors;
    let metrics = &theme.metrics;
    let mut args = FluentArgs::new();
    args.set("count", workbench.extensions.enabled_count());
    let line = |text: SharedString, size: f32, color: gpui::Hsla| {
        div()
            .px(px(metrics.size.section_header_inset))
            .text_size(px(size))
            .text_color(color)
            .child(text)
    };
    div()
        .flex()
        .flex_col()
        .flex_none()
        .gap(px(metrics.space.lg))
        .pb(px(metrics.space.lg))
        .child(
            div()
                .mx(px(metrics.space.lg))
                .border_t_1()
                .border_color(colors.border_panel),
        )
        .child(SectionHeader::group(
            workbench.text("extensions-workspace"),
            None,
        ))
        .child(line(
            workbench
                .i18n
                .text_with("extensions-enabled-count", &args)
                .into(),
            metrics.font_size.base,
            colors.text_secondary.into(),
        ))
        .child(line(
            workbench.text("extensions-up-to-date"),
            metrics.font_size.sm,
            colors.text_muted.into(),
        ))
        .into_any_element()
}

pub(crate) fn render(
    workbench: &Workbench,
    cx: &mut Context<Workbench>,
) -> impl IntoElement + use<> {
    let tabs = tabs(workbench, cx);
    let list = match workbench.extensions.list_tab {
        ListTab::Installed => installed(workbench, cx),
        ListTab::Marketplace => empty(workbench.text("extensions-marketplace-later"), cx),
    };
    let summary = workspace_summary(workbench, cx);
    let theme = cx.theme();
    let field = workbench.extensions.query.as_ref().map(|(query, _)| {
        div()
            .px(px(theme.metrics.space.lg))
            .child(TextField::new(query).raised().icon(IconName::Search))
    });
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .gap(px(theme.metrics.space.lg))
        .children(field)
        .child(tabs)
        .child(
            div()
                .id("extensions-list")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .child(list),
        )
        .child(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, summary: &str) -> ExtensionEntry {
        ExtensionEntry {
            id: format!("test.{name}").into(),
            name: name.to_owned().into(),
            publisher: "test".into(),
            summary: summary.to_owned().into(),
            description: SharedString::default(),
            icon: IconName::Package,
            enabled: true,
            overview: ExtensionOverview::default(),
            contributions: Vec::new(),
        }
    }

    #[test]
    fn the_filter_matches_name_publisher_and_summary() {
        let entries = [entry("Alpha", "Themes"), entry("Beta", "Debugging")];
        assert_eq!(filter(&entries, ""), vec![0, 1]);
        assert_eq!(filter(&entries, "alp"), vec![0]);
        assert_eq!(filter(&entries, "debug"), vec![1]);
        assert_eq!(filter(&entries, "test"), vec![0, 1]);
        assert!(filter(&entries, "nothing").is_empty());
    }
}
