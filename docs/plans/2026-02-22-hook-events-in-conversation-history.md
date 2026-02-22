# Hook Events in Conversation History — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Display hook events from SQLite (`hook_events` table) in conversation history, replacing the less-informative `hook_progress` entries from JSONL when richer data is available.

**Architecture:** Frontend-only merge with deduplication. Fetch hook events from the existing `/api/sessions/:id/hook-events` endpoint via the existing `useHookEvents` hook. Convert to `Message[]` / `RichMessage[]`. When hook events exist (session was live-monitored), suppress `hook_progress` from JSONL to avoid duplicates — same underlying event, richer data wins. When no hook events exist (old/unmonitored session), `hook_progress` entries from JSONL display as before.

**Dedup rationale:** `hook_progress` (JSONL) and `hook_events` (SQLite) represent the **same hook execution** captured by two different systems. `hook_progress` has `hookEvent + hookName + command + output`. `hook_events` has `eventName + toolName + label + group + context`. Showing both produces confusing duplicate amber cards at the same timestamp. Precedent: Slack/Discord deduplicate system messages from multiple sources rather than double-display. We prefer the richer source (`hook_events`) when available.

**Tech Stack:** React (TypeScript), existing `useHookEvents` hook, existing REST endpoint, Virtuoso list

---

## Current State (already exists)

- `hook_events` SQLite table (migration 24)
- `insert_hook_events()` / `get_hook_events()` in `crates/db/src/queries/hook_events.rs`
- `GET /api/sessions/:id/hook-events` endpoint in `crates/server/src/routes/sessions.rs`
- `useHookEvents()` React hook in `src/hooks/use-hook-events.ts`
- `HookEventItem` type in `src/components/live/action-log/types.ts`
- `HookProgressCard` renders `hook_progress` from JSONL (already in conversation history)
- `HookProgressCard.test.tsx` — test pattern to follow
- **`HookEventRow`** component in `src/components/live/action-log/HookEventRow.tsx` — already renders rich hook event data with expand/collapse, amber styling, event badges, context pretty-printing. Takes `{ event: HookEventItem }`.

## What This Plan Adds

- Reuses **existing `HookEventRow`** component (no new card component needed)
- `hook-events-to-messages.ts` converter + tests — transforms `HookEventItem[]` → `Message[]` / `RichMessage[]`, carrying the original `HookEventItem` in metadata
- Hook events merged into **Chat view** (MessageTyped/Virtuoso) as synthetic Message objects
- Hook events merged into **Rich view** (RichPane) as synthetic RichMessage objects
- **Dedup:** When `hook_events` exist, `hook_progress` from JSONL is suppressed (richer data wins)
- MessageTyped dispatch test for the new `hook_event` subtype

---

### ~~Task 1: REMOVED — reuse existing HookEventRow~~

**No new component needed.** `HookEventRow` (`src/components/live/action-log/HookEventRow.tsx`) already handles:
- Event badge mapping (`EVENT_BADGE` record with Pre/Post/Fail/Perm/Start/End/etc.)
- Expand/collapse with context pretty-printing (`formatContext`)
- Amber styling, toolName display, timestamp display
- Takes `{ event: HookEventItem }` — the converter in Task 2 carries the original `HookEventItem` in metadata

---

### Task 2: Create hook-events-to-messages converter + tests

**Files:**
- Create: `src/lib/hook-events-to-messages.ts`
- Create: `src/lib/hook-events-to-messages.test.ts`

**Step 1: Write the converter**

Converts `HookEventItem[]` (from `useHookEvents`) into synthetic `Message[]` for Chat view and `RichMessage[]` for Rich view. Also provides a sorted-merge utility and a dedup filter.

Key decisions:
- **UUID uses `e.id`** (not array index) — stable across re-renders and re-fetches
- **Timestamps:** For `Message[]`, store ISO string (matches existing Message.timestamp type). For `RichMessage[]`, store epoch seconds directly (matches RichMessage.ts type).
- **`_sortTs` on Message metadata:** Carry the numeric epoch-second timestamp to avoid re-parsing ISO strings during merge. The merge getter reads `metadata._sortTs` first, falls back to `Date.parse(timestamp)`.
- **`_hookEvent` on metadata:** Carry the original `HookEventItem` so rendering code can pass it directly to `HookEventRow` without reconstructing it from individual fields.

