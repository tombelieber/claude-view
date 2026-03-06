---
status: pending
date: 2026-03-06
phase: K
depends_on: I
---

# Phase K: Codex TDD + Evidence-First Verification Plan

**Goal:** Deliver Codex support for:
1. sessions history for dashboard/analytics/metrics
2. live monitor

**Hard constraints:**
1. TDD-only delivery (no implementation without failing tests first)
2. no fabricated data in parser/indexer validation
3. evidence must come from real usage traces (redacted + consented)
4. all claims must be reproducible from committed artifacts and scripts

## Scope

- In scope:
  - Codex history parsing + indexing for analytics
  - Codex live monitor integration
  - Evidence corpus, statistical validation, and release gates
- Out of scope:
  - bit-perfect transcript replay
  - synthetic-only correctness claims

## Target Files (implementation touch map)

- Backend discovery/indexing:
  - `crates/core/src/discovery.rs`
  - `crates/db/src/indexer_parallel.rs`
  - `crates/db/src/migrations.rs`
  - `crates/db/src/queries/sessions.rs`
  - `crates/db/src/queries/dashboard.rs`
- Backend live:
  - `crates/server/src/live/manager.rs`
  - `crates/server/src/live/state.rs`
  - `crates/server/src/routes/live.rs`
  - `crates/server/src/routes/sessions.rs`
- Sidecar/UI:
  - `sidecar/src/session-manager.ts`
  - `sidecar/src/types.ts`
  - `apps/web/src/hooks/use-sessions-infinite.ts`
  - `apps/web/src/components/live/use-live-sessions.ts`

## TDD Workflow (strict, test-first)

For every task in every phase, enforce this sequence:

1. Write failing tests first (`RED`)
2. Capture failing test evidence (logs + command + commit SHA)
3. Implement minimal code to pass (`GREEN`)
4. Refactor while preserving behavior (`REFACTOR`)
5. Re-run full affected suites + statistical checks
6. Merge only if release-gate thresholds pass

### RED Gate Requirements

Each PR must include:

1. test names added before implementation
2. failing output snapshot in PR notes
3. explicit behavior claim linked to failing test IDs

### GREEN Gate Requirements

Each PR must include:

1. passing unit + integration suites for changed modules
2. no skipped tests for new behavior
3. parser/indexer evidence checks re-run on evidence corpus

### REFACTOR Gate Requirements

Each PR must include:

1. no fixture drift unless fixture manifest updated
2. no metric/output contract drift unless approved in plan update
3. idempotent indexing diff == 0 on validation corpus

## Anti-Fabrication Controls (mandatory)

1. Real usage corpus only for correctness claims.
2. Synthetic fixtures may exist for edge cases, but cannot be used alone to claim correctness.
3. Every fixture must have provenance metadata:
   - source kind (`real` or `synthetic`)
   - capture timestamp
   - anonymization method
   - raw hash and redacted hash
4. Parser/indexer assertions must cite corpus item IDs, not hand-written expected values without source linkage.
5. Any fabricated benchmark/metric invalidates the gate and blocks merge.

## Evidence Corpus Policy

Store corpus metadata and derived artifacts under:

- `artifacts/codex-evidence/`
- `artifacts/codex-evidence/manifests/`
- `artifacts/codex-evidence/reports/`

Minimum evidence coverage before GA:

1. short sessions (<= 20 events): at least 100
2. medium sessions (21-200 events): at least 100
3. long sessions (> 200 events): at least 50
4. sessions with tool calls: at least 80
5. sessions with live monitor events: at least 50

## Phase Execution Plan

### Phase K1: Foundation Contracts (2-3 days)

1. Define canonical provider-aware session identity:
   - `source`
   - `source_session_id`
   - canonical composite ID (`codex:<id>`)
2. Freeze event taxonomy for v1 live monitor:
   - `session_started`
   - `message_user`
   - `message_assistant`
   - `tool_started`
   - `tool_result`
   - `task_started`
   - `task_completed`
   - `token_snapshot`
   - `session_completed`
   - `error`
3. Add failing contract tests before schema or adapter changes.

Exit criteria:

1. contract tests exist and pass
2. no Claude regression in existing contract tests

### Phase K2: History Parsing + Indexing for Analytics (7-11 days)

1. Add Codex discovery and parser adapters with RED-first tests.
2. Add source-aware indexing path and analytics query support.
3. Add source filter (`all|claude|codex`) to API and UI hooks.
4. Validate metrics extraction against real evidence corpus.

Exit criteria:

1. parse success rate and field-level accuracy meet thresholds in protocol doc
2. dashboard and trends include Codex data with source filtering
3. reindex idempotency diff == 0

### Phase K3: Live Monitor Integration (9-13 days)

1. v1 transport via `codex exec --json` adapter.
2. Normalize + dedupe + bounded reorder window in live state pipeline.
3. Add degraded mode semantics (transport drop, stale heartbeat).
4. Verify live status correctness on recorded real live traces.

Exit criteria:

1. Codex live sessions appear in Mission Control under load
2. live event ordering/dedupe tests pass
3. degraded mode tests pass with deterministic assertions

### Phase K4: Hardening + Release Gates (3-5 days)

1. Run full parser/indexer/live statistical verification protocol.
2. Run Claude zero-regression suites.
3. Canary rollout behind feature flag `ENABLE_CODEX_SOURCE`.

Exit criteria:

1. all release gates pass
2. no unresolved P0/P1 data-integrity defects
3. signed verification report generated

## Definition of Done

All must be true:

1. TDD evidence present for each merged task (RED->GREEN->REFACTOR)
2. evidence corpus provenance manifests are complete
3. statistical verification report passes thresholds
4. parser/indexer claims traceable to real corpus items
5. Claude path regression report is clean

## Required PR Template Additions

Every Codex PR must include:

1. `Test-First Evidence` section with failing then passing command outputs
2. `Evidence Corpus Coverage` section with corpus IDs used
3. `Statistical Check` link to latest report in `artifacts/codex-evidence/reports/`
4. `No Fabrication Declaration` checkbox

## Effort and Confidence

1. 1 engineer: 22-34 engineer-days
2. 2 engineers: 24-36 engineer-days total, 3-5 weeks calendar
3. confidence: medium, with main risk from Codex schema drift and live ordering behavior
