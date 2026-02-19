---
status: done
date: 2026-02-06
theme: "Theme 2: Dashboard Analytics — PR Review Fixes"
---

# Dashboard Analytics PR Review Fixes

> **Context:** PR #4 (`feature/dashboard-analytics`) was reviewed by 5 specialized agents (code review, silent failure hunting, test coverage, comment accuracy, type design). This plan addresses all findings, organized by priority.

## Summary

| Priority | Count | Description |
|----------|-------|-------------|
| **Critical** | 7 | Runtime crashes, silent data corruption, label/data mismatch |
| **Important** | 10 | Error handling gaps, missing tests, a11y claims, CLAUDE.md violations |
| **Suggestions** | 6 | Naming, dead code, minor inconsistencies |

**Estimated scope:** ~400 lines changed across ~20 files. No new dependencies. No migrations.

---

## Critical Fixes (must fix before merge)

### C1. Fix `undefined !== null` crash in fetch functions

**Files:** `src/hooks/use-dashboard.ts:26`, `src/hooks/use-ai-generation.ts:26`

**Bug:** When `params` is `undefined` (all-time view), `params?.from` evaluates to `undefined`, and `undefined !== null` is `true`. The code enters the `if` block and crashes on `params.from.toString()`.

**Fix:**
```typescript
// BEFORE (crashes when params is undefined)
if (params?.from !== null && params?.to !== null) {

// AFTER (catches both null and undefined)
if (params?.from != null && params?.to != null) {
```

**Test:** Add unit test calling `fetchDashboardStats(undefined)` and `fetchAIGenerationStats(undefined)` — must not throw.

### C2. Fix DateRangePicker draft state — use `prevIsOpenRef` pattern

**File:** `src/components/ui/DateRangePicker.tsx:45-49`

**Bug:** `useEffect([value])` resets draft state on any parent re-render (React Query refetch, etc.), violating CLAUDE.md rule: *"Popovers with draft state must only reset on open-transition."*

**Fix:**
```typescript
// BEFORE (resets on every value change)
useEffect(() => {
    setTempFrom(value?.from)
    setTempTo(value?.to)
}, [value])

// AFTER (only resets when popover opens)
const prevIsOpenRef = useRef(false)
useEffect(() => {
    if (isOpen && !prevIsOpenRef.current) {
        setTempFrom(value?.from)
        setTempTo(value?.to)
    }
    prevIsOpenRef.current = isOpen
}, [isOpen, value])
```

### C3. Show error state in AIGenerationStats instead of `return null`

**File:** `src/components/AIGenerationStats.tsx:27-29`

**Bug:** Entire section silently disappears on error. Users can't tell if data is missing or server is broken.

**Fix:** Replace `return null` with an inline error card matching `StatsDashboard`'s pattern:
```tsx
if (error) {
    return (
        <div className="bg-white dark:bg-gray-900 rounded-xl border border-red-200 dark:border-red-800 p-4">
            <div className="flex items-center gap-2 text-red-500 text-sm">
                <AlertCircle className="w-4 h-4" />
                <span>Failed to load AI generation stats</span>
                <button onClick={() => refetch()} className="underline ml-2">Retry</button>
            </div>
        </div>
    )
}
```

### C4. Log errors instead of `unwrap_or(0)` in storage stats

**File:** `crates/server/src/routes/stats.rs:291-300`

**Bug:** Five `unwrap_or(0)` calls produce fake "0 sessions, 0 projects" data when database errors occur.

**Fix:** Add `tracing::warn!` before each fallback:
```rust
let session_count = match state.db.get_session_count().await {
    Ok(count) => count,
    Err(e) => {
        tracing::warn!(error = %e, "Failed to get session count");
        0
    }
};
// Repeat for project_count, commit_count, sqlite_bytes
// For oldest_session_date, log on Err before .ok().flatten()
```

### C5. Fix heatmap label: "Last 30 Days" → "Last 90 Days"

**File:** `src/components/StatsDashboard.tsx:291`

**Bug:** UI says "Activity (Last 30 Days)" but backend always fetches 90 days of heatmap data (documented in `queries.rs:1287`).

**Fix:** Change label to `Activity (Last 90 Days)`.

### C6. Validate `DashboardQuery` — reject inverted/half-specified ranges

**File:** `crates/server/src/routes/stats.rs` — `dashboard_stats` handler

**Bug:** `from > to` silently produces wrong trends (negative duration inverts comparison period). Only one of `from`/`to` present silently treated as all-time.

**Fix:** Add validation at the top of the handler:
```rust
// Reject half-specified ranges
if query.from.is_some() != query.to.is_some() {
    return Err(ApiError::bad_request("Both 'from' and 'to' must be provided together"));
}
// Reject inverted ranges
if let (Some(from), Some(to)) = (query.from, query.to) {
    if from >= to {
        return Err(ApiError::bad_request("'from' must be less than 'to'"));
    }
}
```

Apply the same validation to the `ai_generation_stats` handler.

### C7. Fix "Others" negative token calculation

**File:** `crates/db/src/queries.rs` — `get_ai_generation_stats`

**Bug:** Total tokens and per-project tokens come from separate queries. Under race conditions or filter differences, `total < sum(top5)` → negative "Others."

**Fix:** Clamp to zero:
```rust
let others_input = (total_input_tokens - top5_input).max(0);
let others_output = (total_output_tokens - top5_output).max(0);
```

---

## Important Fixes (should fix)

### I1. Use consistent timestamp column for time-range filtering

**Files:** `crates/db/src/queries.rs:1260` (`last_message_at`), `:1634` (`first_message_at`)

