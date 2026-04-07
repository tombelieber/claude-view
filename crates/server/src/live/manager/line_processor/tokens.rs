//! Token accumulation and per-turn cost calculation.

use std::collections::HashMap;

use claude_view_core::pricing::{calculate_cost, ModelPricing, TokenUsage};

use super::super::accumulator::SessionAccumulator;

pub(super) fn accumulate_tokens(
    line: &claude_view_core::live_parser::LiveLine,
    acc: &mut SessionAccumulator,
) {
    if let Some(input) = line.input_tokens {
        acc.tokens.input_tokens += input;
        acc.tokens.total_tokens += input;
    }
    if let Some(output) = line.output_tokens {
        acc.tokens.output_tokens += output;
        acc.tokens.total_tokens += output;
    }
    if let Some(cache_read) = line.cache_read_tokens {
        acc.tokens.cache_read_tokens += cache_read;
        acc.tokens.total_tokens += cache_read;
    }
    if let Some(cache_creation) = line.cache_creation_tokens {
        acc.tokens.cache_creation_tokens += cache_creation;
        acc.tokens.total_tokens += cache_creation;
    }
    if let Some(tokens_5m) = line.cache_creation_5m_tokens {
        acc.tokens.cache_creation_5m_tokens += tokens_5m;
    }
    if let Some(tokens_1hr) = line.cache_creation_1hr_tokens {
        acc.tokens.cache_creation_1hr_tokens += tokens_1hr;
    }
}

pub(super) fn accumulate_turn_cost(
    line: &claude_view_core::live_parser::LiveLine,
    acc: &mut SessionAccumulator,
    pricing: &HashMap<String, ModelPricing>,
) {
    let has_tokens = line.input_tokens.is_some()
        || line.output_tokens.is_some()
        || line.cache_read_tokens.is_some()
        || line.cache_creation_tokens.is_some()
        || line.cache_creation_5m_tokens.is_some()
        || line.cache_creation_1hr_tokens.is_some();
    if !has_tokens {
        return;
    }
    let turn_tokens = TokenUsage {
        input_tokens: line.input_tokens.unwrap_or(0),
        output_tokens: line.output_tokens.unwrap_or(0),
        cache_read_tokens: line.cache_read_tokens.unwrap_or(0),
        cache_creation_tokens: line.cache_creation_tokens.unwrap_or(0),
        cache_creation_5m_tokens: line.cache_creation_5m_tokens.unwrap_or(0),
        cache_creation_1hr_tokens: line.cache_creation_1hr_tokens.unwrap_or(0),
        total_tokens: 0,
    };
    let turn_cost = calculate_cost(&turn_tokens, acc.model.as_deref(), pricing);
    acc.accumulated_cost.input_cost_usd += turn_cost.input_cost_usd;
    acc.accumulated_cost.output_cost_usd += turn_cost.output_cost_usd;
    acc.accumulated_cost.cache_read_cost_usd += turn_cost.cache_read_cost_usd;
    acc.accumulated_cost.cache_creation_cost_usd += turn_cost.cache_creation_cost_usd;
    acc.accumulated_cost.cache_savings_usd += turn_cost.cache_savings_usd;
    acc.accumulated_cost.total_usd += turn_cost.total_usd;
    acc.accumulated_cost.unpriced_input_tokens += turn_cost.unpriced_input_tokens;
    acc.accumulated_cost.unpriced_output_tokens += turn_cost.unpriced_output_tokens;
    acc.accumulated_cost.unpriced_cache_read_tokens += turn_cost.unpriced_cache_read_tokens;
    acc.accumulated_cost.unpriced_cache_creation_tokens += turn_cost.unpriced_cache_creation_tokens;
    acc.accumulated_cost.has_unpriced_usage |= turn_cost.has_unpriced_usage;
}
