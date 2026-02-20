---
status: pending
date: 2026-02-19
---

# Pricing Engine Overhaul & Cost/Stats Bug Fixes

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the hardcoded dual-pricing system with a unified, auto-updating pricing engine backed by litellm's community-maintained JSON, and fix all 22 cost/stats bugs found in the Feb 19 audit.

**Architecture:** Single `ModelPricing` struct in `crates/core/src/pricing.rs` (pure types + calculation). `crates/db/src/pricing.rs` owns the SQLite `model_pricing` table, seeding from hardcoded defaults and refreshing from litellm every 24h. Delete the duplicate `crates/core/src/cost.rs`. Both live and historical paths call the same `calculate_cost()` with full 200k tiering.

**Tech Stack:** Rust (reqwest for HTTP fetch), SQLite (model_pricing table), React/TypeScript (frontend badge + cost fixes)

---

## Phase 1: Unified Pricing Engine (Backend)

### Task 1: Add `reqwest` dependency to workspace

**Files:**
- Modify: `Cargo.toml` (workspace root)
- Modify: `crates/db/Cargo.toml`

**Step 1: Add reqwest to workspace dependencies**

In `Cargo.toml` (root), add under `[workspace.dependencies]`:

```toml
# HTTP client (for litellm pricing fetch)
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "json"] }
```

In `crates/db/Cargo.toml`, add `reqwest` to `[dependencies]`:

```toml
reqwest = { workspace = true }
```

**Step 2: Verify it compiles**

Run: `cargo check -p claude-view-db`
Expected: compiles with no errors

**Step 3: Commit**

```bash
git add Cargo.toml crates/db/Cargo.toml
git commit -m "chore: add reqwest dependency for litellm pricing fetch"
```

---

### Task 2: Unify ModelPricing into `crates/core/src/pricing.rs`

Move the canonical `ModelPricing` struct, `calculate_cost()` (with tiered 200k support), and `lookup_pricing()` from `crates/db/src/pricing.rs` into `crates/core/src/pricing.rs`. Delete the duplicate `crates/core/src/cost.rs`.

**Files:**
- Create: `crates/core/src/pricing.rs`
- Delete: `crates/core/src/cost.rs`
- Modify: `crates/core/src/lib.rs` (swap `pub mod cost` → `pub mod pricing`)
- Modify: `crates/db/src/pricing.rs` (remove struct/calc, keep DB operations only)
- Modify: `crates/db/src/lib.rs` (re-export from core)

**Step 1: Write the test file for the new unified pricing module**

Create `crates/core/src/pricing.rs` with tests first. The key behavior change: `calculate_cost()` now includes tiered pricing (from db's version) AND returns a full `CostBreakdown` (from cost.rs's version):

