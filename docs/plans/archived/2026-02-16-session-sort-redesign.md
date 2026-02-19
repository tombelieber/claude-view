---
status: done
date: 2026-02-16
---

# Session Sort Redesign — FIFO Waiting, Cache as Visual Hint

## Problem

The Kanban view sorts the "Needs You" column by **cache warmth first**, then urgency, then recency. This causes:

1. **Warm idle sessions sit above cold errors** — a low-priority session with warm cache ranks above a broken session with expired cache
2. **Cards jump every 5 minutes** when cache transitions warm→cold, reordering the entire column
3. **Cold sessions are dimmed to 70% opacity** even when they have urgent errors
4. **No visible explanation** for why sessions are ordered the way they are

The sort optimizes for cost efficiency (resume while cache is warm) instead of the user's actual mental model.

## User Mental Model

Through brainstorming, the user's mental model was identified as:

- **Waiting** = "Claude needs me to do something" (any state: error, permission, input, idle)
- **Running** = "Claude is working, leave it alone"
- Within Waiting, the user doesn't think in urgency tiers — they just want a **stable, intuitive queue**
- Cache warm/cold is recognized as a **cost hint**, not a **priority signal**

## Design

### Column Names

| Current | New |
|---------|-----|
| Needs You | **Waiting** |
| Running | Running (unchanged) |

### Sort Logic

**Waiting column:** `lastActivityAt` ascending (longest waiting first = FIFO)

```
┌─ Waiting (5) ────────────────────┐
│                                   │
│ [Error] payment webhook           │  ← been waiting longest
│ [Permission] deploy               │
│ [Approval] billing                │
│ [Input] dark mode                 │
│ [Idle] auth refactor              │  ← just stopped
│                                   │
└───────────────────────────────────┘
```

**Running column:** `lastActivityAt` descending (most recently active first) — unchanged.

### Visual Changes

| Aspect | Current | New |
|--------|---------|-----|
| Cache ring | Affects sort + dims cards | Removed (replaced by inline countdown in Plan: Indicator Simplification) |
| Cold session opacity | 70% | 100% (full opacity) |
| Section dividers | None | None (flat list, FIFO is self-explanatory) |

### Why FIFO

- **Stable ordering** — cards only move up as sessions above them get handled, never jump
- **Cache countdown in spinner** — Plan: Indicator Simplification adds an inline cache TTL countdown to the SessionSpinner, so users can still see cache urgency without the ring

## Files to Change

| File | Change |
|------|--------|
| `src/components/live/KanbanView.tsx` | Replace cache→urgency→recency sort with `a.lastActivityAt - b.lastActivityAt`. Rename column title to "Waiting". |
| `src/components/live/KanbanColumn.tsx` | Remove `opacity-70` conditional for cold-cache sessions. |
| `src/components/live/MobileStatusTabs.tsx` | Rename "Needs You" tab label to "Waiting". |
