// crates/server/src/jobs/state.rs
//! Atomic state tracking for a single background job.

use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::RwLock;
use tokio::sync::broadcast;

use super::types::{JobId, JobProgress, JobStatus};

/// Atomic state for a single job.
///
/// All fields use lock-free atomics (except `message` which uses a RwLock)
/// so progress updates don't block the main server thread.
pub struct JobState {
    id: JobId,
    job_type: String,
    status: AtomicU8,
    current: AtomicU64,
    total: AtomicU64,
    message: RwLock<Option<String>>,
    progress_tx: broadcast::Sender<JobProgress>,
}

impl JobState {
    /// Create a new job state with the given ID, type, and total count.
    pub fn new(id: JobId, job_type: String, total: u64) -> Self {
        let (progress_tx, _) = broadcast::channel(64);
        Self {
            id,
            job_type,
            status: AtomicU8::new(JobStatus::Pending as u8),
            current: AtomicU64::new(0),
            total: AtomicU64::new(total),
            message: RwLock::new(None),
            progress_tx,
        }
    }

    /// Transition the job to Running status.
    pub fn set_running(&self) {
        self.status
            .store(JobStatus::Running as u8, Ordering::Relaxed);
        self.broadcast_progress();
    }

    /// Increment the progress counter and broadcast an update.
    /// Returns the new current value.
    pub fn increment(&self) -> u64 {
        let new = self.current.fetch_add(1, Ordering::Relaxed) + 1;
        self.broadcast_progress();
        new
    }

    /// Set the human-readable progress message and broadcast.
    pub fn set_message(&self, msg: impl Into<String>) {
        match self.message.write() {
            Ok(mut guard) => *guard = Some(msg.into()),
            Err(e) => tracing::error!("RwLock poisoned writing message: {e}"),
        }
        self.broadcast_progress();
    }

    /// Mark the job as completed.
    pub fn complete(&self) {
        self.status
            .store(JobStatus::Completed as u8, Ordering::Relaxed);
        self.broadcast_progress();
    }

    /// Mark the job as failed with an error message.
    pub fn fail(&self, error: impl Into<String>) {
        self.status
            .store(JobStatus::Failed as u8, Ordering::Relaxed);
        match self.message.write() {
            Ok(mut guard) => *guard = Some(error.into()),
            Err(e) => tracing::error!("RwLock poisoned writing error message: {e}"),
        }
        self.broadcast_progress();
    }

    /// Subscribe to progress updates for this specific job.
    pub fn subscribe(&self) -> broadcast::Receiver<JobProgress> {
        self.progress_tx.subscribe()
    }

    /// Get a snapshot of the current job state.
    pub fn snapshot(&self) -> JobProgress {
        JobProgress {
            job_id: self.id,
            job_type: self.job_type.clone(),
            status: self.status_string(),
            current: self.current.load(Ordering::Relaxed),
            total: self.total.load(Ordering::Relaxed),
            message: match self.message.read() {
                Ok(g) => g.clone(),
                Err(e) => {
                    tracing::error!("RwLock poisoned reading message: {e}");
                    None
                }
            },
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    fn broadcast_progress(&self) {
        let progress = self.snapshot();
        // Ignore send errors (no subscribers is fine).
        let _ = self.progress_tx.send(progress);
    }

    fn status_string(&self) -> String {
        match self.status.load(Ordering::Relaxed) {
            0 => "pending".into(),
            1 => "running".into(),
            2 => "completed".into(),
            3 => "cancelled".into(),
            _ => "failed".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_state_lifecycle() {
        let state = JobState::new(1, "test".to_string(), 100);

        // Initial state
        let snap = state.snapshot();
        assert_eq!(snap.status, "pending");
        assert_eq!(snap.current, 0);
        assert_eq!(snap.total, 100);

        // Running
        state.set_running();
        assert_eq!(state.snapshot().status, "running");

        // Increment progress
        assert_eq!(state.increment(), 1);
        assert_eq!(state.increment(), 2);
        assert_eq!(state.snapshot().current, 2);

        // Set message
        state.set_message("Processing batch 1...");
        assert_eq!(
            state.snapshot().message,
            Some("Processing batch 1...".to_string())
        );

        // Complete
        state.complete();
        assert_eq!(state.snapshot().status, "completed");
    }

    #[test]
    fn test_job_state_failure() {
        let state = JobState::new(2, "test".to_string(), 50);
        state.set_running();
        state.fail("Connection timeout");
        assert_eq!(state.snapshot().status, "failed");
        assert_eq!(
            state.snapshot().message,
            Some("Connection timeout".to_string())
        );
    }

    #[tokio::test]
    async fn test_job_state_subscribe() {
        let state = JobState::new(3, "test".to_string(), 10);
        let mut rx = state.subscribe();

        state.set_running();

        let progress = rx.recv().await.unwrap();
        assert_eq!(progress.status, "running");
        assert_eq!(progress.job_id, 3);
    }
}
