//! Summary and pricing endpoints + the shared `build_summary` helper.

use std::collections::HashMap;
use std::sync::Arc;

use axum::{extract::State, response::Json};

use crate::live::state::{AgentStateGroup, LiveSession};
use crate::state::AppState;

/// GET /api/live/summary -- Aggregate statistics across all live sessions.
#[utoipa::path(get, path = "/api/live/summary", tag = "live",
    responses(
        (status = 200, description = "Aggregated live session statistics", body = serde_json::Value),
    )
)]
pub async fn get_live_summary(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let map = state.live_sessions.read().await;
    let process_count = state
        .live_manager
        .as_ref()
        .map(|m| m.process_count())
        .unwrap_or(0);
    let summary = build_summary(&map, process_count);
    match serde_json::to_value(&summary) {
        Ok(v) => Json(v),
        Err(e) => {
            tracing::error!("failed to serialize live summary: {e}");
            Json(serde_json::json!({ "error": "internal serialization error" }))
        }
    }
}

/// GET /api/live/pricing -- Return the model pricing table.
///
/// Exposes per-model costs in a frontend-friendly format (cost per million tokens).
#[utoipa::path(get, path = "/api/live/pricing", tag = "live",
    responses(
        (status = 200, description = "Model pricing table with per-token costs", body = serde_json::Value),
    )
)]
pub async fn get_pricing(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let pricing = &*state.pricing;
    let models: HashMap<String, serde_json::Value> = pricing
        .iter()
        .map(|(name, p)| {
            let mut model = serde_json::json!({
                "inputPerMillion": p.input_cost_per_token * 1_000_000.0,
                "outputPerMillion": p.output_cost_per_token * 1_000_000.0,
                "cacheReadPerMillion": p.cache_read_cost_per_token * 1_000_000.0,
                "cacheWritePerMillion": p.cache_creation_cost_per_token * 1_000_000.0,
            });
            if let Some(rate) = p.input_cost_per_token_above_200k {
                model["inputPerMillionAbove200k"] = serde_json::json!(rate * 1_000_000.0);
            }
            if let Some(rate) = p.output_cost_per_token_above_200k {
                model["outputPerMillionAbove200k"] = serde_json::json!(rate * 1_000_000.0);
            }
            (name.clone(), model)
        })
        .collect();
    Json(serde_json::json!({
        "models": models,
        "modelCount": models.len(),
        "source": "anthropic-pricing",
    }))
}

/// Build a summary JSON object from the current live sessions map.
pub(crate) fn build_summary(
    map: &HashMap<String, LiveSession>,
    process_count: u32,
) -> serde_json::Value {
    let mut needs_you_count = 0usize;
    let mut autonomous_count = 0usize;
    let mut total_cost = 0.0f64;
    let mut total_tokens = 0u64;

    for session in map.values() {
        if session.closed_at.is_some() {
            continue; // Recently closed -- excluded from active counts
        }
        match session.hook.agent_state.group {
            AgentStateGroup::NeedsYou => needs_you_count += 1,
            AgentStateGroup::Autonomous => autonomous_count += 1,
        }
        total_cost += session.jsonl.cost.total_usd;
        total_tokens += session.jsonl.tokens.total_tokens;
    }

    serde_json::json!({
        "needsYouCount": needs_you_count,
        "autonomousCount": autonomous_count,
        "totalCostTodayUsd": total_cost,
        "totalTokensToday": total_tokens,
        "processCount": process_count,
    })
}
