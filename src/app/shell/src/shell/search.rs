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
use workbench::{FileIcons, OpenMatch, ReplaceRequest, SearchRequest, SearchView, Workbench};

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
) -> (Entity<SearchView>, [Subscription; 3]) {
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
    let replace = cx.subscribe(&view, |shell, _, request: &ReplaceRequest, cx| {
        shell.replace_in_project(request, cx);
    });
    (view, [requests, open, replace])
}

/// `[search] debounce_ms`.
pub(super) fn debounce(settings: &SettingsStore) -> Duration {
    Duration::from_millis(settings.get::<SearchSettings>(None).debounce_ms)
}

/// Most replacements in one file.
const MAX_FILE_REPLACEMENTS: usize = 1_000_000;

impl Shell {
    /// Replace All from the Search view: open editors get one undoable
    /// edit each (left unsaved, as typed edits are); other files are read,
    /// replaced and written in the background. Then the view searches
    /// again.
    fn replace_in_project(&mut self, request: &ReplaceRequest, cx: &mut Context<Self>) {
        let editors: Vec<_> = request
            .files
            .iter()
            .filter_map(|path| {
                let editor = self.workbench.read(cx).editor_for(path, cx)?;
                let snapshot = editor.read(cx);
                let rope = snapshot.document().buffer().rope().clone();
                let version = snapshot.document().version();
                Some((path.clone(), editor, rope, version))
            })
            .collect();
        let closed: Vec<PathBuf> = request
            .files
            .iter()
            .filter(|path| !editors.iter().any(|(open, ..)| open == *path))
            .cloned()
            .collect();
        let open_texts: Vec<_> = editors.iter().map(|(_, _, rope, _)| rope.clone()).collect();
        let query = request.query.clone();
        let replacement = request.replacement.clone();
        let fs = self.fs.clone();
        let work = cx.background_executor().spawn(async move {
            let open_edits: Vec<_> = open_texts
                .into_iter()
                .map(|rope| {
                    query
                        .replacements(&rope.to_string(), &replacement, MAX_FILE_REPLACEMENTS)
                        .unwrap_or_default()
                })
                .collect();
            let mut written = (0, 0);
            let mut failed = 0;
            for path in closed {
                match replace_in_file(fs.as_ref(), &path, &query, &replacement).await {
                    Ok(0) => {}
                    Ok(count) => {
                        written.0 += count;
                        written.1 += 1;
                    }
                    Err(error) => {
                        failed += 1;
                        tracing::error!(path = %path.display(), "cannot replace: {error:#}");
                    }
                }
            }
            (open_edits, written, failed)
        });
        let view = self.search.downgrade();
        let workbench = self.workbench.downgrade();
        let i18n = self.i18n.clone();
        cx.spawn(async move |_, cx| {
            let (open_edits, (written, written_files), failed) = work.await;
            cx.update(|cx| {
                let open_count: usize = open_edits.iter().map(Vec::len).sum();
                let open_files = open_edits.iter().filter(|edits| !edits.is_empty()).count();
                let mut args = i18n::FluentArgs::new();
                args.set("count", written + open_count);
                args.set("files", written_files + open_files);
                let mut notices = vec![workbench::Notification {
                    kind: workbench::NotificationKind::Info,
                    message: i18n.text_with("notification-replaced", &args).into(),
                }];
                if failed > 0 {
                    let mut args = i18n::FluentArgs::new();
                    args.set("files", failed);
                    notices.push(workbench::Notification {
                        kind: workbench::NotificationKind::Error,
                        message: i18n.text_with("notification-replace-failed", &args).into(),
                    });
                }
                let _ = workbench.update(cx, |workbench, cx| {
                    for notice in notices {
                        workbench.notify(notice, cx);
                    }
                });
                for ((_, editor, _, version), edits) in editors.into_iter().zip(open_edits) {
                    editor.update(cx, |editor, cx| {
                        // Edited meanwhile: its matches moved; skip it.
                        if editor.document().version() == version && !edits.is_empty() {
                            editor.edit_document(cx, |document| {
                                document.replace_ranges(&edits);
                            });
                        }
                    });
                }
                let _ = view.update(cx, SearchView::replaced);
            });
        })
        .detach();
    }

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

/// Replaces every match in a file on disk.
async fn replace_in_file(
    fs: &dyn fs::Fs,
    path: &std::path::Path,
    query: &search::SearchQuery,
    replacement: &str,
) -> anyhow::Result<usize> {
    let text = fs.read_to_string(path).await?;
    let edits = query.replacements(&text, replacement, MAX_FILE_REPLACEMENTS)?;
    if edits.is_empty() {
        return Ok(0);
    }
    let mut result = String::with_capacity(text.len());
    let mut at = 0;
    for (range, new_text) in &edits {
        result.push_str(&text[at..range.start]);
        result.push_str(new_text);
        at = range.end;
    }
    result.push_str(&text[at..]);
    fs.write(path, result.as_bytes()).await?;
    Ok(edits.len())
}

#[cfg(test)]
mod tests {
    use fs::{FakeFs, Fs as _};
    use futures::executor::block_on;

    use super::*;

    #[test]
    fn replaces_every_match_in_a_file() {
        let fs = FakeFs::new();
        let path = std::path::Path::new("/project/a.rs");
        fs.insert_file("/project/a.rs", "let a = 1;\nlet b = a;\n");
        let query = search::SearchQuery {
            pattern: "a".to_owned(),
            whole_word: true,
            case_sensitive: true,
            ..search::SearchQuery::default()
        };
        assert_eq!(
            block_on(replace_in_file(&fs, path, &query, "alpha")).unwrap(),
            2
        );
        assert_eq!(
            block_on(fs.read_to_string(path)).unwrap(),
            "let alpha = 1;\nlet b = alpha;\n"
        );
    }
}
