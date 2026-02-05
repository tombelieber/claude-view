// crates/server/src/classify_state.rs
//! Lock-free atomic state for classification progress tracking.
//!
//! Used by the SSE stream and status endpoints to report real-time
//! classification progress without blocking the server.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::RwLock;

/// Status of the current classification job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ClassifyStatus {
    Idle = 0,
    Running = 1,
    Completed = 2,
    Failed = 3,
    Cancelled = 4,
}

impl ClassifyStatus {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Idle,
            1 => Self::Running,
            2 => Self::Completed,
            3 => Self::Failed,
            4 => Self::Cancelled,
            _ => Self::Idle,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

/// Lock-free state for classification progress tracking.
///
/// All numeric fields use atomics for wait-free reads from the SSE handler.
/// Only the string fields (job_id, current_batch, error_message) use RwLock.
pub struct ClassifyState {
    status: AtomicU8,
    total: AtomicU64,
    classified: AtomicU64,
    errors: AtomicU64,
    cancel_requested: AtomicBool,
    job_id: RwLock<Option<String>>,
    db_job_id: AtomicU64,
    current_batch: RwLock<Option<String>>,
    error_message: RwLock<Option<String>>,
    started_at: AtomicU64,
    cost_cents: AtomicU64,
}

impl ClassifyState {
    /// Create a new idle classify state.
    pub fn new() -> Self {
        Self {
            status: AtomicU8::new(ClassifyStatus::Idle as u8),
            total: AtomicU64::new(0),
            classified: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            cancel_requested: AtomicBool::new(false),
            job_id: RwLock::new(None),
            db_job_id: AtomicU64::new(0),
            current_batch: RwLock::new(None),
            error_message: RwLock::new(None),
            started_at: AtomicU64::new(0),
            cost_cents: AtomicU64::new(0),
        }
    }

    /// Get the current status.
    pub fn status(&self) -> ClassifyStatus {
        ClassifyStatus::from_u8(self.status.load(Ordering::Relaxed))
    }

    /// Get the total number of sessions to classify.
    pub fn total(&self) -> u64 {
        self.total.load(Ordering::Relaxed)
    }

    /// Get the number of sessions classified so far.
    pub fn classified(&self) -> u64 {
        self.classified.load(Ordering::Relaxed)
    }

    /// Get the number of errors so far.
    pub fn errors(&self) -> u64 {
        self.errors.load(Ordering::Relaxed)
    }

    /// Get the database job ID (0 if none).
    pub fn db_job_id(&self) -> i64 {
        self.db_job_id.load(Ordering::Relaxed) as i64
    }

    /// Get the job ID string.
    pub fn job_id(&self) -> Option<String> {
        self.job_id.read().ok().and_then(|g| g.clone())
    }

    /// Get the current batch description.
    pub fn current_batch(&self) -> Option<String> {
        self.current_batch.read().ok().and_then(|g| g.clone())
    }

    /// Get the error message.
    pub fn error_message(&self) -> Option<String> {
        self.error_message.read().ok().and_then(|g| g.clone())
    }

    /// Get the start timestamp (unix seconds).
    pub fn started_at(&self) -> u64 {
        self.started_at.load(Ordering::Relaxed)
    }

    /// Get the accumulated cost in cents.
    pub fn cost_cents(&self) -> u64 {
        self.cost_cents.load(Ordering::Relaxed)
    }

    /// Transition to running state.
    pub fn set_running(&self, job_id: String, db_job_id: i64, total: u64) {
        self.status.store(ClassifyStatus::Running as u8, Ordering::Relaxed);
        self.total.store(total, Ordering::Relaxed);
        self.classified.store(0, Ordering::Relaxed);
        self.errors.store(0, Ordering::Relaxed);
        self.cancel_requested.store(false, Ordering::Relaxed);
        self.db_job_id.store(db_job_id as u64, Ordering::Relaxed);
        self.cost_cents.store(0, Ordering::Relaxed);
        self.started_at.store(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            Ordering::Relaxed,
        );
        if let Ok(mut guard) = self.job_id.write() {
            *guard = Some(job_id);
        }
        if let Ok(mut guard) = self.current_batch.write() {
            *guard = None;
        }
        if let Ok(mut guard) = self.error_message.write() {
            *guard = None;
        }
    }

    /// Increment the classified count by `count`.
    pub fn increment_classified(&self, count: u64) {
        self.classified.fetch_add(count, Ordering::Relaxed);
    }

    /// Increment the error count.
    pub fn increment_errors(&self) {
        self.errors.fetch_add(1, Ordering::Relaxed);
    }

    /// Set the current batch description.
    pub fn set_current_batch(&self, batch: String) {
        if let Ok(mut guard) = self.current_batch.write() {
            *guard = Some(batch);
        }
    }

    /// Add to accumulated cost.
    pub fn add_cost_cents(&self, cents: u64) {
        self.cost_cents.fetch_add(cents, Ordering::Relaxed);
    }

    /// Mark as completed.
    pub fn set_completed(&self) {
        self.status.store(ClassifyStatus::Completed as u8, Ordering::Relaxed);
    }

