use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use async_broadcast::{broadcast, Receiver, Sender};
use futures_core::Stream;
use pin_project_lite::pin_project;

use crate::{Progress, ProgressUpdate};

/// A handle for updating progress during execution of a future.
///
/// This struct allows you to report progress updates that will be broadcast
/// to listeners via the progress stream. It maintains internal state and
/// automatically handles cancellation when dropped.
#[derive(Debug, Clone)]
pub struct ProgressUpdater {
    state: ProgressUpdate,
    sender: Sender<ProgressUpdate>,
}

impl ProgressUpdater {
    #[allow(clippy::missing_const_for_fn)]
    fn new(total: u64, sender: Sender<ProgressUpdate>) -> Self {
        let state = ProgressUpdate::new(total);
        Self { state, sender }
    }

    /// Updates the progress with the given current value and message.
    ///
    /// This will broadcast the update to all progress stream listeners.
    /// The message will be included in the progress update.
    pub fn update_with_message(&mut self, current: u64, message: impl Into<String>) {
        self.state.current = current;
        self.state.message = Some(message.into());
        let _ = self.sender.try_broadcast(self.state.clone());
    }

    /// Updates the progress with the given current value.
    ///
    /// This will broadcast the update to all progress stream listeners.
    /// Any previous message will be cleared.
    pub fn update(&mut self, current: u64) {
        self.state.current = current;
        self.state.message = None;
        let _ = self.sender.try_broadcast(self.state.clone());
    }
}

impl Drop for ProgressUpdater {
    fn drop(&mut self) {
        if !self.state.is_cancelled {
            self.state.is_cancelled = true;
            let _ = self.sender.try_broadcast(self.state.clone());
        }
    }
}

pin_project! {
    struct ProgressFuture<Fut>
    where
        Fut: Future,
    {
        receiver: Receiver<ProgressUpdate>,
        #[pin]
        fut: Fut,
    }
}

impl<Fut> Future for ProgressFuture<Fut>
where
    Fut: Future,
{
    type Output = Fut::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().fut.poll(cx)
    }
}

impl<Fut> Progress for ProgressFuture<Fut>
where
    Fut: Future,
{
    fn progress(&self) -> impl Stream<Item = ProgressUpdate> + Send + 'static {
        self.receiver.clone()
    }
}

/// Creates a progress-tracked future from a closure.
///
/// This function takes a total progress value and a closure that receives a
/// [`ProgressUpdater`]. The closure should use the updater to report progress
/// as it executes. The returned future implements [`Progress`] and can be
/// used to monitor the progress stream.
///
/// # Examples
///
/// ```
/// use progressor::{updater::progress, Progress};
/// use futures_util::StreamExt;
///
/// # async fn example() {
/// let task = progress(100, |mut updater| async move {
///     for i in 0..=100 {
///         updater.update(i);
///         // Do some work...
///     }
///     "completed"
/// });
///
/// // Monitor progress
/// let mut progress_stream = Box::pin(task.progress());
/// // ... handle progress updates
/// # }
/// ```
pub fn progress<F, Fut>(total: u64, f: F) -> impl Progress<Output = Fut::Output>
where
    F: FnOnce(ProgressUpdater) -> Fut,
    Fut: Future,
{
    let (sender, receiver) = broadcast(32);
    let updater = ProgressUpdater::new(total, sender);
    let fut = f(updater);
    ProgressFuture { receiver, fut }
}
