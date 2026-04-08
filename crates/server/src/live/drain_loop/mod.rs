//! Smart drain loop for oMLX phase classification.
//!
//! Two mechanisms eliminate 93% of wasted classify calls (proven by .debug/omlx.jsonl):
//!
//! 1. **Exponential backoff:** Same phase result -> double budget (5s->10s->20s->40s->60s).
//!    Phase change or user-turn signal -> reset to 5s. Naturally prioritises dynamic
//!    sessions (short budget = ready sooner in round-robin).
//!
//! 2. **Lifecycle gate:** NeedsYou sessions get ONE final classify then freeze.
//!    Only Running (Autonomous) sessions actively classify.
//!
//! User `ClassifyMode` applies a budget multiplier (0.5x/1.0x/2.0x).
//! `avg_latency_ms` EMA is tracked for future auto-tune (not yet used).

mod loop_runner;
mod state;
mod types;

#[cfg(test)]
mod tests;

pub(crate) use loop_runner::run_drain_loop;
pub(crate) use types::DirtySignal;
