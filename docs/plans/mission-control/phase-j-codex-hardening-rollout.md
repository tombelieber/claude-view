---
status: pending
date: 2026-02-16
phase: J
depends_on: H,I
---

# Phase J: Codex Hardening, Backfill, and Rollout Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make Codex support production-grade with migration safety, deterministic reindexing, robust tests, observability, and controlled rollout.

**Architecture:** Add source-aware validation layers around indexing and live monitoring, enforce test fixtures for known Codex variants, instrument parser/indexer health metrics, and ship behind explicit rollout controls.

**Tech Stack:** Rust test harnesses, SQLite migration checks, benchmark examples, server metrics instrumentation, frontend smoke tests.

---

## Scope

- In scope:
  - Data correctness hardening for mixed-source datasets
  - Reindex/backfill tooling and guards
  - Performance baselines and parser diagnostics
  - Rollout controls and operational docs
- Out of scope:
  - New product features beyond Codex + Claude parity

## Quality Bar

1. No data corruption on migration/backfill.
2. No source cross-contamination (Claude parser never used for Codex file and vice versa).
3. No silent parser regressions; failures must be measurable.
4. Rollback path must be documented and tested.

---

### Task 1: Create Canonical Codex Fixture Corpus

**Files:**
- Create: `crates/core/tests/fixtures/codex/minimal.jsonl`
- Create: `crates/core/tests/fixtures/codex/tool_heavy.jsonl`
- Create: `crates/core/tests/fixtures/codex/compacted.jsonl`
- Create: `crates/core/tests/fixtures/codex/aborted_turn.jsonl`
- Create: `crates/core/tests/codex_fixture_contract_test.rs`

**Step 1: Write failing fixture contract tests**

Add tests asserting each fixture:
- parses without panic
- emits expected message counts
- exposes expected token totals when present

**Step 2: Run test to verify it fails**

Run: `cargo test -p claude-view-core codex_fixture_contract -- --nocapture`
Expected: FAIL because fixture files/tests do not exist.

**Step 3: Add fixtures + contract test harness**

- Commit small, representative JSONL samples directly from validated local Codex patterns.
- Keep fixture size small and deterministic.

**Step 4: Run test to verify it passes**

Run: `cargo test -p claude-view-core codex_fixture_contract -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/core/tests/fixtures/codex crates/core/tests/codex_fixture_contract_test.rs
git commit -m "test(core): add codex fixture corpus and parser contract tests"
```

---

### Task 2: Harden Migration + Backfill with Idempotency Tests

**Files:**
- Modify: `crates/db/src/migrations.rs`
- Modify: `crates/db/tests/acceptance_tests.rs`
- Modify: `crates/db/src/indexer_parallel.rs`

**Step 1: Write failing backfill/idempotency tests**

Add tests for:
- running migration twice leaves same data
- reindexing mixed Claude/Codex twice is idempotent
- source/source_session_id never null/empty after migration

**Step 2: Run test to verify it fails**

Run: `cargo test -p claude-view-db migration_source_backfill_idempotent -- --nocapture`
Expected: FAIL before hardening assertions are added.

**Step 3: Implement migration guards**

- Add explicit post-migration validation checks in tests.
- Ensure backfill SQL handles already-prefixed and non-prefixed IDs safely.
- Ensure indexer re-upserts use canonical identity consistently.

**Step 4: Run test to verify it passes**

Run: `cargo test -p claude-view-db migration_source_backfill_idempotent -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/db/src/migrations.rs crates/db/tests/acceptance_tests.rs crates/db/src/indexer_parallel.rs

git commit -m "test(db): harden source migration and reindex idempotency"
```

---

### Task 3: Add Source-Aware Reindex Controls in System Routes

**Files:**
- Modify: `crates/server/src/routes/system.rs`
- Modify: `crates/server/src/main.rs`
- Modify: `src/types/generated/*` (generated)
- Test: `crates/server/src/routes/system.rs`

**Step 1: Write failing route tests**

Add tests for source-scoped reindex request payload:

```json
{ "sources": ["claude", "codex"] }
```

Test behaviors:
- omitted -> all sources
- invalid source -> 400
- codex-only reindex allowed

**Step 2: Run test to verify it fails**

Run: `cargo test -p claude-view-server system_reindex_sources -- --nocapture`
Expected: FAIL because endpoint does not accept source scope.

**Step 3: Implement source-scoped reindex contract**

- Add request model for source selection.
- Thread source scope into background indexing kickoff.
- Keep backward compatibility with current `POST /api/system/reindex` behavior.

**Step 4: Run test to verify it passes**

Run: `cargo test -p claude-view-server system_reindex_sources -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/server/src/routes/system.rs crates/server/src/main.rs src/types/generated

git commit -m "feat(system): add source-scoped reindex controls"
```

