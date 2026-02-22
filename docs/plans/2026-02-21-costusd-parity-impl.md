# costUSD Parity Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Use per-entry `costUSD` from JSONL (with LiteLLM fallback) for cost aggregation, matching ccusage's default "auto" mode.

**Architecture:** Add `costUSD` field to `AssistantLine` struct, accumulate per-session in `ExtendedMetadata`, store in new `total_cost_usd` DB column, and use `SUM(total_cost_usd)` in the aggregate endpoint instead of recalculating from tokens.

**Tech Stack:** Rust (serde, sqlx), SQLite migration

---

### Task 1: Add `costUSD` to `AssistantLine` struct

**Files:**
- Modify: `crates/db/src/indexer_parallel.rs:247-256` (AssistantLine struct)

**Step 1: Add `cost_usd` field to `AssistantLine`**

In `crates/db/src/indexer_parallel.rs`, add the field to `AssistantLine`:

```rust
#[derive(Deserialize)]
struct AssistantLine {
    timestamp: Option<TimestampValue>,
    uuid: Option<String>,
    #[serde(rename = "parentUuid")]
    parent_uuid: Option<String>,
    #[serde(rename = "requestId")]
    request_id: Option<String>,
    message: Option<AssistantMessage>,
    #[serde(rename = "costUSD")]
    cost_usd: Option<f64>,
}
```

