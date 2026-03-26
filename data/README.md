# Anthropic Model Pricing

## `anthropic-pricing.json`

Static pricing table for Claude models. Embedded into the binary at compile time — no network dependency, no runtime fetch. Verified against [official Anthropic pricing](https://platform.claude.com/docs/en/docs/about-claude/pricing) on 2026-03-26.

### Architecture

```
data/anthropic-pricing.json
  │  include_str!() at compile time
  ▼
crates/core/src/pricing/
  ├── loader.rs    load_pricing() → PricingTable  (OnceLock cached, parsed once)
  ├── lookup.rs    lookup_pricing(model_id, &table) → &ModelPricing
  ├── calculate.rs calculate_cost(&TokenUsage, model, &table) → CostBreakdown
  └── types.rs     PricingTable, ModelPricing, TokenUsage, CostBreakdown, CacheStatus
         │
         │  re-exported via crates/db/src/lib.rs
         ▼
crates/server/
  └── state.rs     pricing: Arc<PricingTable>  (immutable, no RwLock)
         │
         ├── SSE /api/live/stream      → CostBreakdown per session (real-time)
         ├── GET /api/sessions/:id/file → CostBreakdown per turn (on-demand)
         ├── GET /api/live/pricing      → rates as $/MTok for frontend display
         ├── GET /api/stats/overview    → aggregate cost breakdown
         └── GET /api/contributions     → per-model cost attribution
```

**Frontend** (`CostBreakdown.tsx`, `CostTooltip.tsx`) receives pre-computed costs via SSE — no client-side pricing calculation.

### Consumers

| Consumer | File | Access pattern |
|---|---|---|
| Live Monitor (SSE) | `server/src/live/manager.rs` | `Arc<PricingTable>` per JSONL line |
| Session detail API | `server/src/routes/sessions.rs` | `Arc::clone` → spawn_blocking |
| Stats aggregation | `server/src/routes/stats.rs` | `&*state.pricing` |
| Contributions | `server/src/routes/contributions.rs` | `&*state.pricing` |
| Cost estimation | `server/src/routes/sessions.rs` | `&*state.pricing` |
| Pricing table API | `server/src/routes/live.rs` | `&*state.pricing` → $/MTok |
| Batch indexer | `db/src/indexer_parallel.rs` | `load_pricing()` (OnceLock) |
| Turn boundary | `core/src/block_accumulator/boundary.rs` | `load_pricing()` (OnceLock) |
| Model seed | `db/src/queries/seed.rs` | `load_pricing().keys()` |

### Schema

All rates are in **USD per million tokens** (same units as the official pricing page).

```jsonc
{
  "source": "https://docs.anthropic.com/en/docs/about-claude/pricing",
  "last_verified": "2026-03-26",
  "unit": "usd_per_million_tokens",
  "models": {
    "claude-opus-4-6": {
      "display_name": "Claude Opus 4.6",
      "input": 5.00,                   // base input rate
      "output": 25.00,                 // base output rate
      "cache_write_5m": 6.25,          // 1.25x input
      "cache_write_1hr": 10.00,        // 2x input
      "cache_read": 0.50,              // 0.1x input
      "max_input_tokens": 1000000,
      "max_output_tokens": 128000,
      "long_context_pricing": null     // null = flat across full context window
    },
    "claude-sonnet-4-5-20250929": {
      // ...base rates...
      "long_context_pricing": {        // non-null = tiered above threshold
        "threshold": 200000,
        "input": 6.00,
        "output": 22.50,
        "cache_write_5m": 7.50,
        "cache_write_1hr": 12.00,
        "cache_read": 0.60
      }
    }
  },
  "aliases": {
    "haiku": "claude-haiku-4-5-20251001",
    "sonnet": "claude-sonnet-4-6",
    "opus": "claude-opus-4-6"
  }
}
```

### Key design decisions

1. **Compile-time embed** — `include_str!()` means zero network dependency. Update pricing = edit JSON + rebuild.
2. **OnceLock caching** — `load_pricing()` parses JSON once per process. Repeated calls return cheap clones.
3. **Immutable `Arc`** — `Arc<PricingTable>` in AppState with no `RwLock`. Zero contention on every request.
4. **Unpriced tracking** — Unknown models get `$0.00` cost with `has_unpriced_usage: true`. Never fake rates.
5. **Per-API-request tiering** — The 200k threshold applies per Anthropic API call, not per session.
6. **Cache TTL awareness** — Distinguishes 5-minute (1.25x) vs 1-hour (2x) cache write rates.

### `long_context_pricing`

- **`null`** — Flat pricing across the full context window. (Opus 4.6, Sonnet 4.6: 1M context at standard rates.)
- **`{ threshold, ... }`** — Tokens above `threshold` in a single API request are charged at premium rates. (Sonnet 4.5/4: 2x above 200k.)

### How to update

When Anthropic updates pricing or launches a new model:

1. Check the [official pricing page](https://platform.claude.com/docs/en/docs/about-claude/pricing)
2. Edit `anthropic-pricing.json` — add/update the model entry
3. Update `last_verified` to today's date
4. Rebuild — no Rust code changes needed

### Verification

20 tests covering all models, tiering, aliases, cache savings, and unpriced tracking:

```sh
cargo test -p claude-view-core -- pricing
```
