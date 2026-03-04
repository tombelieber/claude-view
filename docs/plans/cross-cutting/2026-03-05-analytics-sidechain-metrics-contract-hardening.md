# Analytics Sidechain/Subagent Metrics Contract Hardening

**Status:** AUDITED DRAFT (execution-ready)  
**Date:** 2026-03-05  
**Priority:** P0  
**Owner:** Platform + API + Web  
**Scope:** `crates/core`, `crates/db`, `crates/server`, `apps/web`, `scripts`, `.github/workflows`

## 1. Objective

Eliminate ambiguity and partial undercounting in Analytics by making sidechain/subagent inclusion explicit, consistent, and test-enforced.

Target outcome:
1. Session count semantics remain intentional and stable.
2. Non-session metrics (lines/files/tokens/cost/tool usage/etc.) are internally consistent with declared data scope.
3. API responses expose machine-readable scope metadata plus explicit session breakdown (primary vs non-primary) so UI and downstream consumers cannot misinterpret numbers.

## 2. Verified Ground Truth (Current Code)

1. `valid_sessions` currently means `is_sidechain = 0` only (no `last_message_at > 0` guard): `crates/db/src/migrations.rs:647`.
2. Aggregated analytics endpoints primarily read from `valid_sessions`:
   - Dashboard: `crates/db/src/queries/dashboard.rs`.
   - Insights (+ categories/trends/benchmarks): `crates/server/src/routes/insights.rs`.
   - Contributions aggregates/trends: `crates/db/src/snapshots.rs`.
   - AI generation stats: `crates/db/src/queries/ai_generation.rs`.
3. Session contribution detail currently reads from `sessions` (not `valid_sessions`): `crates/db/src/snapshots.rs:1593`.
4. Subagent content merge in pass-2 currently merges tokens/cache/turns/models, but not full productivity signals derived from raw invocations: `crates/db/src/indexer_parallel.rs:2508-2530`.
5. AI contribution lines are computed from `result.raw_invocations` only: `crates/db/src/indexer_parallel.rs:1363-1372`.
6. Insights trends includes proxy formulas (`lines = files_edited_count * 50`, token-proxy cost/line): `crates/db/src/insights_trends.rs:98-100`.
7. ts-rs default export dir is web generated types (`apps/web/src/types/generated`): `.cargo/config.toml:2`.
8. Existing relevant web tests include:
   - `apps/web/src/components/StatsDashboard.test.tsx`
   - `apps/web/src/components/InsightsPage.test.tsx`
   - `apps/web/src/components/AIGenerationStats.test.tsx`
   - `apps/web/src/hooks/use-trends-data.test.ts`
9. Existing CI gate script pattern exists at `scripts/ci/check-no-implicit-30d.sh`; no analytics-scope gate exists yet.

## 3. Problem Statement

Current analytics semantics are internally inconsistent:
1. `sessions` counts exclude sidechains (intentional).
2. Some workload metrics include subagent signal (tokens/cost via merged turns).
3. Some productivity metrics can miss subagent signal (AI lines/files/tool-derived counters).
4. Some endpoints that represent analytics data do not share one explicit machine-readable scope contract.

This creates trust risk: one UI can show numerically valid fields that imply different underlying inclusion rules.

## 4. Contract Decisions (Target)

### 4.1 Data Scope Vocabulary

Canonical scope enum (Rust + generated TS):

```ts
type AnalyticsDataScope =
  | "primary_sessions_only"
  | "primary_plus_subagent_work";
```

### 4.2 Session Count Rule

1. Existing `session`/`totalSessions` fields remain `primary_sessions_only` (non-sidechain) for backward compatibility.
2. Add additive explicit session breakdown in metadata:
   - `primarySessions`
   - `sidechainSessions`
   - `otherSessions` (reserved; currently 0 unless a non-primary non-sidechain class is introduced)
   - `totalObservedSessions` (`primary + sidechain + other`)
3. This makes main-vs-subagent/others explicit without silently redefining existing session fields.

### 4.3 Non-Session Metric Rule

All non-session aggregate metrics in a response must use one consistent workload scope:
1. Default workload scope: `primary_plus_subagent_work`.
2. Parent session attribution for subagent work (project/branch/date inherited from parent session).

### 4.4 API Metadata Requirement

Every in-scope response must include:

