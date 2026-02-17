---
status: pending
date: 2026-02-16
phase: G
depends_on: A
---

# Phase G: Codex Multi-Provider Foundation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a source-aware architecture so Claude and Codex sessions can coexist safely across indexing, API, and UI without ID collisions or source-specific hacks.

**Architecture:** Introduce an explicit `SessionSource` model (`claude`, `codex`) at core + DB layers, enforce source-aware identity (`source`, `source_session_id`, canonical `id`), and route discovery/parsing/indexing through provider adapters instead of Claude-only code paths.

**Tech Stack:** Rust (`axum`, `sqlx`, `serde`, `tokio`), TypeScript/React, SQLite migrations, existing `vibe_recall_core` + `vibe_recall_db` crates.

---

## Scope

- In scope:
  - Source-aware domain model and DB schema
  - Provider abstraction and startup wiring
  - API contract updates needed for future Codex phases
- Out of scope:
  - Codex historical parsing logic (Phase H)
  - Codex live Mission Control logic (Phase I)
  - Contributions/fluency/insights (explicitly excluded)

## Non-Negotiable Decisions

1. Keep `sessions.id` as stable TEXT PK, but make it canonical: `<source>:<source_session_id>`.
2. Add first-class columns `sessions.source` and `sessions.source_session_id` with uniqueness guard.
3. No hidden inference from file paths in API handlers; source must be explicit in DB row.
4. All new source handling must be additive and backward-compatible for existing Claude data.

---

### Task 1: Introduce Source Model and Canonical ID Helpers

**Files:**
- Create: `crates/core/src/source.rs`
- Modify: `crates/core/src/lib.rs`
- Modify: `crates/core/src/types.rs`
- Test: `crates/core/src/source.rs`

**Step 1: Write failing tests for source parsing and canonical IDs**

```rust
#[test]
fn canonical_id_for_claude() {
    assert_eq!(canonical_session_id(SessionSource::Claude, "abc"), "claude:abc");
}

#[test]
fn canonical_id_for_codex() {
    assert_eq!(canonical_session_id(SessionSource::Codex, "019c..."), "codex:019c...");
}

#[test]
fn split_canonical_id_round_trip() {
    let (source, raw) = split_canonical_session_id("codex:xyz").unwrap();
    assert_eq!(source, SessionSource::Codex);
    assert_eq!(raw, "xyz");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p vibe-recall-core canonical_id_for_codex -- --nocapture`
Expected: FAIL with unresolved `SessionSource` / helper symbols.

**Step 3: Implement minimal source model**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionSource {
    Claude,
    Codex,
}

pub fn canonical_session_id(source: SessionSource, source_session_id: &str) -> String { ... }
pub fn split_canonical_session_id(id: &str) -> Option<(SessionSource, &str)> { ... }
```

Also add on `SessionInfo`:

```rust
pub source: SessionSource,
pub source_session_id: String,
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p vibe-recall-core source::tests -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/core/src/source.rs crates/core/src/lib.rs crates/core/src/types.rs
git commit -m "feat(core): add session source model and canonical id helpers"
```

---

### Task 2: Add Source Columns + Constraints in SQLite Schema

**Files:**
- Modify: `crates/db/src/migrations.rs`
- Modify: `crates/db/src/lib.rs`
- Modify: `crates/db/src/queries/row_types.rs`
- Test: `crates/db/src/migrations.rs`

**Step 1: Write failing migration tests**

Add tests asserting:
- `sessions.source` exists and defaults to `claude`
- `sessions.source_session_id` exists and is non-empty after migration
- unique index on `(source, source_session_id)` exists

**Step 2: Run test to verify it fails**

Run: `cargo test -p vibe-recall-db test_migration_.*source -- --nocapture`
Expected: FAIL because columns/index do not exist.

**Step 3: Implement migration and backfill**

Add migration steps:

```sql
ALTER TABLE sessions ADD COLUMN source TEXT NOT NULL DEFAULT 'claude' CHECK (source IN ('claude','codex'));
ALTER TABLE sessions ADD COLUMN source_session_id TEXT;
UPDATE sessions SET source_session_id = CASE
  WHEN instr(id, ':') > 0 THEN substr(id, instr(id, ':') + 1)
  ELSE id
