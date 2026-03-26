# Anthropic Model Pricing

## `anthropic-pricing.json`

Static, community-maintainable pricing table for Claude models. Embedded into the binary at compile time — no network dependency, no runtime fetch.

**Source of truth:** [Anthropic Official Pricing](https://docs.anthropic.com/en/docs/about-claude/pricing)

### Why this exists

Previously, pricing was fetched at runtime from [LiteLLM's community JSON](https://github.com/BerriAI/litellm/blob/main/model_prices_and_context_window.json) and merged with hardcoded defaults through a 3-layer pipeline:

```
hardcoded defaults → LiteLLM HTTP fetch → fill_tiering_gaps() multipliers
```

This had problems:

1. **Data quality bug** — LiteLLM provided incorrect above-200k tiered rates for Opus 4.6 and Sonnet 4.6. These models have flat pricing across the full 1M context window per Anthropic's official docs, but LiteLLM applied 2x/1.5x multipliers, overcharging by up to 60% on turns with >200k cached tokens.
2. **Complexity** — 3-tier fallback (LiteLLM → SQLite cache → defaults), background 24h refresh loop, `reqwest` dependency, merge logic with gap-filling multipliers.
3. **No official pricing API** — Anthropic's `/v1/models` endpoint returns capabilities but not pricing. There is no machine-readable pricing feed.

### How it works now

```
anthropic-pricing.json ──include_str!()──→ load_pricing() ──→ HashMap<String, ModelPricing>
```

One file. One function call at startup. No network. No cache. No merge.

### Schema

All rates are in **USD per million tokens** (same units as the official pricing page for easy verification).

```jsonc
{
  "source": "https://docs.anthropic.com/en/docs/about-claude/pricing",
  "last_verified": "2026-03-26",       // date someone last checked against official page
  "unit": "usd_per_million_tokens",
  "models": {
    "claude-opus-4-6": {
      "display_name": "Claude Opus 4.6",
      "input": 5.00,                   // base input rate
      "output": 25.00,                 // base output rate
      "cache_write_5m": 6.25,          // 5-minute TTL cache write (1.25x input)
      "cache_write_1hr": 10.00,        // 1-hour TTL cache write (2x input)
      "cache_read": 0.50,              // cache hit rate (0.1x input)
      "max_input_tokens": 1000000,     // informational, not used for pricing
      "max_output_tokens": 128000,     // informational
      "long_context_pricing": null     // null = flat pricing at all context sizes
    },
    "claude-sonnet-4-5-20250929": {
      // ...base rates...
      "long_context_pricing": {        // non-null = tiered pricing above threshold
        "threshold": 200000,
        "input": 6.00,                 // rate for input tokens above 200k
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

### `long_context_pricing`

- **`null`** — Model has flat pricing across its full context window. All tokens charged at base rate regardless of request size. (e.g., Opus 4.6: "A 900k-token request is billed at the same per-token rate as a 9k-token request.")
- **`{ threshold, ... }`** — Tokens above `threshold` in a single API request are charged at premium rates. (e.g., Sonnet 4.5: input doubles from $3 to $6/MTok above 200k.)

### How to update

When Anthropic updates pricing or launches a new model:

1. Check the [official pricing page](https://docs.anthropic.com/en/docs/about-claude/pricing)
2. Edit `anthropic-pricing.json` — add/update the model entry
3. Update `last_verified` to today's date
4. Submit a PR

The Rust code converts $/MTok to $/token at load time. You don't need to touch any Rust code to add or update a model.

### What was removed

| Component | Status |
|-----------|--------|
| `fetch_litellm_pricing()` | Deleted |
| `merge_pricing()` | Deleted |
| `fill_tiering_gaps()` + multiplier constants | Deleted |
| `default_pricing()` hardcoded table | Replaced by JSON |
| `save/load_pricing_cache()` SQLite persistence | Deleted |
| `refresh_pricing()` + 24h background loop | Deleted |
| `reqwest` dependency in db crate | Removed |

### Verification

The pricing module has 20 tests covering:

- All 15 models load with correct base rates
- Opus 4.6 / Sonnet 4.6 have no above-200k tiering (the bug fix)
- Sonnet 4.5 / Sonnet 4 retain correct above-200k tiering
- Aliases resolve correctly
- Cache savings calculated correctly for both flat and tiered models
- 5m vs 1hr cache TTL splitting
- Unknown models tracked as unpriced (never fake rates)

```sh
cargo test -p claude-view-core -- pricing
```
