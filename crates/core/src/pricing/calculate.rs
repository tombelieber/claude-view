use std::collections::HashMap;

use super::lookup::lookup_pricing;
use super::types::{CostBreakdown, ModelPricing, TokenBreakdown, TokenUsage};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pricing::load_pricing;

    fn pricing() -> HashMap<String, ModelPricing> {
        load_pricing()
    }

    #[test]
    fn test_opus_46_flat_pricing() {
        let pricing = pricing();
        let tokens = TokenUsage {
            input_tokens: 500_000,
            total_tokens: 500_000,
            ..Default::default()
        };
        let cost = calculate_cost(&tokens, Some("claude-opus-4-6"), &pricing);
        // Opus 4.6 has flat 1M pricing: 500k * $5/MTok = $2.50
        assert!((cost.total_usd - 2.50).abs() < 0.001);
        assert!(!cost.has_unpriced_usage);
    }

    #[test]
    fn test_sonnet_45_tiered_pricing() {
        let pricing = pricing();
        let tokens = TokenUsage {
            input_tokens: 500_000,
            total_tokens: 500_000,
            ..Default::default()
        };
        let cost = calculate_cost(&tokens, Some("claude-sonnet-4-5-20250929"), &pricing);
        // Sonnet 4.5 has tiered pricing: 200k * $3/MTok + 300k * $6/MTok = $0.60 + $1.80 = $2.40
        assert!((cost.total_usd - 2.40).abs() < 0.001);
    }

    #[test]
    fn test_unknown_model_has_no_fake_usd() {
        let pricing = pricing();
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
        let pricing = pricing();
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
        let pricing = pricing();
        let tokens = TokenUsage::default();
        let cost = calculate_cost(&tokens, Some("claude-opus-4-6"), &pricing);
        assert_eq!(cost.total_usd, 0.0);
        assert!(!cost.has_unpriced_usage);
    }

    #[test]
    fn test_cache_savings_opus_46() {
        let pricing = pricing();
        let tokens = TokenUsage {
            cache_read_tokens: 1_000_000,
            total_tokens: 1_000_000,
            ..Default::default()
        };
        let cost = calculate_cost(&tokens, Some("claude-opus-4-6"), &pricing);
        // Opus 4.6 flat: 1M cache_read at $0.50/MTok = $0.50
        assert!((cost.cache_read_cost_usd - 0.50).abs() < 0.001);
        // savings: input cost ($5.00) - cache read cost ($0.50) = $4.50
        assert!((cost.cache_savings_usd - 4.50).abs() < 0.001);
    }

    #[test]
    fn test_cache_savings_sonnet_45_tiered() {
        let pricing = pricing();
        let tokens = TokenUsage {
            cache_read_tokens: 500_000,
            total_tokens: 500_000,
            ..Default::default()
        };
        let cost = calculate_cost(&tokens, Some("claude-sonnet-4-5-20250929"), &pricing);
        // Sonnet 4.5 tiered cache_read: 200k * $0.30/MTok + 300k * $0.60/MTok = $0.06 + $0.18 = $0.24
        assert!((cost.cache_read_cost_usd - 0.24).abs() < 0.001);
    }

    #[test]
    fn test_1hr_cache_tokens_use_higher_rate() {
        let pricing = pricing();
        let tokens = TokenUsage {
            cache_creation_tokens: 100_000,
            cache_creation_5m_tokens: 0,
            cache_creation_1hr_tokens: 100_000,
            ..Default::default()
        };
        let cost = calculate_cost(&tokens, Some("claude-opus-4-6"), &pricing);
        // 100k tokens at 1hr rate $10/MTok = $1.00
        assert!((cost.cache_creation_cost_usd - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_mixed_5m_and_1hr_cache_tokens() {
        let pricing = pricing();
        let tokens = TokenUsage {
            cache_creation_tokens: 200_000,
            cache_creation_5m_tokens: 100_000,
            cache_creation_1hr_tokens: 100_000,
            ..Default::default()
        };
        let cost = calculate_cost(&tokens, Some("claude-opus-4-6"), &pricing);
        // 100k at 5m rate $6.25/MTok = $0.625
        // 100k at 1hr rate $10/MTok = $1.00
        // total = $1.625
        assert!((cost.cache_creation_cost_usd - 1.625).abs() < 0.001);
    }

    #[test]
    fn test_no_split_falls_back_to_total() {
        let pricing = pricing();
        let tokens = TokenUsage {
            cache_creation_tokens: 100_000,
            cache_creation_5m_tokens: 0,
            cache_creation_1hr_tokens: 0,
            ..Default::default()
        };
        let cost = calculate_cost(&tokens, Some("claude-opus-4-6"), &pricing);
        // Opus 4.6 flat: 100k at $6.25/MTok = $0.625
        assert!((cost.cache_creation_cost_usd - 0.625).abs() < 0.001);
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
            total_tokens: 1500,
            ..Default::default()
        };
        finalize_cost_breakdown(&mut cost, &tokens);
        assert!(cost.has_unpriced_usage);
        assert_eq!(cost.total_cost_source, "computed_priced_tokens_partial");
        assert!((cost.priced_token_coverage - (1000.0 / 1500.0)).abs() < 1e-9);
    }
}
