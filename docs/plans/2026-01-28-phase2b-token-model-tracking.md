---
status: approved
date: 2026-01-28
phase: 2B
depends_on: Phase 2A-2 (Invocable Registry)
---

# Phase 2B: Token & Model Tracking

> Track per-API-call token usage and model identity for every assistant message in indexed sessions.

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Granularity | Per API call | Each assistant JSONL entry = 1 row. No aggregation logic during indexing. |
| Models table | Yes, slimmed | `id`, `provider`, `family`, `first_seen`, `last_seen`. No `display_name`. |
| Session aggregates | Computed from turns | No denormalized columns on `sessions`. `GROUP BY session_id` on turns is fast enough (<200 rows/session). |
| Primary key | `(session_id, uuid)` | Natural key from JSONL. Free idempotency via `INSERT OR IGNORE`. |
| Ordering | `seq INTEGER` column | Position in JSONL file. Stable across re-indexes. |
| Token columns | 4 flat totals + service_tier | `input_tokens`, `output_tokens`, `cache_read_tokens`, `cache_creation_tokens`. No cache tier breakdown. |
| stop_reason / has_error | Skipped | Not present in JSONL data. Add later if Claude Code starts writing them. |
| Endpoints | 2 new + extend existing | `GET /api/models`, `GET /api/stats/tokens`. Extend session responses with token data. |

## JSONL Data Shape (Verified)

Every `type: "assistant"` entry in JSONL contains:

```json
{
  "type": "assistant",
  "uuid": "7885dc6f-...",
  "parentUuid": "439bc9d5-...",
  "timestamp": "2026-01-28T...",
  "message": {
    "model": "claude-opus-4-5-20251101",
    "role": "assistant",
    "content": [{"type": "text", "text": "..."}],
    "usage": {
      "input_tokens": 3,
      "output_tokens": 9,
      "cache_read_input_tokens": 24206,
      "cache_creation_input_tokens": 12454,
      "cache_creation": {
        "ephemeral_5m_input_tokens": 0,
        "ephemeral_1h_input_tokens": 16566
      },
      "service_tier": "standard"
    }
  }
}
```

Key observations:
- Each assistant entry is one API call, not one "turn". A user→assistant turn has 5-20 entries chained by `parentUuid`.
- `input_tokens` is small (2-5k) because most context is served from cache.
- `stop_reason` is always `None` in observed data — not stored.
- `cache_creation` tier breakdown exists but is not stored (YAGNI).

## Schema

### Migration 6

```sql
-- models: normalize model IDs across sessions
CREATE TABLE IF NOT EXISTS models (
    id         TEXT PRIMARY KEY,  -- "claude-opus-4-5-20251101"
    provider   TEXT,              -- "anthropic", "openai", "google"
    family     TEXT,              -- "opus", "sonnet", "haiku"
    first_seen INTEGER,           -- unix timestamp
    last_seen  INTEGER            -- unix timestamp
);

-- turns: one row per assistant API call
CREATE TABLE IF NOT EXISTS turns (
    session_id            TEXT NOT NULL,
    uuid                  TEXT NOT NULL,
    seq                   INTEGER NOT NULL,    -- position in JSONL file
    model_id              TEXT REFERENCES models(id),
    parent_uuid           TEXT,
    content_type          TEXT,                 -- 'text', 'tool_use', 'thinking'
    input_tokens          INTEGER,
    output_tokens         INTEGER,
    cache_read_tokens     INTEGER,
    cache_creation_tokens INTEGER,
    service_tier          TEXT,
    timestamp             INTEGER,              -- unix seconds
    PRIMARY KEY (session_id, uuid)
);

CREATE INDEX IF NOT EXISTS idx_turns_session ON turns(session_id, seq);
CREATE INDEX IF NOT EXISTS idx_turns_model   ON turns(model_id);
```

No changes to `sessions` table. Aggregates computed at query time.

## Extraction Pipeline

Runs inside existing Pass 2 — no new pass, no architectural changes.

### New Rust types

```rust
// crates/core/src/types.rs
pub struct RawTurn {
    pub uuid: String,
    pub parent_uuid: Option<String>,
    pub seq: u32,
    pub model_id: String,
    pub content_type: String,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub cache_creation_tokens: Option<u64>,
    pub service_tier: Option<String>,
    pub timestamp: Option<i64>,
}

// Extend ParseResult
pub struct ParseResult {
    // ...existing fields...
    pub turns: Vec<RawTurn>,
    pub models_seen: Vec<String>,
}
```

### SIMD pre-filter

Two new Finders hoisted at top of `parse_bytes()`:

```rust
let model_finder = memmem::Finder::new(b"\"model\":\"claude");
let usage_finder = memmem::Finder::new(b"\"input_tokens\":");
```

Only parse usage data from lines where `usage_finder` matches. The `model_finder` pattern will need adjustment for multi-provider (e.g. `"model":"gpt`) — for now, keyed to `"input_tokens"` which is provider-agnostic.

### Extraction logic

