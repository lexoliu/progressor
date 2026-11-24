//! A modern, async-first progress tracking library for Rust.
//!
//! This crate provides types and utilities for tracking progress of long-running operations
//! in an async context. Operations expose a `Stream` of latest-value progress snapshots via
//! [`Progress`], supporting both the built-in [`ProgressUpdate`] and domain-specific `Copy`
//! snapshot types via [`watch`] and [`track`].
//!
//! # Features
//!
//! - **Async-first**: Built around Rust's `Future` and `Stream` APIs
//! - **Coalescing watch channel**: Latest-value in-place updates with no ring overflow
//! - **Custom snapshot types**: Parameterized [`Progress::Update`] for zero-allocation `Copy` updates
//! - **Optional totals**: Distinguish unknown totals (`None`) from zero-byte totals (`Some(0)`)
//! - **Convenient observing**: [`ProgressExt::observe`] and [`ProgressExt::observe_local`] accept `FnMut` and drain the final snapshot
//!
//! # Examples
//!
//! ## Using `observe` (recommended)
//!
//! ```
//! # #[cfg(feature = "std")]
//! # {
//! use progressor::{ProgressExt, progress};
//!
//! # async fn example() {
//! let mut latest = 0_u64;
//! let result = progress(Some(100), |mut updater| async move {
//!     for i in 0..=100 {
//!         if i % 25 == 0 {
//!             updater.update_with_message(i, format!("Milestone: {i}%"));
//!         } else {
//!             updater.update(i);
//!         }
//!     }
//!     updater.complete();
//!     "Task completed!"
//! })
//! .observe(|update| {
//!     latest = update.current();
//!     if let Some(fraction) = update.completed_fraction() {
//!         println!("Progress: {:.0}%", fraction * 100.0);
//!     }
//!     if let Some(message) = update.message() {
//!         println!("  {message}");
//!     }
//! })
//! .await;
//!
//! assert_eq!(result, "Task completed!");
//! assert_eq!(latest, 100);
//! # }
//! # }
//! ```
//!
//! ## Custom `Copy` snapshots with `watch` and `track`
//!
//! ```
//! # #[cfg(feature = "std")]
//! # {
//! use progressor::{ProgressExt, track, watch};
//!
//! #[derive(Clone, Copy, Debug, PartialEq, Eq)]
//! struct TransferProgress {
//!     bytes: u64,
//!     total: Option<u64>,
//!     active_workers: u16,
//! }
//!
//! # async fn example() {
//! let (tx, rx) = watch(None);
//! let task = track(rx, async move {
//!     tx.send(TransferProgress {
//!         bytes: 4096,
//!         total: None,
//!         active_workers: 4,
//!     });
//!     4096_u64
//! });
//!
//! let mut last = None;
//! let downloaded = task.observe(|snapshot| {
//!     last = Some(snapshot);
//! }).await;
//!
//! assert_eq!(downloaded, 4096);
//! assert_eq!(
//!     last,
//!     Some(TransferProgress {
//!         bytes: 4096,
//!         total: None,
//!         active_workers: 4,
//!     })
//! );
//! # }
//! # }
//! ```
//!
//! ## Manual stream monitoring with `tokio::select!`
//!
//! ```
//! # #[cfg(feature = "std")]
//! # {
//! use progressor::{Progress, progress};
//! use futures_util::StreamExt;
//!
//! # async fn example() {
//! let task = progress(Some(100), |mut updater| async move {
//!     for i in 0..=100 {
//!         updater.update(i);
//!     }
//!     updater.complete();
//!     "Task completed!"
//! });
//!
//! let mut progress_stream = task.progress();
//! tokio::select! {
//!     result = task => {
//!         println!("Result: {result}");
//!     }
//!     () = async {
//!         while let Some(update) = progress_stream.next().await {
//!             if let Some(fraction) = update.completed_fraction() {
//!                 println!("Progress: {:.0}%", fraction * 100.0);
//!             }
//!         }
//!     } => {}
//! }
//! # }
//! # }
//! ```

#![cfg_attr(docsrs, feature(doc_cfg))]

mod ext;
pub use ext::ProgressExt;
#[cfg(feature = "std")]
mod updater;
#[cfg(feature = "std")]
mod watch;

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
pub use updater::{ProgressUpdater, progress};
#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
pub use watch::{Tracked, WatchReceiver, WatchSender, track, watch};

use core::future::Future;
use futures_core::Stream;