```tsx
// src/lib/hook-events-to-messages.ts
import type { Message } from '../types/generated'
import type { RichMessage } from '../components/live/RichPane'
import type { HookEventItem } from '../components/live/action-log/types'

/**
 * Convert hook events (from SQLite) into synthetic Message objects
 * compatible with the Chat view's MessageTyped component.
 *
 * These arrive with role "progress" and metadata.type "hook_event",
 * which triggers the HookEventRow rendering path in renderProgressSubtype().
 * The original HookEventItem is carried in metadata._hookEvent so the
 * existing HookEventRow component can be reused directly.
 */
export function hookEventsToMessages(events: HookEventItem[]): Message[] {
  return events.map((e) => ({
    uuid: `hook-event-${e.id}`,
    role: 'progress' as const,
    content: `Hook: ${e.eventName} — ${e.label}`,
    timestamp: e.timestamp > 0
      ? new Date(e.timestamp * 1000).toISOString()
      : null,
    metadata: {
      type: 'hook_event',
      _sortTs: e.timestamp > 0 ? e.timestamp : undefined,
      _hookEvent: e,
    },
    thinking: null,
    tool_calls: null,
    category: 'hook',
  }))
}

/**
 * Convert hook events (from SQLite) into RichMessage objects
 * compatible with the Rich view's ProgressMessageCard component.
 * Same as above — carries the original HookEventItem in metadata._hookEvent.
 */
export function hookEventsToRichMessages(events: HookEventItem[]): RichMessage[] {
  return events.map((e) => ({
    type: 'progress' as const,
    content: `Hook: ${e.eventName} — ${e.label}`,
    ts: e.timestamp > 0 ? e.timestamp : undefined,
    category: 'hook' as const,
    metadata: {
      type: 'hook_event',
      _hookEvent: e,
    },
  }))
}

/**
 * Extract a numeric timestamp (epoch seconds) from a Message for sorting.
 * Prefers metadata._sortTs (set by hookEventsToMessages) to avoid
 * re-parsing ISO strings.
 */
export function getMessageSortTs(m: Message): number | undefined {
  const fast = m.metadata?._sortTs
  if (typeof fast === 'number' && fast > 0) return fast
  if (!m.timestamp) return undefined
  const ms = Date.parse(m.timestamp)
  return !isNaN(ms) && ms > 0 ? ms / 1000 : undefined
}

/**
 * Merge two sorted-by-timestamp arrays into one, maintaining order.
 * Items without timestamps go at the end.
 *
 * Both inputs must already be sorted by the key returned by getTs.
 * hook_events from SQLite: ORDER BY timestamp ASC, id ASC
 * Messages from JSONL: chronological order from parser
 */
export function mergeByTimestamp<T>(
  a: T[],
  b: T[],
  getTs: (item: T) => number | undefined,
): T[] {
  if (b.length === 0) return a
  if (a.length === 0) return b

  const merged: T[] = []
  let ai = 0
  let bi = 0

  while (ai < a.length && bi < b.length) {
    const tsA = getTs(a[ai]) ?? Infinity
    const tsB = getTs(b[bi]) ?? Infinity
    if (tsA <= tsB) {
      merged.push(a[ai++])
    } else {
      merged.push(b[bi++])
    }
  }

  while (ai < a.length) merged.push(a[ai++])
  while (bi < b.length) merged.push(b[bi++])

  return merged
}

/**
 * Filter out hook_progress messages from a Message array.
 * Used when hook_events from SQLite are available — the richer data
 * replaces the sparser hook_progress from JSONL, avoiding duplicates.
 */
export function suppressHookProgress(messages: Message[]): Message[] {
  return messages.filter(m => m.metadata?.type !== 'hook_progress')
}

/**
 * Filter out hook_progress RichMessages.
 * Same dedup logic as suppressHookProgress but for the Rich view pipeline.
 */
export function suppressRichHookProgress(messages: RichMessage[]): RichMessage[] {
  return messages.filter(m => m.metadata?.type !== 'hook_progress')
}
```

