//! The Problems panel: every file with problems, its problems below it,
//! in a virtualized list. A click or Enter on a problem opens its place,
//! on a file the file.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::{
    AnyElement, App, Context, EventEmitter, FocusHandle, Hsla, InteractiveElement as _,
    IntoElement, KeyDownEvent, ParentElement as _, Render, ScrollStrategy, SharedString,
    Styled as _, UniformListScrollHandle, Window, div, px, uniform_list,
};
use language::{Diagnostic, Severity};
use text::Point;
use theme::{ActiveTheme as _, Theme};
use ui::{Icon, IconName, ListRow};

use crate::contrib::explorer::file_icons::{self, FileIcons};
use crate::contrib::problems::model::{ProblemRow, Problems};

/// Emitted when a problem (or a file) is opened from the panel.
pub struct OpenProblem {
    /// The file.
    pub path: PathBuf,
    /// Where the cursor goes.
    pub position: Point,
}

/// Texts of the panel, already localized.
#[derive(Clone)]
pub struct ProblemsTexts {
    /// Shown when there are no problems.
    pub empty: SharedString,
    /// Formats the position of a problem from its line and column, both
    /// from 1.
    pub position: Arc<dyn Fn(usize, usize) -> SharedString>,
}

/// The Problems panel.
pub struct ProblemsView {
    problems: Problems,
    /// Folder that paths are shown relative to.
    root: Option<PathBuf>,
    selected: Option<usize>,
    icons: Arc<FileIcons>,
    texts: ProblemsTexts,
    focus_handle: FocusHandle,
    scroll: UniformListScrollHandle,
}

impl EventEmitter<OpenProblem> for ProblemsView {}

impl ProblemsView {
    /// An empty panel.
    pub fn new(icons: Arc<FileIcons>, texts: ProblemsTexts, cx: &mut Context<Self>) -> Self {
        Self {
            problems: Problems::default(),
            root: None,
            selected: None,
            icons,
            texts,
            focus_handle: cx.focus_handle(),
            scroll: UniformListScrollHandle::new(),
        }
    }

    /// `source` (a language service) now reports `diagnostics` for `path`.
    pub fn set_diagnostics(
        &mut self,
        source: &str,
        path: &Path,
        diagnostics: &[Diagnostic],
        cx: &mut Context<Self>,
    ) {
        self.problems.set(source, path, diagnostics);
        self.clamp_selection();
        cx.notify();
    }

    /// Forgets the problems `source` reported (its server stopped).
    pub fn clear_source(&mut self, source: &str, cx: &mut Context<Self>) {
        self.problems.clear_source(source);
        self.clamp_selection();
        cx.notify();
    }

    /// A new folder is open: paths show relative to `root`, and the
    /// problems of the previous folder go.
    pub fn set_root(&mut self, root: Option<PathBuf>, cx: &mut Context<Self>) {
        self.root = root;
        self.problems.clear();
        self.selected = None;
        cx.notify();
    }

    /// Errors and warnings of the project.
    pub fn counts(&self) -> (usize, usize) {
        self.problems.counts()
    }

    /// Problems of every listed severity.
    pub fn total(&self) -> usize {
        self.problems.total()
    }

    fn clamp_selection(&mut self) {
        let count = self.problems.rows().len();
        self.selected = self
            .selected
            .filter(|_| count > 0)
            .map(|index| index.min(count - 1));
    }

    fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.problems.rows().len() {
            self.selected = Some(index);
            self.scroll.scroll_to_item(index, ScrollStrategy::Nearest);
            cx.notify();
        }
    }

    fn open(&mut self, index: usize, cx: &mut Context<Self>) {
        self.selected = Some(index);
        let event = match self.problems.rows().get(index) {
            Some(ProblemRow::File { path, .. }) => OpenProblem {
                path: path.to_path_buf(),
                position: Point::new(0, 0),
            },
            Some(ProblemRow::Problem { path, diagnostic }) => OpenProblem {
                path: path.to_path_buf(),
                position: diagnostic.range.start,
            },
            None => return,
        };
        cx.emit(event);
        cx.notify();
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let modifiers = event.keystroke.modifiers;
        if modifiers.control || modifiers.alt || modifiers.platform || modifiers.shift {
            return;
        }
        let count = self.problems.rows().len();
        if count == 0 {
            return;
        }
        let current = self.selected;
        match event.keystroke.key.as_str() {
            "down" => self.select(current.map_or(0, |index| (index + 1).min(count - 1)), cx),
            "up" => self.select(current.map_or(0, |index| index.saturating_sub(1)), cx),
            "home" => self.select(0, cx),
            "end" => self.select(count - 1, cx),
            "enter" | "space" => match current {
                Some(index) => self.open(index, cx),
                None => return,
            },
            _ => return,
        }
        cx.stop_propagation();
    }

    /// The folder of `path` relative to the root, for the file rows.
    fn folder_of(&self, path: &Path) -> String {
        let parent = path.parent().unwrap_or(path);
        let relative = self
            .root
            .as_ref()
            .and_then(|root| parent.strip_prefix(root).ok())
            .unwrap_or(parent);
        relative.display().to_string()
    }
}

