use core::future::Future;

use crate::{Progress, ProgressUpdate, State, WatchSender, track, watch};

/// A handle for updating progress during execution of a future.
///
/// Updates are published into a coalescing latest-value watch channel so high-frequency
/// writes overwrite stale snapshots in place without overflowing a ring buffer.
/// Dropping `ProgressUpdater` without calling [`complete`](Self::complete) or
/// [`cancel`](Self::cancel) simply closes the stream cleanly without fabricating
/// [`State::Cancelled`].
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
#[derive(Debug, Clone)]
pub struct ProgressUpdater {
    total: Option<u64>,
    current: u64,
    finished: bool,
    sender: WatchSender<ProgressUpdate>,
}

impl ProgressUpdater {
    const fn new(total: Option<u64>, sender: WatchSender<ProgressUpdate>) -> Self {
        Self {
            total,
            current: 0,
            finished: false,
            sender,
        }
    }

    /// Updates the progress with the given current value and message.
    pub fn update_with_message(&mut self, current: u64, message: impl Into<String>) {
        self.current = current;
        let update = ProgressUpdate::new(self.total, current, State::Working, Some(message.into()));
        self.sender.send(update);
    }

    /// Updates the progress with the given current value.
    pub fn update(&mut self, current: u64) {
        self.current = current;
        let update = ProgressUpdate::new(self.total, current, State::Working, None);
        self.sender.send(update);
    }

    /// Pauses the progress operation.
    pub fn pause(&self) {
        let update = ProgressUpdate::new(self.total, self.current, State::Paused, None);
        self.sender.send(update);
    }

    /// Marks the progress operation as completed.
    ///
    /// Subsequent calls to `complete` or [`cancel`](Self::cancel) have no effect.
    pub fn complete(&mut self) {
        if !self.finished {
            self.finished = true;
            let update = ProgressUpdate::new(self.total, self.current, State::Completed, None);
            self.sender.send(update);
        }
    }

    /// Pauses the progress operation with a descriptive message.
    pub fn pause_with_message(&self, message: impl Into<String>) {
        let update = ProgressUpdate::new(
            self.total,
            self.current,
            State::Paused,
            Some(message.into()),
        );
        self.sender.send(update);
    }

    /// Updates the total expected value for the progress operation.
    ///
    /// Pass `None` if the total is unknown, or `Some(total)` once known.
    pub fn set_total(&mut self, total: Option<u64>) {
        self.total = total;
        let update = ProgressUpdate::new(self.total, self.current, State::Working, None);
        self.sender.send(update);
    }

    /// Explicitly cancels the progress operation and emits [`State::Cancelled`].
    ///
    /// Has no effect if [`complete`](Self::complete) was already called.
    pub fn cancel(mut self) {
        if !self.finished {
            self.finished = true;
            let update = ProgressUpdate::new(self.total, self.current, State::Cancelled, None);
            self.sender.send(update);
        }
    }
}

/// Creates a progress-tracked future from a closure.
///
/// Pass `Some(total)` when the total amount of work is known upfront, or `None` when
/// indeterminate (it can be updated later with [`ProgressUpdater::set_total`]).
///
/// # Examples
///
/// ```
/// use progressor::{Progress, State, progress};
/// use futures_util::StreamExt;
///
/// # async fn example() {
/// let task = progress(Some(100), |mut updater| async move {
///     for i in 0..=100 {
///         if i == 50 {
///             updater.pause();
///         } else {
///             updater.update(i);
///         }
///     }
///     updater.complete();
///     "completed"
/// });
///
/// let mut progress_stream = task.progress();
/// while let Some(update) = progress_stream.next().await {
///     match update.state() {
///         State::Working => {
///             if let Some(fraction) = update.completed_fraction() {
///                 println!("Progress: {:.0}%", fraction * 100.0);
///             }
///         }
///         State::Paused => println!("Task paused at {}", update.current()),
///         State::Completed => println!("Task completed!"),
///         State::Cancelled => println!("Task cancelled!"),
///     }
/// }
/// # }
/// ```
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
pub fn progress<F, Fut>(
    total: Option<u64>,
    f: F,
) -> impl Progress<Output = Fut::Output, Update = ProgressUpdate>
where
    F: FnOnce(ProgressUpdater) -> Fut,
    Fut: Future,
{
    let (sender, receiver) = watch(None);
    let updater = ProgressUpdater::new(total, sender);
    let fut = f(updater);
    track(receiver, fut)
}
