# Unified Message Counts Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make terminal tab and log tab show identical badge counts, make live/history views produce structurally equivalent `RichMessage[]` arrays, and ensure NO message type is dropped in verbose mode or the log tab.

**Architecture:** First normalize the live and history pipelines so they produce identical `RichMessage` shapes (same types, categories, metadata). Then extract `computeCategoryCounts()` as a shared utility. Parent components compute counts once from the canonical `RichMessage[]` and pass them down. Expand `useActionItems` to handle ALL message types including thinking.

**Tech Stack:** React, TypeScript, Vitest

---

### Task 0: Normalize live/history pipeline asymmetries

**Problem:** Three asymmetries cause live and history views to produce different `RichMessage[]` for the same session:

| Asymmetry | Live (`parseRichMessage` / `useLiveSessionMessages`) | History (`messagesToRichMessages` / `hookEventsToRichMessages`) |
|---|---|---|
| Hook event type | `type: 'hook'` (no metadata) | `type: 'progress'` with `metadata.type: 'hook_event'` |
| System category default | `category ?? 'system'` | `category ?? undefined` |
| Summary category | not set | not set |

**Files:**
- Modify: `src/hooks/use-live-session-messages.ts` (hook event shape)
- Modify: `src/lib/message-to-rich.ts` (system + summary category defaults)
- Modify: `src/components/live/RichPane.tsx` (summary category in parseRichMessage)

**Step 1: Write failing tests**

Create `src/lib/message-to-rich.test.ts` (add to existing if present, or create):

```ts
import { describe, it, expect } from 'vitest'
import { messagesToRichMessages } from './message-to-rich'
import type { Message } from '../types/generated'

describe('messagesToRichMessages normalization', () => {
  it('defaults system category to "system" when not set', () => {
    const msgs: Message[] = [{
      uuid: '1', role: 'system', content: 'turn ended', timestamp: null,
      thinking: null, tool_calls: null,
      metadata: { type: 'turn_duration', durationMs: 1500 },
    }]
    const rich = messagesToRichMessages(msgs)
    expect(rich).toHaveLength(1)
    expect(rich[0].category).toBe('system')
  })

  it('sets summary category to "system"', () => {
    const msgs: Message[] = [{
      uuid: '1', role: 'summary', content: 'Session summary', timestamp: null,
      thinking: null, tool_calls: null,
      metadata: { summary: 'Session summary', leafUuid: 'abc' },
    }]
    const rich = messagesToRichMessages(msgs)
    expect(rich).toHaveLength(1)
    expect(rich[0].category).toBe('system')
  })
})
```

**Step 2: Run test to verify it fails**

Run: `bunx vitest run src/lib/message-to-rich.test.ts`
Expected: FAIL — system category is `undefined`, summary category is `undefined`

**Step 3: Fix `messagesToRichMessages` — system default + summary category**

In `src/lib/message-to-rich.ts`, change the `system` case (line ~130):

```ts
case 'system': {
  const content = stripCommandTags(msg.content)
  result.push({
    type: 'system',
    content: content || '',
    ts,
    category: (msg.category as ActionCategory) ?? 'system',  // was: ?? undefined
    metadata: msg.metadata ?? undefined,
  })
  break
}
```

And the `summary` case (line ~153):

```ts
case 'summary': {
  result.push({
    type: 'summary',
    content: msg.content || '',
    ts,
    category: 'system' as ActionCategory,  // NEW: summaries always counted as system
    metadata: msg.metadata ?? undefined,
  })
  break
}
```

**Step 4: Fix `parseRichMessage` — summary category**

In `src/components/live/RichPane.tsx`, change the summary handler (line ~179):

```ts
if (msg.type === 'summary') {
  return {
    type: 'summary',
    content: typeof msg.content === 'string' ? msg.content : '',
    ts: parseTimestamp(msg.ts),
    category: 'system' as ActionCategory,  // NEW: summaries always counted as system
    metadata: msg.metadata,
  }
}
```

