//! The Search view of the sidebar, as in `search.svg`: the query with its
//! options, the (later) replace row, the files to include, a summary and
//! the results tree. The application runs the searches it asks for.

use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, EventEmitter, FocusHandle,
    InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, Render, ScrollStrategy,
    SharedString, StatefulInteractiveElement as _, Styled as _, Subscription, Task,
    UniformListScrollHandle, WeakEntity, Window, div, px, uniform_list,
};
use gpui_component::input::{InputEvent, InputState};
use i18n::{FluentArgs, I18n};
use search::{FileMatches, SearchError, SearchQuery, SearchSummary};
use theme::ActiveTheme as _;
use ui::{Icon, IconButton, IconName, ListRow, TextField, ToggleButton};

use crate::contrib::explorer::file_icons::{self, FileIcons};
use crate::contrib::search::model::{ResultRow, Results};

/// Asks the application to run `query`; results come back through
/// [`SearchView::push_results`] and [`SearchView::finish`] with the same
/// `generation`. An empty query only cancels the running search.
#[derive(Debug, Clone)]
pub struct SearchRequest {
    /// Identifies this search; results of older ones are dropped.
    pub generation: u64,
    /// What to search for.
    pub query: SearchQuery,
}

/// Emitted when a match is opened from the results.
#[derive(Debug, Clone)]
pub struct OpenMatch {
    /// The file.
    pub path: PathBuf,
    /// Zero-based line.
    pub line: usize,
    /// Byte range of the match in the line.
    pub range: Range<usize>,
}

/// Asks the application to replace every match of `query` in `files` with
/// `replacement` (Replace All in the Search view). Open editors get the
/// edit as one undoable change; other files are written.
#[derive(Debug, Clone)]
pub struct ReplaceRequest {
    /// What was searched.
    pub query: SearchQuery,
    /// What replaces each match (`$1` for a regular expression's group).
    pub replacement: String,
    /// The files with matches.
    pub files: Vec<PathBuf>,
}

/// Where the current search is.
#[derive(Debug, Clone)]
enum Status {
    /// Nothing to search for.
    Idle,
    Searching,
    Done(SearchSummary),
    Failed(SearchError),
}

/// The Search view.
// The three search options and the replace row are independent toggles.
#[allow(clippy::struct_excessive_bools)]
pub struct SearchView {
    i18n: Arc<I18n>,
    icons: Arc<FileIcons>,
    query: Entity<InputState>,
    include: Entity<InputState>,
    replace: Entity<InputState>,
    /// The replace row is expanded.
    replace_open: bool,
    case_sensitive: bool,
    whole_word: bool,
    regex: bool,
    debounce: Duration,
    pending: Option<Task<()>>,
    generation: u64,
    status: Status,
    results: Results,
    selected: Option<usize>,
    list_focus: FocusHandle,
    scroll: UniformListScrollHandle,
    /// Name of the open folder; `None` without one.
    workspace: Option<SharedString>,
    _inputs: [Subscription; 3],
}

impl EventEmitter<SearchRequest> for SearchView {}
impl EventEmitter<OpenMatch> for SearchView {}
impl EventEmitter<ReplaceRequest> for SearchView {}

impl SearchView {
    /// An empty view; typing waits `debounce` before searching.
    pub fn new(
        i18n: Arc<I18n>,
        icons: Arc<FileIcons>,
        debounce: Duration,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let query_placeholder = i18n.text("search-placeholder");
        let include_placeholder = i18n.text("search-include-placeholder");
        let query = cx.new(|cx| InputState::new(window, cx).placeholder(query_placeholder));
        let include = cx.new(|cx| InputState::new(window, cx).placeholder(include_placeholder));
        let replace_placeholder = i18n.text("search-replace-placeholder");
        let replace = cx.new(|cx| InputState::new(window, cx).placeholder(replace_placeholder));
        let inputs = [
            cx.subscribe(&query, Self::on_input),
            cx.subscribe(&include, Self::on_input),
            cx.subscribe(&replace, Self::on_replace_input),
        ];
        Self {
            i18n,
            icons,
            query,
            include,
            replace,
            replace_open: false,
            case_sensitive: false,
            whole_word: false,
            regex: false,
            debounce,
            pending: None,
            generation: 0,
            status: Status::Idle,
            results: Results::default(),
            selected: None,
            list_focus: cx.focus_handle(),
            scroll: UniformListScrollHandle::new(),
            workspace: None,
            _inputs: inputs,
        }
    }

    /// The open folder changed: old results go, the query runs again.
    pub fn set_workspace(&mut self, name: Option<SharedString>, cx: &mut Context<Self>) {
        self.workspace = name;
        self.request(cx);
    }

