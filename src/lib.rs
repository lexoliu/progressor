//! A modern, async-first progress tracking library for Rust.
//!
//! This crate provides types and utilities for tracking progress of long-running operations
//! in an async context. It uses Rust's `Stream` API to emit progress updates with support for
//! different states (working, paused, completed, cancelled).
//!
//! # Features
//!
//! - **Async-first**: Built around Rust's async/await and Stream APIs
//! - **Zero-allocation progress updates**: Efficient progress reporting
//! - **Flexible progress tracking**: Support for current/total, messages, and cancellation
//! - **Type-safe**: Full Rust type safety with meaningful error messages
//! - **Lightweight**: Minimal dependencies and fast compilation
//! - **Convenient observing**: Extension methods for easy progress monitoring
//!
//! # Examples
//!
//! ## Using the observe extension (recommended)
//!
//! The [`ProgressExt::observe`] method provides a convenient way to monitor progress
//! without manually managing streams and select macros:
//!
//! ```
//! # #[cfg(feature = "std")]
//! # {
//! use progressor::{progress, ProgressExt};
//!
//! # async fn example() {
//! let result = progress(100, |mut updater| async move {
//!     for i in 0..=100 {
//!         // Update progress
//!         updater.update(i);
//!         
//!         // Add messages for important milestones
//!         if i % 25 == 0 {
//!             updater.update_with_message(i, format!("Milestone: {}%", i));
//!         }
//!     }
//!     "Task completed!"
//! })
//! .observe(|update| {
//!     println!("Progress: {}%", (update.completed_fraction() * 100.0) as u32);
//!     if let Some(message) = update.message() {
//!         println!("  {}", message);
//!     }
//! })
//! .await;
//!
//! println!("Result: {}", result);
//! # }
//! # }
//! ```
//!
//! ## Manual stream monitoring with `tokio::select!`
//!
//! For more control, you can manually monitor the progress stream:
//!
//! ```
//! # #[cfg(feature = "std")]
//! # {
//! use progressor::{progress, Progress};
//! use futures_util::StreamExt;
//!
//! # async fn example() {
//! let task = progress(100, |mut updater| async move {
//!     for i in 0..=100 {
//!         // Update progress
//!         updater.update(i);
//!         
//!         // Add messages for important milestones
//!         if i % 25 == 0 {
//!             updater.update_with_message(i, format!("Milestone: {}%", i));
//!         }
//!     }
//!     "Task completed!"
//! });
//!
//! // Monitor progress concurrently
//! let mut progress_stream = task.progress();
//! tokio::select! {
//!     result = task => {
//!         println!("Result: {}", result);
//!     }
//!     _ = async {
//!         while let Some(update) = progress_stream.next().await {
//!             println!("Progress: {}%", (update.completed_fraction() * 100.0) as u32);
//!             if let Some(message) = update.message() {
//!                 println!("  {}", message);
//!             }
//!         }
//!     } => {}
//! }
//! # }
//! # }
//! ```
//!
//! ## Advanced usage with state handling
//!
//! Monitor different progress states and handle pause/cancel operations:
//!
//! ```
//! # #[cfg(feature = "std")]
//! # {
//! use progressor::{progress, ProgressExt, State};
//!
//! # async fn example() {
//! let result = progress(100, |mut updater| async move {
//!     for i in 0..=100 {
//!         // Update progress
//!         updater.update(i);
//!         
//!         // Pause at 50%
//!         if i == 50 {
//!             updater.pause();
//!             // Simulate some async work during pause
//!             tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
//!         }
//!     }
//!     "Task completed!"
//! })
//! .observe(|update| {
//!     match update.state() {
//!         State::Working => println!("Working: {}%", (update.completed_fraction() * 100.0) as u32),
//!         State::Paused => println!("Paused at {}%", (update.completed_fraction() * 100.0) as u32),
//!         State::Completed => println!("Completed!"),
//!         State::Cancelled => println!("Cancelled!"),
//!     }
//! })
//! .await;
//!
//! println!("Result: {}", result);
//! # }
//! # }
//! ```

#![cfg_attr(docsrs, feature(doc_cfg))]

mod ext;
pub use ext::ProgressExt;
#[cfg(feature = "std")]
mod updater;

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
pub use updater::{ProgressUpdater, progress};