**Step 2: Write tests**

```tsx
// src/lib/hook-events-to-messages.test.ts
import { describe, it, expect } from 'vitest'
import type { HookEventItem } from '../components/live/action-log/types'
import type { Message } from '../types/generated'
import type { RichMessage } from '../components/live/RichPane'
import {
  hookEventsToMessages,
  hookEventsToRichMessages,
  getMessageSortTs,
  mergeByTimestamp,
  suppressHookProgress,
  suppressRichHookProgress,
} from './hook-events-to-messages'

function makeHookEvent(overrides: Partial<HookEventItem> = {}): HookEventItem {
  return {
    id: 'hook-1',
    type: 'hook_event',
    timestamp: 1706400000,
    eventName: 'PreToolUse',
    label: 'Running: git status',
    group: 'autonomous',
    ...overrides,
  }
}

describe('hookEventsToMessages', () => {
  it('converts a hook event to a synthetic Message with original event in metadata', () => {
    const event = makeHookEvent()
    const msgs = hookEventsToMessages([event])

    expect(msgs).toHaveLength(1)
    expect(msgs[0].role).toBe('progress')
    expect(msgs[0].uuid).toBe('hook-event-hook-1')
    expect(msgs[0].category).toBe('hook')
    expect(msgs[0].metadata.type).toBe('hook_event')
    expect(msgs[0].metadata._hookEvent).toBe(event) // same reference
  })

  it('sets timestamp as ISO string when positive', () => {
    const msgs = hookEventsToMessages([makeHookEvent({ timestamp: 1706400000 })])
    expect(msgs[0].timestamp).toBe(new Date(1706400000 * 1000).toISOString())
  })

  it('sets timestamp to null when zero', () => {
    const msgs = hookEventsToMessages([makeHookEvent({ timestamp: 0 })])
    expect(msgs[0].timestamp).toBeNull()
  })

  it('carries _sortTs in metadata for fast merge', () => {
    const msgs = hookEventsToMessages([makeHookEvent({ timestamp: 1706400000 })])
    expect(msgs[0].metadata._sortTs).toBe(1706400000)
  })

  it('returns empty array for empty input', () => {
    expect(hookEventsToMessages([])).toEqual([])
  })
})

describe('hookEventsToRichMessages', () => {
  it('converts a hook event to a RichMessage with original event in metadata', () => {
    const event = makeHookEvent()
    const rich = hookEventsToRichMessages([event])

    expect(rich).toHaveLength(1)
    expect(rich[0].type).toBe('progress')
    expect(rich[0].category).toBe('hook')
    expect(rich[0].ts).toBe(1706400000)
    expect(rich[0].metadata!.type).toBe('hook_event')
    expect(rich[0].metadata!._hookEvent).toBe(event) // same reference
  })

  it('sets ts to undefined when timestamp is zero', () => {
    const rich = hookEventsToRichMessages([makeHookEvent({ timestamp: 0 })])
    expect(rich[0].ts).toBeUndefined()
  })

  it('returns empty array for empty input', () => {
    expect(hookEventsToRichMessages([])).toEqual([])
  })
})

describe('getMessageSortTs', () => {
  it('returns _sortTs from metadata when available', () => {
    const msg = { metadata: { _sortTs: 1000 }, timestamp: '2026-01-01T00:00:00Z' } as any
    expect(getMessageSortTs(msg)).toBe(1000)
  })

  it('falls back to parsing ISO timestamp', () => {
    const msg = { metadata: {}, timestamp: '2026-01-28T10:00:00Z' } as any
    const expected = Date.parse('2026-01-28T10:00:00Z') / 1000
    expect(getMessageSortTs(msg)).toBe(expected)
  })

  it('returns undefined for null timestamp', () => {
    const msg = { metadata: {}, timestamp: null } as any
    expect(getMessageSortTs(msg)).toBeUndefined()
  })

  it('returns undefined for invalid timestamp string', () => {
    const msg = { metadata: {}, timestamp: 'not-a-date' } as any
    expect(getMessageSortTs(msg)).toBeUndefined()
  })
})

describe('mergeByTimestamp', () => {
  const getTs = (n: { ts?: number }) => n.ts

  it('merges two sorted arrays maintaining order', () => {
    const a = [{ ts: 1 }, { ts: 3 }, { ts: 5 }]
    const b = [{ ts: 2 }, { ts: 4 }]
    const merged = mergeByTimestamp(a, b, getTs)
    expect(merged.map(x => x.ts)).toEqual([1, 2, 3, 4, 5])
  })

  it('returns a when b is empty', () => {
    const a = [{ ts: 1 }]
    const result = mergeByTimestamp(a, [], getTs)
    expect(result).toBe(a) // Same reference, no copy
  })

  it('returns b when a is empty', () => {
    const b = [{ ts: 1 }]
    const result = mergeByTimestamp([], b, getTs)
    expect(result).toBe(b)
  })

  it('pushes items without timestamps to the end', () => {
    const a = [{ ts: 1 }, { ts: undefined }]
    const b = [{ ts: 2 }]
    const merged = mergeByTimestamp(a, b, getTs)
    expect(merged.map(x => x.ts)).toEqual([1, 2, undefined])
  })

  it('preserves order for equal timestamps (stable)', () => {
    const a = [{ ts: 1, src: 'a' }]
    const b = [{ ts: 1, src: 'b' }]
    const merged = mergeByTimestamp(a, b, (x) => x.ts)
    expect(merged[0].src).toBe('a') // a comes first when equal
  })
})

describe('suppressHookProgress', () => {
  it('filters out hook_progress messages', () => {
    const messages = [
      { role: 'user', content: 'hi', metadata: null } as any as Message,
      { role: 'progress', content: '', metadata: { type: 'hook_progress' } } as any as Message,
      { role: 'progress', content: '', metadata: { type: 'bash_progress' } } as any as Message,
    ]
    const filtered = suppressHookProgress(messages)
    expect(filtered).toHaveLength(2)
    expect(filtered[0].role).toBe('user')
    expect(filtered[1].metadata?.type).toBe('bash_progress')
  })

  it('returns all messages when none are hook_progress', () => {
    const messages = [
      { role: 'user', content: 'hi', metadata: null } as any as Message,
    ]
    expect(suppressHookProgress(messages)).toHaveLength(1)
  })

  it('handles messages with null metadata', () => {
    const messages = [
      { role: 'user', content: 'hi', metadata: null } as any as Message,
    ]
    expect(suppressHookProgress(messages)).toHaveLength(1)
  })
})

describe('suppressRichHookProgress', () => {
  it('filters out hook_progress RichMessages', () => {
    const messages: RichMessage[] = [
      { type: 'user', content: 'hi' },
      { type: 'progress', content: '', metadata: { type: 'hook_progress' } },
      { type: 'progress', content: '', metadata: { type: 'hook_event' } },
    ]
    const filtered = suppressRichHookProgress(messages)
    expect(filtered).toHaveLength(2)
    expect(filtered[1].metadata!.type).toBe('hook_event')
  })
})
```

