# Analytics Sidechain/Subagent Metrics Contract Hardening

**Status:** DRAFT (release-blocking for analytics trust)  
**Date:** 2026-03-05  
**Priority:** P0  
**Owner:** Platform + API + Web  
**Scope:** `crates/db`, `crates/server`, `apps/web`, `packages/shared`, `scripts`, `.github/workflows`

## 1. Objective

Eliminate ambiguity and partial undercounting in Analytics by making sidechain/subagent inclusion explicit, consistent, and test-enforced.

Target outcome:
1. Session count semantics remain intentional and stable.
2. Non-session metrics (lines/files/tokens/cost/tool usage/etc.) are internally consistent with declared data scope.
3. API responses expose machine-readable scope metadata so UI and downstream consumers cannot misinterpret numbers.

## 2. Verified Ground Truth (Current Code)

1. User-facing analytics endpoints query `valid_sessions` (non-sidechain):
   - `valid_sessions` view: `is_sidechain = 0` in `crates/db/src/migrations.rs:647`.
   - Dashboard uses `valid_sessions`: `crates/db/src/queries/dashboard.rs:65`.
   - Insights uses `valid_sessions`: `crates/server/src/routes/insights.rs:498`.
   - Contributions uses `valid_sessions`: `crates/db/src/snapshots.rs` (`get_*` methods).
   - AI generation stats also query `valid_sessions`: `crates/db/src/queries/ai_generation.rs`.

2. Subagent content is only partially merged into parent session during deep indexing:
   - Merged from `subagents/*.jsonl`: tokens + cache tokens + turns + models in `crates/db/src/indexer_parallel.rs:2508-2530`.

3. AI contribution lines are computed from parent `raw_invocations` before/without merging subagent invocations:
   - `count_ai_lines(...)` from `result.raw_invocations` in `crates/db/src/indexer_parallel.rs:1363-1372`.

4. Contributions trend intentionally fills missing days with zero values:
   - `fill_date_gaps(...)` in `crates/db/src/snapshots.rs:26-74`.

5. Observed local data confirms >30-day data exists in DB, but can be sparse:
   - Earliest `valid_sessions` day observed: `2026-01-17`.
   - Sessions older than 30 days exist.

## 3. Problem Statement

Current analytics semantics are internally inconsistent:
1. `sessions` counts exclude sidechains (by design).
2. Some heavy-usage metrics include subagent signal (tokens/cost via merged turns).
3. Other productivity metrics can miss subagent signal (AI lines/files/tool-use derived from parent-only invocations).

This creates trust risk: users see one "all-time" view, but underlying fields are not aligned to one scope.

## 4. Contract Decisions (Target)

## 4.1 Data Scope Vocabulary

Introduce canonical scope enum (shared type):

```ts
type AnalyticsDataScope =
  | "primary_sessions_only"
  | "primary_plus_subagent_work";
```

## 4.2 Session Count Rule

`session` count remains `primary_sessions_only` (non-sidechain). This is intentional and should not change silently.

## 4.3 Non-Session Metric Rule

All non-session aggregate metrics in analytics pages must use one consistent scope within a response.

Recommended default:
1. `primary_plus_subagent_work` for workload/cost/productivity metrics.
2. Parent session attribution for subagent work (project/branch/date inherited from parent session).

## 4.4 API Metadata Requirement

Every in-scope analytics response must include scope metadata:

```json
{
  "meta": {
    "dataScope": {
      "sessions": "primary_sessions_only",
      "workload": "primary_plus_subagent_work"
    }
  }
}
```

## 4.5 Backward Compatibility

1. Existing numeric fields remain additive/compatible where possible.
2. Scope metadata is additive and required for all new clients.
3. If behavior changes materially, gate behind temporary env flag and announce in release notes.

## 5. In Scope

1. `GET /api/stats/dashboard`
2. `GET /api/contributions`
3. `GET /api/insights`
4. `GET /api/insights/categories`
5. `GET /api/insights/trends`
6. `GET /api/stats/ai-generation` (scope alignment only)
7. Frontend analytics labels/tooltips for scope disclosure

## 6. Out of Scope (This Patch)

