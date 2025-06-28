use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use async_broadcast::{broadcast, Receiver, Sender};
use futures_core::Stream;
use pin_project_lite::pin_project;

use crate::{Progress, ProgressUpdate, State};

/// A handle for updating progress during execution of a future.
///
/// This struct allows you to report progress updates that will be broadcast
/// to listeners via the progress stream. It maintains internal state and
/// automatically handles cancellation when dropped.
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
#[derive(Debug, Clone)]
pub struct ProgressUpdater {
    total: u64,
    current: u64,
    completed: bool,
    sender: Sender<ProgressUpdate>,
}

impl ProgressUpdater {
    const fn new(total: u64, sender: Sender<ProgressUpdate>) -> Self {
        Self {
            total,
            current: 0,
            completed: false,
            sender,
        }
    }

    /// Updates the progress with the given current value and message.
    ///
    /// This will broadcast the update to all progress stream listeners.
    pub fn update_with_message(&mut self, current: u64, message: impl Into<String>) {
        self.current = current;
        let update = ProgressUpdate::new(self.total, current, State::Working, Some(message.into()));
        self.broadcast(update);
    }

    /// Updates the progress with the given current value.
    ///
    /// This will broadcast the update to all progress stream listeners.
    pub fn update(&mut self, current: u64) {
        self.current = current;
        let update = ProgressUpdate::new(self.total, current, State::Working, None);
        self.broadcast(update);
    }

    /// Pauses the progress operation.
    ///
    /// This method sets the progress state to paused and broadcasts the update to all listeners.
    pub fn pause(&self) {
        let update = ProgressUpdate::new(self.total, self.current, State::Paused, None);
        self.broadcast(update);
    }

    /// Marks the progress operation as completed.
    ///
    /// This method sets the completed flag and broadcasts a completion update.
    /// Subsequent calls to this method have no effect.
    pub fn complete(&mut self) {
        if !self.completed {
            self.completed = true;
            let update = ProgressUpdate::new(self.total, self.current, State::Completed, None);
            self.broadcast(update);
        }
    }

    /// Pauses the progress operation with a descriptive message.
    ///
    /// This method sets the progress state to paused and broadcasts the update to all listeners.
    pub fn pause_with_message(&self, message: impl Into<String>) {
        let update = ProgressUpdate::new(
            self.total,
            self.current,
            State::Paused,
            Some(message.into()),
        );
        self.broadcast(update);
    }

    /// Updates the total expected value for the progress operation.
    ///
    /// This method changes the total value and broadcasts an update with the current progress.
    pub fn set_total(&mut self, total: u64) {
        self.total = total;
        let update = ProgressUpdate::new(self.total, self.current, State::Working, None);
        self.broadcast(update);
    }

    fn broadcast(&self, update: ProgressUpdate) {
        let _ = self.sender.try_broadcast(update);
    }
    /// Cancels the progress operation.
    pub fn cancel(self) {
        // Drop will handle cancellation automatically
    }
}

impl Drop for ProgressUpdater {
    fn drop(&mut self) {
        if !self.completed {
            let _ = self.sender.try_broadcast(ProgressUpdate::new(
                self.total,
                self.current,
                State::Cancelled,
                None,
            ));
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
    fn progress(&self) -> impl Stream<Item = ProgressUpdate> + Unpin + Send + 'static {
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
/// use progressor::{progress, Progress, State};
/// use futures_util::StreamExt;
///
/// # async fn example() {
/// let task = progress(100, |mut updater| async move {
///     for i in 0..=100 {
///         if i == 50 {
///             // Pause at halfway point
///             updater.pause();
///         } else {
///             updater.update(i);
///         }
///         // Do some work...
///     }
///     "completed"
/// });
///
/// // Monitor progress - no need for Box::pin with Unpin
/// let mut progress_stream = task.progress();
/// while let Some(update) = progress_stream.next().await {
///     match update.state() {
///         State::Working => println!("Progress: {}%", (update.completed_fraction() * 100.0) as u32),
///         State::Paused => println!("Task paused at {}%", (update.completed_fraction() * 100.0) as u32),
///         State::Completed => println!("Task completed!"),
///         State::Cancelled => println!("Task cancelled!"),
///     }
/// }
/// # }
/// ```
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
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