**Step 3: Run tests**

Run: `bunx vitest run src/lib/hook-events-to-messages.test.ts --reporter=verbose`
Expected: All tests pass.

**Step 4: Commit**

```bash
git add src/lib/hook-events-to-messages.ts src/lib/hook-events-to-messages.test.ts
git commit -m "feat(frontend): add hook event converters with dedup and tests"
```

---

### Task 3: Wire HookEventRow into MessageTyped (Chat view) + test

**Files:**
- Modify: `src/components/MessageTyped.tsx` — add `hook_event` case to `renderProgressSubtype()` (line 227-243)
- Modify: `src/components/MessageTyped.test.tsx` — add dispatch test

**Step 1: Add import and case**

Add import at the top of `MessageTyped.tsx`:
```tsx
import { HookEventRow } from './live/action-log/HookEventRow'
```

Add case in `renderProgressSubtype()`, before the `default` case:
```tsx
    case 'hook_event':
      return metadata._hookEvent
        ? <HookEventRow event={metadata._hookEvent} />
        : null
```

**Step 2: Add mock and dispatch test**

In `MessageTyped.test.tsx`, add mock alongside the existing card mocks:
```tsx
vi.mock('./live/action-log/HookEventRow', () => ({
  HookEventRow: (props: any) => <div data-testid="hook-event-row" data-event={JSON.stringify(props.event)} />
}))
```