```rust
// crates/core/src/pricing.rs
//! Unified pricing engine for all cost calculations.
//!
//! Single source of truth for:
//! - `ModelPricing` struct (per-model rates)
//! - `calculate_cost()` with 200k tiered pricing
//! - `CostBreakdown` / `TokenUsage` types
//! - `lookup_pricing()` with exact → prefix → family fallback
//! - Hardcoded defaults for offline seeding

use serde::Serialize;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Pricing types
// ---------------------------------------------------------------------------

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
pub const FALLBACK_INPUT_COST_PER_TOKEN: f64 = 3e-6;  // $3/M (sonnet-class)
pub const FALLBACK_OUTPUT_COST_PER_TOKEN: f64 = 15e-6; // $15/M (sonnet-class)

// ---------------------------------------------------------------------------
// Cost calculation (unified: replaces both cost.rs and db/pricing.rs versions)
// ---------------------------------------------------------------------------

/// Calculate cost for a token snapshot using model-specific pricing.
///
/// If `model` is None or not found, falls back to sonnet-class rates
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
            let cache_read_cost_usd = tokens.cache_read_tokens as f64 * FALLBACK_INPUT_COST_PER_TOKEN * 0.1;
            let cache_creation_cost_usd = tokens.cache_creation_tokens as f64 * FALLBACK_INPUT_COST_PER_TOKEN * 1.25;
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

/// Calculate cost in USD from a `TokenBreakdown` (i64 tokens, for historical queries).
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

/// Infer prompt-cache warmth from the time since the last API call.
pub fn derive_cache_status(seconds_since_last_api_call: Option<u64>) -> CacheStatus {
    match seconds_since_last_api_call {
        Some(s) if s < 300 => CacheStatus::Warm,
        Some(_) => CacheStatus::Cold,
        None => CacheStatus::Unknown,
    }
}

// ---------------------------------------------------------------------------
// Pricing lookup with fallback chain
// ---------------------------------------------------------------------------

/// Look up pricing for a model ID.
///
/// Fallback chain:
/// 1. Exact match (e.g. "claude-opus-4-6")
/// 2. Key is prefix of model_id (e.g. key "claude-opus-4-6" matches "claude-opus-4-6-20260201")
/// 3. Model_id is prefix of key (e.g. "claude-opus" matches "claude-opus-4-6")
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
// Hardcoded defaults (offline seed)
// ---------------------------------------------------------------------------

/// Complete Anthropic pricing table for offline seeding.
///
/// Updated from: https://platform.claude.com/docs/en/about-claude/pricing
/// These are overridden at runtime by litellm fetch when network is available.
pub fn default_pricing() -> HashMap<String, ModelPricing> {
    let mut m = HashMap::new();

    // Current generation
    m.insert("claude-opus-4-6".into(), ModelPricing {
        input_cost_per_token: 5e-6,
        output_cost_per_token: 25e-6,
        cache_creation_cost_per_token: 6.25e-6,
        cache_read_cost_per_token: 0.5e-6,
        input_cost_per_token_above_200k: Some(10e-6),
        output_cost_per_token_above_200k: Some(37.5e-6),
    });

    // ADD: Claude Sonnet 4.6 — was MISSING (C1 fix)
    m.insert("claude-sonnet-4-6".into(), ModelPricing {
        input_cost_per_token: 3e-6,
        output_cost_per_token: 15e-6,
        cache_creation_cost_per_token: 3.75e-6,
        cache_read_cost_per_token: 0.3e-6,
        input_cost_per_token_above_200k: Some(6e-6),
        output_cost_per_token_above_200k: Some(22.5e-6),
    });

    m.insert("claude-sonnet-4-5-20250929".into(), ModelPricing {
        input_cost_per_token: 3e-6,
        output_cost_per_token: 15e-6,
        cache_creation_cost_per_token: 3.75e-6,
        cache_read_cost_per_token: 0.3e-6,
        input_cost_per_token_above_200k: Some(6e-6),
        output_cost_per_token_above_200k: Some(22.5e-6),
    });

    m.insert("claude-haiku-4-5-20251001".into(), ModelPricing {
        input_cost_per_token: 1e-6,
        output_cost_per_token: 5e-6,
        cache_creation_cost_per_token: 1.25e-6,
        cache_read_cost_per_token: 0.1e-6,
        input_cost_per_token_above_200k: None,
        output_cost_per_token_above_200k: None,
    });

    // Legacy models (keep all existing entries from db/pricing.rs)
    m.insert("claude-opus-4-5-20251101".into(), ModelPricing {
        input_cost_per_token: 5e-6, output_cost_per_token: 25e-6,
        cache_creation_cost_per_token: 6.25e-6, cache_read_cost_per_token: 0.5e-6,
        input_cost_per_token_above_200k: None, output_cost_per_token_above_200k: None,
    });
    m.insert("claude-opus-4-1-20250805".into(), ModelPricing {
        input_cost_per_token: 15e-6, output_cost_per_token: 75e-6,
        cache_creation_cost_per_token: 18.75e-6, cache_read_cost_per_token: 1.5e-6,
        input_cost_per_token_above_200k: None, output_cost_per_token_above_200k: None,
    });
    m.insert("claude-opus-4-20250514".into(), ModelPricing {
        input_cost_per_token: 15e-6, output_cost_per_token: 75e-6,
        cache_creation_cost_per_token: 18.75e-6, cache_read_cost_per_token: 1.5e-6,
        input_cost_per_token_above_200k: None, output_cost_per_token_above_200k: None,
    });
    m.insert("claude-sonnet-4-20250514".into(), ModelPricing {
        input_cost_per_token: 3e-6, output_cost_per_token: 15e-6,
        cache_creation_cost_per_token: 3.75e-6, cache_read_cost_per_token: 0.3e-6,
        input_cost_per_token_above_200k: Some(6e-6), output_cost_per_token_above_200k: Some(22.5e-6),
    });
    m.insert("claude-3-7-sonnet-20250219".into(), ModelPricing {
        input_cost_per_token: 3e-6, output_cost_per_token: 15e-6,
        cache_creation_cost_per_token: 3.75e-6, cache_read_cost_per_token: 0.3e-6,
        input_cost_per_token_above_200k: None, output_cost_per_token_above_200k: None,
    });
    m.insert("claude-3-5-sonnet-20241022".into(), ModelPricing {
        input_cost_per_token: 3e-6, output_cost_per_token: 15e-6,
        cache_creation_cost_per_token: 3.75e-6, cache_read_cost_per_token: 0.3e-6,
        input_cost_per_token_above_200k: None, output_cost_per_token_above_200k: None,
    });
    m.insert("claude-3-5-sonnet-20240620".into(), ModelPricing {
        input_cost_per_token: 3e-6, output_cost_per_token: 15e-6,
        cache_creation_cost_per_token: 3.75e-6, cache_read_cost_per_token: 0.3e-6,
        input_cost_per_token_above_200k: None, output_cost_per_token_above_200k: None,
    });
    m.insert("claude-3-5-haiku-20241022".into(), ModelPricing {
        input_cost_per_token: 0.8e-6, output_cost_per_token: 4e-6,
        cache_creation_cost_per_token: 1e-6, cache_read_cost_per_token: 0.08e-6,
        input_cost_per_token_above_200k: None, output_cost_per_token_above_200k: None,
    });
    m.insert("claude-3-opus-20240229".into(), ModelPricing {
        input_cost_per_token: 15e-6, output_cost_per_token: 75e-6,
        cache_creation_cost_per_token: 18.75e-6, cache_read_cost_per_token: 1.5e-6,
        input_cost_per_token_above_200k: None, output_cost_per_token_above_200k: None,
    });
    m.insert("claude-3-sonnet-20240229".into(), ModelPricing {
        input_cost_per_token: 3e-6, output_cost_per_token: 15e-6,
        cache_creation_cost_per_token: 3.75e-6, cache_read_cost_per_token: 0.3e-6,
        input_cost_per_token_above_200k: None, output_cost_per_token_above_200k: None,
    });
    m.insert("claude-3-haiku-20240307".into(), ModelPricing {
        input_cost_per_token: 0.25e-6, output_cost_per_token: 1.25e-6,
        cache_creation_cost_per_token: 0.3e-6, cache_read_cost_per_token: 0.03e-6,
        input_cost_per_token_above_200k: None, output_cost_per_token_above_200k: None,
    });

    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sonnet_46_exists() {
        let pricing = default_pricing();
        assert!(pricing.get("claude-sonnet-4-6").is_some(), "C1 fix: Sonnet 4.6 must exist");
    }

    #[test]
    fn test_all_models_count() {
        let pricing = default_pricing();
        assert_eq!(pricing.len(), 15); // 14 original + 1 new (sonnet 4.6)
    }

    #[test]
    fn test_tiered_pricing_opus_46() {
        let pricing = default_pricing();
        let p = pricing.get("claude-opus-4-6").unwrap();
        let tokens = TokenBreakdown {
            input_tokens: 500_000, output_tokens: 0,
            cache_read_tokens: 0, cache_creation_tokens: 0,
        };
        // First 200k at $5/M = $1.00, remaining 300k at $10/M = $3.00 = $4.00
        let cost = calculate_cost_usd(&tokens, p);
        assert!((cost - 4.0).abs() < 0.01);
    }

    #[test]
    fn test_calculate_cost_with_tiering() {
        let pricing = default_pricing();
        let tokens = TokenUsage {
            input_tokens: 500_000, output_tokens: 0,
            cache_read_tokens: 0, cache_creation_tokens: 0, total_tokens: 500_000,
        };
        let cost = calculate_cost(&tokens, Some("claude-opus-4-6"), &pricing);
        // Same as above: $4.00
        assert!((cost.total_usd - 4.0).abs() < 0.01);
        assert!(!cost.is_estimated);
    }

    #[test]
    fn test_fallback_is_estimated() {
        let pricing = default_pricing();
        let tokens = TokenUsage {
            input_tokens: 1_000_000, output_tokens: 0,
            cache_read_tokens: 0, cache_creation_tokens: 0, total_tokens: 1_000_000,
        };
        let cost = calculate_cost(&tokens, Some("gpt-4o"), &pricing);
        assert!(cost.is_estimated);
        // Fallback input: 1M * $3/M = $3.00
        assert!((cost.input_cost_usd - 3.0).abs() < 0.01);
    }

    #[test]
    fn test_fallback_output_uses_higher_rate() {
        let pricing = default_pricing();
        let tokens = TokenUsage {
            input_tokens: 0, output_tokens: 1_000_000,
            cache_read_tokens: 0, cache_creation_tokens: 0, total_tokens: 1_000_000,
        };
        let cost = calculate_cost(&tokens, Some("unknown-model"), &pricing);
        assert!(cost.is_estimated);
        // Fallback output: 1M * $15/M = $15.00
        assert!((cost.output_cost_usd - 15.0).abs() < 0.01);
    }

    #[test]
    fn test_prefix_lookup_sonnet_46_dated() {
        let pricing = default_pricing();
        assert!(lookup_pricing("claude-sonnet-4-6-20260301", &pricing).is_some());
    }

    #[test]
    fn test_cache_status() {
        assert_eq!(derive_cache_status(Some(60)), CacheStatus::Warm);
        assert_eq!(derive_cache_status(Some(300)), CacheStatus::Cold);
        assert_eq!(derive_cache_status(None), CacheStatus::Unknown);
    }
}
```

