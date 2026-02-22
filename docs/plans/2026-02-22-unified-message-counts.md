# Unified Message Counts Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make terminal tab and log tab show identical badge counts, and make live/history views produce the same canonical `RichMessage[]` array.

**Architecture:** Extract `computeCategoryCounts()` as a shared utility. Parent components (`SessionDetailPanel` for live, `ConversationView` for history) compute counts once from the canonical `RichMessage[]` and pass them down to both `RichPane` and `ActionLogTab`. Expand `useActionItems` to handle system/progress/summary message types so the log tab shows everything except chat text.

**Tech Stack:** React, TypeScript, Vitest

---

### Task 1: Add `computeCategoryCounts` shared utility

**Files:**
- Create: `src/lib/compute-category-counts.ts`
- Create: `src/lib/compute-category-counts.test.ts`

**Step 1: Write the failing test**

Create `src/lib/compute-category-counts.test.ts`:

```ts
import { describe, it, expect } from 'vitest'
import { computeCategoryCounts } from './compute-category-counts'
import type { RichMessage } from '../components/live/RichPane'

describe('computeCategoryCounts', () => {
  it('returns zero counts for empty array', () => {
    const counts = computeCategoryCounts([])
    expect(counts.builtin).toBe(0)
    expect(counts.hook).toBe(0)
    expect(counts.hook_progress).toBe(0)
    expect(counts.system).toBe(0)
  })

  it('counts each category from RichMessage array', () => {
    const messages: RichMessage[] = [
      { type: 'user', content: 'hi' },
      { type: 'tool_use', content: '', category: 'builtin' },
      { type: 'tool_use', content: '', category: 'builtin' },
      { type: 'tool_use', content: '', category: 'skill' },
      { type: 'tool_result', content: 'ok', category: 'builtin' },
      { type: 'system', content: '', category: 'system' },
      { type: 'progress', content: '', category: 'hook_progress' },
      { type: 'error', content: 'fail', category: 'error' },
    ]
    const counts = computeCategoryCounts(messages)
    expect(counts.builtin).toBe(3) // 2 tool_use + 1 tool_result
    expect(counts.skill).toBe(1)
    expect(counts.system).toBe(1)
    expect(counts.hook_progress).toBe(1)
    expect(counts.error).toBe(1)
  })

  it('ignores messages without a category', () => {
    const messages: RichMessage[] = [
      { type: 'user', content: 'hi' },
      { type: 'assistant', content: 'hello' },
      { type: 'thinking', content: 'hmm' },
    ]
    const counts = computeCategoryCounts(messages)
    const total = Object.values(counts).reduce((a, b) => a + b, 0)
    expect(total).toBe(0)
  })
})
```

**Step 2: Run test to verify it fails**

Run: `bunx vitest run src/lib/compute-category-counts.test.ts`
Expected: FAIL — module not found

**Step 3: Write minimal implementation**

Create `src/lib/compute-category-counts.ts`:

```ts
import type { RichMessage } from '../components/live/RichPane'
import type { ActionCategory } from '../components/live/action-log/types'

export type CategoryCounts = Record<ActionCategory, number>

const EMPTY: CategoryCounts = { skill: 0, mcp: 0, builtin: 0, agent: 0, hook: 0, hook_progress: 0, error: 0, system: 0, snapshot: 0, queue: 0 }

export function computeCategoryCounts(messages: RichMessage[]): CategoryCounts {
  const counts = { ...EMPTY }
  for (const m of messages) {
    if (m.category) {
      counts[m.category] = (counts[m.category] || 0) + 1
    }
  }
  return counts
}
```

**Step 4: Run test to verify it passes**

Run: `bunx vitest run src/lib/compute-category-counts.test.ts`
Expected: PASS

**Step 5: Commit**

```bash
git add src/lib/compute-category-counts.ts src/lib/compute-category-counts.test.ts
git commit -m "feat: add computeCategoryCounts shared utility"
```