Add test case in the `'Progress event subtypes'` describe block:
```tsx
    it('dispatches hook_event to HookEventRow', () => {
      const hookEvent = { id: '1', type: 'hook_event', timestamp: 1706400000, eventName: 'PreToolUse', toolName: 'Bash', label: 'Running: git status', group: 'autonomous', context: '{}' }
      render(
        <MessageTyped
          message={makeMessage()}
          messageType="progress"
          metadata={{ type: 'hook_event', _hookEvent: hookEvent }}
        />
      )
      expect(screen.getByTestId('hook-event-row')).toBeInTheDocument()
    })
```

**Step 3: Run tests**

Run: `bunx vitest run src/components/MessageTyped.test.tsx --reporter=verbose`
Expected: All tests pass including new case.

**Step 4: Commit**

```bash
git add src/components/MessageTyped.tsx src/components/MessageTyped.test.tsx
git commit -m "feat(frontend): wire hook_event into MessageTyped with dispatch test"
```

---

### Task 4: Wire HookEventRow into RichPane (Rich view)

**Files:**
- Modify: `src/components/live/RichPane.tsx` — add `hook_event` case to `ProgressMessageCard` (line 709-750)

**Step 1: Add import and case**

Add import at the top of `RichPane.tsx`:
```tsx
import { HookEventRow } from './action-log/HookEventRow'
```

In `ProgressMessageCard` switch (line 713-728), add case before `default`:
```tsx
      case 'hook_event':
        return m._hookEvent
          ? <HookEventRow event={m._hookEvent} />
          : null
```

**Step 2: Run type check**

Run: `bunx tsc --noEmit --pretty`
Expected: No errors.

**Step 3: Commit**

```bash
git add src/components/live/RichPane.tsx
git commit -m "feat(frontend): wire hook_event into RichPane ProgressMessageCard"
```

---

### Task 5: Merge hook events into ConversationView with dedup

**Files:**
- Modify: `src/components/ConversationView.tsx`

This is the main wiring task. `ConversationView` has two view modes:
- **Chat view** (default): renders `filteredMessages` (Message[]) via `MessageTyped` in Virtuoso (line 549-606)
- **Rich view** (verbose): renders `richMessages` (RichMessage[]) via `HistoryRichPane` (line 610)

**Dedup strategy:** When `hookEvents.length > 0` (session was live-monitored), filter out `hook_progress` entries from both pipelines before merging in the richer `hook_events` data. When `hookEvents` is empty (old/unmonitored session), `hook_progress` entries from JSONL display as before — no data loss.

**Step 1: Add imports**

At the top of `ConversationView.tsx`:

```tsx
import { useHookEvents } from '../hooks/use-hook-events'
import {
  hookEventsToMessages,
  hookEventsToRichMessages,
  getMessageSortTs,
  mergeByTimestamp,
  suppressHookProgress,
  suppressRichHookProgress,
} from '../lib/hook-events-to-messages'
```

**Step 2: Fetch hook events**

Inside the component, after the existing data hooks (after line ~253):

```tsx
  // Fetch stored hook events from SQLite (enabled for all historical sessions)
  const hookEvents = useHookEvents(sessionId ?? '', !!sessionId)
```

**Step 3: Wire Chat view pipeline**

After the existing `filteredMessages` / `hiddenCount` derivation (lines 241-245), add:

