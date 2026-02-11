// crates/core/src/cost.rs
//! Live-session cost calculator for Mission Control.
//!
//! Provides real-time cost breakdowns as tokens stream in, including
//! cache savings estimation and cache warmth status.
//!
//! This module defines its own [`ModelPricing`] struct so that `core` stays
//! leaf-level (no dependency on `vibe-recall-db`). The server layer is
//! responsible for converting `vibe_recall_db::ModelPricing` into this type
//! when calling [`calculate_live_cost`].

use serde::Serialize;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Pricing types (mirror of vibe_recall_db::ModelPricing, cycle-free)
// ---------------------------------------------------------------------------

/// Per-model pricing in USD per token.
///
/// Field-compatible with `vibe_recall_db::ModelPricing` so that the server
/// layer can do a trivial `From` conversion.
#[derive(Debug, Clone)]
pub struct ModelPricing {
    pub input_cost_per_token: f64,
    pub output_cost_per_token: f64,
    pub cache_creation_cost_per_token: f64,
    pub cache_read_cost_per_token: f64,
}

/// Blended fallback rate for unknown models (USD per token).
pub const FALLBACK_COST_PER_TOKEN_USD: f64 = 2.5e-6; // $2.50 / M tokens

/// Look up pricing for a model ID. Tries exact match, then prefix match
/// (e.g. "claude-opus-4-6-20260101" matches key "claude-opus-4-6"), then
/// checks if model_id is a prefix of any key.
pub fn lookup_pricing<'a>(
    model_id: &str,
    pricing: &'a HashMap<String, ModelPricing>,
) -> Option<&'a ModelPricing> {
    if let Some(p) = pricing.get(model_id) {
        return Some(p);
    }
    for (key, p) in pricing {
        if model_id.starts_with(key.as_str()) {
            return Some(p);
        }
    }
    for (key, p) in pricing {
        if key.starts_with(model_id) {
            return Some(p);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Token / cost types
// ---------------------------------------------------------------------------

/// Accumulated token counts for a live session.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub total_tokens: u64,
}

/// Itemised cost breakdown in USD.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CostBreakdown {
    pub total_usd: f64,
    pub input_cost_usd: f64,
    pub output_cost_usd: f64,
    pub cache_read_cost_usd: f64,
    pub cache_creation_cost_usd: f64,
    pub cache_savings_usd: f64,
}

/// Whether the Anthropic prompt cache is likely warm or cold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheStatus {
    Warm,
    Cold,
    Unknown,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Calculate cost for a live token snapshot.
///
/// Looks up per-model pricing from the provided map. If the model is not found
/// (or `model` is `None`), falls back to [`FALLBACK_COST_PER_TOKEN_USD`].
pub fn calculate_live_cost(
    tokens: &TokenUsage,
    model: Option<&str>,
    pricing: &HashMap<String, ModelPricing>,
) -> CostBreakdown {
    let model_pricing = model.and_then(|m| lookup_pricing(m, pricing));

    match model_pricing {
        Some(mp) => {
            let input_cost_usd = tokens.input_tokens as f64 * mp.input_cost_per_token;
            let output_cost_usd = tokens.output_tokens as f64 * mp.output_cost_per_token;
            let cache_read_cost_usd =
                tokens.cache_read_tokens as f64 * mp.cache_read_cost_per_token;
            let cache_creation_cost_usd =
                tokens.cache_creation_tokens as f64 * mp.cache_creation_cost_per_token;
            let cache_savings_usd = tokens.cache_read_tokens as f64
                * (mp.input_cost_per_token - mp.cache_read_cost_per_token);
            let total_usd =
                input_cost_usd + output_cost_usd + cache_read_cost_usd + cache_creation_cost_usd;

            CostBreakdown {
                total_usd,
                input_cost_usd,
                output_cost_usd,
                cache_read_cost_usd,
                cache_creation_cost_usd,
                cache_savings_usd,
            }
        }
        None => {
            let fallback = FALLBACK_COST_PER_TOKEN_USD;
            let input_cost_usd = tokens.input_tokens as f64 * fallback;
            let output_cost_usd = tokens.output_tokens as f64 * fallback;
            let cache_read_cost_usd = tokens.cache_read_tokens as f64 * fallback;
            let cache_creation_cost_usd = tokens.cache_creation_tokens as f64 * fallback;
            // No meaningful cache savings when we don't know the model
            let cache_savings_usd = 0.0;
            let total_usd =
                input_cost_usd + output_cost_usd + cache_read_cost_usd + cache_creation_cost_usd;

            CostBreakdown {
                total_usd,
                input_cost_usd,
                output_cost_usd,
                cache_read_cost_usd,
                cache_creation_cost_usd,
                cache_savings_usd,
            }
        }
    }
}

