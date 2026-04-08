//! Coaching rules API routes.
//!
//! Manages coaching rule files in `~/.claude/rules/coaching-*.md`.
//! Rules are generated from behavioral pattern insights and written
//! as Markdown files that Claude Code can pick up as custom instructions.
//!
//! - GET    /coaching/rules      -- List all coaching rules
//! - POST   /coaching/rules      -- Apply (create) a coaching rule
//! - DELETE  /coaching/rules/{id} -- Remove a coaching rule

mod handlers;
mod types;

#[cfg(test)]
mod tests;

use axum::{
    routing::{delete, get, post},
    Router,
};
use std::sync::Arc;

use crate::state::AppState;

// Re-export all public types.
pub use types::{ApplyRuleRequest, CoachingRule, ListRulesResponse, RemoveRuleResponse};

// Re-export handlers for external use.
pub use handlers::{apply_rule, list_rules, remove_rule};

// Re-export utoipa hidden path types.
pub use handlers::__path_apply_rule;
pub use handlers::__path_list_rules;
pub use handlers::__path_remove_rule;

// ============================================================================
// Constants
// ============================================================================

/// Maximum number of coaching rules allowed at once.
const MAX_RULES: usize = 8;

// ============================================================================
// Router
// ============================================================================

/// Coaching rules API router.
///
/// ## Dependencies
/// - `state.rules_dir` — filesystem path for rule storage (`PathBuf`)
///
/// **ISP profile:** Minimal — single field access across all 3 handlers.
/// Target trait: `CoachingDeps` (Phase 3).
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/coaching/rules", get(handlers::list_rules))
        .route("/coaching/rules", post(handlers::apply_rule))
        .route("/coaching/rules/{id}", delete(handlers::remove_rule))
}
