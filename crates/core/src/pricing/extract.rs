//! Shared helper for extracting typed `TokenUsage` from SDK usage JSON objects.
//!
//! This is the canonical path for block accumulators and sub-agent cost calculations
//! that need a fully-typed `TokenUsage` from a raw `usage` JSON blob. It extracts
//! both flat fields (input_tokens, output_tokens, cache_read_input_tokens,
//! cache_creation_input_tokens) AND the nested `cache_creation` object
//! (ephemeral_5m_input_tokens, ephemeral_1h_input_tokens).
//!
//! Callers that need `Option<u64>` to distinguish "missing" from "zero" (e.g.
//! `live_parser::extract_usage`) should continue to use that path. This helper
//! treats missing fields as 0.

use super::types::TokenUsage;

/// Extract token counts from an SDK `usage` JSON object into typed `TokenUsage`.
///
/// Missing fields are treated as 0 (not unpriced). Call this on an SDK usage
/// object before feeding tokens to `pricing::calculate_cost()`.
///
/// # Example
///
/// ```
/// use claude_view_core::pricing::extract_usage_tokens;
/// let usage = serde_json::json!({
///     "input_tokens": 100,
///     "output_tokens": 50,
///     "cache_creation_input_tokens": 1000,
///     "cache_creation": {
///         "ephemeral_5m_input_tokens": 0,
///         "ephemeral_1h_input_tokens": 1000
///     }
/// });
/// let tokens = extract_usage_tokens(&usage);
/// assert_eq!(tokens.input_tokens, 100);
/// assert_eq!(tokens.cache_creation_1hr_tokens, 1000);
/// ```
pub fn extract_usage_tokens(usage: &serde_json::Value) -> TokenUsage {
    let input = usage
        .get("input_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let output = usage
        .get("output_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let cache_read = usage
        .get("cache_read_input_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let cache_creation = usage
        .get("cache_creation_input_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    // Extract ephemeral cache breakdown when present
    let (cache_creation_5m, cache_creation_1hr) = usage
        .get("cache_creation")
        .map(|cc| {
            let t5m = cc
                .get("ephemeral_5m_input_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let t1h = cc
                .get("ephemeral_1h_input_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            (t5m, t1h)
        })
        .unwrap_or((0, 0));

    TokenUsage {
        input_tokens: input,
        output_tokens: output,
        cache_read_tokens: cache_read,
        cache_creation_tokens: cache_creation,
        cache_creation_5m_tokens: cache_creation_5m,
        cache_creation_1hr_tokens: cache_creation_1hr,
        total_tokens: input + output + cache_read + cache_creation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_flat_fields() {
        let usage = serde_json::json!({
            "input_tokens": 100,
            "output_tokens": 50,
            "cache_read_input_tokens": 20,
            "cache_creation_input_tokens": 10
        });
        let tokens = extract_usage_tokens(&usage);
        assert_eq!(tokens.input_tokens, 100);
        assert_eq!(tokens.output_tokens, 50);
        assert_eq!(tokens.cache_read_tokens, 20);
        assert_eq!(tokens.cache_creation_tokens, 10);
        assert_eq!(tokens.cache_creation_5m_tokens, 0);
        assert_eq!(tokens.cache_creation_1hr_tokens, 0);
        assert_eq!(tokens.total_tokens, 180);
    }

    #[test]
    fn extracts_nested_cache_creation_5m_and_1h() {
        let usage = serde_json::json!({
            "input_tokens": 100,
            "output_tokens": 50,
            "cache_creation_input_tokens": 1000,
            "cache_creation": {
                "ephemeral_5m_input_tokens": 400,
                "ephemeral_1h_input_tokens": 600
            }
        });
        let tokens = extract_usage_tokens(&usage);
        assert_eq!(tokens.cache_creation_tokens, 1000);
        assert_eq!(tokens.cache_creation_5m_tokens, 400);
        assert_eq!(tokens.cache_creation_1hr_tokens, 600);
    }

    #[test]
    fn extracts_1hr_only_caching() {
        // This is the Claude Code default: all cache_creation goes to 1hr TTL.
        let usage = serde_json::json!({
            "input_tokens": 3,
            "output_tokens": 30,
            "cache_read_input_tokens": 16416,
            "cache_creation_input_tokens": 26109,
            "cache_creation": {
                "ephemeral_5m_input_tokens": 0,
                "ephemeral_1h_input_tokens": 26109
            }
        });
        let tokens = extract_usage_tokens(&usage);
        assert_eq!(tokens.cache_creation_5m_tokens, 0);
        assert_eq!(tokens.cache_creation_1hr_tokens, 26109);
    }

    #[test]
    fn missing_nested_object_yields_zeros() {
        // Older JSONL without the nested breakdown.
        let usage = serde_json::json!({
            "input_tokens": 100,
            "cache_creation_input_tokens": 500
        });
        let tokens = extract_usage_tokens(&usage);
        assert_eq!(tokens.cache_creation_tokens, 500);
        assert_eq!(tokens.cache_creation_5m_tokens, 0);
        assert_eq!(tokens.cache_creation_1hr_tokens, 0);
    }

    #[test]
    fn empty_usage_object_yields_zeros() {
        let usage = serde_json::json!({});
        let tokens = extract_usage_tokens(&usage);
        assert_eq!(tokens.input_tokens, 0);
        assert_eq!(tokens.output_tokens, 0);
        assert_eq!(tokens.cache_read_tokens, 0);
        assert_eq!(tokens.cache_creation_tokens, 0);
        assert_eq!(tokens.cache_creation_5m_tokens, 0);
        assert_eq!(tokens.cache_creation_1hr_tokens, 0);
        assert_eq!(tokens.total_tokens, 0);
    }

    #[test]
    fn non_object_cache_creation_is_ignored() {
        // If cache_creation happens to be a number (malformed), skip nested extraction.
        let usage = serde_json::json!({
            "input_tokens": 100,
            "cache_creation_input_tokens": 500,
            "cache_creation": 500
        });
        let tokens = extract_usage_tokens(&usage);
        assert_eq!(tokens.cache_creation_tokens, 500);
        assert_eq!(tokens.cache_creation_5m_tokens, 0);
        assert_eq!(tokens.cache_creation_1hr_tokens, 0);
    }

    #[test]
    fn integrates_with_calculate_cost_for_1hr_rate() {
        // End-to-end: extracted tokens → pricing::calculate_cost → 1hr rate applied.
        use crate::pricing::{calculate_cost, load_pricing};

        let usage = serde_json::json!({
            "input_tokens": 0,
            "output_tokens": 0,
            "cache_creation_input_tokens": 100_000,
            "cache_creation": {
                "ephemeral_5m_input_tokens": 0,
                "ephemeral_1h_input_tokens": 100_000
            }
        });
        let tokens = extract_usage_tokens(&usage);
        let pricing = load_pricing();
        let cost = calculate_cost(&tokens, Some("claude-opus-4-6"), &pricing);

        // Opus 4.6: 100k at 1hr rate ($10/MTok) = $1.00
        // Pre-fix this would have been 100k at 5m rate ($6.25/MTok) = $0.625
        assert!(
            (cost.cache_creation_cost_usd - 1.0).abs() < 0.001,
            "expected $1.00 (1hr rate), got ${}",
            cost.cache_creation_cost_usd
        );
    }
}
