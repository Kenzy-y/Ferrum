//! Requests and notifications over a transport: ids, waiting answers,
//! timeouts and cancellation.

use std::collections::HashMap;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex, PoisonError, Weak, mpsc};
use std::task::{Context, Poll};
use std::time::Duration;

use futures::channel::oneshot;
use language::ServiceError;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::timer::Timer;
use crate::transport::{
    CONTENT_MODIFIED, REQUEST_CANCELLED, ResponseError, SERVER_CANCELLED, WriterJob, notification,
    request, response,
};

type Answer = Result<Value, ServiceError>;

struct Inner {
    next_id: AtomicI64,
    pending: Mutex<HashMap<i64, oneshot::Sender<Answer>>>,
    writer: mpsc::Sender<WriterJob>,
    timer: Timer,
    timeout: Duration,
}

impl Inner {
    fn take_pending(&self, id: i64) -> Option<oneshot::Sender<Answer>> {
        self.pending
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&id)
    }

    fn cancel(&self, id: i64) {
        let body = notification("$/cancelRequest", &serde_json::json!({ "id": id }));
        let _ = self.writer.send(WriterJob::Body(body));
    }
}

/// The client side of one server connection. Cheap to clone.
#[derive(Clone)]
pub(crate) struct Client {
    inner: Arc<Inner>,
}

impl Client {
    /// A client writing to `writer`; requests time out after `timeout`.
    pub(crate) fn new(writer: mpsc::Sender<WriterJob>, timer: Timer, timeout: Duration) -> Self {
        Self {
            inner: Arc::new(Inner {
                next_id: AtomicI64::new(1),
                pending: Mutex::new(HashMap::new()),
                writer,
                timer,
                timeout,
            }),
        }
    }

    /// Sends request `R`. Dropping the future before it ends cancels the
    /// request (`$/cancelRequest`).
    pub(crate) fn request<R: lsp_types::request::Request>(
        &self,
        params: &R::Params,
    ) -> RequestFuture<R::Result> {
        self.request_with_timeout::<R>(params, self.inner.timeout)
    }

    /// [`Client::request`] with its own timeout.
    pub(crate) fn request_with_timeout<R: lsp_types::request::Request>(
        &self,
        params: &R::Params,
        timeout: Duration,
    ) -> RequestFuture<R::Result> {
        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = oneshot::channel();
        self.inner
            .pending
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(id, sender);
        let body = request(id, R::METHOD, &to_value(params));
        if self.inner.writer.send(WriterJob::Body(body)).is_err()
            && let Some(sender) = self.inner.take_pending(id)
        {
            let _ = sender.send(Err(ServiceError::NotRunning));
        }
        let weak = Arc::downgrade(&self.inner);
        self.inner.timer.after(timeout, move || {
            if let Some(inner) = weak.upgrade()
                && let Some(sender) = inner.take_pending(id)
            {
                tracing::debug!(method = R::METHOD, "language server request timed out");
                inner.cancel(id);
                let _ = sender.send(Err(ServiceError::Timeout));
            }
        });
        RequestFuture {
            id,
            receiver,
            client: Arc::downgrade(&self.inner),
            done: false,
            result: PhantomData,
        }
    }

    /// Sends notification `N`.
    pub(crate) fn notify<N: lsp_types::notification::Notification>(&self, params: &N::Params) {
        self.send(WriterJob::Body(notification(N::METHOD, &to_value(params))));
    }

    /// Sends a message built on the writer thread.
    pub(crate) fn send_built(&self, build: impl FnOnce() -> Option<Vec<u8>> + Send + 'static) {
        self.send(WriterJob::Build(Box::new(build)));
    }

    /// Answers the server's request `id`.
    pub(crate) fn respond(&self, id: &Value, result: Result<Value, ResponseError>) {
        self.send(WriterJob::Body(response(id, result)));
    }

    fn send(&self, job: WriterJob) {
        // A closed writer means the server is gone; that is reported by
        // the reader.
        let _ = self.inner.writer.send(job);
    }

    /// The server answered request `id`.
    pub(crate) fn handle_response(&self, id: i64, result: Result<Value, ResponseError>) {
        let Some(sender) = self.inner.take_pending(id) else {
            // Cancelled or timed out already.
            return;
        };
        let answer = result.map_err(|error| match error.code {
            REQUEST_CANCELLED | SERVER_CANCELLED | CONTENT_MODIFIED => ServiceError::Cancelled,
            _ => ServiceError::Server(error.message),
        });
        let _ = sender.send(answer);
    }

    /// The server is gone: every waiting request fails.
    pub(crate) fn fail_all(&self) {
        let pending = std::mem::take(
            &mut *self
                .inner
                .pending
                .lock()
                .unwrap_or_else(PoisonError::into_inner),
        );
        for sender in pending.into_values() {
            let _ = sender.send(Err(ServiceError::NotRunning));
        }
    }
}

fn to_value(params: &impl Serialize) -> Value {
    // The protocol types always serialize.
    serde_json::to_value(params).unwrap_or(Value::Null)
}

/// The answer to a request, deserialized where it is awaited (in the
/// background for large answers like completion lists).
pub(crate) struct RequestFuture<T> {
    id: i64,
    receiver: oneshot::Receiver<Answer>,
    client: Weak<Inner>,
    done: bool,
    result: PhantomData<fn() -> T>,
}

impl<T: DeserializeOwned> Future for RequestFuture<T> {
    type Output = Result<T, ServiceError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let answer = match Pin::new(&mut self.receiver).poll(cx) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(Ok(answer)) => answer,
            Poll::Ready(Err(oneshot::Canceled)) => Err(ServiceError::NotRunning),
        };
        self.done = true;
        Poll::Ready(answer.and_then(|value| {
            serde_json::from_value(value).map_err(|error| ServiceError::Protocol(error.to_string()))
        }))
    }
}

impl<T> Drop for RequestFuture<T> {
    fn drop(&mut self) {
        if self.done {
            return;
        }
        if let Some(inner) = self.client.upgrade()
            && inner.take_pending(self.id).is_some()
        {
            inner.cancel(self.id);
        }
    }
}
