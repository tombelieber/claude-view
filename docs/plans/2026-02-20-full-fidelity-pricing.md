---
status: approved
date: 2026-02-20
---

# Full-Fidelity Pricing from litellm

## Problem

Our cost calculations have three accuracy gaps:

1. **Missing cache tiering** — Cache write/read costs above 200k tokens are charged at the base rate, but Anthropic charges 2x above 200k (e.g., Opus 4.6 cache write: $6.25/M base vs $12.50/M above 200k).

2. **Missing 1hr cache pricing** — Claude Code uses 1-hour ephemeral caching exclusively (`ephemeral_1h_input_tokens`), but we charge at the 5-minute rate. For Opus 4.6: $6.25/M (5m) vs $10/M (1hr) — a 60% undercharge on every cache write.

3. **litellm tiering ignored** — `fetch_litellm_pricing()` hardcodes `input_cost_per_token_above_200k: None` for all litellm entries, even though litellm now provides `*_above_200k_tokens` fields for most Claude models. Our merge logic then falls back to hardcoded defaults for tiering, which only covers models we manually added.

## Data Sources

**litellm JSON** (fetched at startup) now provides these fields per model:

| Field | Example (Opus 4.6) | Currently used? |
|-------|-------------------|-----------------|
| `input_cost_per_token` | 5e-6 | Yes |
| `output_cost_per_token` | 25e-6 | Yes |
| `cache_creation_input_token_cost` | 6.25e-6 | Yes |
| `cache_read_input_token_cost` | 0.5e-6 | Yes |
| `input_cost_per_token_above_200k_tokens` | 10e-6 | **No** (hardcoded None) |
| `output_cost_per_token_above_200k_tokens` | 37.5e-6 | **No** (hardcoded None) |
| `cache_creation_input_token_cost_above_200k_tokens` | 12.5e-6 | **No** (no field exists) |
| `cache_read_input_token_cost_above_200k_tokens` | 1e-6 | **No** (no field exists) |
| `cache_creation_input_token_cost_above_1hr` | 10e-6 | **No** (no field exists) |

**JSONL** (from Claude Code API responses) already provides cache TTL breakdown:

```json
"cache_creation": {
  "ephemeral_5m_input_tokens": 0,
  "ephemeral_1h_input_tokens": 57339
}
```

We parse `cache_creation_input_tokens` (the total) but ignore this sub-breakdown.

## Design

### 1. Expand `ModelPricing` struct

Add 3 new `Option<f64>` fields to `crates/core/src/pricing.rs`:

```rust
pub struct ModelPricing {
    pub input_cost_per_token: f64,
    pub output_cost_per_token: f64,
    pub cache_creation_cost_per_token: f64,
    pub cache_read_cost_per_token: f64,
    pub input_cost_per_token_above_200k: Option<f64>,
    pub output_cost_per_token_above_200k: Option<f64>,
    // NEW:
    pub cache_creation_cost_per_token_above_200k: Option<f64>,
    pub cache_read_cost_per_token_above_200k: Option<f64>,
    pub cache_creation_cost_per_token_1hr: Option<f64>,
}
```

### 2. Expand `TokenUsage` struct

Add split cache creation fields alongside the existing total:

```rust
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,     // total (unchanged, backward compat)
    pub cache_creation_5m_tokens: u64,  // NEW
    pub cache_creation_1hr_tokens: u64, // NEW
    pub total_tokens: u64,
}
```

`cache_creation_tokens` remains the sum for backward compatibility. The split fields enable accurate 1hr pricing when available.

### 3. Update `calculate_cost()`

```
cache_write_cost =
  IF 1hr tokens available AND 1hr rate exists:
    5m_tokens * cache_creation_rate (with 200k tiering)
    + 1hr_tokens * cache_creation_1hr_rate
  ELSE:
    cache_creation_tokens * cache_creation_rate (with 200k tiering)

cache_read_cost =
  tiered_cost(cache_read_tokens, base_rate, above_200k_rate)
```

