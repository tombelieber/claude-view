# Expandable Raw Stats (ReportDetails) — Design

**Date:** 2026-02-21
**Status:** Approved
**Scope:** Build the missing expandable raw stats section from the original Work Reports design

## Context

The Work Reports feature is functionally complete (9 bugs fixed, 36+ tests passing). The one missing UI piece is the expandable raw stats section specified in the original design doc — a collapsible panel showing cost, tokens, tools, skills, and per-project breakdowns.

## Approach

**Approach A + tokens:** Parse `context_digest` JSON on the frontend. Add token aggregation (input/output) to `ContextDigest` struct and the `build_context_digest` flow.

## Data Flow

### Backend (small change)

1. Add `total_input_tokens: u64` and `total_output_tokens: u64` to `ContextDigest` struct in `crates/core/src/report.rs`
2. In `build_context_digest()` (in `crates/server/src/routes/reports.rs`), sum token counts from the sessions query — sessions already fetched via `get_sessions_in_range()`
3. Token counts serialize into `context_digest` JSON column automatically via serde

### Frontend

1. `context_digest` string is already part of `ReportRow` (stored in DB, returned by API)
2. Parse with `JSON.parse()` wrapped in try/catch in the new component
3. New `ReportDetails` component renders parsed data

## Component: ReportDetails

**File:** `src/components/reports/ReportDetails.tsx`

### Layout (collapsed by default)

```
[Details v]
```

### Layout (expanded)

```
[Details ^]
─────────────────────────────────────────
Cost: $6.80 · Tokens: 847K in / 124K out
Top tools: Read (89) · Edit (47) · Bash (23)
Top skills: /commit · /review-pr
─────────────────────────────────────────
claude-view    5 sessions · 2h 48m · 3 commits
  └ feat/reports (3) · main (2)
vicky-wiki     2 sessions · 45m · 1 commit
  └ main (2)
```

### Behavior

- Collapsed by default (`useState(false)`)
- Chevron rotates on toggle
- `max-height` CSS transition for smooth open/close
- Graceful fallback if `context_digest` is null or unparseable (hide section entirely)

### Placement in ReportCard

COMPLETE state only, between `<ReportContent>` and action buttons:

```
┌─ ReportCard ──────────────────────────┐
│  ReportContent (markdown)             │
│                                       │
│  [Details v]  ← ReportDetails         │
│    cost · tokens · tools · projects   │
│                                       │
│  [Copy] [Export .md]                  │
└───────────────────────────────────────┘
```

Also shown for saved reports viewed from ReportHistory.

## TypeScript Type

```typescript
interface ContextDigest {
  report_type: string;
  date_range: string;
  summary_line: string;
  total_input_tokens?: number;
  total_output_tokens?: number;
  projects: {
    name: string;
    session_count: number;
    commit_count: number;
    total_duration_secs: number;
    branches: {
      name: string;
      sessions: { first_prompt: string; category: string; duration_secs: number }[];
    }[];
  }[];
  top_tools: string[];
  top_skills: string[];
}
```

## Formatting Helpers

- **Cost:** `$X.XX` from `totalCostCents / 100`
- **Tokens:** `formatTokens(n)` — e.g., 847000 → "847K", 1234567 → "1.2M"
- **Duration:** reuse existing `formatDuration()` from ReportCard
- **Tools:** show name + count if available from `top_tools` array (currently just names, counts may need extraction from invocations data)

## Testing

- Backend: unit test for `ContextDigest` with token fields (serialization round-trip)
- Frontend: no component tests (matches existing pattern), manual E2E verification by user

## Non-Goals

- Custom date picker (separate feature)
- Auto-generation scheduling (deferred)
- Automated E2E tests (user tests manually)
