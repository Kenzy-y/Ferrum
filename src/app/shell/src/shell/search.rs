//! Project-wide search: the Search view asks, the shell runs the search
//! in the background (one at a time, the previous one is cancelled) and
//! passes the results back in batches; opening a match selects it.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use futures::StreamExt as _;
use gpui::{AppContext as _, Context, Entity, Subscription, Task, Window};
use i18n::I18n;
use search::{SearchOptions, SearchSettings};
use settings::SettingsStore;
use text::Point;
use workbench::{FileIcons, OpenMatch, SearchRequest, SearchView, Workbench};

use super::Shell;

/// Most files passed to the view in one update.
const BATCH_FILES: usize = 64;

/// A running search.
pub(super) struct RunningSearch {
    cancel: Arc<AtomicBool>,
    _task: Task<()>,
}

impl Drop for RunningSearch {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// The view, shown by `workbench`, and the subscriptions that run its
/// searches and open its matches.
pub(super) fn create(
    i18n: &Arc<I18n>,
    icons: Arc<FileIcons>,
    settings: &SettingsStore,
    workbench: &Entity<Workbench>,
    window: &mut Window,
    cx: &mut Context<Shell>,
) -> (Entity<SearchView>, [Subscription; 2]) {
    let debounce = debounce(settings);
    let i18n = i18n.clone();
    let view = cx.new(|cx| SearchView::new(i18n, icons, debounce, window, cx));
    workbench.update(cx, |workbench, cx| workbench.set_search(view.clone(), cx));
    let requests = cx.subscribe(&view, |shell, _, request: &SearchRequest, cx| {
        shell.run_search(request, cx);
    });
    let open = cx.subscribe_in(&view, window, |shell, _, event: &OpenMatch, window, cx| {
        let start = Point::new(event.line, event.range.start);
        let end = Point::new(event.line, event.range.end);
        shell.open_file(event.path.clone(), Some(start..end), window, cx);
    });
    (view, [requests, open])
}

/// `[search] debounce_ms`.
pub(super) fn debounce(settings: &SettingsStore) -> Duration {
    Duration::from_millis(settings.get::<SearchSettings>(None).debounce_ms)
}

impl Shell {
    fn run_search(&mut self, request: &SearchRequest, cx: &mut Context<Self>) {
        // Dropping the previous search cancels it.
        self.running_search = None;
        let Some(workspace) = &self.workspace else {
            return;
        };
        if request.query.is_empty() {
            return;
        }
        let root = workspace.root().to_path_buf();
        let query = request.query.clone();
        let generation = request.generation;
        // Unsaved editors are searched as shown. Ropes clone in constant
        // time; the text is built in the background.
        let unsaved: Vec<(PathBuf, _)> = self
            .workbench
            .read(cx)
            .editors()
            .filter_map(|editor| {
                let editor = editor.read(cx);
                let path = editor.path()?.to_path_buf();
                editor
                    .is_dirty()
                    .then(|| (path, editor.document().buffer().rope().clone()))
            })
            .collect();
        let mut options = SearchOptions {
            settings: self.settings.get::<SearchSettings>(None),
            hidden: self.settings.get::<project::FilesSettings>(None).hidden,
            overrides: std::collections::HashMap::new(),
        };
        let fs = self.fs.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let (sender, receiver) = futures::channel::mpsc::unbounded();
        let search_cancel = cancel.clone();
        let search = cx.background_executor().spawn(async move {
            options.overrides = unsaved
                .into_iter()
                .map(|(path, rope)| (path, Arc::<str>::from(rope.to_string())))
                .collect();
            search::run(fs, &root, &query, &options, &search_cancel, |file| {
                let _ = sender.unbounded_send(file);
            })
            .await
        });
        let view = self.search.downgrade();
        let task = cx.spawn(async move |_, cx| {
            let mut batches = receiver.ready_chunks(BATCH_FILES);
            while let Some(batch) = batches.next().await {
                let shown = view.update(cx, |view, cx| view.push_results(generation, batch, cx));
                if shown.is_err() {
                    return;
                }
            }
            let outcome = search.await;
            if let Ok(summary) = &outcome {
                tracing::debug!(
                    files = summary.files_searched,
                    matches = summary.matches,
                    "search finished"
                );
            }
            let _ = view.update(cx, |view, cx| view.finish(generation, outcome, cx));
        });
        self.running_search = Some(RunningSearch {
            cancel,
            _task: task,
        });
    }
}
