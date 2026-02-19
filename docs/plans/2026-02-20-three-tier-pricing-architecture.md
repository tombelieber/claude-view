# Three-Tier Pricing Architecture

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the hardcoded 15-model × 9-field pricing table with a three-tier resolution system: litellm fetch → SQLite cache → minimal defaults, so pricing stays accurate with zero maintenance.

**Architecture:** litellm is the single source of truth, fetched at startup and cached to SQLite for durability. Hardcoded defaults shrink to 4 base rates per model (cold-start safety net only). A `fill_tiering_gaps()` pass derives tiering fields from Anthropic's known pricing multipliers when litellm doesn't provide them explicitly. The SQLite cache means defaults are almost never needed in practice.

**Tech Stack:** Rust — `vibe_recall_core` (pricing engine), `vibe_recall_db` (SQLite cache + litellm fetch), `vibe_recall_server` (startup orchestration)

---

### Task 1: Add Serialize/Deserialize to ModelPricing

**Files:**
- Modify: `crates/core/src/pricing.rs:14-30`

**Step 1: Add serde derives to ModelPricing**

Change line 14-15 from:

```rust
#[derive(Debug, Clone)]
pub struct ModelPricing {
```

to:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModelPricing {
```

`serde` is already a dependency of `vibe_recall_core` (used by `TokenUsage`, `CostBreakdown`).

**Step 2: Verify compilation**

Run: `cargo check -p vibe-recall-core`
Expected: Compiles with no errors.

**Step 3: Commit**

```bash
git add crates/core/src/pricing.rs
git commit -m "feat(pricing): derive Serialize/Deserialize on ModelPricing for cache persistence"
```

---

### Task 2: Add pricing_cache SQLite migration

**Files:**
- Modify: `crates/db/src/migrations.rs:557` (append to MIGRATIONS array)

**Step 1: Add the new migration**

Before the closing `];` of the `MIGRATIONS` array (line 557), add:

```rust
    // Migration 23: Pricing cache for three-tier resolution (litellm → SQLite → defaults)
    r#"CREATE TABLE IF NOT EXISTS pricing_cache (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    data TEXT NOT NULL,
    fetched_at INTEGER NOT NULL
)"#,
```

This stores the full merged pricing map as a single JSON blob. One row, one upsert, one read. The `CHECK (id = 1)` constraint ensures only one row ever exists.

**Step 2: Verify compilation**

Run: `cargo check -p vibe-recall-db`
Expected: Compiles.

**Step 3: Run DB tests to verify migration applies cleanly**

Run: `cargo test -p vibe-recall-db -- --test-threads=1`
Expected: All existing tests pass (in-memory DB runs all migrations including the new one).

**Step 4: Commit**

```bash
git add crates/db/src/migrations.rs
git commit -m "feat(pricing): add pricing_cache table for persistent litellm data"
```

---

### Task 3: Add save/load pricing cache functions

**Files:**
- Modify: `crates/db/src/pricing.rs` (add 2 new functions + tests)
- Modify: `crates/db/src/lib.rs` (re-export new functions)

**Step 1: Write failing tests for cache save/load**

Add to the `mod tests` block in `crates/db/src/pricing.rs`:

```rust
#[tokio::test]
async fn test_save_and_load_pricing_cache() {
    let db = crate::Database::new_in_memory().await.unwrap();
    let mut map = HashMap::new();
    map.insert(
        "claude-opus-4-6".to_string(),
        ModelPricing {
            input_cost_per_token: 5e-6,
            output_cost_per_token: 25e-6,
            cache_creation_cost_per_token: 6.25e-6,
            cache_read_cost_per_token: 0.5e-6,
            input_cost_per_token_above_200k: Some(10e-6),
            output_cost_per_token_above_200k: Some(37.5e-6),
            cache_creation_cost_per_token_above_200k: Some(12.5e-6),
            cache_read_cost_per_token_above_200k: Some(1e-6),
            cache_creation_cost_per_token_1hr: Some(10e-6),
        },
    );

    save_pricing_cache(&db, &map).await.unwrap();
    let loaded = load_pricing_cache(&db).await.unwrap();
    assert!(loaded.is_some());
    let loaded = loaded.unwrap();
    assert_eq!(loaded.len(), 1);
    let m = loaded.get("claude-opus-4-6").unwrap();
    assert_eq!(m.input_cost_per_token, 5e-6);
    assert_eq!(m.cache_creation_cost_per_token_1hr, Some(10e-6));
}