---

### Task 2: Expand `useActionItems` to handle system, progress, and summary types

**Files:**
- Modify: `src/components/live/action-log/use-action-items.ts`

Currently `useActionItems` only handles: user/assistant (turn separators), tool_use, tool_result, hook_progress progress, and error. It silently drops system, non-hook progress, summary, and thinking. We need to add handling for system, progress (all subtypes), and summary.

**Step 1: Write failing tests**

Create `src/components/live/action-log/use-action-items.test.ts`:

```ts
import { describe, it, expect } from 'vitest'
import type { RichMessage } from '../RichPane'
import type { ActionItem, TurnSeparator } from './types'

// We need to test the pure logic, not the hook. Extract or call directly.
// useActionItems uses useMemo internally — for unit tests, we test the
// transform logic by calling a pure version. We'll refactor useActionItems
// to call a pure function internally.

import { buildActionItems } from './use-action-items'

describe('buildActionItems', () => {
  it('creates TurnSeparators for user and assistant', () => {
    const msgs: RichMessage[] = [
      { type: 'user', content: 'hello' },
      { type: 'assistant', content: 'hi there' },
    ]
    const items = buildActionItems(msgs)
    expect(items).toHaveLength(2)
    expect(items[0]).toMatchObject({ type: 'turn', role: 'user' })
    expect(items[1]).toMatchObject({ type: 'turn', role: 'assistant' })
  })

  it('drops thinking messages', () => {
    const msgs: RichMessage[] = [
      { type: 'thinking', content: 'pondering...' },
    ]
    const items = buildActionItems(msgs)
    expect(items).toHaveLength(0)
  })

  it('creates ActionItem for system messages', () => {
    const msgs: RichMessage[] = [
      { type: 'system', content: 'turn ended', category: 'system', metadata: { type: 'turn_duration', durationMs: 1500 } },
    ]
    const items = buildActionItems(msgs)
    expect(items).toHaveLength(1)
    const item = items[0] as ActionItem
    expect(item.category).toBe('system')
    expect(item.toolName).toBe('turn_duration')
    expect(item.status).toBe('success')
  })

  it('creates ActionItem for system snapshot messages', () => {
    const msgs: RichMessage[] = [
      { type: 'system', content: '', category: 'snapshot', metadata: { type: 'file-history-snapshot' } },
    ]
    const items = buildActionItems(msgs)
    expect(items).toHaveLength(1)
    const item = items[0] as ActionItem
    expect(item.category).toBe('snapshot')
  })

  it('creates ActionItem for system queue messages', () => {
    const msgs: RichMessage[] = [
      { type: 'system', content: '', category: 'queue', metadata: { type: 'queue-operation', operation: 'enqueue' } },
    ]
    const items = buildActionItems(msgs)
    expect(items).toHaveLength(1)
    const item = items[0] as ActionItem
    expect(item.category).toBe('queue')
  })

  it('creates ActionItem for non-hook progress (agent_progress)', () => {
    const msgs: RichMessage[] = [
      { type: 'progress', content: '', metadata: { type: 'agent_progress', agentId: 'a1', prompt: 'do stuff' } },
    ]
    const items = buildActionItems(msgs)
    expect(items).toHaveLength(1)
    const item = items[0] as ActionItem
    expect(item.category).toBe('agent')
    expect(item.toolName).toBe('agent_progress')
  })

  it('creates ActionItem for bash_progress', () => {
    const msgs: RichMessage[] = [
      { type: 'progress', content: '', metadata: { type: 'bash_progress', command: 'ls -la' } },
    ]
    const items = buildActionItems(msgs)
    expect(items).toHaveLength(1)
    const item = items[0] as ActionItem
    expect(item.category).toBe('builtin')
    expect(item.label).toContain('ls -la')
  })

  it('creates ActionItem for mcp_progress', () => {
    const msgs: RichMessage[] = [
      { type: 'progress', content: '', metadata: { type: 'mcp_progress', server: 'my-server', method: 'query' } },
    ]
    const items = buildActionItems(msgs)
    expect(items).toHaveLength(1)
    const item = items[0] as ActionItem
    expect(item.category).toBe('mcp')
  })

  it('creates ActionItem for summary messages', () => {
    const msgs: RichMessage[] = [
      { type: 'summary', content: 'Session summary text', metadata: { summary: 'Session summary text', leafUuid: 'abc' } },
    ]
    const items = buildActionItems(msgs)
    expect(items).toHaveLength(1)
    const item = items[0] as ActionItem
    expect(item.toolName).toBe('summary')
    expect(item.label).toContain('Session summary')
  })

  it('still pairs tool_use + tool_result correctly', () => {
    const msgs: RichMessage[] = [
      { type: 'tool_use', content: '', name: 'Read', input: '{"file_path":"/foo"}', category: 'builtin' },
      { type: 'tool_result', content: 'file contents' },
    ]
    const items = buildActionItems(msgs)
    expect(items).toHaveLength(1)
    const item = items[0] as ActionItem
    expect(item.toolName).toBe('Read')
    expect(item.output).toBe('file contents')
    expect(item.status).toBe('success')
  })

  it('still handles hook_progress from JSONL', () => {
    const msgs: RichMessage[] = [
      { type: 'progress', content: '', category: 'hook_progress', metadata: { type: 'hook_progress', hookEvent: 'PreToolUse', command: 'lint' } },
    ]
    const items = buildActionItems(msgs)
    expect(items).toHaveLength(1)
    const item = items[0] as ActionItem
    expect(item.category).toBe('hook_progress')
  })

  it('merges hookEvents into timeline sorted by timestamp', () => {
    const msgs: RichMessage[] = [
      { type: 'tool_use', content: '', name: 'Read', category: 'builtin', ts: 100 },
    ]
    const hookEvents = [
      { id: 'h1', type: 'hook_event' as const, timestamp: 50, eventName: 'PreToolUse', label: 'lint', group: 'autonomous' as const },
    ]
    const items = buildActionItems(msgs, hookEvents)
    // Hook event at ts=50 should come before tool_use at ts=100
    expect(items[0]).toMatchObject({ type: 'hook_event' })
  })
})
```