Note: Need to add `import type { ActionCategory } from './action-log/types'` if not already imported. Check existing imports first — it's already imported at line 20.

**Step 5: Fix `useLiveSessionMessages` — normalize hook event shape**

In `src/hooks/use-live-session-messages.ts`, change the hook_event handling (lines 43-51):

FROM:
```ts
setMessages((prev) => [...prev, {
  type: 'hook' as const,
  content: json.label,
  name: json.eventName,
  input: json.context,
  ts: json.timestamp,
  category: 'hook' as const,
}])
```

TO:
```ts
// Push normalized shape matching history pipeline (hookEventsToRichMessages)
setMessages((prev) => [...prev, {
  type: 'progress' as const,
  content: `Hook: ${json.eventName} — ${json.label}`,
  ts: json.timestamp,
  category: 'hook' as ActionCategory,
  metadata: {
    type: 'hook_event',
    _hookEvent: {
      id: `hook-${prev.length}`,
      type: 'hook_event' as const,
      timestamp: json.timestamp,
      eventName: json.eventName,
      toolName: json.toolName,
      label: json.label,
      group: json.group,
      context: json.context,
    },
  },
}])
```

Note: Need to add `import type { ActionCategory } from '../components/live/action-log/types'` at the top of the file.

**Step 6: Run tests**

Run: `bunx vitest run src/lib/message-to-rich.test.ts`
Expected: PASS

Run: `bunx tsc --noEmit`
Expected: No errors

**Step 7: Commit**

```bash
git add src/hooks/use-live-session-messages.ts src/lib/message-to-rich.ts src/components/live/RichPane.tsx src/lib/message-to-rich.test.ts
git commit -m "fix: normalize live/history pipelines for identical RichMessage shape"
```

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
      { type: 'summary', content: 'summary', category: 'system' },
    ]
    const counts = computeCategoryCounts(messages)
    expect(counts.builtin).toBe(3) // 2 tool_use + 1 tool_result
    expect(counts.skill).toBe(1)
    expect(counts.system).toBe(2) // 1 system + 1 summary
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

### Task 2: Expand `useActionItems` to handle ALL message types (including thinking)

**Files:**
- Modify: `src/components/live/action-log/use-action-items.ts`
- Create: `src/components/live/action-log/use-action-items.test.ts`

Currently `useActionItems` only handles: user/assistant (turn separators), tool_use, tool_result, hook_progress progress, and error. It silently drops system, non-hook progress, summary, and thinking. We need to handle ALL of them. **No message type should be dropped.**

**hookEvents param removed.** After Task 0 normalization, hook events are already in `messages[]` as `type: 'progress'` with `metadata.type: 'hook_event'` and `category: 'hook'`. So `buildActionItems` processes them directly from messages via the progress handler — no separate `hookEvents` merge needed. This means:
- `hook_progress` (JSONL) → ActionItem with `category: 'hook_progress'`
- `hook_event` (SQLite) → ActionItem with `category: 'hook'`
- Both present. No dedup. No skipping.

**Step 1: Write failing tests**

Create `src/components/live/action-log/use-action-items.test.ts`:

```ts
import { describe, it, expect } from 'vitest'
import type { RichMessage } from '../RichPane'
import type { ActionItem, TurnSeparator } from './types'

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

  it('creates ActionItem for thinking messages (not dropped)', () => {
    const msgs: RichMessage[] = [
      { type: 'thinking', content: 'pondering the meaning of life...' },
    ]
    const items = buildActionItems(msgs)
    expect(items).toHaveLength(1)
    const item = items[0] as ActionItem
    expect(item.category).toBe('system')
    expect(item.toolName).toBe('thinking')
    expect(item.label).toContain('pondering')
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

  it('creates ActionItem for hook_event progress (from SQLite via normalization)', () => {
    const msgs: RichMessage[] = [
      { type: 'progress', content: 'Hook: PreToolUse — lint', category: 'hook', metadata: { type: 'hook_event', _hookEvent: {} } },
    ]
    const items = buildActionItems(msgs)
    expect(items).toHaveLength(1)
    const item = items[0] as ActionItem
    expect(item.category).toBe('hook')
    expect(item.toolName).toBe('hook_event')
  })

  it('creates ActionItem for summary messages', () => {
    const msgs: RichMessage[] = [
      { type: 'summary', content: 'Session summary text', category: 'system', metadata: { summary: 'Session summary text', leafUuid: 'abc' } },
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

  it('handles hook type messages (legacy live path, before normalization)', () => {
    const msgs: RichMessage[] = [
      { type: 'hook', content: 'lint check', name: 'PreToolUse', category: 'hook', ts: 100 },
    ]
    const items = buildActionItems(msgs)
    expect(items).toHaveLength(1)
    const item = items[0] as ActionItem
    expect(item.category).toBe('hook')
  })
})
```

