//! Unified pricing engine for all cost calculations.
//!
//! Single source of truth for:
//! - `ModelPricing` struct (per-model rates)
//! - `calculate_cost()` with 200k tiered pricing
//! - `CostBreakdown` / `TokenUsage` types
//! - `lookup_pricing()` with exact → prefix fallback
//! - Hardcoded defaults for offline seeding

use serde::Serialize;
use std::collections::HashMap;

/// Per-model pricing in USD per token.
#[derive(Debug, Clone)]
pub struct ModelPricing {
    pub input_cost_per_token: f64,
    pub output_cost_per_token: f64,
    pub cache_creation_cost_per_token: f64,
    pub cache_read_cost_per_token: f64,
    /// If set, input tokens above 200k are charged at this rate.
    pub input_cost_per_token_above_200k: Option<f64>,
    /// If set, output tokens above 200k are charged at this rate.
    pub output_cost_per_token_above_200k: Option<f64>,
    /// If set, cache creation tokens above 200k are charged at this rate.
    pub cache_creation_cost_per_token_above_200k: Option<f64>,
    /// If set, cache read tokens above 200k are charged at this rate.
    pub cache_read_cost_per_token_above_200k: Option<f64>,
    /// If set, 1-hour ephemeral cache creation tokens use this rate instead of the base cache_creation rate.
    pub cache_creation_cost_per_token_1hr: Option<f64>,
}

/// Token breakdown for cost calculation.
pub struct TokenBreakdown {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_creation_tokens: i64,
}

/// Accumulated token counts for a live session.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    /// Cache creation tokens with 5-minute TTL (from JSONL ephemeral_5m_input_tokens).
    pub cache_creation_5m_tokens: u64,
    /// Cache creation tokens with 1-hour TTL (from JSONL ephemeral_1h_input_tokens).
    pub cache_creation_1hr_tokens: u64,
    pub total_tokens: u64,
}

/// Itemized cost breakdown in USD.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CostBreakdown {
    pub total_usd: f64,
    pub input_cost_usd: f64,
    pub output_cost_usd: f64,
    pub cache_read_cost_usd: f64,
    pub cache_creation_cost_usd: f64,
    pub cache_savings_usd: f64,
    /// True when model was not found and fallback rate was used.
    pub is_estimated: bool,
}

/// Whether the Anthropic prompt cache is likely warm or cold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheStatus {
    Warm,
    Cold,
    Unknown,
}

/// Blended fallback rate for unknown models (USD per token).
/// Applied to input tokens only; output uses 5x this rate.
pub const FALLBACK_INPUT_COST_PER_TOKEN: f64 = 3e-6; // $3/M (sonnet-class)
pub const FALLBACK_OUTPUT_COST_PER_TOKEN: f64 = 15e-6; // $15/M (sonnet-class)

/// Calculate cost for a token snapshot using model-specific pricing.
///
/// If `model` is `None` or not found, falls back to sonnet-class rates
/// and sets `is_estimated = true`.
pub fn calculate_cost(
    tokens: &TokenUsage,
    model: Option<&str>,
    pricing: &HashMap<String, ModelPricing>,
) -> CostBreakdown {
    let model_pricing = model.and_then(|m| lookup_pricing(m, pricing));

    match model_pricing {
        Some(mp) => {
            let input_cost_usd = tiered_cost(
                tokens.input_tokens as i64,
                mp.input_cost_per_token,
                mp.input_cost_per_token_above_200k,
            );
            let output_cost_usd = tiered_cost(
                tokens.output_tokens as i64,
                mp.output_cost_per_token,
                mp.output_cost_per_token_above_200k,
            );
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
                is_estimated: false,
            }
        }
        None => {
            let input_cost_usd = tokens.input_tokens as f64 * FALLBACK_INPUT_COST_PER_TOKEN;
            let output_cost_usd = tokens.output_tokens as f64 * FALLBACK_OUTPUT_COST_PER_TOKEN;
            let cache_read_cost_usd =
                tokens.cache_read_tokens as f64 * FALLBACK_INPUT_COST_PER_TOKEN * 0.1;
            let cache_creation_cost_usd =
                tokens.cache_creation_tokens as f64 * FALLBACK_INPUT_COST_PER_TOKEN * 1.25;
            let total_usd =
                input_cost_usd + output_cost_usd + cache_read_cost_usd + cache_creation_cost_usd;

            CostBreakdown {
                total_usd,
                input_cost_usd,
                output_cost_usd,
                cache_read_cost_usd,
                cache_creation_cost_usd,
                cache_savings_usd: 0.0,
                is_estimated: true,
            }
        }
    }
}

