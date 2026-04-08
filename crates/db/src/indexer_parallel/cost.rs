// crates/db/src/indexer_parallel/cost.rs
// Cost calculation, model normalization, and pricing utilities.

use claude_view_core::pricing::{calculate_cost, load_pricing, ModelPricing, TokenUsage};
use std::collections::HashMap;

/// Normalize model IDs to canonical family names.
/// Maps dated variants (e.g., "claude-3-5-sonnet-20241022") to their
/// canonical names (e.g., "claude-3.5-sonnet") so token breakdowns
/// don't fragment across model versions.
pub(crate) fn normalize_model_id(model_id: &str) -> String {
    // Strip date suffixes like -20241022, -20250514, etc.
    // Pattern: ends with -YYYYMMDD
    let stripped = if model_id.len() > 9 {
        let suffix = &model_id[model_id.len() - 9..];
        if suffix.starts_with('-') && suffix[1..].chars().all(|c| c.is_ascii_digit()) {
            &model_id[..model_id.len() - 9]
        } else {
            model_id
        }
    } else {
        model_id
    };

    // Normalize known aliases (hyphen vs dot variants)
    match stripped {
        "claude-3-5-sonnet" => "claude-3.5-sonnet".to_string(),
        "claude-3-5-haiku" => "claude-3.5-haiku".to_string(),
        "claude-3-opus" => "claude-3-opus".to_string(),
        _ => stripped.to_string(),
    }
}

/// Compute the primary model for a session: the model_id with the most turns.
pub(crate) fn compute_primary_model(turns: &[claude_view_core::RawTurn]) -> Option<String> {
    if turns.is_empty() {
        return None;
    }
    let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for turn in turns {
        *counts.entry(&turn.model_id).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .max_by_key(|&(_, count)| count)
        .map(|(model, _)| model.to_string())
}

/// Compute total cost by summing per-turn costs (each turn = one API call).
/// This avoids inflating cost by applying 200k tiered pricing to cumulative tokens.
/// Returns `None` if any turn had tokens for an unpriced model.
pub(crate) fn calculate_per_turn_cost(
    turns: &[claude_view_core::RawTurn],
    pricing: &HashMap<String, ModelPricing>,
) -> Option<f64> {
    let mut total = 0.0;
    let mut has_unpriced_usage = false;
    for turn in turns {
        let tokens = TokenUsage {
            input_tokens: turn.input_tokens.unwrap_or(0),
            output_tokens: turn.output_tokens.unwrap_or(0),
            cache_read_tokens: turn.cache_read_tokens.unwrap_or(0),
            cache_creation_tokens: turn.cache_creation_tokens.unwrap_or(0),
            cache_creation_5m_tokens: turn.cache_creation_5m_tokens.unwrap_or(0),
            cache_creation_1hr_tokens: turn.cache_creation_1hr_tokens.unwrap_or(0),
            total_tokens: 0,
        };
        let turn_cost = calculate_cost(&tokens, Some(&turn.model_id), pricing);
        total += turn_cost.total_usd;
        has_unpriced_usage |= turn_cost.has_unpriced_usage;
    }
    if has_unpriced_usage {
        None
    } else {
        Some(total)
    }
}

pub(crate) fn load_indexing_pricing() -> HashMap<String, ModelPricing> {
    load_pricing()
}