    /// Mark as failed.
    pub fn set_failed(&self, message: String) {
        self.status.store(ClassifyStatus::Failed as u8, Ordering::Relaxed);
        if let Ok(mut guard) = self.error_message.write() {
            *guard = Some(message);
        }
    }

    /// Mark as cancelled.
    pub fn set_cancelled(&self) {
        self.status.store(ClassifyStatus::Cancelled as u8, Ordering::Relaxed);
    }

    /// Check if cancellation has been requested.
    pub fn is_cancel_requested(&self) -> bool {
        self.cancel_requested.load(Ordering::Relaxed)
    }

    /// Request cancellation.
    pub fn request_cancel(&self) {
        self.cancel_requested.store(true, Ordering::Relaxed);
    }

    /// Reset to idle state.
    pub fn reset(&self) {
        self.status.store(ClassifyStatus::Idle as u8, Ordering::Relaxed);
        self.total.store(0, Ordering::Relaxed);
        self.classified.store(0, Ordering::Relaxed);
        self.errors.store(0, Ordering::Relaxed);
        self.cancel_requested.store(false, Ordering::Relaxed);
        self.db_job_id.store(0, Ordering::Relaxed);
        self.cost_cents.store(0, Ordering::Relaxed);
        self.started_at.store(0, Ordering::Relaxed);
        if let Ok(mut guard) = self.job_id.write() {
            *guard = None;
        }
        if let Ok(mut guard) = self.current_batch.write() {
            *guard = None;
        }
        if let Ok(mut guard) = self.error_message.write() {
            *guard = None;
        }
    }

    /// Calculate the progress percentage (0.0 - 100.0).
    pub fn percentage(&self) -> f64 {
        let total = self.total();
        if total == 0 {
            return 0.0;
        }
        (self.classified() as f64 / total as f64) * 100.0
    }

    /// Estimate remaining time in seconds based on current progress rate.
    pub fn eta_secs(&self) -> Option<u64> {
        let classified = self.classified();
        let total = self.total();
        if classified == 0 || total == 0 {
            return None;
        }

        let elapsed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .saturating_sub(self.started_at());

        if elapsed == 0 {
            return None;
        }

        let rate = classified as f64 / elapsed as f64; // sessions/sec
        let remaining = total.saturating_sub(classified);
        Some((remaining as f64 / rate) as u64)
    }

    /// Format ETA as human-readable string (e.g. "6m 42s").
    pub fn eta_string(&self) -> String {
        match self.eta_secs() {
            Some(secs) if secs >= 60 => {
                let mins = secs / 60;
                let rem_secs = secs % 60;
                format!("{}m {}s", mins, rem_secs)
            }
            Some(secs) => format!("{}s", secs),
            None => "calculating...".to_string(),
        }
    }
}

impl Default for ClassifyState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_state_lifecycle() {
        let state = ClassifyState::new();

        // Initially idle
        assert_eq!(state.status(), ClassifyStatus::Idle);
        assert_eq!(state.total(), 0);
        assert_eq!(state.classified(), 0);

        // Set running
        state.set_running("job-1".to_string(), 42, 100);
        assert_eq!(state.status(), ClassifyStatus::Running);
        assert_eq!(state.total(), 100);
        assert_eq!(state.classified(), 0);
        assert_eq!(state.job_id(), Some("job-1".to_string()));
        assert_eq!(state.db_job_id(), 42);

        // Increment progress
        state.increment_classified(5);
        assert_eq!(state.classified(), 5);
        assert!((state.percentage() - 5.0).abs() < 0.01);

        // Set batch
        state.set_current_batch("Jan 1-5, 2026".to_string());
        assert_eq!(state.current_batch(), Some("Jan 1-5, 2026".to_string()));

        // Complete
        state.set_completed();
        assert_eq!(state.status(), ClassifyStatus::Completed);
    }

    #[test]
    fn test_classify_state_cancellation() {
        let state = ClassifyState::new();
        state.set_running("job-2".to_string(), 1, 50);

        assert!(!state.is_cancel_requested());
        state.request_cancel();
        assert!(state.is_cancel_requested());

        state.set_cancelled();
        assert_eq!(state.status(), ClassifyStatus::Cancelled);
    }

    #[test]
    fn test_classify_state_failure() {
        let state = ClassifyState::new();
        state.set_running("job-3".to_string(), 1, 50);

        state.set_failed("Connection timeout".to_string());
        assert_eq!(state.status(), ClassifyStatus::Failed);
        assert_eq!(state.error_message(), Some("Connection timeout".to_string()));
    }

    #[test]
    fn test_classify_state_reset() {
        let state = ClassifyState::new();
        state.set_running("job-4".to_string(), 1, 50);
        state.increment_classified(25);
        state.set_completed();

        state.reset();
        assert_eq!(state.status(), ClassifyStatus::Idle);
        assert_eq!(state.total(), 0);
        assert_eq!(state.classified(), 0);
        assert_eq!(state.job_id(), None);
    }

    #[test]
    fn test_percentage_zero_total() {
        let state = ClassifyState::new();
        assert_eq!(state.percentage(), 0.0);
    }
}