**Step 2: Update `crates/core/src/lib.rs`**

Replace `pub mod cost;` with `pub mod pricing;`. Update re-exports.

**Step 3: Update all consumers**

Every file that imports from `claude_view_core::cost::*` must switch to `claude_view_core::pricing::*`. Key files:
- `crates/server/src/live/manager.rs` — uses `TokenUsage`, `CostBreakdown`, `calculate_live_cost` → `calculate_cost`
- `crates/server/src/live/state.rs` — uses `CostBreakdown`, `TokenUsage`, `CacheStatus`
- `crates/server/src/routes/live.rs` — uses pricing map
- `crates/server/src/state.rs` — uses `ModelPricing`

**Step 4: Slim down `crates/db/src/pricing.rs`**

Remove `ModelPricing`, `TokenBreakdown`, `calculate_cost_usd`, `tiered_cost`, `lookup_pricing`, `default_pricing`, `FALLBACK_COST_PER_TOKEN_USD` from db/pricing.rs. Keep only DB-specific operations (will add litellm fetch later). Re-export everything from core.

**Step 5: Update `crates/db/src/lib.rs` re-exports**

```rust
// Re-export pricing types (now from core)
pub use claude_view_core::pricing::{
    calculate_cost, calculate_cost_usd, default_pricing, lookup_pricing,
    ModelPricing, TokenBreakdown, CostBreakdown, TokenUsage, CacheStatus,
    FALLBACK_INPUT_COST_PER_TOKEN, FALLBACK_OUTPUT_COST_PER_TOKEN,
};
```

