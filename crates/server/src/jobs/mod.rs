// crates/server/src/jobs/mod.rs
//! Background job system for long-running async tasks.
//!
//! Provides:
//! - `JobRunner` — central manager for spawning and tracking jobs
//! - `JobState` — atomic progress tracking per job
//! - `JobHandle` — cancellation handle
//! - `JobProgress` — SSE-compatible progress updates

pub mod runner;
pub mod state;
pub mod types;

pub use runner::JobRunner;
pub use state::JobState;
pub use types::{JobHandle, JobId, JobProgress, JobStatus};