    /// Switches the interface language.
    pub fn set_i18n(&mut self, i18n: Arc<I18n>, cx: &mut Context<Self>) {
        self.i18n = i18n;
        cx.notify();
    }

    /// Changes the pause after typing (`[search] debounce_ms`).
    pub fn set_debounce(&mut self, debounce: Duration) {
        self.debounce = debounce;
    }

    /// Moves the keyboard focus to the query.
    pub fn focus_query(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.query.update(cx, |input, cx| input.focus(window, cx));
    }

    /// More files of search `generation`.
    pub fn push_results(
        &mut self,
        generation: u64,
        files: Vec<FileMatches>,
        cx: &mut Context<Self>,
    ) {
        if generation != self.generation {
            return;
        }
        for file in files {
            self.results.push(file);
        }
        cx.notify();
    }

    /// Search `generation` ended.
    pub fn finish(
        &mut self,
        generation: u64,
        outcome: Result<SearchSummary, SearchError>,
        cx: &mut Context<Self>,
    ) {
        if generation != self.generation {
            return;
        }
        self.status = match outcome {
            Ok(summary) => Status::Done(summary),
            Err(error) => Status::Failed(error),
        };
        cx.notify();
    }

    fn on_input(&mut self, _: Entity<InputState>, event: &InputEvent, cx: &mut Context<Self>) {
        match event {
            InputEvent::Change => self.schedule(cx),
            InputEvent::PressEnter { .. } => self.request(cx),
            _ => {}
        }
    }

    fn on_replace_input(
        &mut self,
        _: Entity<InputState>,
        event: &InputEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            // The previews show the new text.
            InputEvent::Change => cx.notify(),
            InputEvent::PressEnter { .. } => self.replace_all(cx),
            _ => {}
        }
    }

    /// Replaces every match shown (Replace All).
    fn replace_all(&mut self, cx: &mut Context<Self>) {
        if self.results.is_empty() || !matches!(self.status, Status::Done(_)) {
            return;
        }
        let files = (0..self.results.file_count())
            .map(|file| self.results.file(file).matches.path.clone())
            .collect();
        cx.emit(ReplaceRequest {
            query: self.query(cx),
            replacement: self.replace.read(cx).value().to_string(),
            files,
        });
    }

    /// The replacement shown in the previews, while the replace row is
    /// open.
    fn preview_replacement(&self, cx: &App) -> Option<String> {
        self.replace_open
            .then(|| self.replace.read(cx).value().to_string())
    }

    /// The replace was done: search again so the results show what is left.
    pub fn replaced(&mut self, cx: &mut Context<Self>) {
        self.request(cx);
    }

    /// Searches once typing pauses.
    fn schedule(&mut self, cx: &mut Context<Self>) {
        let debounce = self.debounce;
        self.pending = Some(cx.spawn(async move |view, cx| {
            cx.background_executor().timer(debounce).await;
            let _ = view.update(cx, SearchView::request);
        }));
    }

    /// Starts a new search with the current fields and options.
    fn request(&mut self, cx: &mut Context<Self>) {
        self.pending = None;
        self.generation += 1;
        self.results.clear();
        self.selected = None;
        let query = self.query(cx);
        self.status = if query.is_empty() || self.workspace.is_none() {
            Status::Idle
        } else {
            Status::Searching
        };
        cx.emit(SearchRequest {
            generation: self.generation,
            query: if self.workspace.is_some() {
                query
            } else {
                SearchQuery::default()
            },
        });
        cx.notify();
    }

    fn query(&self, cx: &App) -> SearchQuery {
        let include = self.include.read(cx).value();
        SearchQuery {
            pattern: self.query.read(cx).value().to_string(),
            case_sensitive: self.case_sensitive,
            whole_word: self.whole_word,
            regex: self.regex,
            include: include
                .split(',')
                .map(str::trim)
                .filter(|glob| !glob.is_empty())
                .map(str::to_owned)
                .collect(),
        }
    }

    fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.results.rows().len() {
            self.selected = Some(index);
            self.scroll.scroll_to_item(index, ScrollStrategy::Nearest);
            cx.notify();
        }
    }

    /// Opens the match of a line row, toggles a file row.
    fn activate(&mut self, index: usize, cx: &mut Context<Self>) {
        self.selected = Some(index);
        match self.results.rows().get(index).copied() {
            Some(ResultRow::File(file)) => {
                let expanded = self.results.file(file).expanded;
                self.selected = self.results.set_expanded(file, !expanded);
            }
            Some(ResultRow::Line(file, line)) => {
                let (path, found) = self.results.line(file, line);
                let event = OpenMatch {
                    path: path.to_path_buf(),
                    line: found.line,
                    range: found.ranges.first().cloned().unwrap_or(0..0),
                };
                cx.emit(event);
            }
            None => {}
        }
        cx.notify();
    }

    /// Left collapses a file (or goes from a line to its file), Right
    /// expands it.
    fn expand_selected(&mut self, expand: bool, cx: &mut Context<Self>) {
        let Some(row) = self
            .selected
            .and_then(|index| self.results.rows().get(index).copied())
        else {
            return;
        };
        match (row, expand) {
            (ResultRow::File(file), _) => {
                self.selected = self.results.set_expanded(file, expand);
            }
            (ResultRow::Line(file, _), false) => {
                self.selected = self
                    .results
                    .rows()
                    .iter()
                    .position(|row| *row == ResultRow::File(file));
            }
            (ResultRow::Line(..), true) => return,
        }
        if let Some(index) = self.selected {
            self.scroll.scroll_to_item(index, ScrollStrategy::Nearest);
        }
        cx.notify();
    }

    fn on_list_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let modifiers = event.keystroke.modifiers;
        if modifiers.control || modifiers.alt || modifiers.platform || modifiers.shift {
            return;
        }
        let count = self.results.rows().len();
        if count == 0 {
            return;
        }
        let current = self.selected;
        match event.keystroke.key.as_str() {
            "down" => self.select(current.map_or(0, |index| (index + 1).min(count - 1)), cx),
            "up" if current == Some(0) => self.focus_query(window, cx),
            "up" => self.select(current.map_or(0, |index| index.saturating_sub(1)), cx),
            "home" => self.select(0, cx),
            "end" => self.select(count - 1, cx),
            "left" => self.expand_selected(false, cx),
            "right" => self.expand_selected(true, cx),
            "enter" | "space" => match current {
                Some(index) => self.activate(index, cx),
                None => return,
            },
            _ => return,
        }
        cx.stop_propagation();
    }

    /// Down in the query field goes to the results.
    fn on_field_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let modifiers = event.keystroke.modifiers;
        if event.keystroke.key != "down"
            || modifiers.control
            || modifiers.alt
            || modifiers.platform
            || self.results.rows().is_empty()
        {
            return;
        }
        self.list_focus.focus(window, cx);
        if self.selected.is_none() {
            self.select(0, cx);
        }
        cx.stop_propagation();
    }

    fn text(&self, id: &str) -> SharedString {
        self.i18n.text(id).into()
    }
}