/// Calculate cost in USD from a `TokenBreakdown` (historical queries).
pub fn calculate_cost_usd(tokens: &TokenBreakdown, pricing: &ModelPricing) -> f64 {
    let input_cost = tiered_cost(
        tokens.input_tokens,
        pricing.input_cost_per_token,
        pricing.input_cost_per_token_above_200k,
    );
    let output_cost = tiered_cost(
        tokens.output_tokens,
        pricing.output_cost_per_token,
        pricing.output_cost_per_token_above_200k,
    );
    let cache_create_cost =
        tokens.cache_creation_tokens as f64 * pricing.cache_creation_cost_per_token;
    let cache_read_cost = tokens.cache_read_tokens as f64 * pricing.cache_read_cost_per_token;

    input_cost + output_cost + cache_create_cost + cache_read_cost
}

fn tiered_cost(tokens: i64, base_rate: f64, above_200k_rate: Option<f64>) -> f64 {
    const THRESHOLD: i64 = 200_000;
    if tokens <= 0 {
        return 0.0;
    }

    match above_200k_rate {
        Some(high_rate) if tokens > THRESHOLD => {
            let below = THRESHOLD as f64 * base_rate;
            let above = (tokens - THRESHOLD) as f64 * high_rate;
            below + above
        }
        _ => tokens as f64 * base_rate,
    }
}

/// Look up pricing for a model ID.
///
/// Fallback chain:
/// 1. Exact match (e.g. "claude-opus-4-6")
/// 2. Key is prefix of model_id (e.g. key "claude-opus-4-6" matches "claude-opus-4-6-20260201")
/// 3. model_id is prefix of key (e.g. "claude-opus" matches "claude-opus-4-6")
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

