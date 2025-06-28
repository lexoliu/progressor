use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use async_broadcast::{broadcast, Receiver, Sender};
use futures_core::Stream;
use pin_project_lite::pin_project;

use crate::{Progress, ProgressUpdate};

#[derive(Debug, Clone)]
pub struct ProgressUpdater {
    state: ProgressUpdate,
    sender: Sender<ProgressUpdate>,
}

impl ProgressUpdater {
    fn new(total: u64, sender: Sender<ProgressUpdate>) -> Self {
        let state = ProgressUpdate::new(total);
        Self { state, sender }
    }

    pub fn update_with_message(&mut self, current: u64, message: impl Into<String>) {
        self.state.current = current;
        self.state.message = Some(message.into());
        let _ = self.sender.try_broadcast(self.state.clone());
    }

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
