---
status: pending
date: 2026-02-06
purpose: Fix issues found in Theme 3 PR review — critical before merge, important as fast follow-up
---

# Theme 3: PR Review Fixes

> **Goal:** Address all critical, important, and suggestion-level issues found during the comprehensive PR review of `feature/theme3-contributions`.

## Priority Tiers

| Tier | Scope | Gate |
|------|-------|------|
| **P0 — Fix before merge** | 3 critical issues | Blocks PR merge |
| **P1 — Fast follow-up** | 6 data integrity issues | Ship within 1 day of merge |
| **P2 — Incremental** | 6 type safety + DRY issues | Address when touching these areas |
| **P3 — Nice-to-have** | 8 suggestions | Opportunistic cleanup |

---

## P0: Fix Before Merge (3 issues)

### P0-1. `i64` → `bigint` TS mismatch

**Problem:** `ts-rs` generates `bigint` for Rust `i64`, but `JSON.parse()` returns `number`. Every generated TS type is a lie — runtime values are `number`, TS thinks `bigint`. Components paper over this with `Number()` casts.

**Files:** All structs with `#[ts(export)]` in `crates/db/src/snapshots.rs`, all `src/types/generated/*.ts`

**Fix:** Add `#[ts(type = "number")]` to all `i64` fields in exported structs. These are counts/lines/tokens that will never exceed `Number.MAX_SAFE_INTEGER`. Remove `Number()` casts from React components.

**Affected structs:** `ContributionSnapshot`, `AggregatedContributions`, `DailyTrendPoint`, `ModelBreakdown`, `BranchBreakdown`, `SessionContribution`, `LinkedCommit`, `ModelStats`, `SkillStats`, `UncommittedWork`, `BranchSession`, `FileImpact`

**Verification:** `npx tsc --noEmit` passes without `Number()` casts

---

### P0-2. `setSearchParams` wipes URL params

**Problem:** `setSearchParams({ range: newRange })` creates a fresh `URLSearchParams`, wiping any other query params. Violates explicit CLAUDE.md rule.

**File:** `src/pages/ContributionsPage.tsx:47`

**Fix:**
```tsx
const handleRangeChange = (newRange: TimeRange) => {
    setRange(newRange)
    const params = new URLSearchParams(searchParams)
    params.set('range', newRange)
    setSearchParams(params)
}
```

**Verification:** Set `?range=week&foo=bar` in URL, change range, confirm `foo=bar` survives

---

### P0-3. `generate_missing_snapshots` — 365 individual transactions

**Problem:** First startup loops 365 days, each calling `generate_daily_snapshot` with 3+ SQL operations in separate implicit transactions. Violates CLAUDE.md batch-writes rule. `rollup_weekly_snapshots` correctly uses a transaction — this function should too.

**File:** `crates/db/src/snapshots.rs:1627-1651`

**Fix:** Wrap entire loop in `self.pool().begin()` / `tx.commit()`. Pass `&mut *tx` to inner queries instead of `self.pool()`.

**Verification:** `cargo test -p db -- snapshots`, startup timing before/after on real data

---

## P1: Data Integrity (6 issues)

### P1-1. Silent git diff stats — no logging

**Problem:** `get_commit_diff_stats` catches timeout, spawn failure, and non-zero exit in a single `_ =>` wildcard, returning `DiffStats::default()` with zero logging. Zeros propagate into AI share calculations.

**File:** `crates/db/src/git_correlation.rs:305-323`

**Fix:** Match each error arm separately with `tracing::warn!` (spawn failure, timeout) or `tracing::debug!` (non-zero exit). Keep returning defaults — just log why.

---

### P1-2. Silent JSON parse failures hide corrupt data

**Problem:** `serde_json::from_str(&json).unwrap_or_default()` on `skills_used` and `files_edited` columns. If JSON is malformed, sessions silently show as "(no skill)" / "0 files" with no indication.

**Files:** `crates/db/src/snapshots.rs:1339` (skills), `crates/db/src/snapshots.rs:1509` (files)