**Step 2: Run test to verify it fails**

Run: `bunx vitest run src/components/live/action-log/use-action-items.test.ts`
Expected: FAIL — `buildActionItems` is not exported

**Step 3: Refactor useActionItems to expose pure function + add new handlers**

Modify `src/components/live/action-log/use-action-items.ts`. The key changes:
1. Extract the inner logic into an exported `buildActionItems()` pure function
2. Add handlers for `system`, non-hook `progress`, and `summary` types
3. `useActionItems` becomes a thin wrapper calling `buildActionItems` inside `useMemo`

```ts
import { useMemo } from 'react'
import type { RichMessage } from '../RichPane'
import type { ActionItem, TurnSeparator, TimelineItem, ActionCategory, HookEventItem } from './types'

function makeLabel(toolName: string, input?: string): string {
  // ... existing makeLabel unchanged ...
}

/** Map progress subtypes to their logical category */
function progressCategory(subtype: string | undefined): ActionCategory {
  switch (subtype) {
    case 'agent_progress': return 'agent'
    case 'bash_progress': return 'builtin'
    case 'mcp_progress': return 'mcp'
    case 'hook_progress': return 'hook_progress'
    case 'waiting_for_task': return 'queue'
    default: return 'system'
  }
}

/** Build a label for a progress ActionItem */
function progressLabel(m: Record<string, any>): string {
  const subtype = m.type ?? 'progress'
  switch (subtype) {
    case 'agent_progress':
      return m.prompt ? `Agent: ${(m.prompt as string).slice(0, 50)}` : 'Agent progress'
    case 'bash_progress':
      return m.command ? `$ ${(m.command as string).split('\n')[0].slice(0, 50)}` : 'Bash progress'
    case 'mcp_progress':
      return m.server ? `${m.server}:${m.method ?? ''}` : 'MCP progress'
    case 'hook_progress':
      return m.command
        ? `${m.hookEvent || m.hookName || 'hook'} → ${m.command}`
        : (m.hookEvent || m.hookName || 'hook progress')
    case 'waiting_for_task':
      return `Waiting (pos ${m.position ?? '?'})`
    default:
      return subtype
  }
}

/**
 * Pure function: convert RichMessage[] + optional HookEventItem[] into TimelineItem[].
 * Exported for unit testing.
 */
export function buildActionItems(messages: RichMessage[], hookEvents?: HookEventItem[]): TimelineItem[] {
  const items: TimelineItem[] = []
  let actionIndex = 0
  const pendingToolUses: ActionItem[] = []

  for (const msg of messages) {
    // Turn separators for user/assistant messages
    if (msg.type === 'user' || msg.type === 'assistant') {
      const text = msg.content.trim()
      if (text) {
        items.push({
          id: `turn-${items.length}`,
          type: 'turn',
          role: msg.type,
          content: text.length > 100 ? text.slice(0, 97) + '...' : text,
          timestamp: msg.ts,
        } satisfies TurnSeparator)
      }
      continue
    }

    // Thinking → dropped (trimmed with chat text)
    if (msg.type === 'thinking') continue

    // Tool use → create pending action
    if (msg.type === 'tool_use' && msg.name) {
      const action: ActionItem = {
        id: `action-${actionIndex++}`,
        timestamp: msg.ts,
        category: (msg.category as ActionCategory) ?? 'builtin',
        toolName: msg.name,
        label: makeLabel(msg.name, msg.input),
        status: 'pending',
        input: msg.input,
      }
      items.push(action)
      pendingToolUses.push(action)
      continue
    }

    // Tool result → pair with most recent pending tool_use
    if (msg.type === 'tool_result') {
      const pending = pendingToolUses.pop()
      if (pending) {
        pending.output = msg.content
        if (pending.timestamp && msg.ts) {
          pending.duration = Math.round((msg.ts - pending.timestamp) * 1000)
        }
        const isError = msg.content.startsWith('Error:') ||
                        msg.content.startsWith('FAILED') ||
                        msg.content.includes('exit code') ||
                        msg.content.includes('Command failed')
        pending.status = isError ? 'error' : 'success'
      }
      continue
    }

    // Progress events (all subtypes)
    if (msg.type === 'progress') {
      const m = msg.metadata ?? {}
      const subtype = m.type as string | undefined
      items.push({
        id: `action-${actionIndex++}`,
        timestamp: msg.ts,
        category: msg.category ?? progressCategory(subtype),
        toolName: subtype ?? 'progress',
        label: progressLabel(m),
        status: 'success',
        output: m.output,
      } satisfies ActionItem)
      continue
    }

    // System events
    if (msg.type === 'system') {
      const m = msg.metadata ?? {}
      const subtype = (m.type ?? m.subtype) as string | undefined
      items.push({
        id: `action-${actionIndex++}`,
        timestamp: msg.ts,
        category: msg.category ?? 'system',
        toolName: subtype ?? 'system',
        label: subtype ?? 'system event',
        status: 'success',
        output: msg.content || undefined,
      } satisfies ActionItem)
      continue
    }

    // Summary events
    if (msg.type === 'summary') {
      const summary = msg.metadata?.summary || msg.content || ''
      items.push({
        id: `action-${actionIndex++}`,
        timestamp: msg.ts,
        category: 'system', // summaries are system-level events
        toolName: 'summary',
        label: summary.length > 60 ? `Session summary (${summary.split(/\s+/).length}w)` : `Summary: ${summary}`,
        status: 'success',
        output: summary,
      } satisfies ActionItem)
      continue
    }

    // Errors
    if (msg.type === 'error') {
      items.push({
        id: `action-${actionIndex++}`,
        timestamp: msg.ts,
        category: 'error',
        toolName: 'Error',
        label: msg.content.length > 60 ? msg.content.slice(0, 57) + '...' : msg.content,
        status: 'error',
        output: msg.content,
      } satisfies ActionItem)
    }
  }

  // Merge hook events into timeline
  if (hookEvents && hookEvents.length > 0) {
    for (const event of hookEvents) {
      items.push(event)
    }
    items.sort((a, b) => {
      const tsA = 'timestamp' in a ? (a.timestamp ?? 0) : 0
      const tsB = 'timestamp' in b ? (b.timestamp ?? 0) : 0
      return tsA - tsB
    })
  }

  return items
}

export function useActionItems(messages: RichMessage[], hookEvents?: HookEventItem[]): TimelineItem[] {
  const hookEventsLength = hookEvents?.length ?? 0
  return useMemo(() => buildActionItems(messages, hookEvents), [messages, hookEventsLength])
}
```

