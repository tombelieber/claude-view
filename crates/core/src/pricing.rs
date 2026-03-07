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
use ts_rs::TS;

/// Per-model pricing in USD per token.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, Default, Serialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
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
#[derive(Debug, Clone, Default, Serialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct CostBreakdown {
    pub total_usd: f64,
    pub input_cost_usd: f64,
    pub output_cost_usd: f64,
    pub cache_read_cost_usd: f64,
    pub cache_creation_cost_usd: f64,
    pub cache_savings_usd: f64,
    /// True when any tokens were excluded from USD due to missing model pricing.
    pub has_unpriced_usage: bool,
    /// Tokens excluded from USD totals (no pricing match).
    pub unpriced_input_tokens: u64,
    pub unpriced_output_tokens: u64,
    pub unpriced_cache_read_tokens: u64,
    pub unpriced_cache_creation_tokens: u64,
    /// Fraction of all tokens priced with real model rates [0.0, 1.0].
    pub priced_token_coverage: f64,
    /// `computed_priced_tokens_full` | `computed_priced_tokens_partial`.
    pub total_cost_source: String,
}

/// Whether the Anthropic prompt cache is likely warm or cold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
#[serde(rename_all = "snake_case")]
pub enum CacheStatus {
    Warm,
    Cold,
    Unknown,
}

/// Anthropic pricing structure multipliers.
///
/// These encode Anthropic's consistent pricing rules across all models
/// (verified: Opus 4.6, Sonnet 4.6, Haiku 4.5 all use identical ratios).
/// Used to derive tiering fields when litellm doesn't provide them explicitly.
const ABOVE_200K_INPUT_MULT: f64 = 2.0;
const ABOVE_200K_OUTPUT_MULT: f64 = 1.5;
const ABOVE_200K_CACHE_CREATE_MULT: f64 = 2.0;
const ABOVE_200K_CACHE_READ_MULT: f64 = 2.0;
const CACHE_1HR_MULT: f64 = 1.6;

/// Fill any `None` tiering fields by deriving from base rates using known
/// Anthropic pricing multipliers.
///
/// Only fills gaps — explicit values from litellm/cache are preserved.
/// Call this as a final pass after merging all pricing sources.
pub fn fill_tiering_gaps(pricing: &mut HashMap<String, ModelPricing>) {
    for mp in pricing.values_mut() {
        if mp.input_cost_per_token_above_200k.is_none() {
            mp.input_cost_per_token_above_200k =
                Some(mp.input_cost_per_token * ABOVE_200K_INPUT_MULT);
        }
        if mp.output_cost_per_token_above_200k.is_none() {
            mp.output_cost_per_token_above_200k =
                Some(mp.output_cost_per_token * ABOVE_200K_OUTPUT_MULT);
        }
        if mp.cache_creation_cost_per_token_above_200k.is_none() {
            mp.cache_creation_cost_per_token_above_200k =
                Some(mp.cache_creation_cost_per_token * ABOVE_200K_CACHE_CREATE_MULT);
        }
        if mp.cache_read_cost_per_token_above_200k.is_none() {
            mp.cache_read_cost_per_token_above_200k =
                Some(mp.cache_read_cost_per_token * ABOVE_200K_CACHE_READ_MULT);
        }
        if mp.cache_creation_cost_per_token_1hr.is_none() {
            mp.cache_creation_cost_per_token_1hr =
                Some(mp.cache_creation_cost_per_token * CACHE_1HR_MULT);
        }
    }
}