/// A trait for futures that can report progress updates.
///
/// This trait extends [`Future`] with an associated [`Update`](Self::Update) snapshot
/// type and a method returning a [`Stream`] of those snapshots.
pub trait Progress: Future {
    /// The progress snapshot type emitted by this operation.
    type Update: Clone + Send + Sync + 'static;

    /// Returns a stream of progress updates for this operation.
    ///
    /// Poll the stream concurrently with the future to observe updates.
    fn progress(&self) -> impl Stream<Item = Self::Update> + Unpin + Send + 'static;
}

/// Represents a single progress update with current value, optional total, state, and message.
///
/// A `total` of `None` indicates indeterminate progress; `Some(0)` indicates a known zero-length
/// operation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProgressUpdate {
    current: u64,
    total: Option<u64>,
    state: State,
    message: Option<String>,
}

/// Represents the state of a progress-tracked operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum State {
    /// The operation is in progress.
    Working,
    /// The operation has been completed successfully.
    Completed,
    /// The operation has been paused.
    Paused,
    /// The operation has been cancelled.
    Cancelled,
}

impl State {
    /// Returns `true` if the state is [`Cancelled`](State::Cancelled).
    #[must_use]
    pub const fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled)
    }

    /// Returns `true` if the state is [`Working`](State::Working).
    #[must_use]
    pub const fn is_working(&self) -> bool {
        matches!(self, Self::Working)
    }

    /// Returns `true` if the state is [`Completed`](State::Completed).
    #[must_use]
    pub const fn is_completed(&self) -> bool {
        matches!(self, Self::Completed)
    }

    /// Returns `true` if the state is [`Paused`](State::Paused).
    #[must_use]
    pub const fn is_paused(&self) -> bool {
        matches!(self, Self::Paused)
    }
}

impl ProgressUpdate {
    /// Creates a new progress update.
    #[must_use]
    pub const fn new(
        total: Option<u64>,
        current: u64,
        state: State,
        message: Option<String>,
    ) -> Self {
        Self {
            current,
            total,
            state,
            message,
        }
    }

    /// Returns the total expected value when known, or `None` if indeterminate.
    #[must_use]
    pub const fn total(&self) -> Option<u64> {
        self.total
    }

    /// Returns the current progress value.
    #[must_use]
    pub const fn current(&self) -> u64 {
        self.current
    }

    /// Returns the completion fraction as a value between `0.0` and `1.0` when the total is known.
    ///
    /// Returns `None` when [`total`](Self::total) is `None`, `Some(0.0)` when `total` is `Some(0)`,
    /// and `Some(current as f64 / total as f64)` otherwise.
    #[must_use]
    pub fn completed_fraction(&self) -> Option<f64> {
        self.total.map(|total| {
            if total == 0 {
                0.0
            } else {
                #[allow(clippy::cast_precision_loss)]
                {
                    self.current as f64 / total as f64
                }
            }
        })
    }

    /// Returns the remaining progress (`total - current`) when the total is known.
    ///
    /// Returns `None` when [`total`](Self::total) is `None`, and uses saturating
    /// subtraction when `current > total`.
    #[must_use]
    pub const fn remaining(&self) -> Option<u64> {
        match self.total {
            Some(total) => Some(total.saturating_sub(self.current)),
            None => None,
        }
    }

    /// Returns `true` if the state is [`Cancelled`](State::Cancelled).
    #[must_use]
    pub const fn is_cancelled(&self) -> bool {
        matches!(self.state, State::Cancelled)
    }

    /// Returns `true` if the state is [`Working`](State::Working).
    #[must_use]
    pub const fn is_working(&self) -> bool {
        matches!(self.state, State::Working)
    }

    /// Returns `true` if the state is [`Completed`](State::Completed).
    #[must_use]
    pub const fn is_completed(&self) -> bool {
        matches!(self.state, State::Completed)
    }

    /// Returns `true` if the state is [`Paused`](State::Paused).
    #[must_use]
    pub const fn is_paused(&self) -> bool {
        matches!(self.state, State::Paused)
    }

