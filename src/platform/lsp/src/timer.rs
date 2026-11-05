//! Delayed callbacks on one thread: request timeouts, debounced
//! diagnostics, restart delays. The thread sleeps until the next deadline,
//! or until something is scheduled, so it never ticks.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::mpsc;
use std::time::{Duration, Instant};

type Callback = Box<dyn FnOnce() + Send>;

struct Entry {
    deadline: Instant,
    sequence: u64,
    callback: Callback,
}

impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        (self.deadline, self.sequence) == (other.deadline, other.sequence)
    }
}

impl Eq for Entry {}

impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Entry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.deadline, self.sequence).cmp(&(other.deadline, other.sequence))
    }
}

/// Runs callbacks after a delay. The thread ends when every clone is
/// dropped; callbacks still waiting are dropped without running.
#[derive(Clone)]
pub(crate) struct Timer {
    sender: mpsc::Sender<(Instant, Callback)>,
}

impl Timer {
    /// Starts the timer thread. If it cannot start, callbacks are dropped
    /// without running (logged).
    pub(crate) fn start(name: &str) -> Self {
        let (sender, receiver) = mpsc::channel::<(Instant, Callback)>();
        if let Err(error) = std::thread::Builder::new()
            .name(format!("lsp-timer-{name}"))
            .spawn(move || run(&receiver))
        {
            tracing::error!(%error, "cannot start the language server timer");
        }
        Self { sender }
    }

    /// Runs `callback` on the timer thread after `delay`. Callbacks must be
    /// quick: they delay the ones after them.
    pub(crate) fn after(&self, delay: Duration, callback: impl FnOnce() + Send + 'static) {
        let _ = self
            .sender
            .send((Instant::now() + delay, Box::new(callback)));
    }
}

fn run(receiver: &mpsc::Receiver<(Instant, Callback)>) {
    let mut waiting = BinaryHeap::new();
    let mut sequence = 0_u64;
    loop {
        let now = Instant::now();
        while waiting
            .peek()
            .is_some_and(|Reverse(entry): &Reverse<Entry>| entry.deadline <= now)
        {
            if let Some(Reverse(entry)) = waiting.pop() {
                (entry.callback)();
            }
        }
        let next = match waiting.peek() {
            Some(Reverse(entry)) => {
                match receiver.recv_timeout(entry.deadline.saturating_duration_since(now)) {
                    Ok(next) => next,
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                }
            }
            None => match receiver.recv() {
                Ok(next) => next,
                Err(mpsc::RecvError) => return,
            },
        };
        sequence += 1;
        waiting.push(Reverse(Entry {
            deadline: next.0,
            sequence,
            callback: next.1,
        }));
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::channel;

    use super::*;

    #[test]
    fn runs_callbacks_in_deadline_order() {
        let timer = Timer::start("test");
        let (sender, receiver) = channel();
        for (delay, label) in [(30, "late"), (0, "now"), (10, "soon")] {
            let sender = sender.clone();
            timer.after(Duration::from_millis(delay), move || {
                sender.send(label).unwrap();
            });
        }
        let order: Vec<_> = (0..3)
            .map(|_| receiver.recv_timeout(Duration::from_secs(5)).unwrap())
            .collect();
        assert_eq!(order, ["now", "soon", "late"]);
    }
}