/// The option toggles inside the query field.
fn toggles(view: &SearchView, cx: &Context<SearchView>) -> [ToggleButton; 3] {
    let entity = cx.entity().downgrade();
    let toggle = |id: &'static str,
                  label: &'static str,
                  title: &str,
                  active: bool,
                  flip: fn(&mut SearchView)| {
        let entity = entity.clone();
        ToggleButton::new(id, label)
            .active(active)
            .tab_stop(false)
            .tooltip(view.text(title))
            .on_activate(move |_, cx| {
                let _ = entity.update(cx, |view, cx| {
                    flip(view);
                    view.request(cx);
                });
            })
    };
    [
        toggle(
            "search-case",
            "Aa",
            "search-match-case",
            view.case_sensitive,
            |view| {
                view.case_sensitive = !view.case_sensitive;
            },
        ),
        toggle(
            "search-word",
            "ab",
            "search-whole-word",
            view.whole_word,
            |view| {
                view.whole_word = !view.whole_word;
            },
        ),
        toggle("search-regex", ".*", "search-regex", view.regex, |view| {
            view.regex = !view.regex;
        }),
    ]
}

impl SearchView {
    /// The replace row: collapsed, a button that opens it; open, the
    /// replacement field and Replace All.
    fn replace_row(&self, cx: &Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let metrics = &theme.metrics;
        let entity = cx.entity().downgrade();
        let toggle = {
            let entity = entity.clone();
            move |_: &mut Window, cx: &mut App| {
                let _ = entity.update(cx, |view, cx| {
                    view.replace_open = !view.replace_open;
                    cx.notify();
                });
            }
        };
        if !self.replace_open {
            return div()
                .id("search-replace")
                .flex()
                .items_center()
                .h(px(metrics.size.input_inset_height))
                .px(px(metrics.space.md))
                .rounded(px(metrics.radius.md))
                .bg(colors.bg_input_inset)
                .cursor_pointer()
                .text_size(px(metrics.font_size.base))
                .text_color(colors.text_secondary)
                .on_click(move |_, window, cx| toggle(window, cx))
                .child(div().flex_1().child(self.text("search-replace")))
                .child(Icon::new(IconName::ChevronRight).color(colors.icon))
                .into_any_element();
        }
        let replace_all = {
            let entity = entity.clone();
            move |_: &mut Window, cx: &mut App| {
                let _ = entity.update(cx, SearchView::replace_all);
            }
        };
        div()
            .flex()
            .items_center()
            .gap(px(metrics.space.xs))
            .child(
                IconButton::new("search-replace-collapse", IconName::ChevronDown)
                    .tooltip(self.text("search-replace-collapse"))
                    .on_activate(toggle),
            )
            .child(div().flex_1().child(TextField::new(&self.replace)))
            .child(
                IconButton::new("search-replace-all", IconName::Check)
                    .tooltip(self.text("search-replace-all"))
                    .disabled(!matches!(self.status, Status::Done(_)) || self.results.is_empty())
                    .on_activate(replace_all),
            )
            .into_any_element()
    }