**Step 4: Run tests to verify they pass**

Run: `bunx vitest run src/components/live/action-log/use-action-items.test.ts`
Expected: ALL PASS

**Step 5: Commit**

```bash
git add src/components/live/action-log/use-action-items.ts src/components/live/action-log/use-action-items.test.ts
git commit -m "feat: expand useActionItems to handle system, progress, summary types"
```

---

### Task 3: Make `RichPane` accept `categoryCounts` as a prop

**Files:**
- Modify: `src/components/live/RichPane.tsx:62-70` (RichPaneProps)
- Modify: `src/components/live/RichPane.tsx:675-689` (RichPane function — remove internal computation)

**Step 1: Update RichPaneProps interface**

At `src/components/live/RichPane.tsx:62-70`, change `RichPaneProps`:

```ts
export interface RichPaneProps {
  messages: RichMessage[]
  isVisible: boolean
  verboseMode?: boolean
  bufferDone?: boolean
  /** Pre-computed category counts from canonical message array. When provided,
   *  used directly for filter chips instead of computing internally. */
  categoryCounts?: Record<ActionCategory, number>
}
```

**Step 2: Update RichPane to use prop or fallback to internal computation**

At `src/components/live/RichPane.tsx:675`, update the function signature and categoryCounts logic:

```ts
export function RichPane({ messages, isVisible, verboseMode = false, bufferDone = false, categoryCounts: countsProp }: RichPaneProps) {
  const verboseFilter = useMonitorStore((s) => s.verboseFilter)
  const setVerboseFilter = useMonitorStore((s) => s.setVerboseFilter)

  // Use prop if provided, otherwise compute internally (backward compat)
  const categoryCounts = useMemo(() => {
    if (countsProp) return countsProp
    const counts: Record<ActionCategory, number> = { skill: 0, mcp: 0, builtin: 0, agent: 0, hook: 0, hook_progress: 0, error: 0, system: 0, snapshot: 0, queue: 0 }
    if (!verboseMode) return counts
    for (const m of messages) {
      if (m.category) {
        counts[m.category] = (counts[m.category] || 0) + 1
      }
    }
    return counts
  }, [countsProp, messages, verboseMode])
```