/// Calculate cost for a token snapshot using model-specific pricing.
///
/// If `model` is `None` or not found, USD is left at zero and tokens are
/// recorded as unpriced (never converted using synthetic fallback rates).
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
            // Cache read: tiered like input/output
            let cache_read_cost_usd = tiered_cost(
                tokens.cache_read_tokens as i64,
                mp.cache_read_cost_per_token,
                mp.cache_read_cost_per_token_above_200k,
            );

            // Cache creation: split by TTL if available, otherwise use total with tiering
            let cache_creation_cost_usd = {
                let has_split =
                    tokens.cache_creation_5m_tokens > 0 || tokens.cache_creation_1hr_tokens > 0;
                if has_split {
                    let cost_5m = tiered_cost(
                        tokens.cache_creation_5m_tokens as i64,
                        mp.cache_creation_cost_per_token,
                        mp.cache_creation_cost_per_token_above_200k,
                    );
                    let cost_1hr = match mp.cache_creation_cost_per_token_1hr {
                        Some(rate_1hr) => tokens.cache_creation_1hr_tokens as f64 * rate_1hr,
                        None => tiered_cost(
                            tokens.cache_creation_1hr_tokens as i64,
                            mp.cache_creation_cost_per_token,
                            mp.cache_creation_cost_per_token_above_200k,
                        ),
                    };
                    cost_5m + cost_1hr
                } else {
                    tiered_cost(
                        tokens.cache_creation_tokens as i64,
                        mp.cache_creation_cost_per_token,
                        mp.cache_creation_cost_per_token_above_200k,
                    )
                }
            };

            let cache_savings_usd = tiered_cost(
                tokens.cache_read_tokens as i64,
                mp.input_cost_per_token,
                mp.input_cost_per_token_above_200k,
            ) - cache_read_cost_usd;
            let total_usd =
                input_cost_usd + output_cost_usd + cache_read_cost_usd + cache_creation_cost_usd;

            CostBreakdown {
                total_usd,
                input_cost_usd,
                output_cost_usd,
                cache_read_cost_usd,
                cache_creation_cost_usd,
                cache_savings_usd,
                has_unpriced_usage: false,
                unpriced_input_tokens: 0,
                unpriced_output_tokens: 0,
                unpriced_cache_read_tokens: 0,
                unpriced_cache_creation_tokens: 0,
                priced_token_coverage: 1.0,
                total_cost_source: "computed_priced_tokens_full".to_string(),
            }
        }
        None => {
            let unpriced_total = tokens.input_tokens
                + tokens.output_tokens
                + tokens.cache_read_tokens
                + tokens.cache_creation_tokens;
            let has_unpriced_usage = unpriced_total > 0;
            CostBreakdown {
                total_usd: 0.0,
                input_cost_usd: 0.0,
                output_cost_usd: 0.0,
                cache_read_cost_usd: 0.0,
                cache_creation_cost_usd: 0.0,
                cache_savings_usd: 0.0,
                has_unpriced_usage,
                unpriced_input_tokens: tokens.input_tokens,
                unpriced_output_tokens: tokens.output_tokens,
                unpriced_cache_read_tokens: tokens.cache_read_tokens,
                unpriced_cache_creation_tokens: tokens.cache_creation_tokens,
                priced_token_coverage: 0.0,
                total_cost_source: if has_unpriced_usage {
                    "computed_priced_tokens_partial".to_string()
                } else {
                    "computed_priced_tokens_full".to_string()
                },
            }
        }
    }
}

/// Finalize source/coverage fields for aggregated session cost.
///
/// Call this after summing per-turn `CostBreakdown`s into a session-level total.
pub fn finalize_cost_breakdown(cost: &mut CostBreakdown, tokens: &TokenUsage) {
    let total_tokens = tokens.input_tokens
        + tokens.output_tokens
        + tokens.cache_read_tokens
        + tokens.cache_creation_tokens;
    let unpriced_tokens = cost.unpriced_input_tokens
        + cost.unpriced_output_tokens
        + cost.unpriced_cache_read_tokens
        + cost.unpriced_cache_creation_tokens;
    let priced_tokens = total_tokens.saturating_sub(unpriced_tokens);

    cost.has_unpriced_usage = unpriced_tokens > 0;
    cost.priced_token_coverage = if total_tokens > 0 {
        priced_tokens as f64 / total_tokens as f64
    } else {
        1.0
    };
    cost.total_cost_source = if cost.has_unpriced_usage {
        "computed_priced_tokens_partial".to_string()
    } else {
        "computed_priced_tokens_full".to_string()
    };
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
    let cache_create_cost = tiered_cost(
        tokens.cache_creation_tokens,
        pricing.cache_creation_cost_per_token,
        pricing.cache_creation_cost_per_token_above_200k,
    );
    let cache_read_cost = tiered_cost(
        tokens.cache_read_tokens,
        pricing.cache_read_cost_per_token,
        pricing.cache_read_cost_per_token_above_200k,
    );

    input_cost + output_cost + cache_create_cost + cache_read_cost
}

