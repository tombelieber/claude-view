# Time-Range Contract Hardening (Eliminate Implicit 30-Day Clamp)

**Status:** DRAFT (release-blocking)  
**Date:** 2026-03-05  
**Priority:** P0  
**Owner:** Platform + Web + API  
**Scope:** `crates/core`, `crates/server`, `crates/db`, `apps/web`, `packages/mcp`, `scripts`, `.github/workflows`

## 1. Objective

Fix the root cause of "30-day limit" behavior by removing hidden default clamps, unifying range semantics across in-scope endpoints, and adding merge-blocking regression gates.

This is a query/aggregation contract bug, not a retention bug.

## 2. Verified Ground Truth (Current Code)

1. `/api/insights` silently defaults to last 30 days:
   - `crates/server/src/routes/insights.rs` uses `query.from.unwrap_or(now - 30 * 86400)`.
2. `/api/insights/trends` silently defaults to `range=6mo`:
   - `crates/server/src/routes/insights.rs` has `default_range() -> "6mo"`.
3. `/insights` route does not render Insights page:
   - `apps/web/src/router.tsx` redirects `/insights` to `/analytics?tab=insights`.
   - `apps/web/src/pages/AnalyticsPage.tsx` only supports `overview|contributions`.
4. `InsightsPage` URL state is initialized once and can desync on back/forward/manual URL edits:
   - `apps/web/src/components/InsightsPage.tsx`.
5. `TrendsTab` widens selected ranges:
   - `apps/web/src/components/insights/TrendsTab.tsx` maps `7d->3mo`, `30d->3mo`, `90d->6mo`.
6. `use-trends-data` drops `from=0` due truthy checks:
   - `apps/web/src/hooks/use-trends-data.ts`.
7. CI has no Playwright job and Playwright config is currently ESM-incompatible:
   - `.github/workflows/ci.yml`, `apps/web/playwright.config.ts`, `apps/web/package.json`.
8. Server binary has a macOS-only platform gate:
   - `crates/server/src/main.rs`.

## 3. Scope Boundaries

## In Scope

1. `GET /api/insights`
2. `GET /api/insights/categories`
3. `GET /api/insights/trends`
4. `GET /api/stats/dashboard` (metadata hardening with section-specific ranges)
5. Insights frontend route/state/query behavior
6. CI and regression scripts for implicit clamp prevention

## Explicitly Out of Scope (This Patch)

1. `GET /api/stats/ai-generation` range semantics
2. Contributions API range vocabulary
3. Benchmarks `7d` support

Rationale: these are separate product contracts and not required to remove the observed implicit 30-day clamp.

## 4. Root Cause

1. Multiple independent range-default implementations with conflicting semantics (`30d`, `6mo`, all-time, widened client mapping).
2. No canonical effective-range metadata attached to all affected responses.
3. Route wiring bug hides Insights page, masking expected behavior and confusing diagnostics.
4. Missing CI gate for implicit-clamp regressions.

## 5. Contract Decisions

## 5.1 Canonical Effective Range Metadata

Shared type (in `crates/core`, not `crates/server`):

```ts
type EffectiveRangeSource =
  | "explicit_from_to"
  | "explicit_range_param"
  | "default_all_time"
  | "legacy_one_sided_coercion";

type EffectiveRangeMeta = {
  from: number; // unix seconds, inclusive
  to: number;   // unix seconds, inclusive
  source: EffectiveRangeSource;
};
```

## 5.2 Shared Resolution Rules (In-Scope Endpoints)

1. Explicit `from` and `to` pair has highest precedence.
2. `from` and `to` are inclusive bounds.
3. `from > to` is invalid.
4. `from == to` is valid.
5. Default (no range inputs) is all-time, resolving `from` to oldest session timestamp (or `0` if unavailable), and `to` to `now`.

## 5.3 One-Sided Input Compatibility Policy

Request validation is intentionally hardened, but to avoid hard production breakage:

1. Add temporary server env guard `ALLOW_LEGACY_ONE_SIDED_RANGES` (default `false`).
2. If enabled, one-sided inputs are coerced to legacy behavior with `source=legacy_one_sided_coercion` and warning log.
3. If disabled (default), one-sided inputs return `400`.
4. Remove guard after one release cycle once telemetry confirms no consumers depend on one-sided inputs.

## 5.4 Dashboard Metadata Specificity

`/api/stats/dashboard` has mixed semantics (for example fixed 90-day heatmap), so it must not expose one misleading global range.

Use section-specific metadata:

```json
{
  "meta": {
    "ranges": {
      "currentPeriod": { "from": 0, "to": 1741132800, "source": "default_all_time" },
      "heatmap": { "from": 1733356800, "to": 1741132800, "source": "explicit_range_param" }
    }
  }
}
```