```json
{
  "meta": {
    "dataScope": {
      "sessions": "primary_sessions_only",
      "workload": "primary_plus_subagent_work"
    },
    "sessionBreakdown": {
      "primarySessions": 120,
      "sidechainSessions": 35,
      "otherSessions": 0,
      "totalObservedSessions": 155
    }
  }
}
```

### 4.5 Backward Compatibility

1. Existing numeric fields remain additive/compatible where possible.
2. Scope metadata is additive.
3. Responses that currently return shared DB DTOs directly must use additive route wrappers (flatten existing payload + `meta`) instead of mutating DB-layer DTOs in place.

## 5. In Scope Endpoints

1. `GET /api/stats/dashboard`
2. `GET /api/stats/ai-generation`
3. `GET /api/contributions`
4. `GET /api/contributions/sessions/{id}`
5. `GET /api/contributions/branches/{name}/sessions`
6. `GET /api/insights`
7. `GET /api/insights/categories`
8. `GET /api/insights/trends`
9. `GET /api/insights/benchmarks`
10. `GET /api/trends` (legacy API still exposed)
11. Frontend analytics disclosure and scope propagation for all above consumers

## 6. Out of Scope (This Patch)

1. Redefining what a "session" is.
2. Re-attributing subagent work to synthetic sessions.
3. Perfect historical reconstruction beyond indexed data.
4. `GET /api/stats/storage` and unrelated non-analytics system endpoints.

## 7. Execution Plan

### Task A: Canonical Scope Contract Types

**Files:**
1. `crates/core/src/analytics_scope_contract.rs` (new)
2. `crates/core/src/lib.rs` (module export)
3. Generated TS outputs in `apps/web/src/types/generated/*`

**Steps:**
1. Create `AnalyticsDataScope`, `AnalyticsScopeMeta`, and `AnalyticsSessionBreakdown` in `crates/core/src/analytics_scope_contract.rs` with `serde` + `TS` derives (`#[cfg_attr(feature = "codegen", ts(export))]`).
2. Add module export from `crates/core/src/lib.rs`.
3. Use these types in server response DTOs (do not duplicate string literals).
4. Regenerate generated types with `bash scripts/generate-types.sh`.
5. Keep `packages/shared/src/types/generated/*` unchanged unless an explicit `ts(export_to=...)` requirement is introduced.

### Task B: Indexer Subagent Parity (Critical)

**Files:**
1. `crates/db/src/indexer_parallel.rs`

**Steps:**
1. Extend subagent merge path so parent session deep metrics include subagent contributions for:
   - AI lines added/removed
   - files read/edited counts
   - tool counters used in analytics (`tool_counts_*`, derived totals)
2. Keep parent-session attribution deterministic.
3. Add focused tests proving parent-only vs parent+subagent parity for all above counters.
4. Preserve existing token/cost merge behavior.

### Task C: DB Query Alignment + Derivation Integrity

**Files:**
1. `crates/db/src/snapshots.rs`
2. `crates/db/src/insights_trends.rs`
3. `crates/db/src/queries/dashboard.rs`
4. `crates/db/src/queries/ai_generation.rs`
5. `crates/db/src/trends.rs`

**Steps:**
1. Change session contribution detail query to respect non-sidechain semantics (`valid_sessions` or explicit `is_sidechain = 0`).
2. Replace proxy formulas in insights trends for contract metrics (`lines`, `cost_per_line`) with canonical stored fields, or explicitly rename/label as estimates and exclude from strict contract.
3. Verify dashboard/ai-generation/trends aggregates read normalized per-session fields that reflect merged subagent parity.
4. Keep existing session counts on `valid_sessions` semantics (`primary_sessions_only`) while adding additive breakdown counts from `sessions` (`is_sidechain = 1` for sidechain).
5. Update stale comments that still claim `valid_sessions` includes `last_message_at > 0` filtering.

### Task D: API Metadata Rollout

**Files:**
1. `crates/server/src/routes/stats.rs`
2. `crates/server/src/routes/contributions.rs`
3. `crates/server/src/routes/insights.rs`
4. `crates/server/src/routes/trends.rs`
5. `apps/web/src/types/generated/*` (regenerated)

