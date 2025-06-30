use futures_util::{FutureExt, StreamExt, pin_mut, select};

use crate::{Progress, ProgressUpdate};

/// Extension trait providing convenient methods for observing progress updates.
///
/// This trait extends the [`Progress`] trait with methods that make it easier to
/// observe progress updates without manually managing the progress stream.
pub trait ProgressExt: Progress {
    /// Observes progress updates while the future executes, calling the provided receiver
    /// function for each update.
    ///
    /// This method monitors the progress stream concurrently with the main future execution.
    /// The receiver function will be called for each progress update until the future completes.
    ///
    /// # Parameters
    ///
    /// - `receiver`: A function that will be called with each [`ProgressUpdate`]
    ///
    /// # Returns
    ///
    /// Returns a future that resolves to the same output as the original future.
    ///
    /// # Example
    ///
    /// ```
    /// # #[cfg(feature = "std")]
    /// # {
    /// use progressor::{progress, ProgressExt};
    ///
    /// # async fn example() {
    /// let task = progress(100, |mut updater| async move {
    ///     for i in 0..=100 {
    ///         updater.update(i);
    ///     }
    ///     "Done"
    /// });
    ///
    /// let result = task.observe(|update| {
    ///     println!("Progress: {}%", (update.completed_fraction() * 100.0) as u32);
    /// }).await;
    /// # }
    /// # }
    /// ```
    fn observe(
        self,
        receiver: impl Fn(ProgressUpdate) + Send,
    ) -> impl Future<Output = Self::Output> + Send
    where
        Self: Send + Sized,
    {
        async move {
            let progress_stream = self.progress().fuse();
            let future = self.fuse();
            pin_mut!(progress_stream, future);

            loop {
                select! {
                    result = future => return result,
                    update = progress_stream.next() => {
                        if let Some(update) = update {
                            receiver(update);
                        }
                    }
                }
            }
        }
    }

    /// Local version of [`observe`](Self::observe) that doesn't require `Send` bounds.
    ///
    /// This method is similar to [`observe`](Self::observe) but works with non-`Send`
    /// closures and futures. It's useful when you don't need to move the observer
    /// across thread boundaries.
    ///
    /// # Parameters
    ///
    /// - `receiver`: A function that will be called with each [`ProgressUpdate`]
    ///
    /// # Returns
    ///
    /// Returns a future that resolves to the same output as the original future.
    ///
    /// # Example
    ///
    /// ```
    /// # #[cfg(feature = "std")]
    /// # {
    /// use progressor::{progress, ProgressExt};
    ///
    /// # async fn example() {
    /// let task = progress(100, |mut updater| async move {
    ///     for i in 0..=100 {
    ///         updater.update(i);
    ///     }
    ///     "Done"
    /// });
    ///
    /// let result = task.observe_local(|update| {
    ///     println!("Progress: {}%", (update.completed_fraction() * 100.0) as u32);
    /// }).await;
    /// # }
    /// # }
    /// ```
    fn observe_local(self, receiver: impl Fn(ProgressUpdate)) -> impl Future<Output = Self::Output>
    where
        Self: Sized,
    {
        async move {
            let progress_stream = self.progress().fuse();
            let future = self.fuse();
            pin_mut!(progress_stream, future);

            loop {
                select! {
                    result = future => return result,
                    update = progress_stream.next() => {
                        if let Some(update) = update {
                            receiver(update);
                        }
                    }
                }
            }
        }
    }
}

impl<T: Progress> ProgressExt for T {}