/// Complete Anthropic pricing table for offline seeding.
///
/// These defaults are overridden at runtime by litellm fetch when network is available.
pub fn default_pricing() -> HashMap<String, ModelPricing> {
    let mut m = HashMap::new();

    // Current generation
    m.insert(
        "claude-opus-4-6".into(),
        ModelPricing {
            input_cost_per_token: 5e-6,
            output_cost_per_token: 25e-6,
            cache_creation_cost_per_token: 6.25e-6,
            cache_read_cost_per_token: 0.5e-6,
            input_cost_per_token_above_200k: Some(10e-6),
            output_cost_per_token_above_200k: Some(37.5e-6),
            cache_creation_cost_per_token_above_200k: None,
            cache_read_cost_per_token_above_200k: None,
            cache_creation_cost_per_token_1hr: None,
        },
    );

    m.insert(
        "claude-sonnet-4-6".into(),
        ModelPricing {
            input_cost_per_token: 3e-6,
            output_cost_per_token: 15e-6,
            cache_creation_cost_per_token: 3.75e-6,
            cache_read_cost_per_token: 0.3e-6,
            input_cost_per_token_above_200k: Some(6e-6),
            output_cost_per_token_above_200k: Some(22.5e-6),
            cache_creation_cost_per_token_above_200k: None,
            cache_read_cost_per_token_above_200k: None,
            cache_creation_cost_per_token_1hr: None,
        },
    );

    m.insert(
        "claude-sonnet-4-5-20250929".into(),
        ModelPricing {
            input_cost_per_token: 3e-6,
            output_cost_per_token: 15e-6,
            cache_creation_cost_per_token: 3.75e-6,
            cache_read_cost_per_token: 0.3e-6,
            input_cost_per_token_above_200k: Some(6e-6),
            output_cost_per_token_above_200k: Some(22.5e-6),
            cache_creation_cost_per_token_above_200k: None,
            cache_read_cost_per_token_above_200k: None,
            cache_creation_cost_per_token_1hr: None,
        },
    );

    m.insert(
        "claude-haiku-4-5-20251001".into(),
        ModelPricing {
            input_cost_per_token: 1e-6,
            output_cost_per_token: 5e-6,
            cache_creation_cost_per_token: 1.25e-6,
            cache_read_cost_per_token: 0.1e-6,
            input_cost_per_token_above_200k: None,
            output_cost_per_token_above_200k: None,
            cache_creation_cost_per_token_above_200k: None,
            cache_read_cost_per_token_above_200k: None,
            cache_creation_cost_per_token_1hr: None,
        },
    );

    // Legacy models
    m.insert(
        "claude-opus-4-5-20251101".into(),
        ModelPricing {
            input_cost_per_token: 5e-6,
            output_cost_per_token: 25e-6,
            cache_creation_cost_per_token: 6.25e-6,
            cache_read_cost_per_token: 0.5e-6,
            input_cost_per_token_above_200k: None,
            output_cost_per_token_above_200k: None,
            cache_creation_cost_per_token_above_200k: None,
            cache_read_cost_per_token_above_200k: None,
            cache_creation_cost_per_token_1hr: None,
        },
    );
    m.insert(
        "claude-opus-4-1-20250805".into(),
        ModelPricing {
            input_cost_per_token: 15e-6,
            output_cost_per_token: 75e-6,
            cache_creation_cost_per_token: 18.75e-6,
            cache_read_cost_per_token: 1.5e-6,
            input_cost_per_token_above_200k: None,
            output_cost_per_token_above_200k: None,
            cache_creation_cost_per_token_above_200k: None,
            cache_read_cost_per_token_above_200k: None,
            cache_creation_cost_per_token_1hr: None,
        },
    );
    m.insert(
        "claude-opus-4-20250514".into(),
        ModelPricing {
            input_cost_per_token: 15e-6,
            output_cost_per_token: 75e-6,
            cache_creation_cost_per_token: 18.75e-6,
            cache_read_cost_per_token: 1.5e-6,
            input_cost_per_token_above_200k: None,
            output_cost_per_token_above_200k: None,
            cache_creation_cost_per_token_above_200k: None,
            cache_read_cost_per_token_above_200k: None,
            cache_creation_cost_per_token_1hr: None,
        },
    );
    m.insert(
        "claude-sonnet-4-20250514".into(),
        ModelPricing {
            input_cost_per_token: 3e-6,
            output_cost_per_token: 15e-6,
            cache_creation_cost_per_token: 3.75e-6,
            cache_read_cost_per_token: 0.3e-6,
            input_cost_per_token_above_200k: Some(6e-6),
            output_cost_per_token_above_200k: Some(22.5e-6),
            cache_creation_cost_per_token_above_200k: None,
            cache_read_cost_per_token_above_200k: None,
            cache_creation_cost_per_token_1hr: None,
        },
    );
    m.insert(
        "claude-3-7-sonnet-20250219".into(),
        ModelPricing {
            input_cost_per_token: 3e-6,
            output_cost_per_token: 15e-6,
            cache_creation_cost_per_token: 3.75e-6,
            cache_read_cost_per_token: 0.3e-6,
            input_cost_per_token_above_200k: None,
            output_cost_per_token_above_200k: None,
            cache_creation_cost_per_token_above_200k: None,
            cache_read_cost_per_token_above_200k: None,
            cache_creation_cost_per_token_1hr: None,
        },
    );
    m.insert(
        "claude-3-5-sonnet-20241022".into(),
        ModelPricing {
            input_cost_per_token: 3e-6,
            output_cost_per_token: 15e-6,
            cache_creation_cost_per_token: 3.75e-6,
            cache_read_cost_per_token: 0.3e-6,
            input_cost_per_token_above_200k: None,
            output_cost_per_token_above_200k: None,
            cache_creation_cost_per_token_above_200k: None,
            cache_read_cost_per_token_above_200k: None,
            cache_creation_cost_per_token_1hr: None,
        },
    );
    m.insert(
        "claude-3-5-sonnet-20240620".into(),
        ModelPricing {
            input_cost_per_token: 3e-6,
            output_cost_per_token: 15e-6,
            cache_creation_cost_per_token: 3.75e-6,
            cache_read_cost_per_token: 0.3e-6,
            input_cost_per_token_above_200k: None,
            output_cost_per_token_above_200k: None,
            cache_creation_cost_per_token_above_200k: None,
            cache_read_cost_per_token_above_200k: None,
            cache_creation_cost_per_token_1hr: None,
        },
    );
    m.insert(
        "claude-3-5-haiku-20241022".into(),
        ModelPricing {
            input_cost_per_token: 0.8e-6,
            output_cost_per_token: 4e-6,
            cache_creation_cost_per_token: 1e-6,
            cache_read_cost_per_token: 0.08e-6,
            input_cost_per_token_above_200k: None,
            output_cost_per_token_above_200k: None,
            cache_creation_cost_per_token_above_200k: None,
            cache_read_cost_per_token_above_200k: None,
            cache_creation_cost_per_token_1hr: None,
        },
    );
    m.insert(
        "claude-3-opus-20240229".into(),
        ModelPricing {
            input_cost_per_token: 15e-6,
            output_cost_per_token: 75e-6,
            cache_creation_cost_per_token: 18.75e-6,
            cache_read_cost_per_token: 1.5e-6,
            input_cost_per_token_above_200k: None,
            output_cost_per_token_above_200k: None,
            cache_creation_cost_per_token_above_200k: None,
            cache_read_cost_per_token_above_200k: None,
            cache_creation_cost_per_token_1hr: None,
        },
    );
    m.insert(
        "claude-3-sonnet-20240229".into(),
        ModelPricing {
            input_cost_per_token: 3e-6,
            output_cost_per_token: 15e-6,
            cache_creation_cost_per_token: 3.75e-6,
            cache_read_cost_per_token: 0.3e-6,
            input_cost_per_token_above_200k: None,
            output_cost_per_token_above_200k: None,
            cache_creation_cost_per_token_above_200k: None,
            cache_read_cost_per_token_above_200k: None,
            cache_creation_cost_per_token_1hr: None,
        },
    );
    m.insert(
        "claude-3-haiku-20240307".into(),
        ModelPricing {
            input_cost_per_token: 0.25e-6,
            output_cost_per_token: 1.25e-6,
            cache_creation_cost_per_token: 0.3e-6,
            cache_read_cost_per_token: 0.03e-6,
            input_cost_per_token_above_200k: None,
            output_cost_per_token_above_200k: None,
            cache_creation_cost_per_token_above_200k: None,
            cache_read_cost_per_token_above_200k: None,
            cache_creation_cost_per_token_1hr: None,
        },
    );

    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sonnet_46_exists() {
        let pricing = default_pricing();
        assert!(pricing.get("claude-sonnet-4-6").is_some());
    }

    #[test]
    fn test_all_models_count() {
        let pricing = default_pricing();
        assert_eq!(pricing.len(), 15);
    }

    #[test]
    fn test_tiered_pricing_opus_46() {
        let pricing = default_pricing();
        let p = pricing.get("claude-opus-4-6").unwrap();
        let tokens = TokenBreakdown {
            input_tokens: 500_000,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
        };
        // First 200k at $5/M = $1.00, remaining 300k at $10/M = $3.00 => $4.00
        let cost = calculate_cost_usd(&tokens, p);
        assert!((cost - 4.0).abs() < 0.01);
    }

    #[test]
    fn test_calculate_cost_with_tiering() {
        let pricing = default_pricing();
        let tokens = TokenUsage {
            input_tokens: 500_000,
            total_tokens: 500_000,
            ..Default::default()
        };
        let cost = calculate_cost(&tokens, Some("claude-opus-4-6"), &pricing);
        assert!((cost.total_usd - 4.0).abs() < 0.01);
        assert!(!cost.is_estimated);
    }

    #[test]
    fn test_fallback_is_estimated() {
        let pricing = default_pricing();
        let tokens = TokenUsage {
            input_tokens: 1_000_000,
            total_tokens: 1_000_000,
            ..Default::default()
        };
        let cost = calculate_cost(&tokens, Some("gpt-4o"), &pricing);
        assert!(cost.is_estimated);
        assert!((cost.input_cost_usd - 3.0).abs() < 0.01);
    }

    #[test]
    fn test_fallback_output_uses_higher_rate() {
        let pricing = default_pricing();
        let tokens = TokenUsage {
            output_tokens: 1_000_000,
            total_tokens: 1_000_000,
            ..Default::default()
        };
        let cost = calculate_cost(&tokens, Some("unknown-model"), &pricing);
        assert!(cost.is_estimated);
        assert!((cost.output_cost_usd - 15.0).abs() < 0.01);
    }

    #[test]
    fn test_zero_tokens() {
        let pricing = default_pricing();
        let tokens = TokenUsage::default();
        let cost = calculate_cost(&tokens, Some("claude-opus-4-6"), &pricing);
        assert_eq!(cost.total_usd, 0.0);
        assert!(!cost.is_estimated);
    }

    #[test]
    fn test_cache_savings() {
        let pricing = default_pricing();
        let tokens = TokenUsage {
            cache_read_tokens: 1_000_000,
            total_tokens: 1_000_000,
            ..Default::default()
        };
        let cost = calculate_cost(&tokens, Some("claude-opus-4-6"), &pricing);
        assert!((cost.cache_read_cost_usd - 0.50).abs() < 1e-9);
        assert!((cost.cache_savings_usd - 4.50).abs() < 1e-9);
    }

    #[test]
    fn test_prefix_lookup_sonnet_46_dated() {
        let pricing = default_pricing();
        assert!(lookup_pricing("claude-sonnet-4-6-20260301", &pricing).is_some());
    }

}
