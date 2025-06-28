//! A modern, async-first progress tracking library for Rust.
//!
//! This crate provides types and utilities for tracking progress of long-running operations
//! in an async context. It uses Rust's `Stream` API to emit progress updates.
//!
//! # Examples
//!
//! ```
//! use progressor::{Progress, ProgressUpdate};
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
/// including the current value, total value, cancellation status, and an optional message.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProgressUpdate {
    /// The current progress value (e.g., bytes downloaded, items processed).
    pub current: u64,
    /// The total expected value when the operation will be complete.
    pub total: u64,
    /// Whether the operation has been cancelled.
    pub is_cancelled: bool,
    /// An optional descriptive message about the current progress.
    pub message: Option<String>,
}

impl ProgressUpdate {
    /// Creates a new progress update with the given total.
    ///
    /// The current progress is initialized to 0, and the operation is not cancelled.
    /// No message is set initially.
    #[must_use]
    pub const fn new(total: u64) -> Self {
        Self {
            current: 0,
            total,
            is_cancelled: false,
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

    /// Sets the cancellation status.
    ///
    /// This is a builder method that consumes and returns `self`.
    #[must_use]
    pub const fn with_cancelled(mut self, cancelled: bool) -> Self {
        self.is_cancelled = cancelled;
        self
    }

    /// Returns `true` if the current progress is greater than or equal to the total.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.current >= self.total
    }

    /// Returns the remaining progress (total - current).
    ///
    /// Uses saturating subtraction, so if current > total, returns 0.
    #[must_use]
    pub const fn remaining(&self) -> u64 {
        self.total.saturating_sub(self.current)
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
        assert!(!update.is_cancelled);
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
            .with_cancelled(false);

        assert_eq!(update.current, 50);
        assert_eq!(update.message, Some("Half complete".to_string()));
        assert!(!update.is_cancelled);
    }

    #[test]
    fn test_is_complete() {
        let mut update = ProgressUpdate::new(100);
        assert!(!update.is_complete());

        update.current = 100;
        assert!(update.is_complete());

        update.current = 150; // exceeding total should also be considered complete
        assert!(update.is_complete());
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