For each assistant-type line that passes the SIMD filter:
1. Parse JSON (already happening for content extraction)
2. Read `message.model` → collect in `models_seen` set
3. Read `message.usage.*` → build `RawTurn`
4. Read `uuid`, `parentUuid`, first content block type
5. Assign `seq` from line position counter
6. Push to `ParseResult.turns`

### Database writes

After parsing each file in `pass_2_deep_index()`:
1. `batch_upsert_models()` — `INSERT OR IGNORE` + `UPDATE last_seen`
2. `batch_insert_turns()` — `INSERT OR IGNORE` (UUID PK = free dedup on re-index)

Same transaction-batched pattern as `batch_insert_invocations`.

## API Endpoints

### `GET /api/models`

Returns all observed models with usage counts.

```json
[
  {
    "id": "claude-opus-4-5-20251101",
    "provider": "anthropic",
    "family": "opus",
    "first_seen": 1706400000,
    "last_seen": 1706486400,
    "total_turns": 892,
    "total_sessions": 45
  }
]
```

Query:
```sql
SELECT m.*, COUNT(t.uuid) as total_turns,
       COUNT(DISTINCT t.session_id) as total_sessions
FROM models m
LEFT JOIN turns t ON t.model_id = m.id
GROUP BY m.id
ORDER BY total_turns DESC;
```

### `GET /api/stats/tokens`

Returns aggregate token economics.

```json
{
  "total_input_tokens": 26180,
  "total_output_tokens": 1370,
  "total_cache_read_tokens": 13538985,
  "total_cache_creation_tokens": 696394,
  "cache_hit_ratio": 0.95,
  "turns_count": 153,
  "sessions_count": 12
}
```

Query:
```sql
SELECT SUM(input_tokens) as total_input_tokens,
       SUM(output_tokens) as total_output_tokens,
       SUM(cache_read_tokens) as total_cache_read_tokens,
       SUM(cache_creation_tokens) as total_cache_creation_tokens,
       COUNT(*) as turns_count,
       COUNT(DISTINCT session_id) as sessions_count
FROM turns;
```

`cache_hit_ratio` computed in Rust: `cache_read / (input + cache_creation)`.

### Extended: session endpoints

Existing session responses gain token fields via LEFT JOIN:

```sql
LEFT JOIN (
    SELECT session_id,
           SUM(input_tokens) as total_input_tokens,
           SUM(output_tokens) as total_output_tokens,
           SUM(cache_read_tokens) as total_cache_read_tokens,
           SUM(cache_creation_tokens) as total_cache_creation_tokens,
           COUNT(*) as turn_count,
           (SELECT model_id FROM turns t2
            WHERE t2.session_id = t.session_id
            GROUP BY model_id ORDER BY COUNT(*) DESC LIMIT 1
           ) as primary_model
    FROM turns t
    GROUP BY session_id
) tok ON tok.session_id = s.id
```

## Implementation Steps

| # | Step | Files | Test |
|---|------|-------|------|
| 1 | Add `RawTurn` to `types.rs`, extend `ParseResult` | `crates/core/src/types.rs` | Compiles |
| 2 | Migration 6: `models` + `turns` tables | `crates/db/src/migrations.rs` | Migration runs, tables exist |
| 3 | Add `batch_upsert_models()` + `batch_insert_turns()` to queries | `crates/db/src/queries.rs` | Unit tests for insert + dedup |
| 4 | Add model parsing helper: `parse_model_id() → (provider, family)` | `crates/core/src/types.rs` or new `models.rs` | Unit tests for known model strings |
| 5 | Add SIMD Finders + turn extraction in `parse_bytes()` | `crates/db/src/indexer_parallel.rs` | Golden parse test with turns |
| 6 | Integrate into `pass_2_deep_index()`: batch upsert models + insert turns | `crates/db/src/indexer_parallel.rs` | Acceptance test: turns populated after index |
| 7 | Add `get_all_models()` + `get_token_stats()` queries | `crates/db/src/queries.rs` | Unit tests |
| 8 | Add `GET /api/models` route | `crates/server/src/routes/models.rs` | curl test |
| 9 | Add `GET /api/stats/tokens` route | `crates/server/src/routes/stats.rs` or extend existing | curl test |
| 10 | Extend session queries with token LEFT JOIN | `crates/db/src/queries.rs` | Existing session tests still pass + new fields present |
| 11 | Update golden parse test fixtures with turn data | `crates/core/tests/fixtures/` | Golden tests pass |
| 12 | Acceptance test: full pipeline with token verification | `crates/db/tests/acceptance_tests.rs` | AC-13+ pass |

## What This Phase Does NOT Do

- No session health classification (Phase 2C)
- No git commit correlation (Phase 2C)
- No circle-back detection (Phase 2C)
- No daily_stats aggregation table (Phase 2C)
- No cache tier breakdown storage (YAGNI)
- No stop_reason or has_error tracking (data not present in JSONL)
- No frontend UI for token/model data (Phase 3)
