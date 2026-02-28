# Task List in Live Monitor — Design

**Date:** 2026-02-28
**Status:** Done (shipped 2026-02-28)

## Problem

1. Session card shows "Tasks 6/18" with 5 items + "+13 more" — no way to see all tasks on hover
2. Side panel Overview has zero task display — `progressItems` not in `SessionPanelData`, not rendered anywhere

## Decision Summary

| Decision | Choice | Why |
|----------|--------|-----|
| Card hover | Radix Tooltip showing all tasks | Project rule: "Use @radix-ui/react-* for overlays." Already installed (v1.2.8) |
| Overview placement | First section, above Cost | User chose this — tasks are the primary monitoring concern |
| Show all tasks | Expanded, no collapse | User chose this — full visibility of progress |
| New tab? | No | A list doesn't warrant its own tab (Sub-Agents has swim lanes + timeline + drill-down) |
| New deps? | None | `@radix-ui/react-tooltip` already in `apps/web/package.json` |
| Backend changes? | None | `progressItems` already flows via SSE `session_updated` events |

## Change 1: Card Hover Tooltip

**File:** `apps/web/src/components/live/TaskProgressList.tsx`

Wrap the task list in Radix `Tooltip.Provider > Tooltip.Root > Tooltip.Trigger + Tooltip.Portal > Tooltip.Content`. On hover, show ALL tasks with status icons.

Follows the exact same pattern used in `SubAgentPills.tsx` (lines 62-114):

```
Tooltip.Provider delayDuration={200}
  Tooltip.Root
    Tooltip.Trigger (wraps existing task list div)
    Tooltip.Portal
      Tooltip.Content (full task list, max-h-64 overflow-y-auto)
        Tooltip.Arrow
```

Uses shared `TOOLTIP_CONTENT_CLASS` / `TOOLTIP_ARROW_CLASS` styling constants.

Also: `export` the `STATUS_ICON` and `STATUS_CLASS` constants (currently module-private) so Change 2 can import them.

## Change 2: Overview Section

### a) Data wiring — `session-panel-data.ts`

Add `progressItems?: ProgressItem[]` to `SessionPanelData` interface and pass through in `liveSessionToPanelData()`.

Not added to `historyToPanelData` — history sessions don't have live progress items. Different scope if needed later.

### b) New component — `TasksOverviewSection.tsx`

Full task list for the Overview tab:

- **Header:** "Tasks 6/18" with progress bar + Lucide `ListChecks` icon
- **Items:** All visible, ordered by array index (sorted by ID from backend)
- **Status icons:** `✓` completed (green + strikethrough), `◼` in_progress (blue + `activeForm` text below), `◻` pending (gray)
- **Scroll:** `max-h-[400px] overflow-y-auto` for 12+ items
- **Card styling:** Same as other Overview sections (`rounded-lg border border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-900 p-3`)
- **Shared constants:** Imports `STATUS_ICON` and `STATUS_CLASS` from `TaskProgressList.tsx`

### c) Insertion point — `SessionDetailPanel.tsx`

Insert `<TasksOverviewSection>` as the FIRST section in the Overview tab, before the Cost + Cache grid:

```tsx
{data.progressItems && data.progressItems.length > 0 && (
  <TasksOverviewSection items={data.progressItems} />
)}
```

## Files Touched

| # | File | Change | ~Lines |
|---|------|--------|--------|
| 1 | `apps/web/src/components/live/TaskProgressList.tsx` | Add Radix Tooltip hover, export constants | ~30 |
| 2 | `apps/web/src/components/live/session-panel-data.ts` | Add `progressItems` field + passthrough | ~3 |
| 3 | `apps/web/src/components/live/TasksOverviewSection.tsx` | **NEW** — full task list component | ~60 |
| 4 | `apps/web/src/components/live/SessionDetailPanel.tsx` | Import + render TasksOverviewSection | ~5 |

## What We're NOT Doing

- No new tab
- No filters/search (YAGNI)
- No new npm deps
- No backend changes
- No drag-and-drop or reordering
- No history session support (separate scope)

## Audit

Design passed prove-it audit with HIGH confidence on all 8 claims. Zero flags remaining.
