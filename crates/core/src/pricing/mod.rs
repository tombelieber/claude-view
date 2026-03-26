//! Pricing engine for Claude model cost calculations.
//!
//! Single source of truth: `data/anthropic-pricing.json` (embedded at compile time).
//! No network dependency, no SQLite cache, no runtime merge.

mod calculate;
mod loader;
mod lookup;
mod types;

pub use calculate::{calculate_cost, calculate_cost_usd, finalize_cost_breakdown, tiered_cost};
pub use loader::load_pricing;
pub use lookup::{lookup_pricing, resolve_model_alias};
pub use types::{CacheStatus, CostBreakdown, ModelPricing, TokenBreakdown, TokenUsage};