pub fn tiered_cost(tokens: i64, base_rate: f64, above_200k_rate: Option<f64>) -> f64 {
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

/// Minimal Anthropic pricing table for cold-start fallback.
///
/// Contains only 4 base rates per model — tiering fields are `None`.
/// At runtime, `fill_tiering_gaps()` derives tiering from multipliers,
/// and litellm fetch replaces everything with full accurate data.
/// This table is only needed for the seconds before litellm responds
/// on first launch with no SQLite cache.
pub fn default_pricing() -> HashMap<String, ModelPricing> {
    let mut m = HashMap::new();

    let models: &[(&str, f64, f64, f64, f64)] = &[
        // Current generation
        ("claude-opus-4-6", 5e-6, 25e-6, 6.25e-6, 0.5e-6),
        ("claude-sonnet-4-6", 3e-6, 15e-6, 3.75e-6, 0.3e-6),
        ("claude-sonnet-4-5-20250929", 3e-6, 15e-6, 3.75e-6, 0.3e-6),
        ("claude-haiku-4-5-20251001", 1e-6, 5e-6, 1.25e-6, 0.1e-6),
        ("claude-opus-4-5-20251101", 5e-6, 25e-6, 6.25e-6, 0.5e-6),
        // Legacy
        ("claude-opus-4-1-20250805", 15e-6, 75e-6, 18.75e-6, 1.5e-6),
        ("claude-opus-4-20250514", 15e-6, 75e-6, 18.75e-6, 1.5e-6),
        ("claude-sonnet-4-20250514", 3e-6, 15e-6, 3.75e-6, 0.3e-6),
        ("claude-3-7-sonnet-20250219", 3e-6, 15e-6, 3.75e-6, 0.3e-6),
        ("claude-3-5-sonnet-20241022", 3e-6, 15e-6, 3.75e-6, 0.3e-6),
        ("claude-3-5-sonnet-20240620", 3e-6, 15e-6, 3.75e-6, 0.3e-6),
        ("claude-3-5-haiku-20241022", 0.8e-6, 4e-6, 1e-6, 0.08e-6),
        ("claude-3-opus-20240229", 15e-6, 75e-6, 18.75e-6, 1.5e-6),
        ("claude-3-sonnet-20240229", 3e-6, 15e-6, 3.75e-6, 0.3e-6),
        ("claude-3-haiku-20240307", 0.25e-6, 1.25e-6, 0.3e-6, 0.03e-6),
    ];

    for &(id, input, output, cache_create, cache_read) in models {
        m.insert(
            id.into(),
            ModelPricing {
                input_cost_per_token: input,
                output_cost_per_token: output,
                cache_creation_cost_per_token: cache_create,
                cache_read_cost_per_token: cache_read,
                input_cost_per_token_above_200k: None,
                output_cost_per_token_above_200k: None,
                cache_creation_cost_per_token_above_200k: None,
                cache_read_cost_per_token_above_200k: None,
                cache_creation_cost_per_token_1hr: None,
            },
        );
    }

    m
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build the pricing map the way the real app does: defaults + gap filling.
    fn test_pricing() -> HashMap<String, ModelPricing> {
        let mut p = default_pricing();
        fill_tiering_gaps(&mut p);
        p
    }

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
        let pricing = test_pricing();
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
        let pricing = test_pricing();
        let tokens = TokenUsage {
            input_tokens: 500_000,
            total_tokens: 500_000,
            ..Default::default()
        };
        let cost = calculate_cost(&tokens, Some("claude-opus-4-6"), &pricing);
        assert!((cost.total_usd - 4.0).abs() < 0.01);
        assert!(!cost.has_unpriced_usage);
    }

    #[test]
    fn test_unknown_model_has_no_fake_usd() {
        let pricing = default_pricing();
        let tokens = TokenUsage {
            input_tokens: 1_000_000,
            total_tokens: 1_000_000,
            ..Default::default()
        };
        let cost = calculate_cost(&tokens, Some("gpt-4o"), &pricing);
        assert_eq!(cost.total_usd, 0.0);
        assert!(cost.has_unpriced_usage);
        assert_eq!(cost.unpriced_input_tokens, 1_000_000);
    }

    #[test]
    fn test_unknown_model_marks_all_unpriced_token_fields() {
        let pricing = default_pricing();
        let tokens = TokenUsage {
            input_tokens: 7,
            output_tokens: 1_000_000,
            cache_read_tokens: 42,
            cache_creation_tokens: 33,
            total_tokens: 1_000_000,
            ..Default::default()
        };
        let cost = calculate_cost(&tokens, Some("unknown-model"), &pricing);
        assert!(cost.has_unpriced_usage);
        assert_eq!(cost.total_usd, 0.0);
        assert_eq!(cost.unpriced_input_tokens, 7);
        assert_eq!(cost.unpriced_output_tokens, 1_000_000);
        assert_eq!(cost.unpriced_cache_read_tokens, 42);
        assert_eq!(cost.unpriced_cache_creation_tokens, 33);
    }

    #[test]
    fn test_zero_tokens() {
        let pricing = default_pricing();
        let tokens = TokenUsage::default();
        let cost = calculate_cost(&tokens, Some("claude-opus-4-6"), &pricing);
        assert_eq!(cost.total_usd, 0.0);
        assert!(!cost.has_unpriced_usage);
    }

    #[test]
    fn test_cache_savings() {
        let pricing = test_pricing();
        let tokens = TokenUsage {
            cache_read_tokens: 1_000_000,
            total_tokens: 1_000_000,
            ..Default::default()
        };
        let cost = calculate_cost(&tokens, Some("claude-opus-4-6"), &pricing);
        // cache_read: 200k at $0.50/M + 800k at $1.00/M = $0.10 + $0.80 = $0.90
        assert!((cost.cache_read_cost_usd - 0.90).abs() < 0.001);
        // savings: tiered input cost ($9.00) - tiered cache read cost ($0.90) = $8.10
        assert!((cost.cache_savings_usd - 8.10).abs() < 0.001);
    }

    #[test]
    fn test_prefix_lookup_sonnet_46_dated() {
        let pricing = default_pricing();
        assert!(lookup_pricing("claude-sonnet-4-6-20260301", &pricing).is_some());
    }

    #[test]
    fn test_tiered_cache_read_above_200k() {
        let pricing = test_pricing();
        let tokens = TokenUsage {
            cache_read_tokens: 500_000,
            ..Default::default()
        };
        let cost = calculate_cost(&tokens, Some("claude-opus-4-6"), &pricing);
        // First 200k at $0.50/M = $0.10, remaining 300k at $1.00/M = $0.30 => $0.40
        assert!((cost.cache_read_cost_usd - 0.40).abs() < 0.001);
        assert!(!cost.has_unpriced_usage);
    }

    #[test]
    fn test_tiered_cache_creation_above_200k() {
        let pricing = test_pricing();
        let tokens = TokenUsage {
            cache_creation_tokens: 500_000,
            ..Default::default()
        };
        let cost = calculate_cost(&tokens, Some("claude-opus-4-6"), &pricing);
        // First 200k at $6.25/M = $1.25, remaining 300k at $12.50/M = $3.75 => $5.00
        assert!((cost.cache_creation_cost_usd - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_1hr_cache_tokens_use_higher_rate() {
        let pricing = test_pricing();
        let tokens = TokenUsage {
            cache_creation_tokens: 100_000,
            cache_creation_5m_tokens: 0,
            cache_creation_1hr_tokens: 100_000,
            ..Default::default()
        };
        let cost = calculate_cost(&tokens, Some("claude-opus-4-6"), &pricing);
        // 100k tokens at 1hr rate $10/M = $1.00
        assert!((cost.cache_creation_cost_usd - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_mixed_5m_and_1hr_cache_tokens() {
        let pricing = test_pricing();
        let tokens = TokenUsage {
            cache_creation_tokens: 200_000,
            cache_creation_5m_tokens: 100_000,
            cache_creation_1hr_tokens: 100_000,
            ..Default::default()
        };
        let cost = calculate_cost(&tokens, Some("claude-opus-4-6"), &pricing);
        // 100k at 5m rate $6.25/M = $0.625
        // 100k at 1hr rate $10/M = $1.00
        // total = $1.625
        assert!((cost.cache_creation_cost_usd - 1.625).abs() < 0.001);
    }

    #[test]
    fn test_no_split_falls_back_to_total_with_tiering() {
        let pricing = test_pricing();
        let tokens = TokenUsage {
            cache_creation_tokens: 500_000,
            cache_creation_5m_tokens: 0,
            cache_creation_1hr_tokens: 0,
            ..Default::default()
        };
        let cost = calculate_cost(&tokens, Some("claude-opus-4-6"), &pricing);
        // 200k at $6.25/M + 300k at $12.50/M = $1.25 + $3.75 = $5.00
        assert!((cost.cache_creation_cost_usd - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_finalize_cost_breakdown_marks_partial_with_unpriced_tokens() {
        let mut cost = CostBreakdown {
            total_usd: 1.0,
            unpriced_output_tokens: 500,
            ..Default::default()
        };
        let tokens = TokenUsage {
            input_tokens: 1000,
            output_tokens: 500,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
            total_tokens: 1500,
            ..Default::default()
        };

        finalize_cost_breakdown(&mut cost, &tokens);

        assert!(cost.has_unpriced_usage);
        assert_eq!(cost.total_cost_source, "computed_priced_tokens_partial");
        assert!((cost.priced_token_coverage - (1000.0 / 1500.0)).abs() < 1e-9);
    }

    /// Helper: assert Option<f64> is Some and approximately equal.
    fn assert_approx(actual: Option<f64>, expected: f64, label: &str) {
        let v = actual.unwrap_or_else(|| panic!("{label}: expected Some, got None"));
        assert!(
            (v - expected).abs() < 1e-15,
            "{label}: expected {expected}, got {v}"
        );
    }

    #[test]
    fn test_fill_tiering_gaps_derives_from_base_rates() {
        let mut pricing = HashMap::new();
        pricing.insert(
            "test-model".to_string(),
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

        fill_tiering_gaps(&mut pricing);
        let m = pricing.get("test-model").unwrap();
        assert_approx(m.input_cost_per_token_above_200k, 10e-6, "input_above_200k");
        assert_approx(
            m.output_cost_per_token_above_200k,
            37.5e-6,
            "output_above_200k",
        );
        assert_approx(
            m.cache_creation_cost_per_token_above_200k,
            12.5e-6,
            "cache_create_above_200k",
        );
        assert_approx(
            m.cache_read_cost_per_token_above_200k,
            1e-6,
            "cache_read_above_200k",
        );
        assert_approx(
            m.cache_creation_cost_per_token_1hr,
            10e-6,
            "cache_create_1hr",
        );
    }

    #[test]
    fn test_fill_tiering_gaps_preserves_explicit_values() {
        let mut pricing = HashMap::new();
        pricing.insert(
            "test-model".to_string(),
            ModelPricing {
                input_cost_per_token: 5e-6,
                output_cost_per_token: 25e-6,
                cache_creation_cost_per_token: 6.25e-6,
                cache_read_cost_per_token: 0.5e-6,
                input_cost_per_token_above_200k: Some(99e-6), // explicit — should NOT be overwritten
                output_cost_per_token_above_200k: None,
                cache_creation_cost_per_token_above_200k: None,
                cache_read_cost_per_token_above_200k: None,
                cache_creation_cost_per_token_1hr: None,
            },
        );

        fill_tiering_gaps(&mut pricing);
        let m = pricing.get("test-model").unwrap();
        assert_approx(
            m.input_cost_per_token_above_200k,
            99e-6,
            "input_above_200k preserved",
        );
        assert_approx(
            m.output_cost_per_token_above_200k,
            37.5e-6,
            "output_above_200k derived",
        );
    }
}