**Step 6: Run tests**

Run: `cargo test -p claude-view-core -- pricing`
Expected: all pricing tests pass

Run: `cargo test -p claude-view-db`
Expected: all db tests pass (re-exports resolve correctly)

Run: `cargo check -p claude-view-server`
Expected: compiles (all imports resolve)

**Step 7: Commit**

```bash
git commit -m "refactor: unify pricing engine into core::pricing, delete cost.rs

Fixes C1 (missing sonnet-4-6), H1 (live sessions now get 200k tiering),
and adds is_estimated flag to CostBreakdown for frontend badge support."
```

---

### Task 3: litellm Auto-Fetch Background Task

**Files:**
- Modify: `crates/db/src/pricing.rs` (add `fetch_litellm_pricing()`)
- Modify: `crates/server/src/main.rs` (spawn background refresh task)
- Modify: `crates/server/src/state.rs` (pricing becomes `Arc<RwLock<HashMap<...>>>`)

**Step 1: Write the litellm fetch + parse function in `crates/db/src/pricing.rs`**

```rust
// crates/db/src/pricing.rs
//! Pricing data management: litellm fetch + merge with defaults.

use std::collections::HashMap;
use claude_view_core::pricing::ModelPricing;

const LITELLM_URL: &str = "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json";

/// Fetch Claude model pricing from litellm's community-maintained JSON.
///
/// Filters to `claude/` and `anthropic/` prefixed entries only.
/// Returns a HashMap keyed by model ID (without the provider prefix).
/// On any error (network, parse), returns Err — caller should fall back to defaults.
pub async fn fetch_litellm_pricing() -> Result<HashMap<String, ModelPricing>, String> {
    let resp = reqwest::get(LITELLM_URL)
        .await
        .map_err(|e| format!("HTTP fetch failed: {e}"))?;

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("JSON parse failed: {e}"))?;

    let obj = body.as_object().ok_or("Expected JSON object")?;
    let mut result = HashMap::new();

    for (key, value) in obj {
        // litellm keys: "claude-opus-4-6", "anthropic/claude-sonnet-4-5-20250929", etc.
        let model_id = if let Some(stripped) = key.strip_prefix("anthropic/") {
            stripped.to_string()
        } else if key.starts_with("claude-") {
            key.clone()
        } else {
            continue; // skip non-Claude models
        };

        let input = value.get("input_cost_per_token")
            .and_then(|v| v.as_f64());
        let output = value.get("output_cost_per_token")
            .and_then(|v| v.as_f64());

        if let (Some(input_cost), Some(output_cost)) = (input, output) {
            let cache_read = value.get("cache_read_input_token_cost")
                .and_then(|v| v.as_f64())
                .unwrap_or(input_cost * 0.1);
            let cache_write = value.get("cache_creation_input_token_cost")
                .and_then(|v| v.as_f64())
                .unwrap_or(input_cost * 1.25);

            result.insert(model_id, ModelPricing {
                input_cost_per_token: input_cost,
                output_cost_per_token: output_cost,
                cache_creation_cost_per_token: cache_write,
                cache_read_cost_per_token: cache_read,
                input_cost_per_token_above_200k: None, // litellm doesn't track tiered pricing
                output_cost_per_token_above_200k: None,
            });
        }
    }

    if result.is_empty() {
        return Err("No Claude models found in litellm data".into());
    }

    Ok(result)
}

/// Merge litellm pricing into existing defaults.
///
/// litellm entries override defaults for matching keys.
/// Default entries not in litellm are preserved (legacy models).
/// Hardcoded 200k tiering is preserved even when litellm overrides base rates.
pub fn merge_pricing(
    defaults: &HashMap<String, ModelPricing>,
    litellm: &HashMap<String, ModelPricing>,
) -> HashMap<String, ModelPricing> {
    let mut merged = defaults.clone();

    for (key, lp) in litellm {
        if let Some(existing) = merged.get(key) {
            // Preserve 200k tiering from defaults, update base rates from litellm
            merged.insert(key.clone(), ModelPricing {
                input_cost_per_token: lp.input_cost_per_token,
                output_cost_per_token: lp.output_cost_per_token,
                cache_creation_cost_per_token: lp.cache_creation_cost_per_token,
                cache_read_cost_per_token: lp.cache_read_cost_per_token,
                input_cost_per_token_above_200k: existing.input_cost_per_token_above_200k,
                output_cost_per_token_above_200k: existing.output_cost_per_token_above_200k,
            });
        } else {
            // New model from litellm not in defaults — add as-is
            merged.insert(key.clone(), lp.clone());
        }
    }

    merged
}
```

