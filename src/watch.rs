use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};
use std::sync::{Arc, Mutex, PoisonError};

use futures_core::Stream;
use pin_project_lite::pin_project;

use crate::Progress;

#[derive(Debug)]
struct Slot<T> {
    version: u64,
    value: Option<T>,
    closed: bool,
    sender_count: usize,
    wakers: Vec<Waker>,
}

/// Sender half of a coalescing latest-value watch channel.
///
/// Each call to [`send`](Self::send) overwrites the stored snapshot in place,
/// increments the version counter, and wakes registered receivers outside the lock.
/// Dropping the last clone closes the channel so receivers terminate cleanly after
/// observing the latest snapshot.
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
#[derive(Debug)]
pub struct WatchSender<T> {
    shared: Arc<Mutex<Slot<T>>>,
}

impl<T> WatchSender<T> {
    /// Stores `value` as the latest snapshot and wakes any waiting receivers.
    pub fn send(&self, value: T) {
        let (previous, wakers) = {
            let mut slot = self.shared.lock().unwrap_or_else(PoisonError::into_inner);
            let previous = slot.value.replace(value);
            slot.version += 1;
            let wakers = core::mem::take(&mut slot.wakers);
            drop(slot);
            (previous, wakers)
        };
        drop(previous);
        for waker in wakers {
            waker.wake();
        }
    }
}

impl<T> Clone for WatchSender<T> {
    fn clone(&self) -> Self {
        let mut slot = self.shared.lock().unwrap_or_else(PoisonError::into_inner);
        slot.sender_count += 1;
        drop(slot);
        Self {
            shared: Arc::clone(&self.shared),
        }
    }
}

impl<T> Drop for WatchSender<T> {
    fn drop(&mut self) {
        let wakers = {
            let mut slot = self.shared.lock().unwrap_or_else(PoisonError::into_inner);
            slot.sender_count -= 1;
            if slot.sender_count == 0 {
                slot.closed = true;
                core::mem::take(&mut slot.wakers)
            } else {
                Vec::new()
            }
        };
        for waker in wakers {
            waker.wake();
        }
    }
}

/// Receiver half of a coalescing latest-value watch channel.
///
/// Implements [`Stream`] yielding the latest unseen snapshot whenever the sender
/// publishes a newer version, then terminates with `None` once all senders have
/// been dropped and the final snapshot has been observed.
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
#[derive(Debug)]
pub struct WatchReceiver<T> {
    shared: Arc<Mutex<Slot<T>>>,
    seen_version: u64,
}

impl<T> Clone for WatchReceiver<T> {
    fn clone(&self) -> Self {
        Self {
            shared: Arc::clone(&self.shared),
            seen_version: self.seen_version,
        }
    }
}

impl<T> Stream for WatchReceiver<T>
where
    T: Clone + Send + Sync + 'static,
{
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        let mut slot = this.shared.lock().unwrap_or_else(PoisonError::into_inner);
        if slot.version > this.seen_version {
            this.seen_version = slot.version;
            if let Some(value) = slot.value.clone() {
                return Poll::Ready(Some(value));
            }
        }
        if slot.closed {
            return Poll::Ready(None);
        }
        let waker = cx.waker();
        if !slot.wakers.iter().any(|existing| existing.will_wake(waker)) {
            slot.wakers.push(waker.clone());
        }
        Poll::Pending
    }
}

/// Creates a coalescing latest-value watch channel.
///
/// If `initial` is `Some(value)`, the channel starts at version `1` so new receivers
/// immediately observe `value` on their first poll. If `initial` is `None`, receivers
/// wait until the first [`WatchSender::send`] call (or terminate with `None` if all
/// senders drop first).
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
#[must_use]
pub fn watch<T>(initial: Option<T>) -> (WatchSender<T>, WatchReceiver<T>) {
    let version = u64::from(initial.is_some());
    let shared = Arc::new(Mutex::new(Slot {
        version,
        value: initial,
        closed: false,
        sender_count: 1,
        wakers: Vec::new(),
    }));
    (
        WatchSender {
            shared: Arc::clone(&shared),
        },
        WatchReceiver {
            shared,
            seen_version: 0,
        },
    )
}

pin_project! {
    /// A future paired with a [`WatchReceiver`] so it implements [`Progress`].
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    #[derive(Debug)]
    pub struct Tracked<Fut, T> {
        receiver: WatchReceiver<T>,
        #[pin]
        fut: Fut,
    }
}

impl<Fut, T> Future for Tracked<Fut, T>
where
    Fut: Future,
{
    type Output = Fut::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().fut.poll(cx)
    }
}

impl<Fut, T> Progress for Tracked<Fut, T>
where
    Fut: Future,
    T: Clone + Send + Sync + 'static,
{
    type Update = T;

    fn progress(&self) -> impl Stream<Item = T> + Unpin + Send + 'static {
        self.receiver.clone()
    }
}

/// Pairs a [`WatchReceiver`] with a future, returning a [`Tracked`] future that implements [`Progress`].
///
/// # Examples
///
/// ```
/// use progressor::{ProgressExt, track, watch};
///
/// #[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// struct BytesProgress {
///     downloaded: u64,
///     total: Option<u64>,
/// }
///
/// # async fn example() {
/// let (tx, rx) = watch(None);
/// let task = track(rx, async move {
///     tx.send(BytesProgress { downloaded: 512, total: Some(1024) });
///     tx.send(BytesProgress { downloaded: 1024, total: Some(1024) });
///     1024_u64
/// });
///
/// let mut last = None;
/// let bytes = task.observe(|snapshot| {
///     last = Some(snapshot);
/// }).await;
///
/// assert_eq!(bytes, 1024);
/// assert_eq!(last, Some(BytesProgress { downloaded: 1024, total: Some(1024) }));
/// # }
/// ```
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
#[must_use]
pub const fn track<Fut, T>(receiver: WatchReceiver<T>, fut: Fut) -> Tracked<Fut, T>
where
    Fut: Future,
    T: Clone + Send + Sync + 'static,
{
    Tracked { receiver, fut }
}
