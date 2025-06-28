//! A modern, async-first progress tracking library for Rust.
//!
//! This crate provides types and utilities for tracking progress of long-running operations
//! in an async context. It uses Rust's `Stream` API to emit progress updates with support for
//! different states (working, paused, completed, cancelled).
//!
//! # Features
//!
//! - **State-aware progress tracking**: Support for working, paused, completed, and cancelled states
//! - **Async-first design**: Built around `Future` and `Stream` APIs
//! - **Zero-cost abstractions**: Efficient progress reporting with minimal overhead
//! - **Type-safe**: Full Rust type safety with meaningful error messages
//! - **Flexible**: Support for current/total values, custom messages, and state management
//!
//! # Examples
//!
//! Basic usage with progress updates:
//!
//! ```
//! use progressor::{Progress, ProgressUpdate, State};
//! use futures_util::StreamExt;
//!
//! # async fn example() {
//! // Create a progress update
//! let update = ProgressUpdate::new(100)
//!     .with_current(50)
//!     .with_message("Half complete");
//!
//! assert_eq!(update.completed_fraction(), 0.5);
//! assert_eq!(update.remaining(), 50);
//! assert_eq!(update.state, State::Working);
//! # }
//! ```
//!
//! Using the progress tracker with a long-running task:
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
//!         // Check if we should pause or cancel
//!         if i == 50 {
//!             updater.pause();
//!             // Resume later...
//!         }
//!     }
//!     "Task completed!"
//! });
//!
//! // Monitor progress concurrently
//! let mut progress_stream = Box::pin(task.progress());
//! tokio::select! {
//!     result = task => {
//!         println!("Result: {}", result);
//!     }
//!     _ = async {
//!         while let Some(update) = progress_stream.next().await {
//!             println!("Progress: {}%", (update.completed_fraction() * 100.0) as u32);
//!             if update.state.is_paused() {
//!                 println!("Task is paused");
//!             }
//!         }
//!     } => {}
//! }
//! # }
//! # }
//! ```

#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "std")]
mod updater;

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
pub use updater::{progress, ProgressUpdater};

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
    fn progress(&self) -> impl Stream<Item = ProgressUpdate> + Send + 'static;
}

/// Represents a single progress update with current status, total, and optional metadata.
///
/// This struct contains all the information about the current state of a progress-tracked operation,
/// including the current value, total value, execution state, and an optional message.
///
/// # Examples
///
/// ```
/// use progressor::{ProgressUpdate, State};
///
/// let mut update = ProgressUpdate::new(100)
///     .with_current(25)
///     .with_message("Processing...");
///
/// assert_eq!(update.current, 25);
/// assert_eq!(update.total, 100);
/// assert_eq!(update.state, State::Working);
/// assert_eq!(update.completed_fraction(), 0.25);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProgressUpdate {
    /// The current progress value (e.g., bytes downloaded, items processed).
    pub current: u64,
    /// The total expected value when the operation will be complete.
    pub total: u64,
    /// The current state of the progress-tracked operation.
    pub state: State,
    /// An optional descriptive message about the current progress.
    pub message: Option<String>,
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
    /// Creates a new progress update with the given total.
    ///
    /// The current progress is initialized to 0, the state is set to [`State::Working`],
    /// and no message is set initially.
    ///
    /// # Examples
    ///
    /// ```
    /// use progressor::{ProgressUpdate, State};
    ///
    /// let update = ProgressUpdate::new(100);
    /// assert_eq!(update.current, 0);
    /// assert_eq!(update.total, 100);
    /// assert_eq!(update.state, State::Working);
    /// assert_eq!(update.message, None);
    /// ```
    #[must_use]
    pub const fn new(total: u64) -> Self {
        Self {
            current: 0,
            total,
            state: State::Working,
            message: None,
        }
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

    /// Sets the current progress value.
    ///
    /// This is a builder method that consumes and returns `self`.
    #[must_use]
    pub const fn with_current(mut self, current: u64) -> Self {
        self.current = current;
        self
    }

    /// Sets an optional message describing the current progress.
    ///
    /// This is a builder method that consumes and returns `self`.
    #[must_use]
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Sets the state of the progress update.
    ///
    /// This is a builder method that consumes and returns `self`.
    #[must_use]
    pub const fn with_state(mut self, state: State) -> Self {
        self.state = state;
        self
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_update_new() {
        let update = ProgressUpdate::new(100);
        assert_eq!(update.current, 0);
        assert_eq!(update.total, 100);
        assert!(!update.is_cancelled());
        assert_eq!(update.message, None);
    }

    #[test]
    fn test_completed_fraction() {
        let mut update = ProgressUpdate::new(100);
        assert!((update.completed_fraction() - 0.0).abs() < f64::EPSILON);

        update.current = 50;
        assert!((update.completed_fraction() - 0.5).abs() < f64::EPSILON);

        update.current = 100;
        assert!((update.completed_fraction() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_completed_fraction_zero_total() {
        let update = ProgressUpdate::new(0);
        assert!((update.completed_fraction() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_builder_methods() {
        let update = ProgressUpdate::new(100)
            .with_current(50)
            .with_message("Half complete")
            .with_state(State::Working);

        assert_eq!(update.current, 50);
        assert_eq!(update.message, Some("Half complete".to_string()));
        assert_eq!(update.state, State::Working);
        assert!(update.is_working());
        assert!(!update.is_cancelled());
        assert!(!update.is_completed());
        assert!(!update.is_paused());
    }

    #[test]
    fn test_is_complete() {
        let mut update = ProgressUpdate::new(100);
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
        let mut update = ProgressUpdate::new(100);
        assert_eq!(update.remaining(), 100);

        update.current = 30;
        assert_eq!(update.remaining(), 70);

        update.current = 100;
        assert_eq!(update.remaining(), 0);

        update.current = 150; // when exceeding total should return 0
        assert_eq!(update.remaining(), 0);
    }
}