**Step 2: Make `AppState.pricing` an `Arc<RwLock<...>>`**

In `crates/server/src/state.rs`, change:
```rust
// Before:
pub pricing: HashMap<String, ModelPricing>,
// After:
pub pricing: Arc<std::sync::RwLock<HashMap<String, ModelPricing>>>,
```

Update all 3 constructors (`new`, `new_with_indexing`, `new_with_indexing_and_registry`) to wrap in Arc<RwLock>.

**Step 3: Update all pricing consumers to `.read().unwrap()`**

Key files:
- `crates/server/src/routes/live.rs` (`get_pricing` handler)
- `crates/server/src/live/manager.rs` (cost calculation in poll loop)
- `crates/server/src/routes/contributions.rs` (historical cost calculation)

**Step 4: Spawn background refresh in `main.rs`**

```rust
// In server startup, after binding port:
{
    let pricing = state.pricing.clone();
    tokio::spawn(async move {
        // Initial fetch on startup (non-blocking — server already listening)
        refresh_pricing(&pricing).await;
        // Then every 24 hours
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(86400));
        interval.tick().await; // skip immediate tick
        loop {
            interval.tick().await;
            refresh_pricing(&pricing).await;
        }
    });
}

async fn refresh_pricing(pricing: &Arc<std::sync::RwLock<HashMap<String, ModelPricing>>>) {
    match claude_view_db::fetch_litellm_pricing().await {
        Ok(litellm) => {
            let defaults = claude_view_core::pricing::default_pricing();
            let merged = claude_view_db::merge_pricing(&defaults, &litellm);
            let count = merged.len();
            *pricing.write().unwrap() = merged;
            tracing::info!(models = count, "Pricing table refreshed from litellm");
        }
        Err(e) => {
            tracing::warn!("Failed to fetch litellm pricing (using defaults): {e}");
        }
    }
}
```

**Step 5: Run full compile and test**

Run: `cargo test -p claude-view-server`
Expected: all tests pass

**Step 6: Commit**