**Problem:** Dashboard stats and AI generation stats filter on different columns for the same time range, producing mismatched numbers on the same page.

**Fix:** Change `get_ai_generation_stats` to filter on `last_message_at` (matching dashboard stats and trends). Add a comment explaining the choice.

### I2. Add DateRangePicker test file

**File:** `src/components/ui/DateRangePicker.test.tsx` (new)

**Coverage needed:**
- Popover open/close via trigger click, Escape, click outside
- Temp state isolation (draft doesn't commit until Apply)
- Date ordering enforcement (from/to swap)
- Apply button disabled when dates incomplete
- Reset on cancel (temp values revert)
- `prevIsOpenRef` pattern (draft only resets on open transition, not on prop change)

### I3. Fix StatusBar polling: add cleanup + error logging

**File:** `src/components/StatusBar.tsx:38-106`

**Fix:**
- Add `mountedRef` pattern: `useEffect(() => () => { mountedRef.current = false }, [])`
- Check `if (!mountedRef.current) return` at top of each `poll()` iteration
- Add `console.warn('Sync status poll failed:', e)` in catch block
- On `!response.ok`, log warning and retry instead of silently stopping

### I4. Remove false accessibility claims from comments

**File:** `src/components/ui/SegmentedControl.tsx:32` — Remove "Keyboard navigation with arrow keys" (only Tab works)
**File:** `src/components/ui/DateRangePicker.tsx:32` — Remove "Focus trap within popover" (not implemented)

### I5. Fix StorageOverview "Tantivy" → "deep index"

**File:** `src/components/StorageOverview.tsx:53`

**Fix:**
- Comment: "Trigger full Tantivy index rebuild" → "Trigger full deep index rebuild"
- Toast text: "Full Tantivy index rebuild initiated" → "Full deep index rebuild initiated"

### I6. Improve StorageOverview error handling

**File:** `src/components/StorageOverview.tsx:69-101`

**Fix:**
- Error state: add retry button + show `error.message`
- Catch block: capture `e`, log to console, show `e.message` instead of hardcoded "network error"
- Non-200/202/409: read `response.text()` and include in toast

### I7. Fix `useTimeRange` searchParams in useEffect deps

**File:** `src/hooks/use-time-range.ts:161`

**Fix:**
```typescript
const urlKey = searchParams.toString()
// use urlKey in dependency array instead of searchParams
```

### I8. Add warning logging for filesystem errors in `calculate_jsonl_size`

**File:** `crates/server/src/routes/stats.rs:325-365`

**Fix:** Add `tracing::warn!` at each `Err(_) => return 0` / `Err(_) => continue` point.

### I9. Add warning logging for empty catch blocks in `useTimeRange`

**File:** `src/hooks/use-time-range.ts:113-115, 146-148`

**Fix:** Add `console.warn('Failed to read time range from localStorage:', e)` in both catch blocks.

### I10. Deduplicate `TimeRangeParams` interface

**Files:** `src/hooks/use-ai-generation.ts:5-10`, `src/hooks/use-dashboard.ts:5-10`

**Fix:** Extract to `src/types/time-range.ts` (or add to `src/types/generated/index.ts`), import from both hooks.

---

## Suggestions (nice to have)

### S1. Rename `CurrentWeekMetrics` → `CurrentPeriodMetrics`

**Files:** `crates/server/src/routes/stats.rs:33`, corresponding TS type

Struct now represents arbitrary time ranges, not just "current week." The field-level doc already says "Current period metrics."

### S2. Remove unused feature flag declarations from `vite-env.d.ts`

**File:** `src/vite-env.d.ts:20,26`

`VITE_FEATURE_HEATMAP_TOOLTIP` and `VITE_FEATURE_SYNC_REDESIGN` are declared but not used in `features.ts`.

### S3. Hide unimplemented `lines_added`/`lines_removed` from UI

**File:** `src/components/AIGenerationStats.tsx`

These fields always return 0. Showing "Lines Generated: +0" is misleading. Either hide the card when values are 0, or show "Coming soon" indicator.

### S4. Replace `SyncAcceptedResponse.status: String` with enum

**File:** `crates/server/src/routes/sync.rs:37-43`

A free-form `String` for what is always `"accepted"` should be a proper `#[derive(Serialize)]` enum.

### S5. Test `formatModelName` fallback parser

**File:** `src/components/AIGenerationStats.tsx:139-184`

The regex-based fallback for unknown model IDs has zero test coverage. Only the lookup table path is tested.

### S6. Update design doc pseudocode to match implementation

**File:** `docs/plans/2026-02-05-dashboard-analytics-design.md:134-142`

Design doc shows 30-day default when no params; implementation returns all-time stats. Add a note that implementation differs.

---

## Implementation Order

```
Phase 1: Critical fixes (C1-C7)        — all independent, can parallelize
Phase 2: Important fixes (I1-I10)       — I2 depends on C2, rest independent
Phase 3: Suggestions (S1-S6)            — all independent, optional
```

**Dependencies:**
- I2 (DateRangePicker tests) should be written after C2 (prevIsOpenRef fix) since the test should verify the new behavior
- All other items are independent and can be done in any order

## Verification

After all fixes:
1. `cargo test -p db -p server` — Rust tests pass
2. `bunx vitest run` — Frontend tests pass (including new DateRangePicker tests)
3. Manual smoke test: select "All" time range (verifies C1 fix doesn't crash)
4. Manual smoke test: select custom date range, trigger React Query refetch (verifies C2)
5. Kill backend, load dashboard (verifies C3 shows error state)
