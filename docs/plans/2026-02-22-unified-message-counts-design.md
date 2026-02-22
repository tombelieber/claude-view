# Unified Message Counts Design

**Date:** 2026-02-22
**Branch:** fix/token-deduplication
**Status:** Approved

## Problem

Four views of the same session show different message counts:

| View | Source | Count |
|------|--------|-------|
| Live terminal tab | WebSocket (raw) | 275 |
| Live log tab | Separate WebSocket → useActionItems | 152 |
| History chat view | REST → filterMessages() | 92 |
| History log tab | REST → messagesToRichMessages → useActionItems | 64 |

Root causes:
1. Live vs history use different data sources with different message sets
2. Terminal tab and log tab compute counts from different intermediate arrays
3. useActionItems drops system, summary, thinking, non-hook progress events
4. Hook events from SQLite and hook_progress from JSONL are both valid but merged inconsistently

## Design

### Core Principle

One `RichMessage[]` per session. Computed once, consumed everywhere. Both terminal and log tabs show the same badge counts.

### Architecture

```
┌─────────────────────────────────────────────┐
│              Data Source Layer               │
│                                             │
│  Live: WebSocket → parseRichMessage()       │
│  History: REST → messagesToRichMessages()   │
└──────────────┬──────────────────────────────┘
               ↓
┌─────────────────────────────────────────────┐
│              Hook Merge                     │
│                                             │
│  1. hookEventsToRichMessages(sqliteEvents)  │
│  2. mergeByTimestamp(richMessages, hookRich) │
│  No dedup — both hook and hook_progress kept│
└──────────────┬──────────────────────────────┘
               ↓
        canonicalMessages: RichMessage[]
               ↓
    ┌──────────┼──────────┐
    ↓          ↓          ↓
 Terminal    Log Tab    Chat View
   Tab      (actions)  (compact)
 (verbose)
```

### Shared Category Counts

Extract `computeCategoryCounts(messages: RichMessage[])` into a shared utility. Compute once from the canonical array at the parent level. Pass to both RichPane and ActionLogTab as a prop.

```ts
function computeCategoryCounts(messages: RichMessage[]): Record<ActionCategory, number> {
  const counts = { skill: 0, mcp: 0, builtin: 0, agent: 0, hook: 0, hook_progress: 0, error: 0, system: 0, snapshot: 0, queue: 0 }
  for (const m of messages) {
    if (m.category) counts[m.category]++
  }
  return counts
}
```

### Terminal Tab

No behavior change. Renders all messages. Uses shared categoryCounts for filter chips. Category filter applies to the display list.

Only change: categoryCounts prop comes from parent instead of being computed internally.

### Log Tab

useActionItems is expanded to handle all message types:

| RichMessage type | Log tab rendering |
|---|---|
| user / assistant | TurnSeparator (slim label, no full text) |
| thinking | Dropped (trimmed with chat text) |
| tool_use + tool_result | Paired ActionItem (collapsed) |
| hook_progress | ActionItem |
| hook (from SQLite) | HookEventItem |
| error | ActionItem |
| system | **NEW:** ActionItem with system subtype label |
| progress (agent/bash/mcp) | **NEW:** ActionItem with progress subtype label |
| summary | **NEW:** ActionItem with summary label |

Filter chips use the shared categoryCounts (same numbers as terminal tab). When a filter is active, the log tab filters its rendered items by category.

### Hook Merge (No Dedup)

Both sources kept as separate categories:
- `hook_progress` from JSONL stays in canonical array (category: `hook_progress`)
- `hook` events from SQLite converted via hookEventsToRichMessages() (category: `hook`)
- Merged by timestamp
- Both appear as separate filter chips
- suppressRichHookProgress / suppressHookProgress are dead code — delete

### Live/History Parity

- SessionDetailPanel (live): useLiveSessionMessages() produces canonical RichMessage[]. Both terminal and log tabs consume it.
- ConversationView (history): REST → messagesToRichMessages() + hook merge → canonical RichMessage[]. Both verbose view and side panel tabs consume it.
- Within each view context, terminal and log tabs always show identical counts.

## Files to Modify

1. `src/components/live/action-log/use-action-items.ts` — expand to handle system, progress, summary
2. `src/components/live/action-log/ActionLogTab.tsx` — accept categoryCounts prop, remove internal counting
3. `src/components/live/RichPane.tsx` — accept categoryCounts prop, remove internal computation
4. `src/components/ConversationView.tsx` — compute canonical RichMessage[] with hook merge, compute shared categoryCounts
5. `src/components/live/SessionDetailPanel.tsx` — compute shared categoryCounts for live sessions
6. `src/lib/hook-events-to-messages.ts` — delete suppressHookProgress/suppressRichHookProgress
7. `src/components/live/action-log/ActionFilterChips.tsx` — no changes needed (already generic)
8. `src/components/live/action-log/types.ts` — may need new ActionItem subtypes for system/progress/summary

## Dead Code to Remove

- `suppressHookProgress()` in hook-events-to-messages.ts
- `suppressRichHookProgress()` in hook-events-to-messages.ts
- `filterMessages()` in ConversationView.tsx (if compact chat view derives from RichMessage[] instead)