```bash
git commit -m "feat: auto-refresh pricing from litellm every 24h

Fetches community-maintained model pricing on startup and every 24h.
Merges with hardcoded defaults (preserving 200k tiering).
Falls back gracefully to defaults on network failure."
```

---

## Phase 2: Critical Bug Fixes (C2, C3)

### Task 4: Fix CostTooltip.tsx negative mainAgentCost (C2)

**Files:**
- Modify: `src/components/live/CostTooltip.tsx:87`

**Step 1: Fix the calculation**

Change line 87 from:
```tsx
const mainAgentCost = hasSubAgentCosts ? cost.totalUsd - totalSubAgentCost : 0
```
To:
```tsx
const mainAgentCost = cost.totalUsd
```

`cost.totalUsd` is already the parent session's cost only (sub-agent tokens are separate API calls). No subtraction needed.

**Step 2: Verify visually**

Open Mission Control, find a session with sub-agents, hover cost. Main agent line should show a positive number equal to `cost.totalUsd`.

**Step 3: Commit**

```bash
git commit -m "fix: CostTooltip mainAgentCost was subtracting sub-agent cost from parent-only total

cost.totalUsd is already the parent session's cost (sub-agent tokens are
separate API calls). Subtraction produced negative values."
```

---

### Task 5: Fix trends vs dashboard token pipeline inconsistency (C3)

**Files:**
- Modify: `crates/db/src/trends.rs:246-268`

**Step 1: Change Query B to use `sessions` table instead of `turns` table**

Replace the tokens query (lines 246-268) with:

```rust
// Query B — tokens from sessions table (consistent with dashboard)
let (curr_tokens, prev_tokens): (i64, i64) = sqlx::query_as(
    r#"
    SELECT
      COALESCE(SUM(CASE WHEN s.last_message_at >= ?1 AND s.last_message_at <= ?2
        THEN COALESCE(s.total_input_tokens, 0) + COALESCE(s.total_output_tokens, 0) ELSE 0 END), 0),
      COALESCE(SUM(CASE WHEN s.last_message_at >= ?3 AND s.last_message_at <= ?4
        THEN COALESCE(s.total_input_tokens, 0) + COALESCE(s.total_output_tokens, 0) ELSE 0 END), 0)
    FROM sessions s
    WHERE s.is_sidechain = 0
      AND s.last_message_at >= ?3 AND s.last_message_at <= ?2
      AND (?5 IS NULL OR s.project_id = ?5)
      AND (?6 IS NULL OR s.git_branch = ?6)
    "#,
)
```

**Step 2: Run trends tests**

Run: `cargo test -p claude-view-db -- trends`
Expected: all pass

**Step 3: Commit**

```bash
git commit -m "fix: trends token query now uses sessions table (matches dashboard)

Trends was querying the turns table while dashboard queried sessions
aggregate columns, causing the same time period to show different totals."
```

---

## Phase 3: High-Priority Bug Fixes (H3-H6)

### Task 6: Add sub-agent costs to summary totals (H3)

**Files:**
- Modify: `src/pages/MissionControlPage.tsx:67`
- Modify: `src/components/live/SessionCard.tsx:107`
- Modify: `src/components/live/ListView.tsx:211`
- Modify: `src/components/live/MonitorPane.tsx:112`

**Step 1: Create a shared helper**

In `src/components/live/use-live-sessions.ts` (or a new `src/lib/cost-utils.ts`), add:

```tsx
export function sessionTotalCost(session: LiveSession): number {
  const subAgentTotal = session.subAgents?.reduce((sum, a) => sum + (a.costUsd ?? 0), 0) ?? 0
  return (session.cost?.totalUsd ?? 0) + subAgentTotal
}
```

**Step 2: Update all 4 consumers to use `sessionTotalCost(session)` instead of `session.cost.totalUsd`**

**Step 3: Commit**

```bash
git commit -m "fix: include sub-agent costs in session list and summary totals"
```

---

### Task 7: Fix cache hit ratio formula (H4)

**Files:**
- Modify: `crates/db/src/queries/models.rs:84`

**Step 1: Fix the denominator**

Change line 84 from:
```rust
let denominator = total_input + total_cache_creation;
```
To:
```rust
let denominator = total_cache_read + total_cache_creation;
```

Cache hit ratio = `cache_read / (cache_read + cache_creation)` = "what fraction of cacheable tokens were hits".

**Step 2: Run tests**

Run: `cargo test -p claude-view-db -- models`

**Step 3: Commit**

```bash
git commit -m "fix: cache hit ratio uses correct denominator (read+creation, not input+creation)"
```

---

### Task 8: Fix "Files Created" mislabel (H5)