**Step 3: Run existing tests**

Run: `bunx vitest run src/components/live/ --reporter=verbose`
Expected: PASS (no behavior change when countsProp is undefined)

**Step 4: Commit**

```bash
git add src/components/live/RichPane.tsx
git commit -m "feat: RichPane accepts optional categoryCounts prop"
```

---

### Task 4: Make `ActionLogTab` accept `categoryCounts` as a prop

**Files:**
- Modify: `src/components/live/action-log/ActionLogTab.tsx:13-17` (props interface)
- Modify: `src/components/live/action-log/ActionLogTab.tsx:28-38` (remove internal counting)

**Step 1: Update ActionLogTabProps and use the prop**

At `src/components/live/action-log/ActionLogTab.tsx`, update the interface and component:

```ts
interface ActionLogTabProps {
  messages: RichMessage[]
  bufferDone: boolean
  hookEvents?: HookEventItem[]
  /** Pre-computed category counts from canonical message array. */
  categoryCounts?: Record<ActionCategory, number>
}

export function ActionLogTab({ messages, bufferDone, hookEvents, categoryCounts: countsProp }: ActionLogTabProps) {
  const allItems = useActionItems(messages, hookEvents)
  const [activeFilter, setActiveFilter] = useState<ActionCategory | 'all'>('all')
  // ...

  // Use prop if provided, otherwise compute from allItems (backward compat)
  const counts = useMemo(() => {
    if (countsProp) return countsProp
    const c: Record<ActionCategory, number> = { skill: 0, mcp: 0, builtin: 0, agent: 0, error: 0, hook: 0, hook_progress: 0, system: 0, snapshot: 0, queue: 0 }
    for (const item of allItems) {
      if (isHookEvent(item)) {
        c.hook++
      } else if (!isTurnSeparator(item)) {
        c[item.category]++
      }
    }
    return c
  }, [countsProp, allItems])
```