**Step 2: Run test to verify it fails**

Run: `bunx vitest run src/components/live/action-log/use-action-items.test.ts`
Expected: FAIL — `buildActionItems` is not exported

**Step 3: Refactor useActionItems to expose pure function + add new handlers**

Modify `src/components/live/action-log/use-action-items.ts`. The key changes:
1. Extract the inner logic into an exported `buildActionItems()` pure function
2. Add handlers for `thinking`, `system`, ALL `progress` subtypes (including `hook_event`), `summary`, and `hook` (legacy)
3. Remove `hookEvents` parameter — after Task 0 normalization, hook events are in `messages[]`
4. `useActionItems` becomes a thin wrapper calling `buildActionItems` inside `useMemo`

```ts
import { useMemo } from 'react'
import type { RichMessage } from '../RichPane'
import type { ActionItem, TurnSeparator, TimelineItem, ActionCategory } from './types'

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
    case 'hook_event': return 'hook'
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
    case 'hook_event': {
      const he = m._hookEvent
      return he ? `${he.eventName} — ${he.label}` : 'Hook event'
    }
    case 'waiting_for_task':
      return `Waiting (pos ${m.position ?? '?'})`
    default:
      return subtype
  }
}

/**
 * Pure function: convert RichMessage[] into TimelineItem[].
 * Exported for unit testing.
 *
 * IMPORTANT: No message type is dropped. Every RichMessage produces a TimelineItem.
 * After Task 0 normalization, hook events are in messages[] as progress subtypes,
 * so no separate hookEvents parameter is needed.
 */
export function buildActionItems(messages: RichMessage[]): TimelineItem[] {
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

    // Thinking → ActionItem (NOT dropped — shown in log tab)
    if (msg.type === 'thinking') {
      const preview = msg.content.split('\n')[0] || 'thinking...'
      items.push({
        id: `action-${actionIndex++}`,
        timestamp: msg.ts,
        category: 'system',
        toolName: 'thinking',
        label: preview.length > 60 ? preview.slice(0, 57) + '...' : preview,
        status: 'success',
        output: msg.content,
      } satisfies ActionItem)
      continue
    }

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

    // Progress events (ALL subtypes — including hook_event and hook_progress)
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
        category: msg.category ?? 'system',
        toolName: 'summary',
        label: summary.length > 60 ? `Session summary (${summary.split(/\s+/).length}w)` : `Summary: ${summary}`,
        status: 'success',
        output: summary,
      } satisfies ActionItem)
      continue
    }

    // Hook messages (legacy live path — before Task 0 normalization)
    // After normalization these become type: 'progress' with metadata.type: 'hook_event',
    // but keep this handler for backwards compatibility.
    if (msg.type === 'hook') {
      items.push({
        id: `action-${actionIndex++}`,
        timestamp: msg.ts,
        category: 'hook',
        toolName: msg.name ?? 'hook',
        label: msg.content.length > 60 ? msg.content.slice(0, 57) + '...' : msg.content,
        status: 'success',
        output: msg.input,
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

  return items
}

export function useActionItems(messages: RichMessage[]): TimelineItem[] {
  return useMemo(() => buildActionItems(messages), [messages])
}
```

