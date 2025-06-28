#[cfg(feature = "std")]
pub mod updater;

use core::future::Future;
use futures_core::Stream;

pub trait Progress: Future {
    fn progress(&self) -> impl Stream<Item = ProgressUpdate> + Send + 'static;
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProgressUpdate {
    pub current: u64,
    pub total: u64,
    pub is_cancelled: bool,
    pub message: Option<String>,
}

impl ProgressUpdate {
    pub const fn new(total: u64) -> Self {
        Self {
            current: 0,
            total,
            is_cancelled: false,
            message: None,
        }
    }

    pub fn completed_fraction(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.current as f64 / self.total as f64
        }
    }

    pub fn with_current(mut self, current: u64) -> Self {
        self.current = current;
        self
    }

    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    pub fn with_cancelled(mut self, cancelled: bool) -> Self {
        self.is_cancelled = cancelled;
        self
    }

    pub fn is_complete(&self) -> bool {
        self.current >= self.total
    }

    pub fn remaining(&self) -> u64 {
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
        assert_eq!(update.completed_fraction(), 0.0);

        update.current = 50;
        assert_eq!(update.completed_fraction(), 0.5);

        update.current = 100;
        assert_eq!(update.completed_fraction(), 1.0);
    }

    #[test]
    fn test_completed_fraction_zero_total() {
        let update = ProgressUpdate::new(0);
        assert_eq!(update.completed_fraction(), 0.0);
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