**Step 2: Run existing tests**

Run: `bunx vitest run src/components/live/action-log/ --reporter=verbose`
Expected: PASS

**Step 3: Commit**

```bash
git add src/components/live/action-log/ActionLogTab.tsx
git commit -m "feat: ActionLogTab accepts optional categoryCounts prop"
```

---

### Task 5: Wire shared `categoryCounts` in `SessionDetailPanel` (live sessions)

**Files:**
- Modify: `src/components/live/SessionDetailPanel.tsx:103-120` (add categoryCounts computation)
- Modify: `src/components/live/SessionDetailPanel.tsx:494-511` (pass prop to both tabs)

**Step 1: Import and compute shared counts**

At `src/components/live/SessionDetailPanel.tsx`, add import and computation:

```ts
// Add import at top
import { computeCategoryCounts } from '../../lib/compute-category-counts'

// After line 119 (const bufferDone = ...), add:
const categoryCounts = useMemo(
  () => computeCategoryCounts(richMessages),
  [richMessages]
)
```

**Step 2: Pass categoryCounts to both terminal and log tabs**

At `src/components/live/SessionDetailPanel.tsx:494-511`:

```ts
        {/* ---- Terminal tab ---- */}
        {activeTab === 'terminal' && (
          <RichPane
            messages={richMessages}
            isVisible={true}
            verboseMode={verboseMode}
            bufferDone={bufferDone}
            categoryCounts={categoryCounts}
          />
        )}

        {/* ---- Log tab ---- */}
        {activeTab === 'log' && (
          <ActionLogTab
            messages={richMessages}
            bufferDone={bufferDone}
            hookEvents={isLive ? liveHookEvents : historicalHookEvents}
            categoryCounts={categoryCounts}
          />
        )}
```

**Step 3: Verify TypeScript compiles**

Run: `bunx tsc --noEmit`
Expected: No errors

**Step 4: Commit**

```bash
git add src/components/live/SessionDetailPanel.tsx
git commit -m "feat: wire shared categoryCounts in SessionDetailPanel"
```

---

### Task 6: Wire shared `categoryCounts` in `ConversationView` (history)

**Files:**
- Modify: `src/components/ConversationView.tsx:19` (add import)
- Modify: `src/components/ConversationView.tsx:39-50` (update HistoryRichPane)
- Modify: `src/components/ConversationView.tsx:276-296` (add categoryCounts computation)
- Modify: `src/components/ConversationView.tsx:647` (pass prop to HistoryRichPane)
- Modify: `src/components/ConversationView.tsx:652-658` (pass prop to SessionDetailPanel)