```tsx
  // Dedup: when hook_events exist (richer data), suppress hook_progress from JSONL
  const hasHookEvents = hookEvents.length > 0
  const dedupedMessages = useMemo(
    () => hasHookEvents ? suppressHookProgress(filteredMessages) : filteredMessages,
    [filteredMessages, hasHookEvents]
  )

  // Convert hook events to synthetic Message objects
  const syntheticHookMessages = useMemo(
    () => hookEventsToMessages(hookEvents),
    [hookEvents]
  )

  // Merge hook events into the message list by timestamp
  const messagesWithHookEvents = useMemo(
    () => mergeByTimestamp(dedupedMessages, syntheticHookMessages, getMessageSortTs),
    [dedupedMessages, syntheticHookMessages]
  )
```

Update Virtuoso `data` prop (line 550):
```tsx
  data={messagesWithHookEvents}
```

Update `initialTopMostItemIndex` (line 552):
```tsx
  initialTopMostItemIndex={Math.max(0, messagesWithHookEvents.length - 1)}
```

Update Footer message count reference (line 591-596): keep using `totalMessages` for the "N messages" count (server-side total unchanged) and keep `hiddenCount` as-is (it still reflects chat-mode filtering). No change needed to the Footer — the existing `totalMessages` and `hiddenCount` variables are still correct since they measure JSONL messages, not synthetic hook events.

Update Header `filteredMessages.length` reference (line 582):
```tsx
  ) : messagesWithHookEvents.length > 0 ? (
```

**Step 4: Wire Rich view pipeline**

After the existing `richMessages` derivation (line 250-253), add:

```tsx
  // Dedup + merge for Rich view
  const dedupedRichMessages = useMemo(
    () => hasHookEvents ? suppressRichHookProgress(richMessages) : richMessages,
    [richMessages, hasHookEvents]
  )

  const richHookMessages = useMemo(
    () => hookEventsToRichMessages(hookEvents),
    [hookEvents]
  )

  const richMessagesWithHookEvents = useMemo(
    () => mergeByTimestamp(dedupedRichMessages, richHookMessages, (m) => m.ts),
    [dedupedRichMessages, richHookMessages]
  )
```

Update `HistoryRichPane` (line 610):
```tsx
  <HistoryRichPane messages={richMessagesWithHookEvents} />
```

Update `historyToPanelData` call (line 258) to use merged rich messages for panel stats:
```tsx
  return historyToPanelData(sessionDetail, richData ?? undefined, sessionInfo, richMessagesWithHookEvents)
```

**Step 5: Run type check**

Run: `bunx tsc --noEmit --pretty`
Expected: No errors.

**Step 6: Commit**

```bash
git add src/components/ConversationView.tsx
git commit -m "feat(frontend): merge hook events into conversation views with dedup"
```

---

### Task 6: Verify end-to-end

**Files:** None — verification only.

**Step 1: Type check**

Run: `bunx tsc --noEmit --pretty`
Expected: No type errors.

**Step 2: Run all frontend tests**

Run: `bunx vitest run --reporter=verbose`
Expected: All tests pass — existing + 2 new test files (hook-events-to-messages, MessageTyped updated).

**Step 3: Visual verification**

1. Start backend: `cargo run -p claude-view-server`
2. Start frontend: `bun run dev`
3. Open a historical session that was **monitored live** (has hook_events in SQLite)
4. **Chat view**: Verify **only** HookEventRow entries appear (amber dot, event badge + label). No duplicate HookProgressCard entries should appear for the same events.
5. **Rich view**: Toggle verbose mode. Verify HookEventRow entries appear in the timeline. No duplicate hook_progress entries.
6. Open a historical session that was **never monitored** (no hook_events in SQLite)
7. **Chat view**: Verify `hook_progress` entries display normally via HookProgressCard (amber, GitBranch icon, "Hook: PreToolUse → eslint"). These are NOT suppressed because there are no hook_events to replace them.
8. **Rich view**: Verify `hook_progress` entries display normally.
9. Open a session with **no hooks at all** — verify no errors, no empty state noise.

**Step 4: Fix any issues found**

If type errors or rendering issues found, fix and commit.

**Step 5: Final commit (only if fixes were needed)**

```bash
git add -A
git commit -m "fix: wire-up fixes for hook events in conversation history"
```