#[tokio::test]
async fn test_load_empty_cache_returns_none() {
    let db = crate::Database::new_in_memory().await.unwrap();
    let loaded = load_pricing_cache(&db).await.unwrap();
    assert!(loaded.is_none());
}

#[tokio::test]
async fn test_save_overwrites_previous_cache() {
    let db = crate::Database::new_in_memory().await.unwrap();

    let mut map1 = HashMap::new();
    map1.insert("model-a".to_string(), ModelPricing {
        input_cost_per_token: 1e-6,
        output_cost_per_token: 5e-6,
        cache_creation_cost_per_token: 1.25e-6,
        cache_read_cost_per_token: 0.1e-6,
        input_cost_per_token_above_200k: None,
        output_cost_per_token_above_200k: None,
        cache_creation_cost_per_token_above_200k: None,
        cache_read_cost_per_token_above_200k: None,
        cache_creation_cost_per_token_1hr: None,
    });
    save_pricing_cache(&db, &map1).await.unwrap();

    let mut map2 = HashMap::new();
    map2.insert("model-b".to_string(), ModelPricing {
        input_cost_per_token: 2e-6,
        output_cost_per_token: 10e-6,
        cache_creation_cost_per_token: 2.5e-6,
        cache_read_cost_per_token: 0.2e-6,
        input_cost_per_token_above_200k: None,
        output_cost_per_token_above_200k: None,
        cache_creation_cost_per_token_above_200k: None,
        cache_read_cost_per_token_above_200k: None,
        cache_creation_cost_per_token_1hr: None,
    });
    save_pricing_cache(&db, &map2).await.unwrap();

    let loaded = load_pricing_cache(&db).await.unwrap().unwrap();
    assert!(!loaded.contains_key("model-a"));
    assert!(loaded.contains_key("model-b"));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p vibe-recall-db -- pricing::tests::test_save`
Expected: FAIL — functions don't exist.

**Step 3: Implement save/load functions**

Add above the `#[cfg(test)]` block in `crates/db/src/pricing.rs`:

```rust
/// Persist the full pricing map to SQLite for cross-restart durability.
///
/// Overwrites any previous cache. Called after successful litellm fetch + merge.
pub async fn save_pricing_cache(
    db: &crate::Database,
    pricing: &HashMap<String, ModelPricing>,
) -> Result<(), String> {
    let json = serde_json::to_string(pricing).map_err(|e| format!("serialize: {e}"))?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    sqlx::query(
        "INSERT OR REPLACE INTO pricing_cache (id, data, fetched_at) VALUES (1, ?, ?)",
    )
    .bind(&json)
    .bind(now)
    .execute(db.pool())
    .await
    .map_err(|e| format!("save pricing cache: {e}"))?;

    Ok(())
}

/// Load the cached pricing map from SQLite.
///
/// Returns `None` if no cache exists (first run). Returns the full map otherwise.
pub async fn load_pricing_cache(
    db: &crate::Database,
) -> Result<Option<HashMap<String, ModelPricing>>, String> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT data FROM pricing_cache WHERE id = 1")
            .fetch_optional(db.pool())
            .await
            .map_err(|e| format!("load pricing cache: {e}"))?;

    match row {
        Some((json,)) => {
            let map: HashMap<String, ModelPricing> =
                serde_json::from_str(&json).map_err(|e| format!("deserialize: {e}"))?;
            Ok(Some(map))
        }
        None => Ok(None),
    }
}
```

Add `use sqlx::Row;` at the top of the file if not already imported (it shouldn't be needed with `query_as` tuple destructuring).

**Step 4: Re-export from `crates/db/src/lib.rs`**

Update the `pub use pricing::` line to include the new functions:

```rust
pub use pricing::{fetch_litellm_pricing, merge_pricing, save_pricing_cache, load_pricing_cache};
```

**Step 5: Run tests**

Run: `cargo test -p vibe-recall-db -- pricing`
Expected: ALL pass.

**Step 6: Commit**

```bash
git add crates/db/src/pricing.rs crates/db/src/lib.rs
git commit -m "feat(pricing): add SQLite pricing cache save/load for three-tier resolution"
```

---

### Task 4: Add fill_tiering_gaps() derivation function

**Files:**
- Modify: `crates/core/src/pricing.rs` (add constants + function + tests)

**Step 1: Write failing tests for derivation**

Add to `mod tests` in `crates/core/src/pricing.rs`:

```rust
#[test]
fn test_fill_tiering_gaps_derives_from_base_rates() {
    let mut pricing = HashMap::new();
    pricing.insert(
        "test-model".to_string(),
        ModelPricing {
            input_cost_per_token: 5e-6,
            output_cost_per_token: 25e-6,
            cache_creation_cost_per_token: 6.25e-6,
            cache_read_cost_per_token: 0.5e-6,
            input_cost_per_token_above_200k: None,
            output_cost_per_token_above_200k: None,
            cache_creation_cost_per_token_above_200k: None,
            cache_read_cost_per_token_above_200k: None,
            cache_creation_cost_per_token_1hr: None,
        },
    );

    fill_tiering_gaps(&mut pricing);
    let m = pricing.get("test-model").unwrap();
    assert_eq!(m.input_cost_per_token_above_200k, Some(10e-6));
    assert_eq!(m.output_cost_per_token_above_200k, Some(37.5e-6));
    assert_eq!(m.cache_creation_cost_per_token_above_200k, Some(12.5e-6));
    assert_eq!(m.cache_read_cost_per_token_above_200k, Some(1e-6));
    assert_eq!(m.cache_creation_cost_per_token_1hr, Some(10e-6));
}

#[test]
fn test_fill_tiering_gaps_preserves_explicit_values() {
    let mut pricing = HashMap::new();
    pricing.insert(
        "test-model".to_string(),
        ModelPricing {
            input_cost_per_token: 5e-6,
            output_cost_per_token: 25e-6,
            cache_creation_cost_per_token: 6.25e-6,
            cache_read_cost_per_token: 0.5e-6,
            input_cost_per_token_above_200k: Some(99e-6), // explicit — should NOT be overwritten
            output_cost_per_token_above_200k: None,
            cache_creation_cost_per_token_above_200k: None,
            cache_read_cost_per_token_above_200k: None,
            cache_creation_cost_per_token_1hr: None,
        },
    );

    fill_tiering_gaps(&mut pricing);
    let m = pricing.get("test-model").unwrap();
    assert_eq!(m.input_cost_per_token_above_200k, Some(99e-6)); // preserved
    assert_eq!(m.output_cost_per_token_above_200k, Some(37.5e-6)); // derived
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p vibe-recall-core -- pricing::tests::test_fill_tiering`
Expected: FAIL — function doesn't exist.

**Step 3: Add constants and derivation function**

Add after the `FALLBACK_OUTPUT_COST_PER_TOKEN` constant (line 81):

```rust
/// Anthropic pricing structure multipliers.
///
/// These encode Anthropic's consistent pricing rules across all models
/// (verified: Opus 4.6, Sonnet 4.6, Haiku 4.5 all use identical ratios).
/// Used to derive tiering fields when litellm doesn't provide them explicitly.
const ABOVE_200K_INPUT_MULT: f64 = 2.0;
const ABOVE_200K_OUTPUT_MULT: f64 = 1.5;
const ABOVE_200K_CACHE_CREATE_MULT: f64 = 2.0;
const ABOVE_200K_CACHE_READ_MULT: f64 = 2.0;
const CACHE_1HR_MULT: f64 = 1.6;

/// Fill any `None` tiering fields by deriving from base rates using known
/// Anthropic pricing multipliers.
///
/// Only fills gaps — explicit values from litellm/cache are preserved.
/// Call this as a final pass after merging all pricing sources.
pub fn fill_tiering_gaps(pricing: &mut HashMap<String, ModelPricing>) {
    for mp in pricing.values_mut() {
        if mp.input_cost_per_token_above_200k.is_none() {
            mp.input_cost_per_token_above_200k =
                Some(mp.input_cost_per_token * ABOVE_200K_INPUT_MULT);
        }
        if mp.output_cost_per_token_above_200k.is_none() {
            mp.output_cost_per_token_above_200k =
                Some(mp.output_cost_per_token * ABOVE_200K_OUTPUT_MULT);
        }
        if mp.cache_creation_cost_per_token_above_200k.is_none() {
            mp.cache_creation_cost_per_token_above_200k =
                Some(mp.cache_creation_cost_per_token * ABOVE_200K_CACHE_CREATE_MULT);
        }
        if mp.cache_read_cost_per_token_above_200k.is_none() {
            mp.cache_read_cost_per_token_above_200k =
                Some(mp.cache_read_cost_per_token * ABOVE_200K_CACHE_READ_MULT);
        }
        if mp.cache_creation_cost_per_token_1hr.is_none() {
            mp.cache_creation_cost_per_token_1hr =
                Some(mp.cache_creation_cost_per_token * CACHE_1HR_MULT);
        }
    }
}
```

**Step 4: Run tests**

Run: `cargo test -p vibe-recall-core -- pricing`
Expected: ALL pass.

**Step 5: Commit**

```bash
git add crates/core/src/pricing.rs
git commit -m "feat(pricing): add fill_tiering_gaps() with Anthropic pricing multipliers"
```

---

### Task 5: Simplify default_pricing() to base rates only

**Files:**
- Modify: `crates/core/src/pricing.rs:253-474`

The key insight: defaults are a cold-start safety net used for seconds before litellm responds. Tiering accuracy during that window doesn't matter. All 5 `Option` fields become `None`.

**Step 1: Replace all 15 model entries**

Replace the entire body of `default_pricing()` (lines 253-474) with:

```rust
/// Minimal Anthropic pricing table for cold-start fallback.
///
/// Contains only 4 base rates per model — tiering fields are `None`.
/// At runtime, `fill_tiering_gaps()` derives tiering from multipliers,
/// and litellm fetch replaces everything with full accurate data.
/// This table is only needed for the seconds before litellm responds
/// on first launch with no SQLite cache.
pub fn default_pricing() -> HashMap<String, ModelPricing> {
    let mut m = HashMap::new();

    let models: &[(&str, f64, f64, f64, f64)] = &[
        // Current generation
        ("claude-opus-4-6",             5e-6,    25e-6,   6.25e-6,  0.5e-6),
        ("claude-sonnet-4-6",           3e-6,    15e-6,   3.75e-6,  0.3e-6),
        ("claude-sonnet-4-5-20250929",  3e-6,    15e-6,   3.75e-6,  0.3e-6),
        ("claude-haiku-4-5-20251001",   1e-6,     5e-6,   1.25e-6,  0.1e-6),
        ("claude-opus-4-5-20251101",    5e-6,    25e-6,   6.25e-6,  0.5e-6),
        // Legacy
        ("claude-opus-4-1-20250805",   15e-6,    75e-6,  18.75e-6,  1.5e-6),
        ("claude-opus-4-20250514",     15e-6,    75e-6,  18.75e-6,  1.5e-6),
        ("claude-sonnet-4-20250514",    3e-6,    15e-6,   3.75e-6,  0.3e-6),
        ("claude-3-7-sonnet-20250219",  3e-6,    15e-6,   3.75e-6,  0.3e-6),
        ("claude-3-5-sonnet-20241022",  3e-6,    15e-6,   3.75e-6,  0.3e-6),
        ("claude-3-5-sonnet-20240620",  3e-6,    15e-6,   3.75e-6,  0.3e-6),
        ("claude-3-5-haiku-20241022",   0.8e-6,   4e-6,   1e-6,     0.08e-6),
        ("claude-3-opus-20240229",     15e-6,    75e-6,  18.75e-6,  1.5e-6),
        ("claude-3-sonnet-20240229",    3e-6,    15e-6,   3.75e-6,  0.3e-6),
        ("claude-3-haiku-20240307",     0.25e-6,  1.25e-6, 0.3e-6,  0.03e-6),
    ];

    for &(id, input, output, cache_create, cache_read) in models {
        m.insert(
            id.into(),
            ModelPricing {
                input_cost_per_token: input,
                output_cost_per_token: output,
                cache_creation_cost_per_token: cache_create,
                cache_read_cost_per_token: cache_read,
                input_cost_per_token_above_200k: None,
                output_cost_per_token_above_200k: None,
                cache_creation_cost_per_token_above_200k: None,
                cache_read_cost_per_token_above_200k: None,
                cache_creation_cost_per_token_1hr: None,
            },
        );
    }

    m
}
```

**Step 2: Update tests that depend on explicit tiering values in defaults**

Tests like `test_tiered_pricing_opus_46`, `test_cache_savings`, `test_tiered_cache_read_above_200k`, etc. rely on defaults having explicit tiering. These tests should call `fill_tiering_gaps()` on the defaults first, since that's how the real app uses them:

Add a helper at the top of the test module:

```rust
/// Build the pricing map the way the real app does: defaults + gap filling.
fn test_pricing() -> HashMap<String, ModelPricing> {
    let mut p = default_pricing();
    fill_tiering_gaps(&mut p);
    p
}
```

Then replace every `default_pricing()` call in tests with `test_pricing()`.

**Step 3: Run tests**

Run: `cargo test -p vibe-recall-core -- pricing`
Expected: ALL pass. The derived values from multipliers match the previously hardcoded values exactly (since the hardcoded values WERE those multiplied values).

**Step 4: Verify the derived values match**

Quick sanity check in the test output:
- Opus 4.6: `5e-6 × 2.0 = 10e-6` (input_above_200k) ✓
- Opus 4.6: `25e-6 × 1.5 = 37.5e-6` (output_above_200k) ✓
- Opus 4.6: `6.25e-6 × 2.0 = 12.5e-6` (cache_create_above_200k) ✓
- Opus 4.6: `0.5e-6 × 2.0 = 1e-6` (cache_read_above_200k) ✓
- Opus 4.6: `6.25e-6 × 1.6 = 10e-6` (cache_create_1hr) ✓

**Step 5: Commit**

```bash
git add crates/core/src/pricing.rs
git commit -m "refactor(pricing): simplify default_pricing() to base rates only, derive tiering via multipliers"
```

---

### Task 6: Rearchitect refresh_pricing() for three-tier resolution

**Files:**
- Modify: `crates/server/src/lib.rs:131-213`

This is the integration task. The current flow is:

```
startup: pricing = default_pricing()
async:   litellm → merge(defaults, litellm) → write to RwLock
```

The new flow is:

```
startup: pricing = default_pricing() + fill_tiering_gaps()
async:   resolve_pricing(db) → write to RwLock
  where resolve_pricing =
    1. Try litellm → merge(defaults, litellm) → fill_tiering_gaps → save to SQLite → return
    2. litellm fails → try load SQLite cache → return
    3. no cache → keep defaults (already gap-filled)
```

**Step 1: Update `create_app_full()` to gap-fill defaults on startup**

Change line 139 from:

```rust
let pricing = Arc::new(std::sync::RwLock::new(vibe_recall_db::default_pricing()));
```

to:

```rust
let mut initial_pricing = vibe_recall_db::default_pricing();
vibe_recall_core::pricing::fill_tiering_gaps(&mut initial_pricing);
let pricing = Arc::new(std::sync::RwLock::new(initial_pricing));
```

**Step 2: Update `refresh_pricing()` to use three-tier resolution**

Change the `refresh_pricing` function signature and body. It now needs `db` access:

```rust
async fn refresh_pricing(
    pricing: &Arc<std::sync::RwLock<HashMap<String, ModelPricing>>>,
    db: &Database,
) {
    // Tier 1: Try litellm fetch
    match vibe_recall_db::fetch_litellm_pricing().await {
        Ok(litellm) => {
            let defaults = vibe_recall_db::default_pricing();
            let mut merged = vibe_recall_db::merge_pricing(&defaults, &litellm);
            vibe_recall_core::pricing::fill_tiering_gaps(&mut merged);
            let count = merged.len();

            // Persist to SQLite for cross-restart durability
            if let Err(e) = vibe_recall_db::save_pricing_cache(db, &merged).await {
                tracing::warn!("Failed to cache pricing to SQLite: {e}");
            }

            *pricing.write().unwrap() = merged;
            tracing::info!(models = count, "Pricing refreshed from litellm + cached to SQLite");
        }
        Err(e) => {
            tracing::warn!("litellm fetch failed: {e}");

            // Tier 2: Try SQLite cache
            match vibe_recall_db::load_pricing_cache(db).await {
                Ok(Some(mut cached)) => {
                    vibe_recall_core::pricing::fill_tiering_gaps(&mut cached);
                    let count = cached.len();
                    *pricing.write().unwrap() = cached;
                    tracing::info!(models = count, "Pricing loaded from SQLite cache");
                }
                Ok(None) => {
                    // Tier 3: Keep defaults (already gap-filled at startup)
                    tracing::info!("No SQLite pricing cache, using defaults");
                }
                Err(e2) => {
                    tracing::warn!("Failed to load pricing cache: {e2}, using defaults");
                }
            }
        }
    }
}
```

**Step 3: Update the tokio::spawn call to pass db**

Change the spawn block (lines 172-184) to clone the db handle:

```rust
{
    let pricing = state.pricing.clone();
    let db = state.db.clone();
    tokio::spawn(async move {
        refresh_pricing(&pricing, &db).await;
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(86_400));
        interval.tick().await;
        loop {
            interval.tick().await;
            refresh_pricing(&pricing, &db).await;
        }
    });
}
```

**Step 4: Update the same pattern in `AppState::new()`, `new_with_indexing()`, `new_with_indexing_and_registry()`**

These factory methods in `crates/server/src/state.rs` also call `default_pricing()`. Update them to gap-fill:

```rust
pricing: Arc::new(RwLock::new({
    let mut p = vibe_recall_db::default_pricing();
    vibe_recall_core::pricing::fill_tiering_gaps(&mut p);
    p
})),
```

Apply this to all 3 factory methods (lines 81, 107, 136).

**Step 5: Verify compilation**

Run: `cargo check`
Expected: Compiles across all crates.

**Step 6: Commit**

```bash
git add crates/server/src/lib.rs crates/server/src/state.rs
git commit -m "feat(pricing): three-tier resolution — litellm → SQLite cache → defaults with derivation"
```

---

### Task 7: Update db pricing tests for new architecture

**Files:**
- Modify: `crates/db/src/pricing.rs` (tests module)

**Step 1: Update `mp()` test helper**

The `mp()` helper currently generates `input_cost_per_token_above_200k: Some(input * 2.0)` etc. Since defaults no longer have explicit tiering, and we're testing merge behavior, keep the helper as-is — it simulates what litellm would provide. No change needed.

**Step 2: Add a test verifying three-tier fallback behavior**

Add to the tests:

```rust
#[tokio::test]
async fn test_three_tier_fallback_litellm_to_cache() {
    use vibe_recall_core::pricing::fill_tiering_gaps;

    let db = crate::Database::new_in_memory().await.unwrap();

    // Simulate: litellm succeeded previously, saved to cache
    let mut original = HashMap::new();
    original.insert(
        "claude-opus-4-6".to_string(),
        ModelPricing {
            input_cost_per_token: 5e-6,
            output_cost_per_token: 25e-6,
            cache_creation_cost_per_token: 6.25e-6,
            cache_read_cost_per_token: 0.5e-6,
            input_cost_per_token_above_200k: Some(10e-6),
            output_cost_per_token_above_200k: Some(37.5e-6),
            cache_creation_cost_per_token_above_200k: Some(12.5e-6),
            cache_read_cost_per_token_above_200k: Some(1e-6),
            cache_creation_cost_per_token_1hr: Some(10e-6),
        },
    );
    save_pricing_cache(&db, &original).await.unwrap();

    // Simulate: litellm fails, load from cache
    let mut cached = load_pricing_cache(&db).await.unwrap().unwrap();
    fill_tiering_gaps(&mut cached);

    let m = cached.get("claude-opus-4-6").unwrap();
    assert_eq!(m.input_cost_per_token, 5e-6);
    assert_eq!(m.input_cost_per_token_above_200k, Some(10e-6));
    assert_eq!(m.cache_creation_cost_per_token_1hr, Some(10e-6));
}
```

**Step 3: Run all pricing tests**

Run: `cargo test -p vibe-recall-db -- pricing && cargo test -p vibe-recall-core -- pricing`
Expected: ALL pass.

**Step 4: Commit**

```bash
git add crates/db/src/pricing.rs
git commit -m "test(pricing): add three-tier fallback integration test"
```

---

### Task 8: Full workspace build and cleanup

**Files:**
- Delete: `docs/plans/2026-02-20-full-fidelity-pricing-impl.md`

**Step 1: Run full workspace check**

Run: `cargo check`
Expected: Clean compilation across all crates.

**Step 2: Run full test suite**

Run: `cargo test -p vibe-recall-core -- pricing && cargo test -p vibe-recall-db -- pricing && cargo test -p vibe-recall-server`
Expected: ALL pass.

**Step 3: Delete the old implementation plan**

The old `2026-02-20-full-fidelity-pricing-impl.md` is superseded by this plan:

```bash
rm docs/plans/2026-02-20-full-fidelity-pricing-impl.md
```

Keep `2026-02-20-full-fidelity-pricing.md` as historical design context.

**Step 4: Commit**

```bash
git add -A
git commit -m "chore: remove superseded pricing implementation plan"
```