**Step 1: Import computeCategoryCounts**

At `src/components/ConversationView.tsx:19` area, add:

```ts
import { computeCategoryCounts, type CategoryCounts } from '../lib/compute-category-counts'
```

**Step 2: Update HistoryRichPane to accept categoryCounts**

```ts
function HistoryRichPane({ messages, categoryCounts }: { messages: import('./live/RichPane').RichMessage[]; categoryCounts?: import('../lib/compute-category-counts').CategoryCounts }) {
  const verboseMode = useMonitorStore((s) => s.verboseMode)
  return (
    <RichPane
      messages={messages}
      isVisible={true}
      verboseMode={verboseMode}
      bufferDone={true}
      categoryCounts={categoryCounts}
    />
  )
}
```

**Step 3: Compute categoryCounts from canonical richMessagesWithHookEvents**

After `richMessagesWithHookEvents` (line ~290), add:

```ts
const categoryCounts = useMemo(
  () => computeCategoryCounts(richMessagesWithHookEvents),
  [richMessagesWithHookEvents]
)
```

**Step 4: Pass categoryCounts to HistoryRichPane**

At line ~647:

```ts
<HistoryRichPane messages={richMessagesWithHookEvents} categoryCounts={categoryCounts} />
```

**Step 5: Pass categoryCounts through panelData to SessionDetailPanel**

This requires updating the `SessionPanelData` type to carry `categoryCounts`. Alternatively, since `ConversationView` already passes `richMessagesWithHookEvents` via `panelData.terminalMessages`, and `SessionDetailPanel` already computes richMessages from that... we can just let `SessionDetailPanel` compute its own counts from `data.terminalMessages` using the same `computeCategoryCounts` function (already wired in Task 5).

Actually — the simplest approach: `SessionDetailPanel` in Task 5 already computes `categoryCounts` from `richMessages` (which in history mode is `data.terminalMessages`). So no additional prop threading is needed here. The history panel will compute counts from the same canonical array.

**Step 6: Verify**

Run: `bunx tsc --noEmit`
Expected: No errors

**Step 7: Commit**

```bash
git add src/components/ConversationView.tsx
git commit -m "feat: wire shared categoryCounts in ConversationView"
```

---

### Task 7: Delete dead code

**Files:**
- Modify: `src/lib/hook-events-to-messages.ts:100-115` (delete suppressHookProgress, suppressRichHookProgress)
- Modify: `src/lib/hook-events-to-messages.test.ts:142-181` (delete corresponding tests)

**Step 1: Remove the functions**

Delete `suppressHookProgress` and `suppressRichHookProgress` from `src/lib/hook-events-to-messages.ts` (lines 100-115).

**Step 2: Remove the tests**

Delete the `describe('suppressHookProgress', ...)` and `describe('suppressRichHookProgress', ...)` blocks from `src/lib/hook-events-to-messages.test.ts` (lines 142-181).

**Step 3: Remove the imports**

In `src/lib/hook-events-to-messages.test.ts`, remove `suppressHookProgress` and `suppressRichHookProgress` from the import.

**Step 4: Verify no other files import these functions**

Run: `grep -r 'suppressHookProgress\|suppressRichHookProgress' src/`
Expected: No matches (they were never called from production code)

**Step 5: Run tests**

Run: `bunx vitest run src/lib/hook-events-to-messages.test.ts`
Expected: PASS

**Step 6: Commit**

```bash
git add src/lib/hook-events-to-messages.ts src/lib/hook-events-to-messages.test.ts
git commit -m "chore: remove dead suppressHookProgress functions"
```

---

### Task 8: ActionRow badge labels for new categories

**Files:**
- Modify: `src/components/live/action-log/ActionRow.tsx:6-12` (extend CATEGORY_BADGE)
- Modify: `src/components/live/action-log/ActionRow.tsx:67-75` (extend badgeLabel logic)