/// Infer prompt-cache warmth from the time since the last API call.
///
/// Anthropic's prompt cache has a 5-minute (300 s) TTL. If fewer than 300 s
/// have elapsed since the last API call, the cache is likely still warm.
pub fn derive_cache_status(seconds_since_last_api_call: Option<u64>) -> CacheStatus {
    match seconds_since_last_api_call {
        Some(s) if s < 300 => CacheStatus::Warm,
        Some(_) => CacheStatus::Cold,
        None => CacheStatus::Unknown,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a small pricing map for tests (Opus 4.6 rates).
    fn test_pricing() -> HashMap<String, ModelPricing> {
        let mut m = HashMap::new();
        m.insert(
            "claude-opus-4-6".into(),
            ModelPricing {
                input_cost_per_token: 5e-6,   // $5/M
                output_cost_per_token: 25e-6,  // $25/M
                cache_creation_cost_per_token: 6.25e-6,
                cache_read_cost_per_token: 0.5e-6,
            },
        );
        m
    }

    #[test]
    fn test_cost_zero_tokens() {
        let pricing = test_pricing();
        let tokens = TokenUsage::default();
        let cost = calculate_live_cost(&tokens, Some("claude-opus-4-6"), &pricing);
        assert_eq!(cost.total_usd, 0.0);
        assert_eq!(cost.input_cost_usd, 0.0);
        assert_eq!(cost.output_cost_usd, 0.0);
        assert_eq!(cost.cache_read_cost_usd, 0.0);
        assert_eq!(cost.cache_creation_cost_usd, 0.0);
        assert_eq!(cost.cache_savings_usd, 0.0);
    }

    #[test]
    fn test_cost_input_only() {
        let pricing = test_pricing();
        let tokens = TokenUsage {
            input_tokens: 100_000,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
            total_tokens: 100_000,
        };
        let cost = calculate_live_cost(&tokens, Some("claude-opus-4-6"), &pricing);
        // 100k tokens * $5/M = $0.50
        assert!((cost.input_cost_usd - 0.50).abs() < 1e-9);
        assert_eq!(cost.output_cost_usd, 0.0);
        assert!((cost.total_usd - 0.50).abs() < 1e-9);
    }

    #[test]
    fn test_cost_cache_savings() {
        let pricing = test_pricing();
        let tokens = TokenUsage {
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 1_000_000,
            cache_creation_tokens: 0,
            total_tokens: 1_000_000,
        };
        let cost = calculate_live_cost(&tokens, Some("claude-opus-4-6"), &pricing);
        // Opus 4.6: input_rate = 5e-6, cache_read_rate = 0.5e-6
        // cache_read_cost = 1M * 0.5e-6 = $0.50
        // cache_savings = 1M * (5e-6 - 0.5e-6) = 1M * 4.5e-6 = $4.50
        assert!((cost.cache_read_cost_usd - 0.50).abs() < 1e-9);
        assert!((cost.cache_savings_usd - 4.50).abs() < 1e-9);
        assert!((cost.total_usd - 0.50).abs() < 1e-9);
    }

    #[test]
    fn test_cost_unknown_model() {
        let pricing = test_pricing();
        let tokens = TokenUsage {
            input_tokens: 1_000_000,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
            total_tokens: 1_000_000,
        };
        let cost = calculate_live_cost(&tokens, Some("gpt-4o"), &pricing);
        // Fallback: 1M * 2.5e-6 = $2.50
        assert!((cost.total_usd - 2.50).abs() < 1e-9);
        assert_eq!(cost.cache_savings_usd, 0.0);
    }

    #[test]
    fn test_cache_status_warm() {
        assert_eq!(derive_cache_status(Some(0)), CacheStatus::Warm);
        assert_eq!(derive_cache_status(Some(60)), CacheStatus::Warm);
        assert_eq!(derive_cache_status(Some(299)), CacheStatus::Warm);
    }

    #[test]
    fn test_cache_status_cold() {
        assert_eq!(derive_cache_status(Some(300)), CacheStatus::Cold);
        assert_eq!(derive_cache_status(Some(600)), CacheStatus::Cold);
        assert_eq!(derive_cache_status(Some(u64::MAX)), CacheStatus::Cold);
    }

    #[test]
    fn test_cache_status_unknown() {
        assert_eq!(derive_cache_status(None), CacheStatus::Unknown);
    }
}