END;
UPDATE sessions SET source_session_id = id WHERE source_session_id IS NULL OR source_session_id = '';
CREATE UNIQUE INDEX IF NOT EXISTS idx_sessions_source_session_id ON sessions(source, source_session_id);
```

Update row mapping structs to include new fields.

**Step 4: Run test to verify it passes**

Run: `cargo test -p vibe-recall-db test_migration_.*source -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/db/src/migrations.rs crates/db/src/lib.rs crates/db/src/queries/row_types.rs
git commit -m "feat(db): add source-aware session identity columns"
```

---

### Task 3: Make Session Insert/Upsert Source-Aware

**Files:**
- Modify: `crates/db/src/queries/sessions.rs`
- Modify: `crates/db/src/indexer_parallel.rs`
- Modify: `crates/db/src/indexer.rs`
- Test: `crates/db/tests/queries_sessions_test.rs`

**Step 1: Write failing DB query tests**

Add a test inserting two sessions with same raw ID from different sources and assert both rows exist.

```rust
assert_ne!(claude_row.id, codex_row.id);
assert_eq!(claude_row.source, "claude");
assert_eq!(codex_row.source, "codex");
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p vibe-recall-db duplicate_raw_id_across_sources -- --nocapture`
Expected: FAIL due to collision on `id` or missing source columns in insert path.

**Step 3: Update insert APIs**

- Extend `insert_session` / `insert_session_from_index` signatures to accept source metadata.
- Ensure all insert paths write:
  - `id = canonical_session_id(source, source_session_id)`
  - `source`
  - `source_session_id`
- Keep legacy callers defaulted to `SessionSource::Claude` until provider wiring lands.

**Step 4: Run test to verify it passes**

Run: `cargo test -p vibe-recall-db duplicate_raw_id_across_sources -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/db/src/queries/sessions.rs crates/db/src/indexer_parallel.rs crates/db/src/indexer.rs crates/db/tests/queries_sessions_test.rs
git commit -m "refactor(db): make session upserts source-aware"
```

---

### Task 4: Introduce Provider Adapter Layer

**Files:**
- Create: `crates/core/src/provider/mod.rs`
- Create: `crates/core/src/provider/claude.rs`
- Create: `crates/core/src/provider/codex.rs`
- Modify: `crates/core/src/lib.rs`
- Test: `crates/core/src/provider/mod.rs`

**Step 1: Write failing provider registry tests**

```rust
#[test]
fn default_provider_registry_contains_claude_and_codex() {
    let providers = default_providers();
    assert!(providers.iter().any(|p| p.source() == SessionSource::Claude));
    assert!(providers.iter().any(|p| p.source() == SessionSource::Codex));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p vibe-recall-core default_provider_registry_contains_claude_and_codex -- --nocapture`
Expected: FAIL (module missing).

**Step 3: Implement provider interfaces**

Define focused traits for this codebase (not generic framework bloat):

```rust
pub trait SessionProvider {
    fn source(&self) -> SessionSource;
    fn data_roots(&self) -> Result<Vec<PathBuf>, DiscoveryError>;
    fn discover_pass1(&self) -> Result<Vec<DiscoveredSession>, DiscoveryError>;
}
```

- `ClaudeProvider` wraps existing Claude discovery/index sources.
- `CodexProvider` initially returns roots + empty discovery implementation placeholder for Phase H.

**Step 4: Run test to verify it passes**

Run: `cargo test -p vibe-recall-core provider::tests -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/core/src/provider/mod.rs crates/core/src/provider/claude.rs crates/core/src/provider/codex.rs crates/core/src/lib.rs
git commit -m "feat(core): add session provider adapter layer"
```

---

### Task 5: Wire Server/Indexer Startup to Provider Roots

**Files:**
- Modify: `crates/server/src/main.rs`
- Modify: `crates/server/src/routes/stats.rs`
- Modify: `crates/core/src/discovery.rs`
- Test: `crates/server/src/main.rs` (new `#[cfg(test)]` module)

**Step 1: Write failing startup tests**

Add tests for source root resolution:
- Claude root present, Codex root missing -> startup still works
- Both roots present -> both included in scan set

**Step 2: Run test to verify it fails**

Run: `cargo test -p vibe-recall-server provider_roots -- --nocapture`
Expected: FAIL (single-root `.claude` assumptions).

**Step 3: Implement source-root aggregation**

- Replace direct `.join(".claude")` assumptions in startup/indexer entrypoints with provider-derived roots.
- Update storage stats JSONL size calculation to sum all provider JSONL roots, not only Claude.
- Keep behavior deterministic if a root is absent: log debug, continue.

**Step 4: Run test to verify it passes**

Run: `cargo test -p vibe-recall-server provider_roots -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/server/src/main.rs crates/server/src/routes/stats.rs crates/core/src/discovery.rs
git commit -m "refactor(server): derive indexing roots from provider registry"
```

---

### Task 6: API Contract Exposure for Source Fields

**Files:**
- Modify: `crates/core/src/types.rs`
- Modify: `crates/server/src/routes/sessions.rs`
- Modify: `src/hooks/use-projects.ts`
- Modify: `src/types/generated/*` (generated)
- Test: `crates/server/src/routes/sessions.rs`

**Step 1: Write failing API tests**

Add assertions that `/api/sessions` and `/api/sessions/:id` include:
- `source`
- `sourceSessionId`

**Step 2: Run test to verify it fails**

Run: `cargo test -p vibe-recall-server sessions_includes_source_fields -- --nocapture`
Expected: FAIL due to missing fields in serialized payload.

**Step 3: Implement response model changes**

- Ensure source fields serialize in existing endpoints.
- Ensure session lookup remains by canonical `id` (`claude:*`, `codex:*`).
- Regenerate TS bindings.

**Step 4: Run test to verify it passes**

Run: `cargo test -p vibe-recall-server sessions_includes_source_fields -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/core/src/types.rs crates/server/src/routes/sessions.rs src/hooks/use-projects.ts src/types/generated

git commit -m "feat(api): expose session source metadata in session endpoints"
```

---

## Exit Criteria

- `SessionSource` is first-class in core, DB, and API models.
- Existing Claude data remains intact and backfilled with `source='claude'`.
- Duplicate raw IDs across sources cannot collide.
- Startup/indexing no longer assumes only `~/.claude`.

## Verification Checklist

Run:
- `cargo test -p vibe-recall-core source::tests provider::tests`
- `cargo test -p vibe-recall-db test_migration_.*source duplicate_raw_id_across_sources`
- `cargo test -p vibe-recall-server sessions_includes_source_fields provider_roots`
- `cargo clippy --workspace --all-targets -- -D warnings`

Expected:
- All tests pass.
- No schema regressions for existing session queries.

## Risks and Mitigations

- Risk: partial migration leaves empty `source_session_id`.
  - Mitigation: migration test explicitly rejects NULL/empty values.
- Risk: accidental runtime dependency on prefixed `id` parsing everywhere.
  - Mitigation: use explicit `source` and `source_session_id` columns in query layer.