use core::future::Future;
use futures_core::Stream;

/// A trait for futures that can report progress updates.
///
/// This trait extends [`Future`] to provide a method for accessing a stream of progress updates.
/// The progress updates are emitted as [`ProgressUpdate`] items through a [`Stream`].
pub trait Progress: Future {
    /// Returns a stream of progress updates for this operation.
    ///
    /// The stream will emit [`ProgressUpdate`] instances as the operation progresses.
    /// The stream should be polled concurrently with the future to receive updates.
    fn progress(&self) -> impl Stream<Item = ProgressUpdate> + Unpin + Send + 'static;
}

/// Represents a single progress update with current status, total, and optional metadata.
///
/// This struct contains all the information about the current state of a progress-tracked operation.
/// It is emitted by progress streams and provides methods to query the current progress state.
///
/// You typically don't create instances of this struct directly. Instead, use the [`progress`] function
/// to create progress-tracked tasks, and receive `ProgressUpdate` instances from the progress stream.
///
/// [`progress`]: crate::progress
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProgressUpdate {
    current: u64,
    total: u64,
    state: State,
    message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// Represents the state of a progress-tracked operation.
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
    ///
    /// This method is primarily used internally by the progress tracking system.
    /// Users should use the [`progress`] function instead of creating updates manually.
    ///
    /// [`progress`]: crate::progress
    #[must_use]
    pub const fn new(total: u64, current: u64, state: State, message: Option<String>) -> Self {
        Self {
            current,
            total,
            state,
            message,
        }
    }

    /// Returns the total expected value when the operation will be complete.
    #[must_use]
    pub const fn total(&self) -> u64 {
        self.total
    }

    /// Returns the current progress value.
    #[must_use]
    pub const fn current(&self) -> u64 {
        self.current
    }

    /// Returns the completion fraction as a value between 0.0 and 1.0.
    ///
    /// If the total is 0, returns 0.0. Otherwise, returns current/total.
    #[must_use]
    pub fn completed_fraction(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            #[allow(clippy::cast_precision_loss)]
            {
                self.current as f64 / self.total as f64
            }
        }
    }

    /// Returns the remaining progress (total - current).
    ///
    /// Uses saturating subtraction, so if current > total, returns 0.
    #[must_use]
    pub const fn remaining(&self) -> u64 {
        self.total.saturating_sub(self.current)
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
        let update = ProgressUpdate::new(100, 0, State::Working, None);
        assert_eq!(update.current(), 0);
        assert_eq!(update.total(), 100);
        assert!(!update.is_cancelled());
        assert_eq!(update.message(), None);
    }

    #[test]
    fn test_completed_fraction() {
        let mut update = ProgressUpdate::new(100, 0, State::Working, None);
        assert!((update.completed_fraction() - 0.0).abs() < f64::EPSILON);

        update.current = 50;
        assert!((update.completed_fraction() - 0.5).abs() < f64::EPSILON);

        update.current = 100;
        assert!((update.completed_fraction() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_completed_fraction_zero_total() {
        let update = ProgressUpdate::new(0, 0, State::Working, None);
        assert!((update.completed_fraction() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_builder_methods() {
        let update =
            ProgressUpdate::new(100, 50, State::Working, Some("Half complete".to_string()));

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
        let mut update = ProgressUpdate::new(100, 0, State::Working, None);
        assert!(!update.is_completed());
        assert!(update.is_working());

        // Setting state to completed
        update.state = State::Completed;
        assert!(update.is_completed());

        // Test other states
        update.state = State::Cancelled;
        assert!(update.is_cancelled());
        assert!(!update.is_completed());

        update.state = State::Paused;
        assert!(update.is_paused());
        assert!(!update.is_completed());
    }

    #[test]
    fn test_remaining() {
        let mut update = ProgressUpdate::new(100, 0, State::Working, None);
        assert_eq!(update.remaining(), 100);

        update.current = 30;
        assert_eq!(update.remaining(), 70);

        update.current = 100;
        assert_eq!(update.remaining(), 0);

        update.current = 150; // when exceeding total should return 0
        assert_eq!(update.remaining(), 0);
    }
}
