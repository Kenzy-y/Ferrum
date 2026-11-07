//! Worker threads that read and match files while the walk goes on.
//!
//! Opening and reading files dominates a search (especially on Windows),
//! so several files are read at once. Jobs carry a sequence number; the
//! caller puts results back in walk order.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;

use fs::Fs;
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use futures::executor::block_on;
use grep_regex::RegexMatcher;
use grep_searcher::{BinaryDetection, Searcher, SearcherBuilder};

use crate::run::search_bytes;
use crate::{LineMatch, SearchSettings};

/// Most threads one search uses; more do not read files faster.
const MAX_WORKERS: usize = 8;

/// Jobs waiting for a worker; the walk pauses when this many are queued.
const QUEUE_LENGTH: usize = 64;

/// A file to search.
pub(crate) struct Job {
    pub seq: usize,
    pub path: PathBuf,
    /// Unsaved text to search instead of the file.
    pub text: Option<Arc<str>>,
}

/// A searched file.
pub(crate) struct Done {
    pub seq: usize,
    pub path: PathBuf,
    /// Matching lines; `None` if the file was unreadable or binary.
    pub lines: Option<Vec<LineMatch>>,
}

/// What a worker needs to search a file.
#[derive(Clone)]
struct Context {
    fs: Arc<dyn Fs>,
    matcher: RegexMatcher,
    settings: SearchSettings,
    stop: Arc<AtomicBool>,
}

impl Context {
    fn process(&self, searcher: &mut Searcher, job: Job) -> Done {
        let bytes = match &job.text {
            Some(text) => Some(text.as_bytes().to_vec()),
            None => match block_on(self.fs.read(&job.path)) {
                Ok(bytes) => Some(bytes),
                Err(error) => {
                    tracing::debug!(%error, path = %job.path.display(), "cannot read file");
                    None
                }
            },
        };
        let lines =
            bytes.and_then(|bytes| search_bytes(searcher, &self.matcher, &bytes, &self.settings));
        Done {
            seq: job.seq,
            path: job.path,
            lines,
        }
    }
}

fn searcher() -> Searcher {
    SearcherBuilder::new()
        .line_number(true)
        .binary_detection(BinaryDetection::quit(0))
        .build()
}

/// The worker threads of one search. Dropping it stops them.
pub(crate) struct Pool {
    jobs: Option<SyncSender<Job>>,
    results: UnboundedReceiver<Done>,
    /// Without threads (they could not start) jobs run on the caller.
    inline: Option<(Context, Searcher, UnboundedSender<Done>)>,
    stop: Arc<AtomicBool>,
    handles: Vec<JoinHandle<()>>,
}

impl Pool {
    pub(crate) fn start(fs: Arc<dyn Fs>, matcher: RegexMatcher, settings: SearchSettings) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let context = Context {
            fs,
            matcher,
            settings,
            stop: stop.clone(),
        };
        let (jobs, queue) = sync_channel::<Job>(QUEUE_LENGTH);
        let queue = Arc::new(Mutex::new(queue));
        let (sender, results) = unbounded();
        let count = std::thread::available_parallelism()
            .map_or(1, std::num::NonZero::get)
            .clamp(1, MAX_WORKERS);
        let mut handles = Vec::with_capacity(count);
        for index in 0..count {
            let context = context.clone();
            let queue = queue.clone();
            let sender = sender.clone();
            let spawned = std::thread::Builder::new()
                .name(format!("search-{index}"))
                .spawn(move || work(&context, &queue, &sender));
            match spawned {
                Ok(handle) => handles.push(handle),
                Err(error) => {
                    tracing::warn!(%error, "cannot start a search thread");
                    break;
                }
            }
        }
        let inline = handles.is_empty().then(|| (context, searcher(), sender));
        Self {
            jobs: Some(jobs),
            results,
            inline,
            stop,
            handles,
        }
    }

    /// Queues `job`; waits while the queue is full.
    pub(crate) fn submit(&mut self, job: Job) {
        if let Some((context, searcher, sender)) = &mut self.inline {
            let done = context.process(searcher, job);
            let _ = sender.unbounded_send(done);
            return;
        }
        if let Some(jobs) = &self.jobs
            && jobs.send(job).is_err()
        {
            tracing::debug!("search threads stopped early");
        }
    }

    /// No more jobs come; the results end once the queued ones are done.
    pub(crate) fn close(&mut self) {
        self.jobs = None;
        self.inline = None;
    }

    /// Results, in completion order.
    pub(crate) fn results(&mut self) -> &mut UnboundedReceiver<Done> {
        &mut self.results
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.close();
        for handle in self.handles.drain(..) {
            if handle.join().is_err() {
                tracing::warn!("a search thread panicked");
            }
        }
    }
}

fn work(context: &Context, queue: &Mutex<Receiver<Job>>, results: &UnboundedSender<Done>) {
    let mut searcher = searcher();
    loop {
        let job = queue.lock().unwrap_or_else(PoisonError::into_inner).recv();
        let Ok(job) = job else {
            return;
        };
        if context.stop.load(Ordering::Relaxed) {
            return;
        }
        if results
            .unbounded_send(context.process(&mut searcher, job))
            .is_err()
        {
            return;
        }
    }
}
