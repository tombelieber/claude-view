// crates/db/src/pricing.rs
//! Per-model pricing for accurate cost calculation.
//!
//! Embeds a static pricing table sourced from Anthropic API docs
//! (https://platform.claude.com/docs/en/about-claude/pricing).
//! Covers all Claude models (current + legacy/deprecated). Unknown models
//! fall back to a blended rate.

use std::collections::HashMap;

/// Per-model pricing in USD per token.
#[derive(Debug, Clone)]
pub struct ModelPricing {
    pub input_cost_per_token: f64,
    pub output_cost_per_token: f64,
    pub cache_creation_cost_per_token: f64,
    pub cache_read_cost_per_token: f64,
    pub input_cost_per_token_above_200k: Option<f64>,
    pub output_cost_per_token_above_200k: Option<f64>,
}

/// Token breakdown for a single model's usage.
pub struct TokenBreakdown {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_creation_tokens: i64,
}

/// Calculate cost in USD for a token breakdown using model-specific pricing.
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

/// Complete Anthropic pricing table (last updated 2026-02-07).
///
/// Source: https://platform.claude.com/docs/en/about-claude/pricing
///
/// Cache pricing multipliers (from Anthropic docs):
///   - 5-min cache write = 1.25x base input
///   - Cache read         = 0.10x base input
pub fn default_pricing() -> HashMap<String, ModelPricing> {
    let mut m = HashMap::new();

    // ===== Current generation =====

    // Claude Opus 4.6 — $5/$25, long context: $10/$37.50 above 200k
    m.insert(
        "claude-opus-4-6".into(),
        ModelPricing {
            input_cost_per_token: 5e-6,   // $5/M
            output_cost_per_token: 25e-6, // $25/M
            cache_creation_cost_per_token: 6.25e-6,
            cache_read_cost_per_token: 0.5e-6,
            input_cost_per_token_above_200k: Some(10e-6),
            output_cost_per_token_above_200k: Some(37.5e-6),
        },
    );

    // Claude Sonnet 4.5 — $3/$15, long context: $6/$22.50 above 200k
    m.insert(
        "claude-sonnet-4-5-20250929".into(),
        ModelPricing {
            input_cost_per_token: 3e-6,   // $3/M
            output_cost_per_token: 15e-6, // $15/M
            cache_creation_cost_per_token: 3.75e-6,
            cache_read_cost_per_token: 0.3e-6,
            input_cost_per_token_above_200k: Some(6e-6),
            output_cost_per_token_above_200k: Some(22.5e-6),
        },
    );

    // Claude Haiku 4.5 — $1/$5
    m.insert(
        "claude-haiku-4-5-20251001".into(),
        ModelPricing {
            input_cost_per_token: 1e-6,  // $1/M
            output_cost_per_token: 5e-6, // $5/M
            cache_creation_cost_per_token: 1.25e-6,
            cache_read_cost_per_token: 0.1e-6,
            input_cost_per_token_above_200k: None,
            output_cost_per_token_above_200k: None,
        },
    );

    // ===== Legacy models (still available) =====

    // Claude Opus 4.5 — $5/$25
    m.insert(
        "claude-opus-4-5-20251101".into(),
        ModelPricing {
            input_cost_per_token: 5e-6,
            output_cost_per_token: 25e-6,
            cache_creation_cost_per_token: 6.25e-6,
            cache_read_cost_per_token: 0.5e-6,
            input_cost_per_token_above_200k: None,
            output_cost_per_token_above_200k: None,
        },
    );

    // Claude Opus 4.1 — $15/$75
    m.insert(
        "claude-opus-4-1-20250805".into(),
        ModelPricing {
            input_cost_per_token: 15e-6,  // $15/M
            output_cost_per_token: 75e-6, // $75/M
            cache_creation_cost_per_token: 18.75e-6,
            cache_read_cost_per_token: 1.5e-6,
            input_cost_per_token_above_200k: None,
            output_cost_per_token_above_200k: None,
        },
    );

    // Claude Opus 4 — $15/$75
    m.insert(
        "claude-opus-4-20250514".into(),
        ModelPricing {
            input_cost_per_token: 15e-6,
            output_cost_per_token: 75e-6,
            cache_creation_cost_per_token: 18.75e-6,
            cache_read_cost_per_token: 1.5e-6,
            input_cost_per_token_above_200k: None,
            output_cost_per_token_above_200k: None,
        },
    );

    // Claude Sonnet 4 — $3/$15, long context: $6/$22.50 above 200k
    m.insert(
        "claude-sonnet-4-20250514".into(),
        ModelPricing {
            input_cost_per_token: 3e-6,
            output_cost_per_token: 15e-6,
            cache_creation_cost_per_token: 3.75e-6,
            cache_read_cost_per_token: 0.3e-6,
            input_cost_per_token_above_200k: Some(6e-6),
            output_cost_per_token_above_200k: Some(22.5e-6),
        },
    );

    // Claude Sonnet 3.7 (deprecated) — $3/$15
    m.insert(
        "claude-3-7-sonnet-20250219".into(),
        ModelPricing {
            input_cost_per_token: 3e-6,
            output_cost_per_token: 15e-6,
            cache_creation_cost_per_token: 3.75e-6,
            cache_read_cost_per_token: 0.3e-6,
            input_cost_per_token_above_200k: None,
            output_cost_per_token_above_200k: None,
        },
    );

    // Claude 3.5 Sonnet v2 (deprecated) — $3/$15
    m.insert(
        "claude-3-5-sonnet-20241022".into(),
        ModelPricing {
            input_cost_per_token: 3e-6,
            output_cost_per_token: 15e-6,
            cache_creation_cost_per_token: 3.75e-6,
            cache_read_cost_per_token: 0.3e-6,
            input_cost_per_token_above_200k: None,
            output_cost_per_token_above_200k: None,
        },
    );

    // Claude 3.5 Sonnet v1 (deprecated) — $3/$15
    m.insert(
        "claude-3-5-sonnet-20240620".into(),
        ModelPricing {
            input_cost_per_token: 3e-6,
            output_cost_per_token: 15e-6,
            cache_creation_cost_per_token: 3.75e-6,
            cache_read_cost_per_token: 0.3e-6,
            input_cost_per_token_above_200k: None,
            output_cost_per_token_above_200k: None,
        },
    );

    // Claude Haiku 3.5 — $0.80/$4
    m.insert(
        "claude-3-5-haiku-20241022".into(),
        ModelPricing {
            input_cost_per_token: 0.8e-6,
            output_cost_per_token: 4e-6,
            cache_creation_cost_per_token: 1e-6,
            cache_read_cost_per_token: 0.08e-6,
            input_cost_per_token_above_200k: None,
            output_cost_per_token_above_200k: None,
        },
    );

    // Claude Opus 3 (deprecated) — $15/$75
    m.insert(
        "claude-3-opus-20240229".into(),
        ModelPricing {
            input_cost_per_token: 15e-6,
            output_cost_per_token: 75e-6,
            cache_creation_cost_per_token: 18.75e-6,
            cache_read_cost_per_token: 1.5e-6,
            input_cost_per_token_above_200k: None,
            output_cost_per_token_above_200k: None,
        },
    );

    // Claude 3 Sonnet (deprecated) — $3/$15
    m.insert(
        "claude-3-sonnet-20240229".into(),
        ModelPricing {
            input_cost_per_token: 3e-6,
            output_cost_per_token: 15e-6,
            cache_creation_cost_per_token: 3.75e-6,
            cache_read_cost_per_token: 0.3e-6,
            input_cost_per_token_above_200k: None,
            output_cost_per_token_above_200k: None,
        },
    );

    // Claude Haiku 3 — $0.25/$1.25
    m.insert(
        "claude-3-haiku-20240307".into(),
        ModelPricing {
            input_cost_per_token: 0.25e-6,
            output_cost_per_token: 1.25e-6,
            cache_creation_cost_per_token: 0.3e-6,
            cache_read_cost_per_token: 0.03e-6,
            input_cost_per_token_above_200k: None,
            output_cost_per_token_above_200k: None,
        },
    );

    m
}