---

### Task 4: Instrument Parser/Indexer Health Metrics by Source

**Files:**
- Modify: `crates/server/src/metrics.rs`
- Modify: `crates/db/src/indexer_parallel.rs`
- Modify: `crates/core/src/codex/parser.rs`
- Test: `crates/server/src/metrics.rs`

**Step 1: Write failing metrics tests**

Add tests asserting counters are emitted with labels:
- `source=claude|codex`
- `stage=pass1|pass2|live`
- `result=ok|parse_error|io_error`

**Step 2: Run test to verify it fails**

Run: `cargo test -p claude-view-server metrics_source_labels -- --nocapture`
Expected: FAIL because labels/counters do not exist.

**Step 3: Implement source-labeled metrics/logging**

Add counters/histograms for:
- files scanned
- parse failures
- sessions indexed
- pass2 parse latency

And structured logs that include `source`, `session_id`, and parser path.

**Step 4: Run test to verify it passes**

Run: `cargo test -p claude-view-server metrics_source_labels -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/server/src/metrics.rs crates/db/src/indexer_parallel.rs crates/core/src/codex/parser.rs

git commit -m "feat(observability): add source-labeled parser and indexer metrics"
```

---

### Task 5: Benchmark and Enforce Performance Budgets

**Files:**
- Create: `crates/db/examples/bench_codex_indexing.rs`
- Modify: `crates/db/examples/bench_indexing.rs`
- Create: `docs/testing/codex-performance-baselines.md`

**Step 1: Write failing benchmark assertions (soft guard)**

Define baseline expectations:
- Pass1 Codex discovery: target `< 150ms` for 1k session files on SSD
- Pass2 deep parse: target `< 35ms` median per session fixture

**Step 2: Run benchmark to capture current baseline**

Run: `cargo run -p claude-view-db --example bench_codex_indexing`
Expected: emits structured benchmark output.

**Step 3: Implement benchmark harness**

- Build deterministic synthetic Codex fixture set.
- Emit machine-readable summary for CI artifact capture.

**Step 4: Re-run benchmark and record baseline**

Run: `cargo run -p claude-view-db --example bench_codex_indexing`
Expected: outputs stable numbers and no panics.

**Step 5: Commit**

```bash
git add crates/db/examples/bench_codex_indexing.rs crates/db/examples/bench_indexing.rs docs/testing/codex-performance-baselines.md

git commit -m "perf(db): add codex indexing benchmark and baseline docs"
```

---

### Task 6: Add Rollout Flags + Operational Playbook

**Files:**
- Modify: `crates/server/src/main.rs`
- Modify: `README.md`
- Create: `docs/testing/codex-rollout-playbook.md`
- Test: `crates/server/src/main.rs` (config tests)

**Step 1: Write failing config tests**

Add tests for feature flag behavior:
- `ENABLE_CODEX_SOURCE=false` disables Codex discovery/live watching
- default enables Codex when directory exists

**Step 2: Run test to verify it fails**

Run: `cargo test -p claude-view-server codex_flag_config -- --nocapture`
Expected: FAIL before env flag support is implemented.

**Step 3: Implement flags + docs**

- Add env flags:
  - `ENABLE_CODEX_SOURCE` (default true)
  - `CODEX_SESSIONS_DIR` (override path)
- Document rollout sequence:
  - schema migration
  - codex-disabled warm start
  - codex-enabled canary
  - full enable
  - rollback plan

**Step 4: Run test to verify it passes**

Run: `cargo test -p claude-view-server codex_flag_config -- --nocapture`
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/server/src/main.rs README.md docs/testing/codex-rollout-playbook.md

git commit -m "docs(ops): add codex rollout flags and operational playbook"
```

---

## Exit Criteria

- Mixed-source system is migration-safe and idempotent under repeated indexing.
- Source-labeled observability exists for parser/indexer/live failures.
- Rollout can be toggled and safely reversed without data loss.

## Verification Checklist

Run:
- `cargo test -p claude-view-core codex_fixture_contract`
- `cargo test -p claude-view-db migration_source_backfill_idempotent`
- `cargo test -p claude-view-server system_reindex_sources metrics_source_labels codex_flag_config`
- `cargo run -p claude-view-db --example bench_codex_indexing`

Expected:
- Full test pass.
- Benchmark outputs recorded in `docs/testing/codex-performance-baselines.md`.

## Risks and Mitigations

- Risk: overly strict parser fails on future Codex payload variants.
  - Mitigation: fixture corpus + tolerant parsing + explicit unknown-event counters.
- Risk: rollout surprises on large local datasets.
  - Mitigation: source flag allows codex-disable rollback without reverting schema.