1. Changing the definition of what a "session" means.
2. Re-attributing subagent work to separate synthetic sessions.
3. Historical perfect reconstruction beyond available indexed data.

## 7. Execution Plan

## Task A: Shared Scope Types

**Files:**
1. `packages/shared/src/types/generated/*` (codegen target)
2. `crates/core/src/types.rs` (or dedicated analytics contract file)

**Steps:**
1. Add `AnalyticsDataScope` enum.
2. Add `AnalyticsScopeMeta` response shape used by server DTOs.

## Task B: Indexer Metric Parity (Critical)

**Files:**
1. `crates/db/src/indexer_parallel.rs`

**Steps:**
1. Ensure subagent merge path also contributes to non-token productivity signals used by analytics.
2. Minimum parity targets:
   - AI lines added/removed.
   - files read/edited counts.
   - tool-use derived counters if shown in analytics.
3. Keep parent session attribution deterministic.
4. Add focused tests for parent+subagent merge parity.

## Task C: DB Query Alignment

**Files:**
1. `crates/db/src/queries/dashboard.rs`
2. `crates/db/src/snapshots.rs`
3. `crates/db/src/queries/ai_generation.rs`
4. `crates/db/src/insights_trends.rs`

**Steps:**
1. Confirm all user-facing aggregates read normalized per-session fields that already include chosen scope.
2. Remove drift where some fields are computed from source data that excludes merged subagent work.
3. Keep `sessions` count unchanged (`valid_sessions`).

## Task D: API Metadata

**Files:**
1. `crates/server/src/routes/stats.rs`
2. `crates/server/src/routes/contributions.rs`
3. `crates/server/src/routes/insights.rs`
4. Generated TS response types in web/shared packages

**Steps:**
1. Add `meta.dataScope` to all in-scope responses.
2. Ensure metadata reflects actual computation scope.
3. Add route tests asserting metadata presence and correctness.

## Task E: Frontend Disclosure

**Files:**
1. `apps/web/src/components/StatsDashboard.tsx`
2. `apps/web/src/pages/ContributionsPage.tsx`
3. `apps/web/src/components/InsightsPage.tsx`

**Steps:**
1. Surface concise tooltip/caption clarifying session vs workload scope.
2. Avoid ambiguous "all-time" wording without scope context.

## Task F: Regression Tests + Gate

**Files:**
1. `crates/db/tests/*`
2. `crates/server/src/routes/*` tests
3. `apps/web/src/**/*test*`
4. `scripts/ci/check-analytics-scope-contract.sh` (new)
5. `.github/workflows/ci.yml`

**Steps:**
1. Add tests proving parent+subagent parity for key metrics.
2. Add API tests for scope metadata.
3. Add CI gate to fail if in-scope endpoints omit `meta.dataScope`.

## 8. Verification Commands

```bash
set -euo pipefail

cargo test -p claude-view-db indexer_parallel -- --nocapture
cargo test -p claude-view-db insights_trends -- --nocapture
cargo test -p claude-view-server routes::stats -- --nocapture
cargo test -p claude-view-server routes::contributions -- --nocapture
cargo test -p claude-view-server routes::insights -- --nocapture

cd apps/web
bun run test -- src/components/StatsDashboard.test.tsx
bun run test -- src/pages/ContributionsPage.test.tsx
bun run test -- src/components/InsightsPage.test.tsx
cd -

bash scripts/ci/check-analytics-scope-contract.sh
```

## 9. Rollout Strategy

1. Stage with metadata first, behavior unchanged if needed.
2. Enable parity behavior in staging and validate before/after deltas.
3. Ship production with changelog note describing scope semantics.
4. Monitor for unexpected metric jumps and provide operator runbook.

## 10. Definition of Done

1. Session counts remain non-sidechain and documented.
2. Non-session metrics are scope-consistent within each response.
3. Subagent contribution is either fully included per contract or explicitly excluded per contract (no mixed mode).
4. `meta.dataScope` exists on all in-scope analytics responses.
5. Frontend presents scope context to avoid misleading interpretation.
6. CI has a merge-blocking contract check for scope metadata.