## 6. Endpoint Behavior Matrix (Target)

| Endpoint | Default behavior | One-sided `from/to` | Inverted range | Equality range | Metadata |
|---|---|---|---|---|---|
| `/api/insights` | all-time | `400` or legacy-coerce (guarded) | `400` | valid | `meta.effectiveRange` + legacy fields kept |
| `/api/insights/categories` | all-time | `400` or legacy-coerce (guarded) | `400` | valid | add `meta.effectiveRange` |
| `/api/insights/trends` | all-time when no `range` and no `from/to` | `400` or legacy-coerce (guarded) | `400` | valid | add `meta.effectiveRange` + keep `periodStart/periodEnd` |
| `/api/stats/dashboard` | existing all-time behavior retained | unchanged (`400` currently) | normalize to `from > to` invalid | `from==to` valid | add `meta.ranges.currentPeriod` and `meta.ranges.heatmap` |

## 7. Execution Plan (Ordered, No Gaps)

## Task A: Shared Types and Resolver

**Files:**

1. `crates/core/src/time_range_contract.rs` (new)
2. `crates/core/src/lib.rs`
3. `crates/server/src/time_range.rs` (new)
4. `crates/server/src/lib.rs`
5. `crates/server/tests/time_range_contract_test.rs` (new)

**Steps:**

1. Add `EffectiveRangeMeta` and `EffectiveRangeSource` in `crates/core`.
2. Add server resolver functions in `crates/server/src/time_range.rs`:
   - `resolve_from_to_or_all_time(...)`
   - `resolve_range_param_or_all_time(...)`
3. Make resolver return resolved bounds plus `source`.
4. Include legacy one-sided coercion path behind `ALLOW_LEGACY_ONE_SIDED_RANGES`.
5. Add deterministic resolver tests in `crates/server/tests/time_range_contract_test.rs` for:
   - explicit pair
   - from-only and to-only behavior (strict and legacy modes)
   - inverted range
   - equality range
   - default all-time with and without oldest timestamp

## Task B: Harden `/api/insights`

**Files:**

1. `crates/server/src/routes/insights.rs`
2. `apps/web/src/types/generated/InsightsMeta.ts` (generated)

**Steps:**

1. Replace current ad-hoc defaults with shared resolver.
2. Preserve existing `meta.timeRangeStart/timeRangeEnd` fields for compatibility.
3. Add additive `meta.effectiveRange`.
4. Add tests:
   - default includes >30-day-old sessions
   - from-only strict rejection
   - to-only strict rejection
   - guarded legacy one-sided coercion behavior
   - inverted rejection
   - equality valid

## Task C: Harden `/api/insights/categories`

**Files:**

1. `crates/server/src/routes/insights.rs`
2. `apps/web/src/types/generated/CategoriesResponse.ts` (generated)

**Steps:**

1. Route bounds through shared resolver.
2. Add additive `meta.effectiveRange`.
3. Add tests for:
   - default all-time includes old sessions
   - from-only/to-only strict rejection
   - guarded legacy one-sided coercion
   - inverted rejection
   - equality valid

## Task D: Harden `/api/insights/trends`

**Files:**

1. `crates/server/src/routes/insights.rs`
2. `apps/web/src/types/generated/InsightsTrendsResponse.ts` (generated)

**Steps:**

1. Change `TrendsQuery.range` to `Option<String>`; remove implicit default function.
2. Apply resolution order:
   - explicit from/to pair
   - explicit range param
   - default all-time
3. Keep supported range values unchanged when explicitly provided.
4. Add `meta.effectiveRange` using a server response wrapper DTO (do not force DB crate to depend on server-local types).
5. Add tests:
   - no params uses all-time
   - from-only strict rejection
   - to-only strict rejection
   - guarded legacy one-sided coercion
   - explicit range uses `explicit_range_param`
   - old data beyond 6 months is included in default all-time

## Task E: Dashboard Metadata Hardening

**Files:**

1. `crates/server/src/routes/stats.rs`
2. `apps/web/src/types/generated/ExtendedDashboardStats.ts` (generated)
3. `packages/mcp/src/tools/stats.ts`

**Steps:**

1. Add additive `meta.ranges.currentPeriod` and `meta.ranges.heatmap`.
2. Keep existing keys unchanged (`currentWeek`, `periodStart`, `periodEnd`, `trends`, etc.).
3. Normalize dashboard validation to allow equality (`from==to`) and reject only `from > to`.
4. Update MCP `get_stats` output additively to include range metadata.
5. Correct MCP date input contract:
   - either require Unix seconds in tool schema, or
   - parse date strings and convert to Unix seconds before request.
6. Add tests for:
   - from-only/to-only rejection
   - inverted rejection
   - equality valid
   - additive metadata presence

## Task F: Observability