**Step 4: Run tests to verify they pass**

Run: `bunx vitest run src/components/live/action-log/use-action-items.test.ts`
Expected: ALL PASS

**Step 5: Commit**

```bash
git add src/components/live/action-log/use-action-items.ts src/components/live/action-log/use-action-items.test.ts
git commit -m "feat: expand useActionItems to handle ALL message types including thinking"
```

---

### Task 3: Make `RichPane` accept `categoryCounts` as a prop

**Files:**
- Modify: `src/components/live/RichPane.tsx:62-70` (RichPaneProps)
- Modify: `src/components/live/RichPane.tsx:637` (RichPane function — use prop or fallback)

**Step 1: Update RichPaneProps interface**

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
Expected: PASS

**Step 4: Commit**

```bash
git add src/components/live/RichPane.tsx
git commit -m "feat: RichPane accepts optional categoryCounts prop"
```

---

### Task 4: Make `ActionLogTab` accept `categoryCounts` as a prop

**Files:**
- Modify: `src/components/live/action-log/ActionLogTab.tsx`

**Step 1: Update ActionLogTabProps and use the prop**

```ts
interface ActionLogTabProps {
  messages: RichMessage[]
  bufferDone: boolean
  /** Pre-computed category counts from canonical message array. */
  categoryCounts?: Record<ActionCategory, number>
}

export function ActionLogTab({ messages, bufferDone, categoryCounts: countsProp }: ActionLogTabProps) {
  const allItems = useActionItems(messages)
  const [activeFilter, setActiveFilter] = useState<ActionCategory | 'all'>('all')
  // ...

  // Use prop if provided, otherwise compute from allItems (backward compat)
  const counts = useMemo(() => {
    if (countsProp) return countsProp
    const c: Record<ActionCategory, number> = { skill: 0, mcp: 0, builtin: 0, agent: 0, error: 0, hook: 0, hook_progress: 0, system: 0, snapshot: 0, queue: 0 }
    for (const item of allItems) {
      if (!isTurnSeparator(item)) {
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
- Modify: `src/components/live/SessionDetailPanel.tsx`

After Task 0 normalization, `richMessages` in live mode includes hook events (as `type: 'progress'` with `category: 'hook'`). So `computeCategoryCounts(richMessages)` correctly counts all categories including hooks.

**Step 1: Import and compute shared counts**

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
- Modify: `src/components/ConversationView.tsx`

**Step 1: Import computeCategoryCounts**

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

```ts
<HistoryRichPane messages={richMessagesWithHookEvents} categoryCounts={categoryCounts} />
```

**Step 5: SessionDetailPanel counts**

`SessionDetailPanel` in Task 5 already computes `categoryCounts` from `richMessages` (which in history mode is `data.terminalMessages` = `richMessagesWithHookEvents`). No additional prop threading needed.

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
- Modify: `src/lib/hook-events-to-messages.ts` (delete suppressHookProgress, suppressRichHookProgress)
- Modify: `src/lib/hook-events-to-messages.test.ts` (delete corresponding tests)

**Step 1: Remove the functions and their tests**

Delete `suppressHookProgress` and `suppressRichHookProgress` from `src/lib/hook-events-to-messages.ts`.
Delete the corresponding `describe(...)` blocks from `src/lib/hook-events-to-messages.test.ts`.
Remove them from the import statement in the test file.

**Step 2: Verify no other files import these functions**

Run: `grep -r 'suppressHookProgress\|suppressRichHookProgress' src/`
Expected: No matches

**Step 3: Run tests**

Run: `bunx vitest run src/lib/hook-events-to-messages.test.ts`
Expected: PASS

**Step 4: Commit**

```bash
git add src/lib/hook-events-to-messages.ts src/lib/hook-events-to-messages.test.ts
git commit -m "chore: remove dead suppressHookProgress functions"
```

---

### Task 8: ActionRow badge labels for new categories

**Files:**
- Modify: `src/components/live/action-log/ActionRow.tsx`

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

```ts
const badgeLabel = action.category === 'builtin'
  ? action.toolName
  : (BADGE_LABELS[action.category] ?? action.category)