**Fix:** Replace `unwrap_or_default()` with `match` + `tracing::warn!` on `Err`. Keep the fallback to empty Vec.

---

### P1-3. `estimate_files_count` returns fabricated data

**Problem:** `(lines / 50).max(0)` presented as real file count. Session with 1000 lines in 1 file shows `files_count: 20`.

**File:** `crates/server/src/routes/contributions.rs:561-565`

**Fix:** Query actual `files_edited_count` from session data (column exists in DB). If unavailable, return `None` instead of a guess.

---

### P1-4. `prompts_per_session` estimated from tokens

**Problem:** `tokens_used / sessions_count / 1000` with `.min(50.0)` clamp. The `user_prompt_count` column exists and is populated. Comment says "this would need prompts data" but the data is already there.

**File:** `crates/server/src/routes/contributions.rs:283-288`

**Fix:** Add `prompts_count` to `contribution_snapshots` schema. Populate from `user_prompt_count` in `generate_daily_snapshot`. Use real data in fluency metrics.

---

### P1-5. Inconsistent cost estimation methods

**Problem:** Two cost estimation approaches: (A) `total_tokens * 0.00025` in snapshots, (B) `(lines_added + lines_removed) * 0.00025` in cost trend. Same magic constant, fundamentally different quantities (tokens vs lines). No "estimated" label in API response.

**Files:** `crates/db/src/snapshots.rs:1974-1978`, `crates/server/src/routes/contributions.rs:343-349`

**Fix:**
1. Extract `BLENDED_COST_PER_TOKEN: f64 = 0.00025` as a named constant
2. Fix cost trend to use token-based estimation (consistent with snapshots)
3. Add `cost_is_estimated: bool` field to `EfficiencyMetrics` response
4. Frontend shows "(estimated)" label when true

---

### P1-6. `get_batch_diff_stats` doesn't actually batch

**Problem:** Despite name and doc comment claiming "single git command," it spawns one `git show` per commit. For 100 commits = 100 sequential processes.

**File:** `crates/db/src/git_correlation.rs:337-354`

**Fix:** Use `git log --format=%H --stat <hash1> <hash2> ...` in a single process, parse combined output. Or at minimum, fix the doc comment to not lie.

---

## P2: Type Safety & DRY (6 issues)

### P2-1. Stringly-typed fields → enums

**Problem:** Four fields use `String` where enums already exist or are trivial to create.

| Field | Current | Fix |
|-------|---------|-----|
| `work_type` | `Option<String>` | `Option<WorkType>` (enum exists in `core`) |
| `FileImpact.action` | `String` | New `enum FileAction { Created, Modified, Deleted }` |
| `ContributionWarning.code` | `String` | New `enum WarningCode { GitSyncIncomplete, CostUnavailable, PartialData }` |
| `ContributionsQuery.range` | `String` | `TimeRange` (enum exists in `db`) |

**Files:** `snapshots.rs:170,260`, `contributions.rs:46,143`

**Benefit:** Serde rejects invalid values at deserialization (400 error), generated TS types become union literals, impossible states unrepresentable

---

### P2-2. `TimeRange::Custom` carries no data

**Problem:** `Custom` variant requires `from`/`to` dates but they travel as separate `Option<&str>` params. Every function needs 3 params (`range`, `from_date`, `to_date`). `Custom` with `None` dates silently produces `1970-01-01..today`.

**File:** `crates/db/src/snapshots.rs:27-42`

**Fix:** Either `Custom { from: String, to: String }` or remove `Custom` and pass a `DateRange` struct.

---

### P2-3. `upsert_snapshot` — 11 positional `i64` params

**Problem:** Easy to swap `ai_lines_removed` and `commits_count` — compiler can't catch it.

**File:** `crates/db/src/snapshots.rs:304-317`

**Fix:** Accept `&ContributionSnapshot` (type already exists) instead of 11 positional params.

---

### P2-4. Date-range calc duplicated 4x

**Problem:** `date_range_from_time_range` helper exists but 4 functions have identical inline copies of the same match block.