    /// The line under the fields: counts, progress or the error.
    fn summary(&self, cx: &App) -> Option<AnyElement> {
        let theme = cx.theme();
        let colors = &theme.colors;
        let (text, color) = match &self.status {
            Status::Idle => return None,
            Status::Searching if self.results.is_empty() => {
                (self.text("search-searching"), colors.text_muted)
            }
            Status::Searching | Status::Done(_) if !self.results.is_empty() => {
                let mut args = FluentArgs::new();
                args.set("results", self.results.match_count());
                args.set("files", self.results.file_count());
                let id = match &self.status {
                    Status::Done(summary) if summary.limit_reached => "search-summary-limited",
                    _ => "search-summary",
                };
                (self.i18n.text_with(id, &args).into(), colors.text_secondary)
            }
            Status::Searching | Status::Done(_) => {
                (self.text("search-no-results"), colors.text_muted)
            }
            Status::Failed(error) => {
                let mut args = FluentArgs::new();
                let (id, detail) = match error {
                    SearchError::InvalidPattern(detail) => ("search-invalid-pattern", detail),
                    SearchError::InvalidGlob(detail) => ("search-invalid-glob", detail),
                };
                args.set("error", detail.clone());
                (
                    self.i18n.text_with(id, &args).into(),
                    colors.diagnostic_error,
                )
            }
        };
        Some(
            div()
                .px(px(theme.metrics.space.xxs))
                .text_size(px(theme.metrics.font_size.md))
                .text_color(color)
                .child(text)
                .into_any_element(),
        )
    }

    fn render_row(&self, index: usize, entity: &WeakEntity<Self>, cx: &App) -> AnyElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let metrics = &theme.metrics;
        let selected = self.selected == Some(index);
        let entity = entity.clone();
        let activate = move |_: &mut Window, cx: &mut App| {
            let _ = entity.update(cx, |view, cx| view.activate(index, cx));
        };
        let height = metrics.size.search_row_height;
        match self.results.rows()[index] {
            ResultRow::File(file) => {
                let result = self.results.file(file);
                let path = &result.matches.path;
                let name = path.file_name().map_or_else(
                    || path.display().to_string(),
                    |name| name.to_string_lossy().into_owned(),
                );
                let icon = file_icons::render(&self.icons.for_file(&name), cx);
                ListRow::new(("search-file", index), name)
                    .height(height)
                    .expanded(result.expanded)
                    .leading(icon)
                    .trailing(
                        div()
                            .flex_none()
                            .text_size(px(metrics.font_size.sm))
                            .text_color(colors.text_muted)
                            .child(result.count.to_string()),
                    )
                    .selected(selected)
                    .tab_stop(false)
                    .on_activate(activate)
                    .into_any_element()
            }
            ResultRow::Line(file, line) => {
                let (_, found) = self.results.line(file, line);
                let number = div()
                    .flex_none()
                    .w(px(metrics.size.search_line_number))
                    .font_family(SharedString::from(theme.fonts.mono.clone()))
                    .text_size(px(metrics.font_size.xs))
                    .text_color(colors.text_muted)
                    .child((found.line + 1).to_string());
                let (text, highlights) = match self.preview_replacement(cx) {
                    Some(replacement) => replaced_preview(&found.preview, &replacement),
                    None => (found.preview.text.clone(), found.preview.highlights.clone()),
                };
                ListRow::new(("search-line", index), text)
                    .flat()
                    .mono()
                    .height(height)
                    .highlights(highlights)
                    .leading(number)
                    .selected(selected)
                    .tab_stop(false)
                    .on_activate(activate)
                    .into_any_element()
            }
        }
    }

    fn results_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let entity = cx.entity().downgrade();
        let count = self.results.rows().len();
        let list = uniform_list("search-results", count, move |range, _window, cx| {
            let Some(view) = entity.upgrade() else {
                return Vec::new();
            };
            let view = view.read(cx);
            range
                .map(|index| view.render_row(index, &entity, cx))
                .collect()
        })
        .track_scroll(&self.scroll)
        .size_full();
        let theme = cx.theme();
        div()
            .id("search-results-frame")
            .key_context("search-results")
            .track_focus(&self.list_focus.clone().tab_stop(count > 0))
            .on_key_down(cx.listener(Self::on_list_key))
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
            })
            .child(list)
            .into_any_element()
    }

    /// Without a folder there is nothing to search.
    fn empty_state(&self, cx: &App) -> AnyElement {
        let theme = cx.theme();
        let metrics = &theme.metrics;
        div()
            .flex()
            .flex_col()
            .items_center()
            .pt(px(metrics.size.empty_state_top))
            .px(px(metrics.size.section_header_inset))
            .gap(px(metrics.space.lg))
            .child(
                Icon::new(IconName::Search)
                    .size(metrics.size.empty_state_icon)
                    .color(theme.colors.icon),
            )
            .child(
                div()
                    .text_center()
                    .text_size(px(metrics.font_size.md))
                    .text_color(theme.colors.text_secondary)
                    .child(self.text("search-no-folder")),
            )
            .into_any_element()
    }
}

