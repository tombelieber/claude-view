// crates/server/src/jobs/runner.rs
//! Central job runner that manages all background jobs.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use tokio::sync::{broadcast, oneshot};

use super::state::JobState;
use super::types::{JobHandle, JobId, JobProgress};

/// Central job runner that manages all background jobs.
///
/// Thread-safe via `Arc` wrapping. Call `start_job` to spawn async work
/// with progress tracking, and `subscribe` to get SSE-compatible updates.
pub struct JobRunner {
    next_id: AtomicU64,
    jobs: RwLock<HashMap<JobId, Arc<JobState>>>,
    global_tx: broadcast::Sender<JobProgress>,
}

impl JobRunner {
    /// Create a new job runner.
    pub fn new() -> Self {
        let (global_tx, _) = broadcast::channel(256);
        Self {
            next_id: AtomicU64::new(1),
            jobs: RwLock::new(HashMap::new()),
            global_tx,
        }
    }

    /// Start a new background job.
    ///
    /// The closure `f` receives:
    /// - `Arc<JobState>` for reporting progress
    /// - `oneshot::Receiver<()>` for cancellation detection
    ///
    /// Returns a `JobHandle` that can be used to cancel the job.
    pub fn start_job<F, Fut>(
        &self,
        job_type: impl Into<String>,
        total: u64,
        f: F,
    ) -> JobHandle
    where
        F: FnOnce(Arc<JobState>, oneshot::Receiver<()>) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<(), String>> + Send + 'static,
    {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let state = Arc::new(JobState::new(id, job_type.into(), total));

        // Store state
        match self.jobs.write() {
            Ok(mut jobs) => { jobs.insert(id, Arc::clone(&state)); }
            Err(e) => tracing::error!("RwLock poisoned writing jobs map: {e}"),
        }

        // Create cancellation channel
        let (cancel_tx, cancel_rx) = oneshot::channel();

        // Forward job progress to global channel
        let global_tx = self.global_tx.clone();
        let state_clone = Arc::clone(&state);
        tokio::spawn(async move {
            let mut rx = state_clone.subscribe();
            while let Ok(progress) = rx.recv().await {
                let _ = global_tx.send(progress);
            }
        });

        // Spawn the job
        let state_for_task = Arc::clone(&state);
        tokio::spawn(async move {
            state_for_task.set_running();
            match f(state_for_task.clone(), cancel_rx).await {
                Ok(()) => state_for_task.complete(),
                Err(e) => state_for_task.fail(e),
            }
        });

        JobHandle::new(id, cancel_tx)
    }

    /// Subscribe to all job progress updates (for SSE streaming).
    pub fn subscribe(&self) -> broadcast::Receiver<JobProgress> {
        self.global_tx.subscribe()
    }

    /// Get current status of a specific job.
    pub fn get_job(&self, id: JobId) -> Option<JobProgress> {
        match self.jobs.read() {
            Ok(jobs) => jobs.get(&id).map(|s| s.snapshot()),
            Err(e) => {
                tracing::error!("RwLock poisoned reading jobs map: {e}");
                None
            }
        }
    }

    /// Get all active (non-completed) jobs.
    pub fn active_jobs(&self) -> Vec<JobProgress> {
        match self.jobs.read() {
            Ok(jobs) => jobs
                .values()
                .map(|s| s.snapshot())
                .filter(|p| p.status != "completed" && p.status != "failed" && p.status != "cancelled")
                .collect(),
            Err(e) => {
                tracing::error!("RwLock poisoned reading jobs: {e}");
                Vec::new()
            }
        }
    }
}

impl Default for JobRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_job_runner_start_and_complete() {
        let runner = JobRunner::new();

        let handle = runner.start_job("test", 10, |state, _cancel_rx| async move {
            for _ in 0..10 {
                state.increment();
            }
            Ok(())
        });

        // Wait for the job to complete
        tokio::time::sleep(Duration::from_millis(100)).await;

        let status = runner.get_job(handle.id);
        assert!(status.is_some());
        let progress = status.unwrap();
        assert_eq!(progress.status, "completed");
        assert_eq!(progress.current, 10);
    }

    #[tokio::test]
    async fn test_job_runner_failure() {
        let runner = JobRunner::new();

        let handle = runner.start_job("test", 5, |_state, _cancel_rx| async move {
            Err("something went wrong".to_string())
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        let progress = runner.get_job(handle.id).unwrap();
        assert_eq!(progress.status, "failed");
    }

    #[tokio::test]
    async fn test_job_runner_cancellation() {
        let runner = JobRunner::new();

        let handle = runner.start_job("test", 100, |state, cancel_rx| async move {
            tokio::select! {
                _ = cancel_rx => {
                    state.set_message("Cancelled by user");
                    return Err("cancelled".to_string());
                }
                _ = async {
                    loop {
                        state.increment();
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                } => {}
            }
            Ok(())
        });

        // Let it run a bit
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Cancel it
        let id = handle.id;
        assert!(handle.cancel());

        tokio::time::sleep(Duration::from_millis(50)).await;

        let progress = runner.get_job(id).unwrap();
        assert_eq!(progress.status, "failed"); // Failed due to Err("cancelled")
    }

    #[tokio::test]
    async fn test_job_runner_active_jobs() {
        let runner = JobRunner::new();

        let _handle = runner.start_job("test", 100, |_state, _cancel_rx| async move {
            tokio::time::sleep(Duration::from_secs(10)).await;
            Ok(())
        });

        tokio::time::sleep(Duration::from_millis(50)).await;

        let active = runner.active_jobs();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].job_type, "test");
    }

    #[tokio::test]
    async fn test_job_runner_subscribe() {
        let runner = JobRunner::new();
        let mut rx = runner.subscribe();

        let _handle = runner.start_job("test", 5, |state, _cancel_rx| async move {
            state.increment();
            Ok(())
        });

        // Should receive at least one progress update
        let progress = tokio::time::timeout(Duration::from_millis(500), rx.recv())
            .await
            .expect("timeout waiting for progress")
            .expect("channel error");

        assert_eq!(progress.job_type, "test");
    }

    #[test]
    fn test_job_runner_default() {
        let runner = JobRunner::default();
        assert!(runner.active_jobs().is_empty());
    }
}