**File:** `crates/db/src/snapshots.rs` — `get_contribution_trend` (678), `get_branch_breakdown` (766), `get_reedit_rate` (1820), `get_commit_rate` (1896)

**Fix:** Replace inline copies with `self.date_range_from_time_range(range, from_date, to_date)`

---

### P2-5. `formatNumber`/`formatDuration`/`formatRelativeTime` duplicated 6x

**Problem:** Identical formatting functions copy-pasted across 6 React component files.

**Files:** `BranchCard.tsx`, `SessionDrillDown.tsx`, `UncommittedWork.tsx`, `OverviewCards.tsx`, `EfficiencyMetrics.tsx`, `ModelComparison.tsx`

**Fix:** Extract to `src/lib/format-utils.ts`, import everywhere

---

### P2-6. Dead code in `insights.rs`

**Problem:** Local `ModelStats` shadows DB crate's `ModelStats`. Functions `model_insight()`, `branch_insight()`, `skill_insight()`, `uncommitted_insight()` are never called from routes — only tested.

**File:** `crates/server/src/insights.rs:168-280`

**Fix:** Remove unused types and functions, or wire them into route handlers

---

## P3: Suggestions (8 issues)

These are nice-to-haves. Address opportunistically when touching these areas.

| # | Issue | File | Fix |
|---|-------|------|-----|
| S1 | `COUNT(*) FILTER (WHERE ...)` may fail on SQLite <3.30 | `snapshots.rs:1921,1938` | Use `SUM(CASE WHEN ... THEN 1 ELSE 0 END)` |
| S2 | `upsert_snapshot` TOCTOU race (SELECT then INSERT) | `snapshots.rs:304-414` | Use `INSERT ... ON CONFLICT DO UPDATE` |
| S3 | `date()` function on every row prevents index usage | `snapshots.rs:1579,803,899` | Convert to timestamp range comparison |
| S4 | Invalid `TimeRange` silently defaults to "week" | `contributions.rs:228,470` | Return 400 Bad Request (fixed by P2-1) |
| S5 | Date fields as `String` — no validation | `snapshots.rs`, `contributions.rs` | Consider `chrono::NaiveDate` |
| S6 | `ModelBreakdown` exported to TS but unused in API | `snapshots.rs:138-148` | Remove `#[ts(export)]` or use it |
| S7 | 4-way NULL match in `upsert_snapshot` | `snapshots.rs:320-357` | Use SQLite `IS` operator |
| S8 | Dead `WarningIndicator` + `getWarningMessage` | `WarningBanner.tsx:83-114` | Remove dead code |

---

## Implementation Order

```
P0-2 (URL params)          ~5 min, 1 file
P0-1 (bigint → number)     ~30 min, regenerate TS types
P0-3 (batch transactions)  ~20 min, 1 function
  ↓ merge PR
P1-2 (JSON parse logging)  ~10 min
P1-1 (git diff logging)    ~10 min
P1-5 (cost constant)       ~20 min
P1-3 (real file count)     ~15 min
P1-4 (real prompt count)   ~30 min (migration)
P1-6 (batch git stats)     ~30 min
  ↓ follow-up PR
P2-5 (format utils)        ~15 min
P2-4 (date-range DRY)      ~15 min
P2-6 (dead code cleanup)   ~10 min
P2-1 (stringly → enums)    ~45 min (migration + TS regen)
P2-2 (TimeRange::Custom)   ~30 min
P2-3 (upsert params)       ~20 min
  ↓ cleanup PR
P3-* (suggestions)         opportunistic
```

## Test Plan

- [ ] `cargo test` — all existing 762 tests still pass after each tier
- [ ] `npx tsc --noEmit` — clean after P0-1 TS type regeneration
- [ ] `cargo clippy` — no new warnings
- [ ] Manual: contributions page loads with real data after P0-3
- [ ] Manual: URL params preserved after range change (P0-2)
- [ ] Manual: cost shows "(estimated)" label after P1-5