**Files:**

1. `crates/server/src/metrics.rs`
2. `crates/server/src/routes/insights.rs`
3. `crates/server/src/routes/stats.rs`

**Steps:**

1. Add metric descriptions:
   - `time_range_resolution_total`
   - `time_range_resolution_error_total`
2. Emit counters with bounded labels `{endpoint,source}` and `{endpoint,reason}`.
3. Emit structured logs for every resolution decision.
4. Add `/metrics` smoke assertion for new counters.

## Task G: Frontend Reachability and Query Alignment

**Files:**

1. `apps/web/src/router.tsx`
2. `apps/web/src/components/InsightsPage.tsx`
3. `apps/web/src/components/insights/TrendsTab.tsx`
4. `apps/web/src/hooks/use-trends-data.ts`
5. `apps/web/src/hooks/use-insights.ts`
6. `apps/web/src/hooks/use-categories.ts`
7. `apps/web/src/hooks/use-time-range.ts`
8. `apps/web/src/hooks/use-time-range.test.tsx`
9. `apps/web/src/hooks/use-trends-data.test.ts` (new)
10. `apps/web/src/components/InsightsPage.test.tsx` (new)
11. `apps/web/src/router.test.tsx` (new)

**Steps:**

1. Route `/insights` directly to `InsightsPage`.
2. Keep `/analytics` tab set unchanged (`overview|contributions`) unless explicitly expanded.
3. In `InsightsPage`, derive `timeRange` and `activeTab` from URL source-of-truth (no one-time-only init drift).
4. Remove hidden trends widening (`7d->3mo`, `30d->3mo`, `90d->6mo`).
5. Update trends query contract to mutually exclusive modes:
   - explicit `from/to`, or
   - explicit `range`,
   - never both.
6. Fix `use-trends-data` serialization to use nullish checks, not truthy checks (`from=0` must serialize).
7. Reconcile `granularity` when time range changes in `TrendsTab`.
8. For `all` selection in Insights hooks, omit explicit `from/to` so backend default-all-time path is exercised and observable.
9. Create and add tests:
   - `/insights` route render
   - URL back/forward/manual edit sync for range+tab
   - trends request payload has no hidden widening
   - trends request never sends both `range` and `from/to`
   - `from=0` serialization case

## Task H: CI, Tooling, and Regression Script

**Files:**

1. `apps/web/playwright.config.ts`
2. `.github/workflows/ci.yml`
3. `scripts/ci/check-no-implicit-30d.sh` (new)
4. `scripts/generate-types.sh`
5. `apps/web/e2e/insights-range-contract.spec.ts` (new)

**Steps:**

1. Make Playwright config ESM-safe (`fileURLToPath(import.meta.url)` instead of `__dirname`).
2. Ensure CI e2e runner is `macos-latest` because server is macOS-gated.
3. In e2e job, install prerequisites:
   - Bun
   - Rust toolchain + cache
   - Playwright browser (`chromium`)
4. Install workspace dependencies in e2e job: `bun install --frozen-lockfile`.
5. Build frontend assets before e2e (`apps/web/dist` required by `STATIC_DIR`).
6. Add blocking `web-e2e` job with artifact upload on failure.
7. Create `scripts/ci` directory explicitly (`mkdir -p scripts/ci`).
8. Add `scripts/ci/check-no-implicit-30d.sh` and make it executable.
9. Wire `bash scripts/ci/check-no-implicit-30d.sh` into `.github/workflows/ci.yml` as a required non-optional step/job (no `continue-on-error`).
10. Add `apps/web/e2e/insights-range-contract.spec.ts` and include it in the CI e2e command.
11. In script, fail on:
   - implicit `unwrap_or(now - 30 * 86400)` in in-scope handlers
   - implicit trends `6mo` default
   - `/insights -> /analytics?tab=insights` redirect
12. Harden `scripts/generate-types.sh` for verification path:
   - remove silent `|| true` for codegen commands
   - include DB codegen export command
   - avoid platform-specific `sed -i ''` behavior in CI path

## Task I: Mechanical Verification Commands (Run After Tasks A-H)

Run from repo root:

```bash
set -euo pipefail

test -x scripts/ci/check-no-implicit-30d.sh

cargo test -p claude-view-server --test time_range_contract_test -- --nocapture
cargo test -p claude-view-server routes::insights -- --nocapture
cargo test -p claude-view-server routes::stats -- --nocapture
cargo test -p claude-view-db insights_trends -- --nocapture

./scripts/generate-types.sh

cd apps/web
bun run test -- src/hooks/use-time-range.test.tsx
bun run test -- src/hooks/use-trends-data.test.ts
bun run test -- src/components/InsightsPage.test.tsx
bun run test -- src/router.test.tsx
bun run test:e2e e2e/dashboard-time-range.spec.ts
bun run test:e2e e2e/insights-range-contract.spec.ts
cd -

bash scripts/ci/check-no-implicit-30d.sh
```

