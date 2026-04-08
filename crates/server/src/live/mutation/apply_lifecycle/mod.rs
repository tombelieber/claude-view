//! Pure function: apply a LifecycleEvent to HookFields.
//!
//! Extracted from `routes/hooks.rs` — no IO, no locks, no broadcasts.
//! The coordinator calls this and then handles side effects separately.

mod apply;
mod helpers;

#[cfg(test)]
mod tests;

pub use apply::apply_lifecycle;
pub use helpers::finalize_orphaned_subagents;
pub use helpers::status_from_agent_state;