/// Look up pricing for a model ID. Tries exact match, then prefix match
/// (e.g., "claude-sonnet-4-20250514" matches key "claude-sonnet-4-20250514",
/// and "claude-opus-4-6-20260101" would match via prefix on "claude-opus-4-6").
pub fn lookup_pricing<'a>(
    model_id: &str,
    pricing: &'a HashMap<String, ModelPricing>,
) -> Option<&'a ModelPricing> {
    // Exact match first
    if let Some(p) = pricing.get(model_id) {
        return Some(p);
    }
    // Check if any key is a prefix of the model_id
    for (key, p) in pricing {
        if model_id.starts_with(key.as_str()) {
            return Some(p);
        }
    }
    // Check if model_id is a prefix of any key
    for (key, p) in pricing {
        if key.starts_with(model_id) {
            return Some(p);
        }
    }
    None
}

/// Fallback: blended rate for unknown models (matches legacy behavior).
pub const FALLBACK_COST_PER_TOKEN_USD: f64 = 2.5e-6; // $2.50/M tokens

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opus_4_6_pricing() {
        let pricing = default_pricing();
        let p = pricing.get("claude-opus-4-6").unwrap();
        let tokens = TokenBreakdown {
            input_tokens: 1_000_000,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
        };
        // First 200k at $5/M = $1.00, remaining 800k at $10/M = $8.00 = $9.00
        let cost = calculate_cost_usd(&tokens, p);
        assert!((cost - 9.0).abs() < 0.01);
    }

    #[test]
    fn test_opus_4_legacy_pricing() {
        let pricing = default_pricing();
        let p = pricing.get("claude-opus-4-20250514").unwrap();
        let tokens = TokenBreakdown {
            input_tokens: 100_000,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
        };
        // 100k at $15/M = $1.50 (no tiered pricing for Opus 4)
        let cost = calculate_cost_usd(&tokens, p);
        assert!((cost - 1.5).abs() < 0.01);
    }

    #[test]
    fn test_sonnet_small() {
        let pricing = default_pricing();
        let p = pricing.get("claude-sonnet-4-20250514").unwrap();
        let tokens = TokenBreakdown {
            input_tokens: 100_000,
            output_tokens: 50_000,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
        };
        // 100k input at $3/M = $0.30, 50k output at $15/M = $0.75 = $1.05
        let cost = calculate_cost_usd(&tokens, p);
        assert!((cost - 1.05).abs() < 0.01);
    }

    #[test]
    fn test_haiku_45_pricing() {
        let pricing = default_pricing();
        let p = pricing.get("claude-haiku-4-5-20251001").unwrap();
        let tokens = TokenBreakdown {
            input_tokens: 1_000_000,
            output_tokens: 500_000,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
        };
        // 1M input at $1/M = $1.00, 500k output at $5/M = $2.50 = $3.50
        let cost = calculate_cost_usd(&tokens, p);
        assert!((cost - 3.50).abs() < 0.01);
    }

    #[test]
    fn test_haiku_3_pricing() {
        let pricing = default_pricing();
        let p = pricing.get("claude-3-haiku-20240307").unwrap();
        let tokens = TokenBreakdown {
            input_tokens: 1_000_000,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
        };
        // 1M at $0.25/M = $0.25
        let cost = calculate_cost_usd(&tokens, p);
        assert!((cost - 0.25).abs() < 0.01);
    }

    #[test]
    fn test_sonnet_35_v2_exists() {
        let pricing = default_pricing();
        assert!(pricing.get("claude-3-5-sonnet-20241022").is_some());
    }

    #[test]
    fn test_sonnet_35_v1_exists() {
        let pricing = default_pricing();
        assert!(pricing.get("claude-3-5-sonnet-20240620").is_some());
    }

    #[test]
    fn test_unknown_model_fallback() {
        let pricing = default_pricing();
        assert!(lookup_pricing("gpt-4o", &pricing).is_none());
    }

    #[test]
    fn test_tiered_boundary() {
        // Exactly 200k tokens should NOT trigger tiered pricing
        let cost = tiered_cost(200_000, 3e-6, Some(6e-6));
        assert!((cost - 0.6).abs() < 0.001); // 200k * $3/M = $0.60

        // 200,001 tokens should trigger tiered pricing for 1 token
        let cost = tiered_cost(200_001, 3e-6, Some(6e-6));
        assert!((cost - 0.600006).abs() < 0.0001);
    }

    #[test]
    fn test_zero_tokens() {
        let pricing = default_pricing();
        let p = pricing.get("claude-opus-4-6").unwrap();
        let tokens = TokenBreakdown {
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
        };
        assert_eq!(calculate_cost_usd(&tokens, p), 0.0);
    }

    #[test]
    fn test_cache_tokens() {
        let pricing = default_pricing();
        let p = pricing.get("claude-opus-4-6").unwrap();
        let tokens = TokenBreakdown {
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 1_000_000,
            cache_creation_tokens: 100_000,
        };
        // cache_read: 1M * $0.50/M = $0.50
        // cache_create: 100k * $6.25/M = $0.625
        let cost = calculate_cost_usd(&tokens, p);
        assert!((cost - 1.125).abs() < 0.01);
    }

    #[test]
    fn test_prefix_lookup() {
        let pricing = default_pricing();
        // "claude-opus-4-6" should match via exact match
        assert!(lookup_pricing("claude-opus-4-6", &pricing).is_some());
        // A longer variant should match via prefix
        assert!(lookup_pricing("claude-opus-4-6-20260101", &pricing).is_some());
    }

    #[test]
    fn test_all_models_count() {
        let pricing = default_pricing();
        // 14 models: 3 current + 11 legacy/deprecated
        assert_eq!(pricing.len(), 14);
    }
}