## 8. Release Strategy

## Stage 1 (Staging)

1. Deploy with strict mode default (`ALLOW_LEGACY_ONE_SIDED_RANGES=false`).
2. Run historical-data validation (dataset older than 30 days).
3. Verify metrics:
   - `time_range_resolution_total`
   - `time_range_resolution_error_total`
4. Verify `/insights` deep-link behavior and URL synchronization.

## Stage 2 (Production)

1. Deploy after staging pass.
2. Watch 24h telemetry for one-sided range errors.
3. If emergency compatibility needed, temporarily enable legacy guard and roll forward with alert.

## Rollback

1. Revert release commit(s) and redeploy previous build.
2. Restore default env (legacy guard off unless needed for emergency).
3. Verify `/api/health` and `/metrics`.

## 9. Definition of Done

1. In-scope endpoints have no silent 30-day or 6-month default clamps.
2. `/insights` route directly renders Insights page.
3. Insights URL state remains consistent through back/forward/manual edits.
4. Trends requests do not widen selected windows.
5. Effective range metadata is present and correct for all in-scope responses.
6. Dashboard metadata uses section-specific ranges (no misleading single global range).
7. Range resolver unit tests and endpoint contract tests all pass.
8. `check-no-implicit-30d.sh` is merge-blocking in CI.
9. Playwright e2e runs in CI on a runner compatible with server platform gate.
10. Re-audit finds zero blocker issues.

## 10. Open Decisions (Must Close Before Merge)

1. Timeline to remove `ALLOW_LEGACY_ONE_SIDED_RANGES`.
2. Whether to extend this contract to `/api/stats/ai-generation` immediately after this patch.
3. Whether to add native `7d` support for benchmarks in a follow-up patch.

## Changelog of Fixes Applied (Audit -> Final Plan)

| # | Issue | Severity | Fix Applied |
|---|---|---|---|
| 1 | Inconsistent metadata naming across plan drafts | Blocker | Standardized canonical effective-range contract and source enum. |
| 2 | Missing `/api/insights/trends` coverage | Blocker | Added dedicated Task D with concrete tests and query precedence rules. |
| 3 | `/insights` route unreachable | Blocker | Added explicit route rewiring and frontend tests. |
| 4 | `InsightsPage` URL desync risk | Blocker | Added URL source-of-truth requirement for tab/range. |
| 5 | Trends widening logic incomplete | Blocker | Explicitly covered `7d->3mo`, `30d->3mo`, `90d->6mo` removal. |
| 6 | `use-trends-data` missing from plan scope | Blocker | Added file to Task G and explicit serializer contract changes. |
| 7 | `from=0` dropped by truthy checks | Blocker | Added nullish-serialization requirement. |
| 8 | Optional DB trends file wording caused non-executable ambiguity | Blocker | Switched to server wrapper DTO approach and removed optional ambiguity. |
| 9 | Shared type placement would create crate dependency problem | Blocker | Moved shared type ownership to `crates/core`. |
| 10 | Trends `range` default prevented source attribution | Blocker | Changed plan to `Option<String>` with no implicit default. |
| 11 | Dashboard single-range metadata was inaccurate (fixed 90d heatmap) | Blocker | Replaced with section-specific ranges metadata. |
| 12 | `scripts/ci/check-no-implicit-30d.sh` path absent | Blocker | Added explicit directory creation and script creation steps. |
| 13 | Playwright/CI prerequisites missing | Blocker | Added macOS runner, Rust toolchain, browser install, and web build prerequisites. |
| 14 | Non-blocking test command (`|| true`) in verification | Blocker | Removed; verification now strictly fail-fast. |
| 15 | Verification did not include resolver tests | Warning | Added `cargo test ... time_range`. |
| 16 | Invariant wording accidentally implied out-of-scope endpoint coverage | Warning | Added explicit scope boundary section. |
| 17 | Missing metrics description updates | Minor | Added explicit `describe_counter!` requirement and `/metrics` assertion. |
| 18 | CI e2e flow did not explicitly install JS deps | Blocker | Added `bun install --frozen-lockfile` to Task H e2e prerequisites. |
| 19 | Regression script existed in plan but not explicitly wired as required CI step | Blocker | Added explicit CI wiring requirement for `check-no-implicit-30d.sh` with non-optional status. |
| 20 | Resolver test selector was non-deterministic and could false-green | Blocker | Added dedicated integration test target `time_range_contract_test` and exact Task I command. |
| 21 | Task I referenced test files/specs not explicitly created in earlier tasks | Blocker | Added explicit creation targets for unit and e2e test files in Task G/H. |
