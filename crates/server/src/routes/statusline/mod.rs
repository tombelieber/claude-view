//! POST /api/live/statusline -- receive per-turn statusline JSON from Claude Code.
//!
//! Claude Code pipes the full statusline JSON to our wrapper script on every
//! assistant turn. The wrapper forwards it here. We extract ground-truth fields
//! that can't be reliably derived from JSONL parsing:
//!   - context_window.context_window_size  (real max: 200K or 1M)
//!   - context_window.used_percentage      (authoritative %, no math needed)
//!   - context_window.current_usage        (current turn input tokens)
//!   - cost.total_cost_usd                 (Claude Code's own cost calculation)
//!   - model.id                            (current model, catches mid-session switches)

mod apply;
mod handler;
mod types;

#[cfg(test)]
mod tests;

// Re-export public API so external `use crate::routes::statusline::*` paths
// continue to resolve without change.
pub use apply::apply_statusline;
pub use handler::{__path_handle_statusline, handle_statusline, router};
pub use types::*;
