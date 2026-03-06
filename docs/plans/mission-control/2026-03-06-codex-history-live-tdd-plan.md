---
status: approved
date: 2026-03-06
phase: K
depends_on: G,H,I,J
---

# Phase K: Codex History + Live TDD Evidence-Only Plan

**Goal:** Deliver Codex support for (1) sessions history analytics/metrics and (2) live monitor, with strict TDD and evidence-driven validation.

**Constraint:** Non-bit-perfect replay is acceptable. Data fabrication is not acceptable.

## Non-Negotiable Rules

1. **TDD first:** every change starts with failing tests (RED), then minimal implementation (GREEN), then cleanup (REFACTOR).
2. **NO fabricated parsing/indexing data:** fixtures must come from real Codex usage sessions (consented, de-identified), with provenance manifest and hash checks.
3. **Traceability:** every parsed/indexed metric must map back to raw input evidence.
4. **Source isolation:** Claude and Codex parsing/indexing/live paths must be explicitly source-scoped.
5. **Verifiability gates:** no phase is complete without objective, reproducible test and statistical pass criteria.

---

## Scope

- In scope:
  - Codex historical session ingestion for dashboards/analytics/metrics.
  - Codex live monitoring in Mission Control.
  - Evidence-backed parser/indexer validation with statistical checks.
- Out of scope:
  - Bit-perfect transcript replay.
  - New insights products beyond current dashboard/monitor surfaces.

---

## Deliverables

1. Source-aware ingestion, parsing, indexing, API, and live monitoring for Codex.
2. Reproducible test suite covering parser, indexer, API contracts, live ordering/dedupe.
3. Evidence pack: real-data fixture manifest, anonymization record, statistical validation report.
4. Rollout gate checklist with measurable pass/fail thresholds.

---

## Phase 0: Evidence Pipeline Setup (2-3 days)

**Objective:** enforce real-data-only fixtures and reproducibility before feature work.

**Primary files:**
- `docs/plans/mission-control/2026-03-06-codex-parsing-indexing-statistical-verification.md`
- `artifacts/codex-evidence/` (manifests/reports)

### TDD Steps

1. Write failing checks for fixture provenance and hash manifest validation.
2. Add CI/local validation script contract (plan-level command contract only).
3. Make checks pass with manifest schema + generation process.

### Exit Criteria

1. Every fixture has provenance metadata, raw hash, sanitized hash, collection timestamp.
2. Validation command fails when fixture has no provenance or hash mismatch.

---

## Phase 1: Source-Aware Foundation for History (4-6 days)

**Objective:** prevent ID collision and enforce source dimension end-to-end.

**Primary files:**
- `crates/core/src/discovery.rs`
- `crates/db/src/migrations.rs`
- `crates/db/src/queries/sessions.rs`
- `crates/db/src/queries/dashboard.rs`
- `crates/server/src/routes/sessions.rs`

### TDD Steps

1. RED: add failing tests for duplicate raw session IDs across sources.
2. GREEN: add `source` + `source_session_id` + canonical ID behavior.
3. REFACTOR: centralize source identity helpers and remove duplicate branching.

### Exit Criteria

1. Claude and Codex with same raw ID coexist without collision.
2. API list/detail/parsed/messages routes are source-aware and pass contract tests.

---

## Phase 2: Codex Historical Parsing + Indexing (6-9 days)

**Objective:** ingest Codex sessions into analytics/metrics with traceable correctness.

**Primary files:**
- `crates/db/src/indexer_parallel.rs`
- `crates/core/src/parser.rs` (or Codex adapter module in core)
- `crates/db/src/queries/dashboard.rs`
- `apps/web/src/hooks/use-sessions-infinite.ts`

### TDD Steps

1. RED: failing parser contract tests from real fixture corpus (role mapping, tool calls, token snapshots, unknown event behavior).
2. GREEN: implement Codex parsing/indexing path with minimal transformations.
3. RED: failing analytics tests for session counts/tokens/tool calls by source.
4. GREEN: implement source-aware query logic and pass tests.
5. REFACTOR: isolate Codex adapter logic from Claude parser path.

### Exit Criteria

1. Session discovery recall and indexing precision meet statistical thresholds in verification protocol.
2. Dashboard metrics include Codex sessions and remain stable across reindex reruns.

---

## Phase 3: Codex Live Monitor (7-10 days)

**Objective:** show Codex live activity reliably, not bit-perfect.

**Primary files:**
- `sidecar/src/session-manager.ts`
- `sidecar/src/types.ts`
- `crates/server/src/live/manager.rs`
- `crates/server/src/live/state.rs`
- `crates/server/src/routes/live.rs`
- `apps/web/src/components/live/use-live-sessions.ts`

### TDD Steps

1. RED: failing tests for event ordering, dedupe, reconnect, degraded-mode transitions.
2. GREEN: implement `codex exec --json` adapter and canonical event normalization.
3. RED: failing SSE contract tests for mixed-source live sessions.
4. GREEN: source-aware live stream and UI updates.
5. REFACTOR: move shared live-state logic into source-agnostic helpers.

### Exit Criteria

1. Live Codex sessions appear in Mission Control within target latency.
2. Duplicate/out-of-order events do not corrupt session state.
3. Degraded mode is explicit and recoverable.

---

## Phase 4: Statistical Verification + Release Gates (3-5 days)

**Objective:** prove correctness with evidence and statistical analysis before rollout.

### Required Evidence Artifacts

1. `artifacts/codex-evidence/fixture-manifest.json`
2. `artifacts/codex-evidence/sampling-report.md`
3. `artifacts/codex-evidence/parsing-indexing-validation.json`
4. `artifacts/codex-evidence/live-monitor-validation.json`

### Release Gates

1. Parser/indexer statistical thresholds pass on holdout dataset.
2. Claude regression suite has zero critical failures.
3. Mixed-source reindex idempotency passes.
4. Canary rollout metrics remain within error budget for 7 days.

---

## Effort and Confidence

1. 1 engineer: 22-34 engineer-days.
2. 2 engineers: 24-36 engineer-days total, 3-5 calendar weeks.
3. Confidence: Medium (main unknowns are Codex schema drift and live event ordering behavior).

---

## Definition of Done

1. Plan artifacts are complete and reproducible.
2. TDD evidence exists for each phase (RED/GREEN/REFACTOR records).
3. Parsing/indexing validation is evidence-backed and statistically accepted.
4. No fabricated fixture data is present in test corpus.