    /// Returns the optional descriptive message about the current progress.
    #[must_use]
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// Returns the current state of the progress operation.
    #[must_use]
    pub const fn state(&self) -> State {
        self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_update_new() {
        let update = ProgressUpdate::new(Some(100), 0, State::Working, None);
        assert_eq!(update.current(), 0);
        assert_eq!(update.total(), Some(100));
        assert!(!update.is_cancelled());
        assert_eq!(update.message(), None);
    }

    #[test]
    fn test_progress_update_unknown_total() {
        let update = ProgressUpdate::new(None, 42, State::Working, None);
        assert_eq!(update.current(), 42);
        assert_eq!(update.total(), None);
        assert_eq!(update.completed_fraction(), None);
        assert_eq!(update.remaining(), None);
    }

    #[test]
    fn test_completed_fraction() {
        let mut update = ProgressUpdate::new(Some(100), 0, State::Working, None);
        assert!((update.completed_fraction().unwrap() - 0.0).abs() < f64::EPSILON);

        update.current = 50;
        assert!((update.completed_fraction().unwrap() - 0.5).abs() < f64::EPSILON);

        update.current = 100;
        assert!((update.completed_fraction().unwrap() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_completed_fraction_zero_total() {
        let update = ProgressUpdate::new(Some(0), 0, State::Working, None);
        assert_eq!(update.total(), Some(0));
        assert!((update.completed_fraction().unwrap() - 0.0).abs() < f64::EPSILON);
        assert_eq!(update.remaining(), Some(0));
    }

    #[test]
    fn test_builder_methods() {
        let update = ProgressUpdate::new(
            Some(100),
            50,
            State::Working,
            Some("Half complete".to_string()),
        );

        assert_eq!(update.current(), 50);
        assert_eq!(update.message(), Some("Half complete"));
        assert_eq!(update.state, State::Working);
        assert!(update.is_working());
        assert!(!update.is_cancelled());
        assert!(!update.is_completed());
        assert!(!update.is_paused());
    }

    #[test]
    fn test_is_complete() {
        let mut update = ProgressUpdate::new(Some(100), 0, State::Working, None);
        assert!(!update.is_completed());
        assert!(update.is_working());

        update.state = State::Completed;
        assert!(update.is_completed());

        update.state = State::Cancelled;
        assert!(update.is_cancelled());
        assert!(!update.is_completed());

        update.state = State::Paused;
        assert!(update.is_paused());
        assert!(!update.is_completed());
    }

    #[test]
    fn test_remaining() {
        let mut update = ProgressUpdate::new(Some(100), 0, State::Working, None);
        assert_eq!(update.remaining(), Some(100));

        update.current = 30;
        assert_eq!(update.remaining(), Some(70));

        update.current = 100;
        assert_eq!(update.remaining(), Some(0));

        update.current = 150;
        assert_eq!(update.remaining(), Some(0));
    }

    #[cfg(feature = "std")]
    mod async_tests {
        use futures_util::StreamExt;

        use super::*;

        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        struct DownloadSnapshot {
            downloaded: u64,
            total: Option<u64>,
            active_workers: u16,
        }

        #[tokio::test]
        async fn watch_initial_and_cloned_receivers() {
            let (tx, mut rx1) = watch(Some(10_u64));
            let mut rx2 = rx1.clone();

            assert_eq!(rx1.next().await, Some(10));
            assert_eq!(rx2.next().await, Some(10));

            tx.send(20);
            tx.send(30);
            drop(tx);

            assert_eq!(rx1.next().await, Some(30));
            assert_eq!(rx1.next().await, None);
            assert_eq!(rx2.next().await, Some(30));
            assert_eq!(rx2.next().await, None);
        }

        #[tokio::test]
        async fn watch_none_closes_cleanly_without_emitting() {
            let (tx, mut rx) = watch::<u64>(None);
            drop(tx);
            assert_eq!(rx.next().await, None);
        }

        #[tokio::test]
        async fn watch_stays_open_until_last_sender_drops() {
            let (tx1, mut rx) = watch(None);
            let tx2 = tx1.clone();

            tx1.send(1_u64);
            drop(tx1);
            assert_eq!(rx.next().await, Some(1));

            tx2.send(2_u64);
            drop(tx2);
            assert_eq!(rx.next().await, Some(2));
            assert_eq!(rx.next().await, None);
        }

        #[tokio::test]
        async fn rapid_100k_updates_coalesce_without_losing_final_snapshot() {
            const COUNT: u64 = 100_000;

            let mut seen = Vec::new();
            let total = progress(Some(COUNT), |mut updater| async move {
                for i in 1..=COUNT {
                    updater.update(i);
                }
                updater.complete();
                COUNT
            })
            .observe(|update| {
                seen.push(update);
            })
            .await;

            assert_eq!(total, COUNT);
            assert!(
                !seen.is_empty(),
                "observer must receive at least the final snapshot"
            );
            assert!(
                seen.len() < usize::try_from(COUNT).unwrap(),
                "updates should coalesce instead of delivering every intermediate write"
            );
            let last = seen.last().unwrap();
            assert_eq!(last.current(), COUNT);
            assert_eq!(last.total(), Some(COUNT));
            assert_eq!(last.state(), State::Completed);
        }

        #[tokio::test]
        async fn rapid_100k_updates_concurrent_coalescing() {
            const COUNT: u64 = 100_000;

            let (tx, mut rx) = watch(None);
            let producer = tokio::spawn(async move {
                for i in 1..=COUNT {
                    tx.send(i);
                    if i % 10_000 == 0 {
                        tokio::task::yield_now().await;
                    }
                }
            });

            let mut last = 0_u64;
            let mut observed_count = 0_u64;
            while let Some(v) = rx.next().await {
                assert!(v > last, "snapshots must advance monotonically");
                last = v;
                observed_count += 1;
            }
            producer.await.unwrap();

            assert_eq!(last, COUNT);
            assert!(observed_count < COUNT);
        }

        #[tokio::test]
        async fn error_exit_does_not_emit_cancelled() {
            let mut states = Vec::new();
            let result: Result<(), &'static str> = progress(None, |mut updater| async move {
                updater.update(10);
                tokio::task::yield_now().await;
                updater.update(20);
                Err("network failure")?;
                updater.complete();
                Ok(())
            })
            .observe(|update| {
                states.push(update.state());
            })
            .await;

            assert_eq!(result, Err("network failure"));
            assert!(!states.contains(&State::Cancelled));
            assert!(!states.contains(&State::Completed));
            assert!(states.contains(&State::Working));
        }

        #[tokio::test]
        async fn explicit_cancel_emits_cancelled() {
            let mut updates = Vec::new();
            let result = progress(Some(100), |mut updater| async move {
                updater.update(40);
                tokio::task::yield_now().await;
                updater.cancel();
                "aborted"
            })
            .observe(|update| {
                updates.push(update);
            })
            .await;

            assert_eq!(result, "aborted");
            let last = updates.last().expect("cancel snapshot must be observed");
            assert_eq!(last.current(), 40);
            assert_eq!(last.state(), State::Cancelled);
        }

        #[tokio::test]
        async fn optional_total_transitions_from_none_to_some() {
            let mut updates = Vec::new();
            progress(None, |mut updater| async move {
                updater.update(25);
                tokio::task::yield_now().await;
                updater.set_total(Some(100));
                tokio::task::yield_now().await;
                updater.update(100);
                updater.complete();
            })
            .observe(|update| {
                updates.push(update);
            })
            .await;

            assert!(updates.iter().any(|u| u.total().is_none()
                && u.completed_fraction().is_none()
                && u.remaining().is_none()));
            let last = updates.last().unwrap();
            assert_eq!(last.total(), Some(100));
            assert_eq!(last.current(), 100);
            assert_eq!(last.remaining(), Some(0));
            assert!((last.completed_fraction().unwrap() - 1.0).abs() < f64::EPSILON);
            assert_eq!(last.state(), State::Completed);
        }

        #[tokio::test]
        async fn observe_local_fnmut_mutates_non_send_state_and_drains_final() {
            use std::cell::RefCell;
            use std::rc::Rc;

            let log = Rc::new(RefCell::new(Vec::new()));
            let log_ref = Rc::clone(&log);

            let out = progress(Some(10), |mut updater| async move {
                updater.update(10);
                updater.complete();
                42_u32
            })
            .observe_local(move |update| {
                log_ref.borrow_mut().push(update);
            })
            .await;

            assert_eq!(out, 42);
            let entries = log.borrow();
            let last = entries.last().expect("final snapshot must be drained");
            assert_eq!(last.current(), 10);
            assert_eq!(last.state(), State::Completed);
        }

        #[tokio::test]
        async fn custom_copy_struct_with_track_and_watch() {
            let (tx, rx) = watch(None);
            let task = track(rx, async move {
                tx.send(DownloadSnapshot {
                    downloaded: 1024,
                    total: None,
                    active_workers: 4,
                });
                tokio::task::yield_now().await;
                tx.send(DownloadSnapshot {
                    downloaded: 4096,
                    total: Some(4096),
                    active_workers: 1,
                });
                "ok"
            });

            let mut snapshots = Vec::new();
            let result = task
                .observe(|snap| {
                    snapshots.push(snap);
                })
                .await;

            assert_eq!(result, "ok");
            assert_eq!(
                snapshots.last().copied(),
                Some(DownloadSnapshot {
                    downloaded: 4096,
                    total: Some(4096),
                    active_workers: 1,
                })
            );
        }
    }
}