The `ActionRow` currently only has badge styles for `skill`, `mcp`, `builtin`, `agent`, `error`. It needs to handle the new categories: `hook_progress`, `system`, `snapshot`, `queue`, `hook`.

**Step 1: Extend CATEGORY_BADGE**

```ts
const CATEGORY_BADGE: Record<string, string> = {
  skill: 'bg-purple-500/10 text-purple-400',
  mcp: 'bg-blue-500/10 text-blue-400',
  builtin: 'bg-gray-500/10 text-gray-400',
  agent: 'bg-indigo-500/10 text-indigo-400',
  error: 'bg-red-500/10 text-red-400',
  hook: 'bg-amber-500/10 text-amber-400',
  hook_progress: 'bg-yellow-500/10 text-yellow-400',
  system: 'bg-cyan-500/10 text-cyan-400',
  snapshot: 'bg-teal-500/10 text-teal-400',
  queue: 'bg-orange-500/10 text-orange-400',
}
```

**Step 2: Extend badgeLabel**

Replace the nested ternary with a map:

```ts
const BADGE_LABELS: Record<string, string | null> = {
  builtin: null, // uses toolName
  mcp: 'MCP',
  skill: 'Skill',
  agent: 'Agent',
  error: 'Error',
  hook: 'Hook',
  hook_progress: 'Hook',
  system: 'System',
  snapshot: 'Snapshot',
  queue: 'Queue',
}

// Inside ActionRow:
const badgeLabel = BADGE_LABELS[action.category] ?? action.toolName
```

Wait — for `builtin`, the existing code shows `action.toolName` (the actual tool name like "Read", "Bash"). Let's preserve that:

```ts
const badgeLabel = action.category === 'builtin'
  ? action.toolName
  : (BADGE_LABELS[action.category] ?? action.category)
```

**Step 3: Run TypeScript check**

Run: `bunx tsc --noEmit`
Expected: No errors

**Step 4: Commit**

```bash
git add src/components/live/action-log/ActionRow.tsx
git commit -m "feat: ActionRow badges for system, hook, snapshot, queue categories"
```

---

### Task 9: Full integration verification

**Step 1: Run all frontend tests**

Run: `bunx vitest run --reporter=verbose`
Expected: ALL PASS

**Step 2: TypeScript check**

Run: `bunx tsc --noEmit`
Expected: No errors

**Step 3: Build check**

Run: `bun run build`
Expected: Success

**Step 4: Manual verification (browser)**

Open the app. For a live session with hooks:
1. Check terminal tab badge counts
2. Check log tab badge counts
3. Verify they match

For a historical session:
1. Open conversation view
2. Check verbose mode terminal badge counts
3. Open side panel, check terminal tab and log tab badge counts
4. Verify all three match

**Step 5: Final commit (if any fixups needed)**

---

## Summary of Changes

| File | Change |
|------|--------|
| `src/lib/compute-category-counts.ts` | NEW — shared `computeCategoryCounts()` utility |
| `src/lib/compute-category-counts.test.ts` | NEW — unit tests |
| `src/components/live/action-log/use-action-items.ts` | Extract `buildActionItems()` pure fn, add system/progress/summary handlers |
| `src/components/live/action-log/use-action-items.test.ts` | NEW — unit tests for pure function |
| `src/components/live/RichPane.tsx` | Accept optional `categoryCounts` prop |
| `src/components/live/action-log/ActionLogTab.tsx` | Accept optional `categoryCounts` prop |
| `src/components/live/SessionDetailPanel.tsx` | Compute + pass shared `categoryCounts` |
| `src/components/ConversationView.tsx` | Compute + pass shared `categoryCounts` |
| `src/lib/hook-events-to-messages.ts` | Delete `suppressHookProgress`, `suppressRichHookProgress` |
| `src/lib/hook-events-to-messages.test.ts` | Delete corresponding tests |
| `src/components/live/action-log/ActionRow.tsx` | Badge colors/labels for all categories |