**Steps:**
1. Add `meta.dataScope` to every in-scope endpoint response.
2. Add `meta.sessionBreakdown` to every in-scope endpoint response (even if existing numeric `session` fields stay primary-only).
3. For existing responses already containing `meta`, add `dataScope` and `sessionBreakdown` additively within existing metadata structs.
3. For endpoints returning DB/core DTOs directly, add route-level wrappers:
   - `AIGenerationStatsResponse` (flatten `AIGenerationStats` + `meta`)
   - `BenchmarksResponseWithMeta` (flatten `BenchmarksResponse` + `meta`)
   - `WeekTrendsResponse` for `/api/trends`
4. Extend `ContributionsResponse`, `SessionContributionResponse`, and `BranchSessionsResponse` with `meta.dataScope` + `meta.sessionBreakdown`.
5. Add route tests for metadata presence and expected values on all in-scope endpoints.

### Task E: Frontend Scope Disclosure + Propagation

**Files:**
1. `apps/web/src/components/StatsDashboard.tsx`
2. `apps/web/src/components/ContributionSummaryCard.tsx`
3. `apps/web/src/components/AIGenerationStats.tsx`
4. `apps/web/src/pages/ContributionsPage.tsx`
5. `apps/web/src/components/InsightsPage.tsx`
6. `apps/web/src/components/insights/CategoriesTab.tsx`
7. `apps/web/src/components/insights/TrendsTab.tsx`
8. `apps/web/src/components/insights/BenchmarksTab.tsx`
9. `apps/web/src/hooks/use-insights.ts`
10. `apps/web/src/hooks/use-categories.ts`
11. `apps/web/src/hooks/use-trends-data.ts`
12. `apps/web/src/hooks/use-benchmarks.ts`

**Steps:**
1. Ensure hooks preserve and expose API `meta.dataScope` instead of dropping or replacing it.
2. Ensure hooks preserve and expose API `meta.sessionBreakdown`.
3. Add concise disclosure copy where metrics are rendered (session vs workload semantics + explicit primary vs sidechain/others counts).
3. Remove ambiguous “all-time” phrasing when scope context is required.
4. Keep disclosure wording consistent across dashboard, contributions, insights, and benchmarks/trends surfaces.

### Task F: Regression Tests + CI Contract Gate

**Files:**
1. `crates/db/src/indexer_parallel.rs` tests
2. `crates/server/src/routes/stats.rs` tests
3. `crates/server/src/routes/contributions.rs` tests
4. `crates/server/src/routes/insights.rs` tests
5. `crates/server/src/routes/trends.rs` tests
6. `apps/web/src/components/AIGenerationStats.test.tsx`
7. `apps/web/src/components/StatsDashboard.test.tsx`
8. `apps/web/src/components/InsightsPage.test.tsx`
9. `apps/web/src/hooks/use-trends-data.test.ts`
10. `scripts/ci/check-analytics-scope-contract.sh` (new)
11. `.github/workflows/ci.yml`

**Steps:**
1. Add DB tests proving parent+subagent metric parity for all contract counters.
2. Add API tests asserting `meta.dataScope` + `meta.sessionBreakdown` for all in-scope endpoints, including contributions drill-down and benchmarks/trends.
3. Add frontend tests for scope disclosure rendering on key analytics surfaces.
4. Add merge-blocking script `scripts/ci/check-analytics-scope-contract.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

cargo test -p claude-view-server test_dashboard_stats_includes_data_scope_meta -- --nocapture
cargo test -p claude-view-server test_ai_generation_includes_data_scope_meta -- --nocapture
cargo test -p claude-view-server test_get_contributions_includes_data_scope_meta -- --nocapture
cargo test -p claude-view-server test_get_session_contribution_includes_data_scope_meta -- --nocapture
cargo test -p claude-view-server test_get_branch_sessions_includes_data_scope_meta -- --nocapture
cargo test -p claude-view-server test_insights_includes_data_scope_meta -- --nocapture
cargo test -p claude-view-server test_categories_includes_data_scope_meta -- --nocapture
cargo test -p claude-view-server test_insights_trends_includes_data_scope_meta -- --nocapture
cargo test -p claude-view-server test_benchmarks_includes_data_scope_meta -- --nocapture
cargo test -p claude-view-server test_trends_includes_data_scope_meta -- --nocapture
cargo test -p claude-view-server test_dashboard_stats_includes_session_breakdown_meta -- --nocapture
cargo test -p claude-view-server test_contributions_includes_session_breakdown_meta -- --nocapture
cargo test -p claude-view-server test_insights_includes_session_breakdown_meta -- --nocapture
```