impl Render for SearchView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(workspace) = self.workspace.clone() else {
            return div().flex().flex_col().flex_1().child(self.empty_state(cx));
        };
        let query_field = toggles(self, cx)
            .into_iter()
            .fold(TextField::new(&self.query).raised(), TextField::trailing);
        let replace = self.replace_row(cx);
        let summary = self.summary(cx);
        let list = self.results_list(cx);
        let theme = cx.theme();
        let colors = &theme.colors;
        let metrics = &theme.metrics;
        let mut footer_args = FluentArgs::new();
        footer_args.set("name", workspace.to_string());
        let footer = self.i18n.text_with("search-in", &footer_args);
        let fields = div()
            .flex()
            .flex_col()
            .px(px(metrics.space.lg))
            .gap(px(metrics.space.md))
            .child(
                div()
                    .on_key_down(cx.listener(Self::on_field_key))
                    .child(query_field),
            )
            .child(replace)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .mt(px(metrics.space.xl))
                    .gap(px(metrics.space.sm))
                    .child(
                        div()
                            .px(px(metrics.space.xxs))
                            .text_size(px(metrics.font_size.sm))
                            .text_color(colors.text_subtle)
                            .child(self.text("search-include")),
                    )
                    .child(TextField::new(&self.include)),
            )
            .children(summary.map(|summary| div().mt(px(metrics.space.lg)).child(summary)));
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .gap(px(metrics.space.lg))
            .child(fields)
            .child(
                div()
                    .mx(px(metrics.space.lg))
                    .flex_none()
                    .border_t_1()
                    .border_color(colors.border_divider),
            )
            .child(list)
            .child(
                div()
                    .flex_none()
                    .px(px(metrics.size.section_header_inset))
                    .pb(px(metrics.space.md))
                    .truncate()
                    .text_size(px(metrics.font_size.sm))
                    .text_color(colors.text_subtle)
                    .child(footer),
            )
    }
}

/// The preview with every match replaced by `replacement`; the new text is
/// highlighted.
pub(crate) fn replaced_preview(
    preview: &search::Preview,
    replacement: &str,
) -> (String, Vec<Range<usize>>) {
    let mut text = String::with_capacity(preview.text.len());
    let mut highlights = Vec::with_capacity(preview.highlights.len());
    let mut at = 0;
    for range in &preview.highlights {
        if range.start < at || range.end > preview.text.len() {
            continue;
        }
        text.push_str(&preview.text[at..range.start]);
        let start = text.len();
        text.push_str(replacement);
        highlights.push(start..text.len());
        at = range.end;
    }
    text.push_str(&preview.text[at..]);
    (text, highlights)
}

#[cfg(test)]
mod replace_tests {
    use super::replaced_preview;

    #[test]
    fn previews_show_the_replacement() {
        let preview = search::Preview {
            text: "let foo = foo;".to_owned(),
            highlights: vec![4..7, 10..13],
        };
        let (text, highlights) = replaced_preview(&preview, "bar_baz");
        assert_eq!(text, "let bar_baz = bar_baz;");
        assert_eq!(highlights, [4..11, 14..21]);
    }
}
