//! SessionCoordinator — single entry point for all session state mutations.
//!
//! 4-phase pipeline:
//!   Phase 1: Buffer — park mutations for undiscovered sessions.
//!   Phase 2+3: Plan + Execute — plan side effects, apply mutation under write lock.
//!   Phase 3b: Execute side effects — IO after lock is dropped (RAII enforced).
//!   Phase 4: Broadcast — single `live_tx.send()` per mutation.
//!
//! Lock ordering (always respected):
//!   1. self.pending (tokio::sync::Mutex)
//!   2. ctx.sessions (tokio::sync::RwLock — write)
//!   3. ctx.transcript_to_session (tokio::sync::RwLock — write)

pub mod dispatch;
pub mod execution;
pub mod pipeline;
pub mod planning;
pub mod session_factory;
pub mod types;

#[cfg(test)]
mod tests;

// Re-export the public API — external code imports from `coordinator::`.
pub use pipeline::SessionCoordinator;
pub use types::MutationContext;