```

Where `BADGE_LABELS` is:
```ts
const BADGE_LABELS: Record<string, string> = {
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

Specifically verify:
- System events appear in both terminal and log tabs
- Hook events appear in both terminal and log tabs
- Thinking blocks appear in the log tab
- Summary events appear in both terminal and log tabs
- Badge counts for "system" include summaries and thinking in the log tab count

**Step 5: Final commit (if any fixups needed)**

---

## Normalization Guarantees

After this plan, both pipelines produce structurally equivalent `RichMessage[]`:

| Field | Live (WebSocket) | History (REST+SQLite) | Match? |
|---|---|---|---|
| Hook event type | `'progress'` | `'progress'` | YES |
| Hook event metadata | `{ type: 'hook_event', _hookEvent: HookEventItem }` | `{ type: 'hook_event', _hookEvent: HookEventItem }` | YES |
| Hook event category | `'hook'` | `'hook'` | YES |
| System category default | `'system'` | `'system'` | YES |
| Summary category | `'system'` | `'system'` | YES |
| Thinking | Preserved (type: 'thinking') | Preserved (type: 'thinking') | YES |

## No-Drop Guarantee

| RichMessage type | Terminal tab (verbose) | Log tab | Counted in badges? |
|---|---|---|---|
| user | Shown (UserMessage) | TurnSeparator | No (chat text) |
| assistant | Shown (AssistantMessage) | TurnSeparator | No (chat text) |
| thinking | Shown (ThinkingMessage) | ActionItem (system) | Yes (system) |
| tool_use | Shown (PairedToolCard) | ActionItem (paired) | Yes (by category) |
| tool_result | Shown (PairedToolCard) | Paired with tool_use | Yes (by category) |
| progress (hook_event) | Shown (ProgressMessageCard) | ActionItem (hook) | Yes (hook) |
| progress (hook_progress) | Shown (ProgressMessageCard) | ActionItem | Yes (hook_progress) |
| progress (other) | Shown (ProgressMessageCard) | ActionItem | Yes (by subtype) |
| system | Shown (SystemMessageCard) | ActionItem | Yes (system/snapshot/queue) |
| summary | Shown (SummaryMessageCard) | ActionItem | Yes (system) |
| error | Shown (ErrorMessage) | ActionItem | Yes (error) |
| hook (legacy) | Shown (HookMessage) | ActionItem | Yes (hook) |

## Summary of Changes

| File | Change |
|------|--------|
| `src/hooks/use-live-session-messages.ts` | Normalize hook event shape to match history pipeline |
| `src/lib/message-to-rich.ts` | Default system category to `'system'`, set summary category to `'system'` |
| `src/lib/message-to-rich.test.ts` | NEW — normalization tests |
| `src/components/live/RichPane.tsx` | Set summary category in `parseRichMessage`, accept optional `categoryCounts` prop |
| `src/lib/compute-category-counts.ts` | NEW — shared `computeCategoryCounts()` utility |
| `src/lib/compute-category-counts.test.ts` | NEW — unit tests |
| `src/components/live/action-log/use-action-items.ts` | Extract `buildActionItems()` pure fn, handle ALL types including thinking, skip hook_event dedup |
| `src/components/live/action-log/use-action-items.test.ts` | NEW — unit tests for pure function |
| `src/components/live/action-log/ActionLogTab.tsx` | Accept optional `categoryCounts` prop |
| `src/components/live/SessionDetailPanel.tsx` | Compute + pass shared `categoryCounts` |
| `src/components/ConversationView.tsx` | Compute + pass shared `categoryCounts` |
| `src/lib/hook-events-to-messages.ts` | Delete `suppressHookProgress`, `suppressRichHookProgress` |
| `src/lib/hook-events-to-messages.test.ts` | Delete corresponding tests |
| `src/components/live/action-log/ActionRow.tsx` | Badge colors/labels for all categories |
