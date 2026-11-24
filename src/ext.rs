use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};

use futures_core::Stream;
use futures_util::{FutureExt, StreamExt, pin_mut, select};

use crate::Progress;

fn drain_ready<S, T>(mut stream: Pin<&mut S>, receiver: &mut impl FnMut(T))
where
    S: Stream<Item = T>,
{
    let mut cx = Context::from_waker(Waker::noop());
    while let Poll::Ready(Some(update)) = stream.as_mut().poll_next(&mut cx) {
        receiver(update);
    }
}

/// Extension trait providing convenient methods for observing progress updates.
///
/// This trait extends [`Progress`] with methods that monitor progress updates
/// concurrently with the future without manually wiring `select!` loops.
pub trait ProgressExt: Progress {
    /// Observes progress updates while the future executes, calling `receiver`
    /// for each update.
    ///
    /// When the future completes, any unseen snapshot already ready in the progress
    /// stream is drained and passed to `receiver` before returning the future's output.
    ///
    /// # Example
    ///
    /// ```
    /// # #[cfg(feature = "std")]
    /// # {
    /// use progressor::{ProgressExt, progress};
    ///
    /// # async fn example() {
    /// let task = progress(Some(100), |mut updater| async move {
    ///     for i in 0..=100 {
    ///         updater.update(i);
    ///     }
    ///     updater.complete();
    ///     "Done"
    /// });
    ///
    /// let mut seen = 0_u64;
    /// let result = task.observe(|update| {
    ///     seen = update.current();
    /// }).await;
    /// assert_eq!(result, "Done");
    /// assert_eq!(seen, 100);
    /// # }
    /// # }
    /// ```
    fn observe(
        self,
        mut receiver: impl FnMut(Self::Update) + Send,
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
                    result = future => {
                        drain_ready(progress_stream.as_mut(), &mut receiver);
                        return result;
                    }
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
    /// Works with non-`Send` closures and futures, and likewise drains any ready final
    /// snapshot from the progress stream once the future resolves.
    ///
    /// # Example
    ///
    /// ```
    /// # #[cfg(feature = "std")]
    /// # {
    /// use progressor::{ProgressExt, progress};
    ///
    /// # async fn example() {
    /// let task = progress(Some(100), |mut updater| async move {
    ///     for i in 0..=100 {
    ///         updater.update(i);
    ///     }
    ///     updater.complete();
    ///     "Done"
    /// });
    ///
    /// let mut seen = 0_u64;
    /// let result = task.observe_local(|update| {
    ///     seen = update.current();
    /// }).await;
    /// assert_eq!(result, "Done");
    /// assert_eq!(seen, 100);
    /// # }
    /// # }
    /// ```
    fn observe_local(
        self,
        mut receiver: impl FnMut(Self::Update),
    ) -> impl Future<Output = Self::Output>
    where
        Self: Sized,
    {
        async move {
            let progress_stream = self.progress().fuse();
            let future = self.fuse();
            pin_mut!(progress_stream, future);

            loop {
                select! {
                    result = future => {
                        drain_ready(progress_stream.as_mut(), &mut receiver);
                        return result;
                    }
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