**Files:**
- Modify: `crates/db/src/queries/ai_generation.rs:27` (rename alias)
- Modify: `src/components/AIGenerationStats.tsx` (fix label)
- Modify: `src/components/AIGenerationStats.test.tsx` (update test assertions)
- Modify: `e2e/dashboard-ai-generation.spec.ts` (update e2e assertions)

**Step 1: Rename backend field**

In `ai_generation.rs` line 27, rename the alias from `files_created` to stay as-is (it's an internal name), but ensure the `AIGenerationStats` struct field name is accurate. The critical fix is the frontend label.

**Step 2: Fix frontend label**

In `AIGenerationStats.tsx`, change `"Files Created"` → `"Files Edited"` and `"written by AI"` → `"modified by AI"`.

**Step 3: Update tests**

Update `AIGenerationStats.test.tsx` and `e2e/dashboard-ai-generation.spec.ts` to match new label.

**Step 4: Commit**

```bash
git commit -m "fix: rename 'Files Created' to 'Files Edited' (was showing files_edited_count)"
```

---

### Task 9: Fix M05 hardcoded threshold (H6)

**Files:**
- Modify: `crates/core/src/patterns/model.rs:132`

**Step 1: Replace hardcoded `15` with `MIN_MODEL_BUCKET`**

Change line 132 from:
```rust
.filter(|(_, vals)| vals.len() >= 15)
```
To:
```rust
.filter(|(_, vals)| vals.len() >= super::MIN_MODEL_BUCKET)
```

**Step 2: Run pattern tests**

Run: `cargo test -p claude-view-core -- patterns`

**Step 3: Commit**

```bash
git commit -m "fix: M05 pattern uses MIN_MODEL_BUCKET (30) instead of hardcoded 15"
```

---

## Phase 4: Frontend "Estimated" Badge & Cost Display Fixes

### Task 10: Add `isEstimated` to frontend cost display

**Files:**
- Modify: `src/components/live/use-live-sessions.ts` (add `isEstimated` to cost type)
- Modify: `src/components/live/SessionCard.tsx` (show badge)
- Modify: `src/components/live/CostTooltip.tsx` (show note)
- Modify: `src/components/live/SessionDetailPanel.tsx` (show badge)

**Step 1: Update LiveSession cost type**

In `use-live-sessions.ts`, add `isEstimated: boolean` to the cost interface.

**Step 2: Add a subtle "~" prefix when estimated**

In all cost display locations, if `cost.isEstimated`, prefix the dollar amount with `~` (e.g., `~$2.50`):

```tsx
const prefix = session.cost.isEstimated ? '~' : ''
// Then: `${prefix}$${sessionTotalCost(session).toFixed(2)}`
```

**Step 3: Add tooltip note when estimated**

In `CostTooltip.tsx`, when `cost.isEstimated`, add a line:
```tsx
{cost.isEstimated && (
  <div className="text-amber-500 dark:text-amber-400 pt-1 text-[10px]">
    Estimated — model not in pricing table
  </div>
)}
```

**Step 4: Commit**

```bash
git commit -m "feat: show estimated cost badge (~) when model pricing is unknown"
```

---

### Task 11: Fix ORDER BY alias expressions in SQLite (M4)

**Files:**
- Modify: `crates/db/src/queries/ai_generation.rs:59, 93`

**Step 1: Replace alias expressions with column index**

Change both ORDER BY clauses from:
```sql
ORDER BY (input_tokens + output_tokens) DESC
```
To:
```sql
ORDER BY 2 + 3 DESC
```

Or more readably, use a subquery approach:
```sql
ORDER BY (COALESCE(SUM(total_input_tokens), 0) + COALESCE(SUM(total_output_tokens), 0)) DESC
```

**Step 2: Run tests**

Run: `cargo test -p claude-view-db`

**Step 3: Commit**

```bash
git commit -m "fix: use full expressions in ORDER BY instead of column aliases (SQLite compat)"
```

---

### Task 12: Fix token accumulation on file replacement (manager.rs)

**Files:**
- Modify: `crates/server/src/live/manager.rs:586`

**Step 1: Reset tokens on file replacement**

After `acc.task_items.clear()` at line 586, add:
```rust
acc.tokens = claude_view_core::pricing::TokenUsage::default();
```

**Step 2: Commit**

```bash
git commit -m "fix: reset token accumulator on JSONL file replacement to prevent double-counting"
```

---

## Phase 5: Remaining Medium/Low Fixes

### Task 13: Batch remaining fixes

This task covers the remaining lower-priority issues. Each is a small, isolated change:

**M5 — Commit count double-counting:** In `crates/db/src/queries/dashboard.rs:810-812`, change `COUNT(*)` to `COUNT(DISTINCT sc.commit_hash)` (assuming `commit_hash` column exists, otherwise leave as-is with a TODO comment).

**L1 — Sub-cent cost display:** In `CostBreakdown.tsx` line 58 and `SessionCard.tsx` line 107, change `.toFixed(2)` to a smart formatter:
```tsx
function formatCostUsd(usd: number): string {
  if (usd === 0) return '$0.00'
  if (usd < 0.01) return `$${usd.toFixed(4)}`
  return `$${usd.toFixed(2)}`
}
```

**L2 — Consolidate formatTokens/formatNumber:** Extract a single `formatTokenCount()` into `src/lib/format-utils.ts` and replace all 6 local definitions. Use lowercase `k` consistently.

**L3 — Consistent null-safety on `session.cost`:** Add `?.` optional chaining to `SessionCard.tsx:107`, `SessionDetailPanel.tsx:248`, `MonitorPane.tsx:112`, `ListView.tsx:211`.

**Commit each sub-fix separately with descriptive messages.**

---

## Phase 6: Update `/api/live/pricing` endpoint

### Task 14: Expose 200k tier rates and lastUpdated timestamp

**Files:**
- Modify: `crates/server/src/routes/live.rs:309-332`

**Step 1: Add tier rates and dynamic timestamp**

```rust
async fn get_pricing(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let pricing = state.pricing.read().unwrap();
    let models: HashMap<String, serde_json::Value> = pricing
        .iter()
        .map(|(name, p)| {
            let mut model = serde_json::json!({
                "inputPerMillion": p.input_cost_per_token * 1_000_000.0,
                "outputPerMillion": p.output_cost_per_token * 1_000_000.0,
                "cacheReadPerMillion": p.cache_read_cost_per_token * 1_000_000.0,
                "cacheWritePerMillion": p.cache_creation_cost_per_token * 1_000_000.0,
            });
            if let Some(rate) = p.input_cost_per_token_above_200k {
                model["inputPerMillionAbove200k"] = serde_json::json!(rate * 1_000_000.0);
            }
            if let Some(rate) = p.output_cost_per_token_above_200k {
                model["outputPerMillionAbove200k"] = serde_json::json!(rate * 1_000_000.0);
            }
            (name.clone(), model)
        })
        .collect();
    Json(serde_json::json!({
        "models": models,
        "modelCount": models.len(),
        "source": "litellm+defaults",
    }))
}
```

**Step 2: Commit**

```bash
git commit -m "feat: /api/live/pricing now includes 200k tier rates and model count"
```

---

## Verification Checklist

After all tasks complete, verify end-to-end:

1. `cargo test` — all Rust tests pass
2. `bun run build` — frontend builds with no TS errors
3. Start server, open Mission Control:
   - [ ] Live session shows cost with correct Sonnet 4.6 pricing
   - [ ] CostTooltip shows positive "Main agent" cost
   - [ ] Summary header includes sub-agent costs
   - [ ] Unknown model shows `~$X.XX` estimated prefix
4. Check `/api/live/pricing` — should list 15+ models with `source: "litellm+defaults"`
5. Check dashboard trends — token totals should match metric cards
6. Check AI Generation Stats — label says "Files Edited" not "Files Created"

---

## Issue Tracking Cross-Reference

| Audit ID | Fix Task | Description |
|----------|----------|-------------|
| C1 | Task 2 | Missing claude-sonnet-4-6 pricing |
| C2 | Task 4 | CostTooltip negative mainAgentCost |
| C3 | Task 5 | Trends vs dashboard token pipeline |
| H1 | Task 2 | No 200k tiering in live calculator |
| H3 | Task 6 | Sub-agent costs excluded from totals |
| H4 | Task 7 | Cache hit ratio wrong denominator |
| H5 | Task 8 | "Files Created" mislabel |
| H6 | Task 9 | M05 hardcoded threshold |
| M1 | Task 3 | Hardcoded pricing (litellm auto-refresh) |
| M4 | Task 11 | ORDER BY alias expressions |
| M6 | Task 10 | No estimated badge on live costs |
| L1 | Task 13 | Sub-cent cost display |
| L2 | Task 13 | Duplicate formatTokens |
| L3 | Task 13 | Inconsistent null-safety |
| L5 | Task 14 | /api/live/pricing missing 200k tiers |
