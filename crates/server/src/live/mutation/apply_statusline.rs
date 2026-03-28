//! Pure function: apply a StatuslinePayload to StatuslineFields.
//!
//! All merge semantics are enforced by the Monotonic/Latest/Transient newtypes.
//! This module replaces the Phase 1 interim merge helpers.

use crate::live::state::StatuslineFields;
use crate::routes::statusline::StatuslinePayload;

/// Apply statusline payload fields to the statusline sub-struct.
/// Pure function -- no IO, no session-level concerns, just field merges.
///
/// Fields that live on LiveSession directly (model, model_display_name,
/// model_set_at, context_window_tokens) are NOT handled here -- they
/// are cross-source fields updated by the route handler.
pub fn apply_statusline(state: &mut StatuslineFields, payload: &StatuslinePayload) {
    // Context window
    if let Some(ref cw) = payload.context_window {
        state.statusline_context_window_size.merge(cw.context_window_size);
        state
            .statusline_used_pct
            .merge(cw.used_percentage.map(|p| p as f32));
        state
            .statusline_remaining_pct
            .merge(cw.remaining_percentage.map(|p| p as f32));
        state
            .statusline_total_input_tokens
            .merge(cw.total_input_tokens);
        state
            .statusline_total_output_tokens
            .merge(cw.total_output_tokens);

        if let Some(ref usage) = cw.current_usage {
            state.statusline_input_tokens.merge(usage.input_tokens);
            state.statusline_output_tokens.merge(usage.output_tokens);
            state
                .statusline_cache_read_tokens
                .merge(usage.cache_read_input_tokens);
            state
                .statusline_cache_creation_tokens
                .merge(usage.cache_creation_input_tokens);
        }
    }

    // Cost -- guard: only merge cost_usd when > 0.0 (zero is "not computed yet")
    if let Some(ref cost) = payload.cost {
        let usd = cost.total_cost_usd.filter(|&v| v > 0.0);
        state.statusline_cost_usd.merge(usd);
        state.statusline_total_duration_ms.merge(cost.total_duration_ms);
        state
            .statusline_api_duration_ms
            .merge(cost.total_api_duration_ms);
        state.statusline_lines_added.merge(cost.total_lines_added);
        state.statusline_lines_removed.merge(cost.total_lines_removed);
    }

    // Workspace (Latest -- only merge when present)
    if let Some(ref ws) = payload.workspace {
        state.statusline_cwd.merge(ws.current_dir.clone());
        state.statusline_project_dir.merge(ws.project_dir.clone());
    } else if let Some(ref cwd) = payload.cwd {
        state.statusline_cwd.merge(Some(cwd.clone()));
    }

    // Top-level scalars (Latest for version/transcript, Transient for exceeds)
    state.statusline_version.merge(payload.version.clone());
    state.exceeds_200k_tokens.merge(payload.exceeds_200k_tokens);
    state
        .statusline_transcript_path
        .merge(payload.transcript_path.clone());

    // Transient fields -- unconditional so stale values clear
    state.statusline_output_style.merge(
        payload
            .output_style
            .as_ref()
            .and_then(|os| os.name.clone()),
    );
    state
        .statusline_vim_mode
        .merge(payload.vim.as_ref().and_then(|v| v.mode.clone()));
    state
        .statusline_agent_name
        .merge(payload.agent.as_ref().and_then(|a| a.name.clone()));

    // Worktree -- unconditional so fields clear if user exits worktree
    let wt = payload.worktree.as_ref();
    state
        .statusline_worktree_name
        .merge(wt.and_then(|w| w.name.clone()));
    state
        .statusline_worktree_path
        .merge(wt.and_then(|w| w.path.clone()));
    state
        .statusline_worktree_branch
        .merge(wt.and_then(|w| w.branch.clone()));
    state
        .statusline_worktree_original_cwd
        .merge(wt.and_then(|w| w.original_cwd.clone()));
    state
        .statusline_worktree_original_branch
        .merge(wt.and_then(|w| w.original_branch.clone()));

    // Rate limits -- unconditional
    let fh = payload
        .rate_limits
        .as_ref()
        .and_then(|rl| rl.five_hour.as_ref());
    let sd = payload
        .rate_limits
        .as_ref()
        .and_then(|rl| rl.seven_day.as_ref());
    state
        .statusline_rate_limit_5h_pct
        .merge(fh.and_then(|w| w.used_percentage));
    state
        .statusline_rate_limit_5h_resets_at
        .merge(fh.and_then(|w| w.resets_at));
    state
        .statusline_rate_limit_7d_pct
        .merge(sd.and_then(|w| w.used_percentage));
    state
        .statusline_rate_limit_7d_resets_at
        .merge(sd.and_then(|w| w.resets_at));

    // Raw blob for debug endpoint
    state.statusline_raw = serde_json::to_value(payload).ok();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::state::StatuslineFields;
    use crate::routes::statusline::{StatuslineCost, StatuslinePayload};

    fn empty_payload() -> StatuslinePayload {
        StatuslinePayload {
            session_id: "test".into(),
            model: None,
            cwd: None,
            workspace: None,
            cost: None,
            context_window: None,
            exceeds_200k_tokens: None,
            transcript_path: None,
            version: None,
            output_style: None,
            vim: None,
            agent: None,
            worktree: None,
            rate_limits: None,
            extra: Default::default(),
        }
    }

    #[test]
    fn duration_preserved_when_cost_sends_none() {
        let mut state = StatuslineFields::default();
        // First payload sets duration
        let mut p1 = empty_payload();
        p1.cost = Some(StatuslineCost {
            total_cost_usd: Some(0.50),
            total_duration_ms: Some(17000),
            total_api_duration_ms: Some(12000),
            total_lines_added: None,
            total_lines_removed: None,
        });
        apply_statusline(&mut state, &p1);
        assert_eq!(state.statusline_total_duration_ms.get(), Some(&17000));

        // Second payload has no cost section at all
        let p2 = empty_payload();
        apply_statusline(&mut state, &p2);
        // Duration preserved (Monotonic: None = no-op)
        assert_eq!(state.statusline_total_duration_ms.get(), Some(&17000));
    }

    #[test]
    fn duration_updates_when_higher() {
        let mut state = StatuslineFields::default();
        let mut p1 = empty_payload();
        p1.cost = Some(StatuslineCost {
            total_cost_usd: None,
            total_duration_ms: Some(12000),
            total_api_duration_ms: None,
            total_lines_added: None,
            total_lines_removed: None,
        });
        apply_statusline(&mut state, &p1);

        let mut p2 = empty_payload();
        p2.cost = Some(StatuslineCost {
            total_cost_usd: None,
            total_duration_ms: Some(17000),
            total_api_duration_ms: None,
            total_lines_added: None,
            total_lines_removed: None,
        });
        apply_statusline(&mut state, &p2);
        assert_eq!(state.statusline_total_duration_ms.get(), Some(&17000));
    }

    #[test]
    fn duration_not_downgraded() {
        let mut state = StatuslineFields::default();
        let mut p1 = empty_payload();
        p1.cost = Some(StatuslineCost {
            total_cost_usd: None,
            total_duration_ms: Some(17000),
            total_api_duration_ms: None,
            total_lines_added: None,
            total_lines_removed: None,
        });
        apply_statusline(&mut state, &p1);

        let mut p2 = empty_payload();
        p2.cost = Some(StatuslineCost {
            total_cost_usd: None,
            total_duration_ms: Some(12000),
            total_api_duration_ms: None,
            total_lines_added: None,
            total_lines_removed: None,
        });
        apply_statusline(&mut state, &p2);
        // Monotonic: lower value rejected
        assert_eq!(state.statusline_total_duration_ms.get(), Some(&17000));
    }

    #[test]
    fn transient_clears_on_none() {
        let mut state = StatuslineFields::default();
        let mut p1 = empty_payload();
        p1.vim = Some(crate::routes::statusline::StatuslineVim {
            mode: Some("NORMAL".into()),
        });
        apply_statusline(&mut state, &p1);
        assert_eq!(
            state.statusline_vim_mode.get(),
            Some(&"NORMAL".to_string())
        );

        // Vim absent from payload -> Transient clears
        let p2 = empty_payload();
        apply_statusline(&mut state, &p2);
        assert_eq!(state.statusline_vim_mode.get(), None);
    }

    #[test]
    fn cost_usd_zero_ignored() {
        let mut state = StatuslineFields::default();
        let mut p = empty_payload();
        p.cost = Some(StatuslineCost {
            total_cost_usd: Some(0.0),
            total_duration_ms: None,
            total_api_duration_ms: None,
            total_lines_added: None,
            total_lines_removed: None,
        });
        apply_statusline(&mut state, &p);
        assert_eq!(state.statusline_cost_usd.get(), None);
    }
}