5. Add CI step in `.github/workflows/ci.yml` (same job style as existing contract gates):

```yaml
- name: Analytics scope contract gate
  run: bash scripts/ci/check-analytics-scope-contract.sh
```

## 8. Verification Commands

Run from repository root:

```bash
set -euo pipefail

# Rust verification
cargo test -p claude-view-db indexer_parallel -- --nocapture
cargo test -p claude-view-db insights_trends -- --nocapture
cargo test -p claude-view-server routes::stats -- --nocapture
cargo test -p claude-view-server routes::contributions -- --nocapture
cargo test -p claude-view-server routes::insights -- --nocapture
cargo test -p claude-view-server routes::trends -- --nocapture

# Type generation contract
bash scripts/generate-types.sh

# Web verification (existing tests + newly added contract tests)
cd apps/web
bun run test -- src/components/StatsDashboard.test.tsx
bun run test -- src/components/AIGenerationStats.test.tsx
bun run test -- src/components/InsightsPage.test.tsx
bun run test -- src/hooks/use-trends-data.test.ts
cd -

# CI gate script
bash scripts/ci/check-analytics-scope-contract.sh
```

## 9. Rollout Strategy

1. Ship additive metadata first (no behavior change), verify consumers tolerate fields.
2. Enable parity behavior in staging and validate before/after deltas for tokens, lines, files, tool counters.
3. Ship production with release note describing scope semantics.
4. Monitor post-release for metric jumps with threshold alerting and rollback path.

## 10. Definition of Done

1. Existing session count fields remain non-sidechain and documented.
2. Non-session metrics are scope-consistent within each response.
3. No mixed-mode inclusion for subagent work in contract metrics.
4. `meta.dataScope` and `meta.sessionBreakdown` exist for every in-scope endpoint listed in section 5.
5. Frontend surfaces scope context and explicit primary/sidechain/other breakdown wherever those metrics are shown.
6. CI has a merge-blocking contract gate script and workflow step.
7. `scripts/generate-types.sh` succeeds and generated types reflect new metadata fields.

## 11. Rollback Plan

1. Revert parity merge changes in `indexer_parallel.rs` behind one commit boundary.
2. Keep additive metadata fields if safe; otherwise remove wrapper fields and regenerate types.
3. Re-run section 8 verification commands on rollback branch before hotfix release.

## Changelog of Fixes Applied (Audit -> Final Plan)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | Missing analytics endpoints in scope (benchmarks, drill-down) | Blocker | Added full endpoint inventory in section 5, including drill-down and benchmarks |
| 2 | Legacy `/api/trends` not covered | Warning | Added `/api/trends` to in-scope endpoints and Task D/F |
| 3 | Wrong TS codegen target (`packages/shared`) | Blocker | Corrected canonical target to `apps/web/src/types/generated` and added explicit generation step |
| 4 | Ambiguous contract type location | Warning | Chose explicit canonical file: `crates/core/src/analytics_scope_contract.rs` |
| 5 | Session detail sidechain leak risk | Warning | Added explicit Task C step to enforce non-sidechain semantics for session detail |
| 6 | Proxy trend formulas not contract-safe | Blocker | Added Task C requirement to replace/rename estimate formulas |
| 7 | AI generation metadata shape ambiguity | Warning | Added explicit route wrapper strategy (`AIGenerationStatsResponse`) |
| 8 | Benchmarks metadata shape ambiguity | Blocker | Added explicit wrapper strategy (`BenchmarksResponseWithMeta`) |
| 9 | Frontend file coverage incomplete | Blocker | Expanded Task E to include actual hooks/tabs/components that consume scoped endpoints |
| 10 | `use-insights` would drop metadata | Blocker | Added explicit hook propagation requirement in Task E |
| 11 | Verification command referenced missing web test file | Blocker | Replaced with valid, existing web tests and removed nonexistent target |
| 12 | Verification command referenced missing CI script | Blocker | Added explicit new script content and CI wiring in Task F |
| 13 | CI step placement was vague | Warning | Added concrete CI step snippet for `.github/workflows/ci.yml` |
| 14 | Verification section lacked cwd precondition | Minor | Added “Run from repository root” precondition |
| 15 | Session counts not explicitly split main vs sub/others | Blocker | Added additive `meta.sessionBreakdown` contract (`primary/sidechain/other/total`) across in-scope endpoints and tests |