Note: `costUSD` is a top-level field in the JSONL entry, not nested inside `message`. Serde will silently ignore it if missing (it's `Option`).

**Step 2: Run tests to verify no regressions**

Run: `cargo test -p claude-view-db -- indexer_parallel`
Expected: All existing tests pass (serde ignores unknown fields by default, and Option fields default to None).

**Step 3: Commit**

```bash
git add crates/db/src/indexer_parallel.rs
git commit -m "feat: parse costUSD from JSONL assistant lines"
```

---

### Task 2: Accumulate `total_cost_usd` in `ExtendedMetadata`

**Files:**
- Modify: `crates/db/src/indexer_parallel.rs:140-220` (ExtendedMetadata struct)
- Modify: `crates/db/src/indexer_parallel.rs:1100-1185` (handle_assistant_line fn, token accumulation block)
- Modify: `crates/db/src/indexer_parallel.rs:1310-1358` (handle_assistant_value fn, token accumulation block)

**Step 1: Add `total_cost_usd` field to `ExtendedMetadata`**

After the `cache_creation_tokens` field (line ~174), add:

```rust
    // Token aggregates (from assistant lines)
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub thinking_block_count: u32,

    /// Accumulated costUSD from JSONL entries (ccusage "auto" mode).
    /// Sum of per-entry `costUSD` when present.
    pub total_cost_usd: f64,
```

This field defaults to `0.0` via `Default` derive. Find where `ExtendedMetadata` is initialized with explicit field values and add `total_cost_usd: 0.0` there too.

**Step 2: Accumulate `cost_usd` in `handle_assistant_line`**

In `handle_assistant_line` (around line 1170-1185), after the token accumulation block inside `if is_first_block`, add:

```rust
        // Extract token usage — only for first content block per API response
        if is_first_block {
            if let Some(ref usage) = message.usage {
                // ... existing token accumulation ...
            }
        }

        // Accumulate costUSD (top-level field, not inside message.usage)
        // Only count on first block to avoid double-counting multi-block responses
        if is_first_block {
            if let Some(cost) = cost_usd {
                deep.total_cost_usd += cost;
            }
        }
```

The `cost_usd` value needs to be passed into `handle_assistant_line`. Add it as a parameter. In the main parsing loop where `handle_assistant_line` is called (around line 792), pass `parsed.cost_usd`.

**Step 3: Accumulate `cost_usd` in `handle_assistant_value` (fallback path)**

In `handle_assistant_value` (around line 1342-1358), after the token accumulation block, add:

```rust
    // Extract token usage from .message.usage — only for first block
    if is_first_block {
        // ... existing token accumulation ...
    }

    // Accumulate costUSD from top-level field
    if is_first_block {
        if let Some(cost) = value.get("costUSD").and_then(|v| v.as_f64()) {
            deep.total_cost_usd += cost;
        }
    }
```

**Step 4: Run tests**

Run: `cargo test -p claude-view-db -- indexer_parallel`
Expected: All tests pass. Existing test fixtures don't have `costUSD`, so `total_cost_usd` stays 0.0.

**Step 5: Commit**

```bash
git add crates/db/src/indexer_parallel.rs
git commit -m "feat: accumulate costUSD per session during JSONL parsing"
```

---

### Task 3: Add `total_cost_usd` DB column and wire into UPDATE

**Files:**
- Modify: `crates/db/src/migrations.rs` (add ALTER TABLE)
- Modify: `crates/db/src/indexer_parallel.rs:37-93` (UPDATE_SESSION_DEEP_SQL, now 55 params)
- Modify: `crates/db/src/indexer_parallel.rs:2313-2369` (rusqlite params block)
- Modify: `crates/db/src/indexer_parallel.rs:30` (bump CURRENT_PARSE_VERSION to 11)
- Modify: `crates/db/src/queries/row_types.rs:134-309` (update_session_deep_fields_tx, now 55 params)

**Step 1: Add migration**

In `crates/db/src/migrations.rs`, add a new migration statement to the migrations array:

```rust
    r#"ALTER TABLE sessions ADD COLUMN total_cost_usd REAL;"#,
```

Note: `REAL` with no `NOT NULL DEFAULT` — `NULL` means "not yet parsed with costUSD support", distinguishable from `0.0` (parsed but zero cost).

**Step 2: Add `total_cost_usd` to UPDATE_SESSION_DEEP_SQL**

In `crates/db/src/indexer_parallel.rs`, extend the SQL constant to include `total_cost_usd = ?55` after `longest_task_preview = ?54`:

```rust
        longest_task_preview = ?54,
        total_cost_usd = ?55
    WHERE id = ?1
```

**Step 3: Add param to rusqlite params block**

In the `update_stmt.execute(rusqlite::params![...])` block (around line 2314-2368), add after `longest_task_preview`:

```rust
                        meta.longest_task_preview,        // ?54 (Option<String>)
                        meta.total_cost_usd,              // ?55 (f64)
```

**Step 4: Bump CURRENT_PARSE_VERSION to 11**

Change line 30:
```rust
pub const CURRENT_PARSE_VERSION: i32 = 11;
```

This triggers re-indexing of all sessions to populate `total_cost_usd`.

**Step 5: Update `update_session_deep_fields_tx` in row_types.rs**

Add `total_cost_usd: f64` parameter and corresponding SQL/bind. This is the sqlx-based variant used for non-parallel writes. Must match the rusqlite SQL exactly (55 params).

**Step 6: Update `update_session_deep_fields` in sessions.rs**

Check `crates/db/src/queries/sessions.rs:385` — if it's a wrapper around `_tx`, update its signature too. Add `total_cost_usd: f64` parameter and pass it through.

**Step 7: Update all callers**

Search for all call sites of `update_session_deep_fields` and `update_session_deep_fields_tx` and add the new parameter. Pass `0.0` for any non-JSONL-parsing callers (e.g., test fixtures, trends).

**Step 8: Run tests**

Run: `cargo test -p claude-view-db`
Expected: All tests pass. Migration test may need updating if it asserts column count.

**Step 9: Commit**

```bash
git add crates/db/src/migrations.rs crates/db/src/indexer_parallel.rs crates/db/src/queries/row_types.rs crates/db/src/queries/sessions.rs
git commit -m "feat: add total_cost_usd column and wire into session UPDATE"
```

---

### Task 4: Use `SUM(total_cost_usd)` in aggregate endpoint

**Files:**
- Modify: `crates/db/src/queries/ai_generation.rs:23-44` (add total_cost_usd to SELECT)
- Modify: `crates/server/src/routes/stats.rs:533-561` (use DB cost instead of recalculating)

**Step 1: Add `total_cost_usd` to the aggregate query**

In `crates/db/src/queries/ai_generation.rs`, extend the SELECT to include cost:

```rust
let (files_created, total_input_tokens, total_output_tokens, cache_read_tokens, cache_creation_tokens, total_cost_usd): (i64, i64, i64, i64, i64, Option<f64>) =
    sqlx::query_as(
        r#"
        SELECT
            COALESCE(SUM(files_edited_count), 0),
            COALESCE(SUM(total_input_tokens), 0),
            COALESCE(SUM(total_output_tokens), 0),
            COALESCE(SUM(cache_read_tokens), 0),
            COALESCE(SUM(cache_creation_tokens), 0),
            SUM(total_cost_usd)
        FROM valid_sessions
        WHERE last_message_at >= ?1
          AND last_message_at <= ?2
          AND (?3 IS NULL OR project_id = ?3)
          AND (?4 IS NULL OR git_branch = ?4)
        "#,
    )
```

Note: `SUM(total_cost_usd)` without COALESCE — returns `NULL` if all sessions have `NULL` cost (not yet re-indexed). This lets the endpoint know to fall back.

**Step 2: Pass `total_cost_usd` back in `AIGenerationStats`**

Add a `total_cost_usd: Option<f64>` field to the return, or just use it in the route handler. Simplest: return it as part of the existing struct. Add to `AIGenerationStats`:

```rust
pub struct AIGenerationStats {
    // ... existing fields ...
    /// Pre-calculated total cost from JSONL costUSD entries.
    /// None when sessions haven't been re-indexed yet.
    #[serde(skip)]
    pub total_cost_usd_from_jsonl: Option<f64>,
}
```

The `#[serde(skip)]` keeps it out of the API response — it's only used server-side to set `cost.total_cost_usd`.

**Step 3: Update the route handler to prefer DB cost**

In `crates/server/src/routes/stats.rs:533-561`, modify the cost computation:

```rust
    // Compute aggregate cost breakdown from per-model token data + pricing engine
    if let Ok(model_tokens) = state.db.get_per_model_token_breakdown(...).await {
        let pricing = state.pricing.read().unwrap_or_else(|e| e.into_inner());
        let mut cost = AggregateCostBreakdown::default();

        // Itemized breakdown: always from tokens × rates (costUSD is a single number)
        for (model_id, input, output, cache_read, cache_create) in &model_tokens {
            if let Some(mp) = pricing_engine::lookup_pricing(model_id, &pricing) {
                cost.input_cost_usd += *input as f64 * mp.input_cost_per_token;
                cost.output_cost_usd += *output as f64 * mp.output_cost_per_token;
                let cr_cost = *cache_read as f64 * mp.cache_read_cost_per_token;
                cost.cache_read_cost_usd += cr_cost;
                cost.cache_creation_cost_usd += *cache_create as f64 * mp.cache_creation_cost_per_token;
                cost.cache_savings_usd += *cache_read as f64 * mp.input_cost_per_token - cr_cost;
            } else {
                cost.input_cost_usd += *input as f64 * pricing_engine::FALLBACK_INPUT_COST_PER_TOKEN;
                cost.output_cost_usd += *output as f64 * pricing_engine::FALLBACK_OUTPUT_COST_PER_TOKEN;
                cost.cache_read_cost_usd += *cache_read as f64 * pricing_engine::FALLBACK_INPUT_COST_PER_TOKEN * 0.1;
                cost.cache_creation_cost_usd += *cache_create as f64 * pricing_engine::FALLBACK_INPUT_COST_PER_TOKEN * 1.25;
            }
        }

        // Total: prefer JSONL costUSD sum (most accurate), fall back to token-based calc
        let token_based_total = cost.input_cost_usd + cost.output_cost_usd + cost.cache_read_cost_usd + cost.cache_creation_cost_usd;
        cost.total_cost_usd = stats.total_cost_usd_from_jsonl.unwrap_or(token_based_total);

        stats.cost = cost;
    }
```

**Step 4: Run tests**

Run: `cargo test -p claude-view-db -p claude-view-server`
Expected: All tests pass.

**Step 5: Commit**

```bash
git add crates/db/src/queries/ai_generation.rs crates/server/src/routes/stats.rs
git commit -m "feat: use JSONL costUSD for aggregate total, token-based for itemized breakdown"
```

---

### Task 5: Add `costUSD` to live parser (`LiveLine`)

**Files:**
- Modify: `crates/core/src/live_parser.rs:68-97` (LiveLine struct)
- Modify: `crates/core/src/live_parser.rs:248-310` (parse_single_line, error return)
- Modify: `crates/core/src/live_parser.rs:371-395` (parse_single_line, extraction block)
- Modify: `crates/core/src/accumulator.rs:82-107` (process_line, token accumulation)

**Step 1: Add `cost_usd` to `LiveLine`**

```rust
pub struct LiveLine {
    // ... existing fields ...
    pub cache_creation_1hr_tokens: Option<u64>,
    /// Pre-calculated cost in USD from JSONL entry (top-level `costUSD` field).
    pub cost_usd: Option<f64>,
    pub timestamp: Option<String>,
    // ... rest ...
}
```

**Step 2: Add to error return path**

In the JSON parse error return (line ~283), add `cost_usd: None`.

**Step 3: Extract `costUSD` in parse_single_line**

After the usage extraction block (around line 390), add:

```rust
    let cost_usd = parsed.get("costUSD").and_then(|v| v.as_f64());
```

And include it in the final `LiveLine { ... }` construction.

**Step 4: Accumulate in `SessionAccumulator::process_line`**

In `crates/core/src/accumulator.rs`, add a `total_cost_usd: f64` field to `SessionAccumulator`, and accumulate in `process_line`:

```rust
    if let Some(cost) = line.cost_usd {
        self.total_cost_usd += cost;
    }
```

**Step 5: Run tests**

Run: `cargo test -p claude-view-core`
Expected: All tests pass.

**Step 6: Commit**

```bash
git add crates/core/src/live_parser.rs crates/core/src/accumulator.rs
git commit -m "feat: parse costUSD in live parser and accumulator"
```

---

### Task 6: Write test with costUSD fixture

**Files:**
- Modify: `crates/db/src/indexer_parallel.rs` (add test)

**Step 1: Write test**

Add a test to the existing `mod tests` block in `indexer_parallel.rs`:

```rust
#[test]
fn test_cost_usd_accumulation() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("session.jsonl");
    let mut f = std::fs::File::create(&path).unwrap();

    // Entry 1: has costUSD
    writeln!(f, r#"{{"type":"assistant","costUSD":0.05,"message":{{"role":"assistant","model":"claude-sonnet-4-6","content":[{{"type":"text","text":"Hello"}}],"usage":{{"input_tokens":1000,"output_tokens":500,"cache_read_input_tokens":200,"cache_creation_input_tokens":50}}}}}}"#).unwrap();
    // Entry 2: has costUSD
    writeln!(f, r#"{{"type":"assistant","costUSD":0.03,"message":{{"role":"assistant","model":"claude-sonnet-4-6","content":[{{"type":"text","text":"World"}}],"usage":{{"input_tokens":800,"output_tokens":300,"cache_read_input_tokens":100,"cache_creation_input_tokens":30}}}}}}"#).unwrap();
    // Entry 3: no costUSD (fallback should not crash)
    writeln!(f, r#"{{"type":"assistant","message":{{"role":"assistant","model":"claude-sonnet-4-6","content":[{{"type":"text","text":"No cost"}}],"usage":{{"input_tokens":500,"output_tokens":200}}}}}}"#).unwrap();

    f.flush().unwrap();

    let result = parse_session_file(&path).unwrap();
    let cost = result.deep.total_cost_usd;

    // 0.05 + 0.03 = 0.08 (third entry has no costUSD, contributes 0)
    assert!((cost - 0.08).abs() < 0.0001, "Expected 0.08, got {cost}");
}
```

**Step 2: Run the test**

Run: `cargo test -p claude-view-db -- test_cost_usd_accumulation`
Expected: PASS

**Step 3: Commit**

```bash
git add crates/db/src/indexer_parallel.rs
git commit -m "test: verify costUSD accumulation from JSONL entries"
```

---

### Task 7: Verify end-to-end

**Step 1: Build and run**

Run: `cargo build`
Expected: Clean build, no warnings related to our changes.

**Step 2: Run full test suite**

Run: `cargo test -p claude-view-core -p claude-view-db -p claude-view-server`
Expected: All tests pass.

**Step 3: Commit any fixes**

If any callers were missed or tests need updating, fix and commit.
