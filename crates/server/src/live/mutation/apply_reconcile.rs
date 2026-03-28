//! Pure function: apply ReconcileData to JsonlFields.
//!
//! Snapshot overwrite with guards — only update fields where incoming
//! value is non-None. Prevents stale JSONL data from reverting richer
//! hook/statusline data.
//!
//! NOTE: model, model_display_name, context_window_tokens are cross-source
//! fields on top-level LiveSession — the coordinator handles them.
//! NOTE: turn_count lives in HookFields — the coordinator handles it.

use crate::live::mutation::types::ReconcileData;
use crate::live::state::JsonlFields;

/// Apply JSONL reconciliation data to the session's JSONL sub-struct.
///
/// Pure function — no IO, no locks. Only non-None fields are overwritten;
/// None means "no update from this reconciliation pass".
pub fn apply_reconcile(jsonl: &mut JsonlFields, data: &ReconcileData) {
    if let Some(ref p) = data.project {
        jsonl.project = p.clone();
    }
    if let Some(ref p) = data.project_display_name {
        jsonl.project_display_name = p.clone();
    }
    if let Some(ref p) = data.project_path {
        jsonl.project_path = p.clone();
    }
    if let Some(ref t) = data.tokens {
        jsonl.tokens = t.clone();
    }
    if let Some(ref c) = data.cost {
        jsonl.cost = c.clone();
    }
    if let Some(ec) = data.edit_count {
        jsonl.edit_count = ec;
    }
    if let Some(ref p) = data.phase {
        jsonl.phase = p.clone();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::state::JsonlFields;
    use claude_view_core::phase::PhaseHistory;
    use claude_view_core::pricing::{CostBreakdown, TokenUsage};

    fn make_reconcile_data() -> ReconcileData {
        ReconcileData {
            project: None,
            project_display_name: None,
            project_path: None,
            model: None,
            model_display_name: None,
            tokens: None,
            context_window_tokens: None,
            cost: None,
            turn_count: None,
            edit_count: None,
            phase: None,
        }
    }

    #[test]
    fn reconcile_updates_present_fields_only() {
        let mut jsonl = JsonlFields::default();

        // Set initial values so we can verify None preserves them
        jsonl.project = "original-project".into();
        jsonl.project_display_name = "Original Project".into();
        jsonl.project_path = "/home/user/original".into();
        jsonl.edit_count = 5;

        // Reconcile with only some fields set
        let mut data = make_reconcile_data();
        data.project_path = Some("/home/user/updated".into());
        data.edit_count = Some(10);
        data.phase = Some(PhaseHistory::default());

        apply_reconcile(&mut jsonl, &data);

        // Updated fields
        assert_eq!(jsonl.project_path, "/home/user/updated");
        assert_eq!(jsonl.edit_count, 10);

        // Preserved fields (None = no change)
        assert_eq!(jsonl.project, "original-project");
        assert_eq!(jsonl.project_display_name, "Original Project");

        // Cross-source fields NOT touched by apply_reconcile:
        // model, model_display_name, context_window_tokens live on LiveSession.
        // turn_count lives in HookFields.
        // These are handled by the coordinator, not this function.
    }

    #[test]
    fn reconcile_all_none_is_noop() {
        let mut jsonl = JsonlFields::default();
        jsonl.project = "keep-me".into();
        jsonl.edit_count = 42;

        let data = make_reconcile_data();
        apply_reconcile(&mut jsonl, &data);

        assert_eq!(jsonl.project, "keep-me");
        assert_eq!(jsonl.edit_count, 42);
    }

    #[test]
    fn reconcile_updates_tokens_and_cost() {
        let mut jsonl = JsonlFields::default();

        let tokens = TokenUsage {
            input_tokens: 1000,
            output_tokens: 500,
            cache_read_tokens: 200,
            cache_creation_tokens: 100,
            cache_creation_5m_tokens: 0,
            cache_creation_1hr_tokens: 0,
            total_tokens: 1800,
        };
        let cost = CostBreakdown {
            total_usd: 0.02205,
            input_cost_usd: 0.003,
            output_cost_usd: 0.015,
            cache_read_cost_usd: 0.0003,
            cache_creation_cost_usd: 0.00375,
            ..CostBreakdown::default()
        };

        let mut data = make_reconcile_data();
        data.tokens = Some(tokens.clone());
        data.cost = Some(cost.clone());

        apply_reconcile(&mut jsonl, &data);

        assert_eq!(jsonl.tokens.input_tokens, 1000);
        assert_eq!(jsonl.tokens.output_tokens, 500);
        assert!((jsonl.cost.total_usd - 0.02205).abs() < f64::EPSILON);
    }
}
