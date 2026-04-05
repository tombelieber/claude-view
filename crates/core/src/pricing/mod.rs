//! Pricing engine for Claude model cost calculations.
//!
//! Single source of truth: `data/anthropic-pricing.json` (embedded at compile time).
//! No network dependency, no SQLite cache, no runtime merge.

mod calculate;
mod extract;
mod loader;
mod lookup;
mod types;

pub use calculate::{calculate_cost, calculate_cost_usd, finalize_cost_breakdown};
pub use extract::extract_usage_tokens;
pub use loader::load_pricing;
pub use lookup::{lookup_pricing, resolve_model_alias};
pub use types::{
    CacheStatus, CostBreakdown, ModelPricing, PricingTable, TokenBreakdown, TokenUsage,
};

/// Version of the pricing computation pipeline.
///
/// Bump this when pricing logic changes in a way that affects historical
/// `total_cost_usd` aggregates stored in the database — new cache tiers,
/// extraction bugfixes, new token breakdown fields, or rate changes that
/// would produce different values for the same JSONL input.
///
/// On next server startup, the combined registry+pricing fingerprint will
/// mismatch the stored hash, triggering `mark_all_sessions_for_reindex()`
/// so stored aggregates catch up to the latest computation.
///
/// ## History
///
/// - **v1**: Initial pricing pipeline.
/// - **v2**: 2026-04-05 — `TurnBoundaryAccumulator::add_usage()` switched
///   from untyped `serde_json::Value` to typed `&TokenUsage`, extracting
///   nested `cache_creation.ephemeral_{5m,1h}_input_tokens`. Pre-v2 turns
///   with 1-hour cache usage were under-priced ~37.5% (5m rate applied to
///   1h tokens).
pub const PRICING_VERSION: u32 = 2;
