use serde::Serialize;
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
    #[ts(type = "number")]
    pub input_tokens: u64,
    #[ts(type = "number")]
    pub output_tokens: u64,
    #[ts(type = "number")]
    pub cache_read_tokens: u64,
    #[ts(type = "number")]
    pub cache_creation_tokens: u64,
    /// Cache creation tokens with 5-minute TTL (from JSONL ephemeral_5m_input_tokens).
    #[ts(type = "number")]
    pub cache_creation_5m_tokens: u64,
    /// Cache creation tokens with 1-hour TTL (from JSONL ephemeral_1h_input_tokens).
    #[ts(type = "number")]
    pub cache_creation_1hr_tokens: u64,
    #[ts(type = "number")]
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
    #[ts(type = "number")]
    pub unpriced_input_tokens: u64,
    #[ts(type = "number")]
    pub unpriced_output_tokens: u64,
    #[ts(type = "number")]
    pub unpriced_cache_read_tokens: u64,
    #[ts(type = "number")]
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