/// Icon and color of a severity.
fn severity_icon(severity: Severity, theme: &Theme) -> (IconName, Hsla) {
    let colors = &theme.colors;
    match severity {
        Severity::Error => (IconName::CircleX, colors.diagnostic_error.into()),
        Severity::Warning => (IconName::TriangleAlert, colors.diagnostic_warning.into()),
        Severity::Information | Severity::Hint => (IconName::Info, colors.diagnostic_info.into()),
    }
}

/// `source(code)` of a diagnostic, as other editors show it.
fn origin(diagnostic: &Diagnostic) -> Option<String> {
    match (&diagnostic.source, &diagnostic.code) {
        (Some(source), Some(code)) => Some(format!("{source}({code})")),
        (Some(source), None) => Some(source.clone()),
        (None, Some(code)) => Some(code.clone()),
        (None, None) => None,
    }
}

/// Muted text at the end of a row.
fn muted(text: impl Into<SharedString>, cx: &App) -> AnyElement {
    let theme = cx.theme();
    div()
        .flex_none()
        .text_size(px(theme.metrics.font_size.sm))
        .text_color(theme.colors.text_muted)
        .child(text.into())
        .into_any_element()
}

impl Render for ProblemsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let container = div()
            .id("problems")
            .key_context("problems")
            .track_focus(&self.focus_handle.clone().tab_stop(true))
            .on_key_down(cx.listener(Self::on_key_down))
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .px(px(theme.metrics.space.sm))
            .border_1()
            .border_color(gpui::transparent_black())
            .rounded(px(theme.metrics.radius.md))
            .focus_visible({
                let ring = theme.colors.border_focus;
                move |style| style.border_color(ring)
            });
        if self.problems.rows().is_empty() {
            return container.child(
                div()
                    .px(px(theme.metrics.space.md))
                    .py(px(theme.metrics.space.sm))
                    .text_size(px(theme.metrics.font_size.base))
                    .text_color(theme.colors.text_muted)
                    .child(self.texts.empty.clone()),
            );
        }
        let entity = cx.entity().downgrade();
        let count = self.problems.rows().len();
        let list = uniform_list("problems-list", count, {
            let entity = entity.clone();
            move |range, _window, cx| {
                let Some(view) = entity.upgrade() else {
                    return Vec::new();
                };
                let view = view.read(cx);
                range
                    .map(|index| view.render_row(index, &entity, cx))
                    .collect()
            }
        })
        .track_scroll(&self.scroll)
        .size_full();
        container.child(list)
    }
}

impl ProblemsView {
    fn render_row(&self, index: usize, entity: &gpui::WeakEntity<Self>, cx: &App) -> AnyElement {
        let theme = cx.theme();
        let entity = entity.clone();
        let selected = self.selected == Some(index);
        let activate = move |_: &mut Window, cx: &mut App| {
            let _ = entity.update(cx, |view, cx| view.open(index, cx));
        };
        match &self.problems.rows()[index] {
            ProblemRow::File { path, count } => {
                let name = path.file_name().map_or_else(
                    || path.display().to_string(),
                    |name| name.to_string_lossy().into_owned(),
                );
                let icon = file_icons::render(&self.icons.for_file(&name), cx);
                let folder = self.folder_of(path);
                ListRow::new(("problems-file", index), name)
                    .flat()
                    .leading(icon)
                    .trailing(
                        div()
                            .flex()
                            .flex_none()
                            .gap(px(theme.metrics.space.md))
                            .child(muted(folder, cx))
                            .child(muted(count.to_string(), cx)),
                    )
                    .selected(selected)
                    .tab_stop(false)
                    .on_activate(activate)
                    .into_any_element()
            }
            ProblemRow::Problem { diagnostic, .. } => {
                let (icon, color) = severity_icon(diagnostic.severity, theme);
                let message = diagnostic
                    .message
                    .lines()
                    .next()
                    .unwrap_or_default()
                    .to_owned();
                let start = diagnostic.range.start;
                let position = (self.texts.position)(start.row + 1, start.column + 1);
                let trailing = div()
                    .flex()
                    .flex_none()
                    .gap(px(theme.metrics.space.md))
                    .children(origin(diagnostic).map(|origin| muted(origin, cx)))
                    .child(muted(position, cx));
                ListRow::new(("problems-row", index), message)
                    .flat()
                    .depth(1)
                    .leading(Icon::new(icon).color(color))
                    .trailing(trailing)
                    .selected(selected)
                    .tab_stop(false)
                    .on_activate(activate)
                    .into_any_element()
            }
        }
    }
}