The `tiered_cost()` helper already handles the 200k threshold for input/output — reuse it for cache tokens.

### 4. Update JSONL parser (`live_parser.rs`)

Extract `cache_creation.ephemeral_5m_input_tokens` and `cache_creation.ephemeral_1h_input_tokens` from the `usage` object when present. Add to `ParsedLine`:

```rust
pub cache_creation_5m_tokens: Option<u64>,
pub cache_creation_1hr_tokens: Option<u64>,
```

### 5. Update litellm fetch (`crates/db/src/pricing.rs`)

Extract all 5 new fields from the litellm JSON:

```rust
let above_200k_input = value.get("input_cost_per_token_above_200k_tokens").and_then(|v| v.as_f64());
let above_200k_output = value.get("output_cost_per_token_above_200k_tokens").and_then(|v| v.as_f64());
let above_200k_cache_create = value.get("cache_creation_input_token_cost_above_200k_tokens").and_then(|v| v.as_f64());
let above_200k_cache_read = value.get("cache_read_input_token_cost_above_200k_tokens").and_then(|v| v.as_f64());
let cache_create_1hr = value.get("cache_creation_input_token_cost_above_1hr").and_then(|v| v.as_f64());
```

### 6. Update merge logic

litellm now overrides tiering fields too — not just base rates. For each field, prefer litellm value if present, otherwise fall back to hardcoded default:

```rust
// For each Optional tiering field:
field: litellm_pricing.field.or(existing.field)
```

This makes hardcoded defaults truly just a fallback for when litellm is unreachable.

### 7. Update `default_pricing()` hardcoded values

Add the 3 new fields to all 15 model entries using values from current litellm data. Key models:

| Model | cache_create_above_200k | cache_read_above_200k | cache_create_1hr |
|-------|------------------------|----------------------|-----------------|
| claude-opus-4-6 | 12.5e-6 | 1e-6 | 10e-6 |
| claude-sonnet-4-6 | 7.5e-6 | 0.6e-6 | 6e-6 |
| claude-haiku-4-5 | None | None | 2e-6 |
| claude-opus-4-5 | None | None | 10e-6 |
| Legacy models | None | None | None |

### 8. Update accumulator (`manager.rs`)

Accumulate the split cache creation tokens:

```rust
if let Some(tokens_5m) = line.cache_creation_5m_tokens {
    acc.tokens.cache_creation_5m_tokens += tokens_5m;
}
if let Some(tokens_1hr) = line.cache_creation_1hr_tokens {
    acc.tokens.cache_creation_1hr_tokens += tokens_1hr;
}
```

## Files Changed

| File | Change |
|------|--------|
| `crates/core/src/pricing.rs` | ModelPricing struct (+3 fields), TokenUsage struct (+2 fields), calculate_cost(), default_pricing(), tests |
| `crates/core/src/live_parser.rs` | ParsedLine (+2 fields), extract_usage() parses cache_creation breakdown |
| `crates/db/src/pricing.rs` | fetch_litellm_pricing() extracts 5 new fields, merge_pricing() updated |
| `crates/server/src/live/manager.rs` | Accumulator uses split cache tokens |

## No Frontend Changes

`CostBreakdown` already has `cacheCreationCostUsd` — the value just becomes more accurate. No new UI fields needed.

## Impact

For a typical Opus 4.6 session with 200k+ context:
- Cache write cost correction: +60% (1hr rate vs 5m rate)
- Cache tiering correction: +100% on tokens above 200k threshold
- Overall session cost estimate: more accurate by ~5-15% depending on cache hit ratio

## Testing

- Update existing tests in `crates/core/src/pricing.rs` for new fields
- Add test: tiered cache pricing above 200k
- Add test: 1hr cache pricing applied when split tokens available
- Add test: litellm fetch extracts all new fields
- Add test: merge preserves litellm tiering over defaults
