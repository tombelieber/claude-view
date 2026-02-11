// crates/server/src/jobs/types.rs
//! Types for the background job system.

use serde::Serialize;
use tokio::sync::oneshot;

/// Unique identifier for a running job.
pub type JobId = u64;

/// Status of a background job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobStatus {
    Pending = 0,
    Running = 1,
    Completed = 2,
    Cancelled = 3,
    Failed = 4,
}

/// Handle to a running job, used for cancellation.
pub struct JobHandle {
    pub id: JobId,
    cancel_tx: Option<oneshot::Sender<()>>,
}

impl JobHandle {
    pub(crate) fn new(id: JobId, cancel_tx: oneshot::Sender<()>) -> Self {
        Self {
            id,
            cancel_tx: Some(cancel_tx),
        }
    }

    /// Cancel the job. Returns true if the cancellation signal was sent.
    pub fn cancel(mut self) -> bool {
        if let Some(tx) = self.cancel_tx.take() {
            tx.send(()).is_ok()
        } else {
            false
        }
    }
}

/// Progress update sent via SSE.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobProgress {
    pub job_id: JobId,
    pub job_type: String,
    pub status: String,
    pub current: u64,
    pub total: u64,
    pub message: Option<String>,
    pub timestamp: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_status_discriminants() {
        assert_eq!(JobStatus::Pending as u8, 0);
        assert_eq!(JobStatus::Running as u8, 1);
        assert_eq!(JobStatus::Completed as u8, 2);
        assert_eq!(JobStatus::Cancelled as u8, 3);
        assert_eq!(JobStatus::Failed as u8, 4);
    }

    #[test]
    fn test_job_handle_cancel() {
        let (tx, mut rx) = oneshot::channel();
        let handle = JobHandle::new(1, tx);
        assert!(handle.cancel());
        // rx should receive the cancellation
        assert!(rx.try_recv().is_ok());
    }

    #[test]
    fn test_job_progress_serialize() {
        let progress = JobProgress {
            job_id: 1,
            job_type: "classification".to_string(),
            status: "running".to_string(),
            current: 50,
            total: 100,
            message: Some("Processing sessions...".to_string()),
            timestamp: "2026-02-05T12:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&progress).unwrap();
        assert!(json.contains("\"jobId\":1"));
        assert!(json.contains("\"jobType\":\"classification\""));
        assert!(json.contains("\"current\":50"));
    }
}
