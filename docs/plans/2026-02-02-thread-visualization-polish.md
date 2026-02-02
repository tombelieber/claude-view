---
status: pending
date: 2026-02-02
---

# Thread Visualization & Dark Mode Polish

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Activate the dead threading code in ConversationView (indent/isChild are never computed), add hover-based thread highlighting, and add dark mode to MessageTyped.

**Architecture:** ConversationView passes `parentUuid` from `message.parent_uuid` but never computes `indent` or `isChildMessage` — those props default to `0` and `false`. We build a `buildThreadMap()` utility that walks parent_uuid chains to compute indent levels, wire it into ConversationView's Virtuoso render, add a lightweight context for hover-based thread highlighting, and apply `dark:` Tailwind variants to MessageTyped.

**Tech Stack:** React 19, Tailwind CSS 4, Vitest + RTL, TypeScript strict

**Scope:** 5 tasks, purely additive — no breaking changes to existing components.

**Key files (read before starting):**
- `src/components/ConversationView.tsx` — where Virtuoso renders MessageTyped
- `src/components/MessageTyped.tsx` — message card with indent/isChild/parentUuid props
- `src/contexts/ExpandContext.tsx` — existing context pattern to follow
- `src/types/generated/Message.ts` — `{ uuid?: string | null, parent_uuid?: string | null, ... }`
- `src/lib/utils.ts` — `cn()` for Tailwind class merging

---

### Task 1: buildThreadMap utility

**Files:**
- Create: `src/lib/thread-map.ts`
- Create: `src/lib/thread-map.test.ts`

**Context:** The generated `Message` type has `uuid?: string | null` and `parent_uuid?: string | null`. MessageTyped already accepts `indent` (capped at `MAX_INDENT_LEVEL = 5`) and `isChildMessage`, but ConversationView never computes them. This utility does the computation.

**Step 1: Write the failing test**

```typescript
// src/lib/thread-map.test.ts
import { describe, it, expect } from 'vitest'
import { buildThreadMap } from './thread-map'

interface MockMsg {
  uuid?: string | null
  parent_uuid?: string | null
}

describe('buildThreadMap', () => {
  it('returns empty map for empty array', () => {
    expect(buildThreadMap([])).toEqual(new Map())
  })

  it('assigns indent 0 and isChild false to root messages', () => {
    const msgs: MockMsg[] = [
      { uuid: 'a', parent_uuid: null },
      { uuid: 'b', parent_uuid: null },
    ]
    const map = buildThreadMap(msgs)
    expect(map.get('a')).toEqual({ indent: 0, isChild: false, parentUuid: undefined })
    expect(map.get('b')).toEqual({ indent: 0, isChild: false, parentUuid: undefined })
  })

  it('assigns indent 1 to direct children', () => {
    const msgs: MockMsg[] = [
      { uuid: 'a', parent_uuid: null },
      { uuid: 'b', parent_uuid: 'a' },
    ]
    const map = buildThreadMap(msgs)
    expect(map.get('b')).toEqual({ indent: 1, isChild: true, parentUuid: 'a' })
  })

  it('assigns incrementing indent to nested chains', () => {
    const msgs: MockMsg[] = [
      { uuid: 'a', parent_uuid: null },
      { uuid: 'b', parent_uuid: 'a' },
      { uuid: 'c', parent_uuid: 'b' },
      { uuid: 'd', parent_uuid: 'c' },
    ]
    const map = buildThreadMap(msgs)
    expect(map.get('a')!.indent).toBe(0)
    expect(map.get('b')!.indent).toBe(1)
    expect(map.get('c')!.indent).toBe(2)
    expect(map.get('d')!.indent).toBe(3)
  })

  it('caps indent at 5 (matching MAX_INDENT_LEVEL)', () => {
    const msgs: MockMsg[] = [
      { uuid: '0', parent_uuid: null },
      { uuid: '1', parent_uuid: '0' },
      { uuid: '2', parent_uuid: '1' },
      { uuid: '3', parent_uuid: '2' },
      { uuid: '4', parent_uuid: '3' },
      { uuid: '5', parent_uuid: '4' },
      { uuid: '6', parent_uuid: '5' },  // depth 6 → capped to 5
      { uuid: '7', parent_uuid: '6' },  // depth 7 → capped to 5
    ]
    const map = buildThreadMap(msgs)
    expect(map.get('5')!.indent).toBe(5)
    expect(map.get('6')!.indent).toBe(5)
    expect(map.get('7')!.indent).toBe(5)
  })

  it('skips messages with null/undefined uuid', () => {
    const msgs: MockMsg[] = [
      { uuid: null, parent_uuid: null },
      { uuid: undefined, parent_uuid: null },
      { uuid: 'b', parent_uuid: null },
    ]
    const map = buildThreadMap(msgs)
    expect(map.size).toBe(1)
    expect(map.has('b')).toBe(true)
  })

  it('treats orphaned children (parent_uuid not in list) as root', () => {
    const msgs: MockMsg[] = [
      { uuid: 'b', parent_uuid: 'nonexistent' },
    ]
    const map = buildThreadMap(msgs)
    expect(map.get('b')).toEqual({ indent: 0, isChild: false, parentUuid: undefined })
  })

  it('handles sibling branches correctly', () => {
    const msgs: MockMsg[] = [
      { uuid: 'root', parent_uuid: null },
      { uuid: 'child1', parent_uuid: 'root' },
      { uuid: 'child2', parent_uuid: 'root' },
      { uuid: 'grandchild1', parent_uuid: 'child1' },
    ]
    const map = buildThreadMap(msgs)
    expect(map.get('child1')!.indent).toBe(1)
    expect(map.get('child2')!.indent).toBe(1)
    expect(map.get('grandchild1')!.indent).toBe(2)
  })

  it('handles messages with empty string uuid', () => {
    const msgs: MockMsg[] = [{ uuid: '', parent_uuid: null }]
    const map = buildThreadMap(msgs)
    // Empty string is falsy — should be skipped
    expect(map.size).toBe(0)
  })

  it('handles parent_uuid pointing to self (cycle)', () => {
    const msgs: MockMsg[] = [{ uuid: 'a', parent_uuid: 'a' }]
    const map = buildThreadMap(msgs)
    // Self-reference: treat as root to avoid infinite loop
    expect(map.get('a')!.indent).toBe(0)
  })

  it('handles circular parent chains', () => {
    const msgs: MockMsg[] = [
      { uuid: 'a', parent_uuid: 'b' },
      { uuid: 'b', parent_uuid: 'a' },
    ]
    const map = buildThreadMap(msgs)
    // Cycle: one will resolve as root, the other as child, no infinite loop
    const indentA = map.get('a')!.indent
    const indentB = map.get('b')!.indent
    expect(indentA + indentB).toBeLessThanOrEqual(2) // bounded, no crash
  })

  it('performs acceptably with 1000 messages', () => {
    const msgs: MockMsg[] = Array.from({ length: 1000 }, (_, i) => ({
      uuid: String(i),
      parent_uuid: i > 0 ? String(i - 1) : null,
    }))
    const start = performance.now()
    const map = buildThreadMap(msgs)
    const elapsed = performance.now() - start
    expect(map.size).toBe(1000)
    expect(elapsed).toBeLessThan(100) // should be well under 100ms
  })
})
```

**Step 2: Run test to verify it fails**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view && npx vitest run src/lib/thread-map.test.ts`
Expected: FAIL — `Cannot find module './thread-map'`

**Step 3: Write minimal implementation**

```typescript
// src/lib/thread-map.ts

const MAX_INDENT = 5

export interface ThreadInfo {
  indent: number
  isChild: boolean
  parentUuid: string | undefined
}

interface HasUuids {
  uuid?: string | null
  parent_uuid?: string | null
}

export function buildThreadMap(messages: HasUuids[]): Map<string, ThreadInfo> {
  const map = new Map<string, ThreadInfo>()

  // Index all known uuids + build parent lookup
  const uuidSet = new Set<string>()
  const parentOf = new Map<string, string>()

  for (const msg of messages) {
    if (!msg.uuid) continue
    uuidSet.add(msg.uuid)
    if (msg.parent_uuid && msg.parent_uuid !== msg.uuid) {
      parentOf.set(msg.uuid, msg.parent_uuid)
    }
  }

  // Compute indent with memoization + cycle detection
  const indentCache = new Map<string, number>()

  function computeIndent(uuid: string, visited: Set<string>): number {
    if (indentCache.has(uuid)) return indentCache.get(uuid)!
    if (visited.has(uuid)) return 0 // cycle detected

    const parent = parentOf.get(uuid)
    if (!parent || !uuidSet.has(parent)) {
      indentCache.set(uuid, 0)
      return 0
    }

    visited.add(uuid)
    const parentIndent = computeIndent(parent, visited)
    const indent = Math.min(parentIndent + 1, MAX_INDENT)
    indentCache.set(uuid, indent)
    return indent
  }

  for (const msg of messages) {
    if (!msg.uuid) continue

    const parent = parentOf.get(msg.uuid)
    const hasValidParent = !!parent && uuidSet.has(parent)
    const indent = computeIndent(msg.uuid, new Set())

    map.set(msg.uuid, {
      indent,
      isChild: hasValidParent && indent > 0,
      parentUuid: hasValidParent ? parent : undefined,
    })
  }

  return map
}
```

**Step 4: Run test to verify it passes**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view && npx vitest run src/lib/thread-map.test.ts`
Expected: PASS — all 12 tests green

**Step 5: Commit**

```bash
git add src/lib/thread-map.ts src/lib/thread-map.test.ts
git commit -m "feat: add buildThreadMap utility for computing thread indent levels

Walks parent_uuid chains to compute indent (capped at 5), handles
cycles, orphans, and null uuids. Tested with 12 cases including
1000-message performance check.

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

### Task 2: Wire threadMap into ConversationView

**Files:**
- Modify: `src/components/ConversationView.tsx` (add import, useMemo, update Virtuoso render)

**Context:** Currently `ConversationView.tsx:246-254` renders:
```tsx
<MessageTyped
  key={message.uuid || index}
  message={message}
  messageIndex={index}
  messageType={message.role}
  metadata={message.metadata}
  parentUuid={message.parent_uuid ?? undefined}
/>
```
It passes `parentUuid` but NOT `indent` or `isChildMessage`. Those default to `0` and `false` in MessageTyped. We add the threadMap computation and pass the missing props.

**Step 1: Write the integration test**

```typescript
// src/components/ConversationView.thread.test.ts
import { describe, it, expect } from 'vitest'
import { buildThreadMap } from '../lib/thread-map'

// Unit test verifying the exact message shape ConversationView will pass
describe('ConversationView thread integration', () => {
  it('computes correct indent for a typical tool-call conversation', () => {
    // This mirrors real JSONL output: user → assistant → tool_use → tool_result → assistant
    const messages = [
      { uuid: 'u1', parent_uuid: null, role: 'user', content: 'Fix the bug' },
      { uuid: 'a1', parent_uuid: 'u1', role: 'assistant', content: 'Let me look...' },
      { uuid: 't1', parent_uuid: 'a1', role: 'tool_use', content: '' },
      { uuid: 'r1', parent_uuid: 't1', role: 'tool_result', content: '' },
      { uuid: 'a2', parent_uuid: 'r1', role: 'assistant', content: 'Fixed.' },
    ]
    const map = buildThreadMap(messages)

    expect(map.get('u1')).toEqual({ indent: 0, isChild: false, parentUuid: undefined })
    expect(map.get('a1')).toEqual({ indent: 1, isChild: true, parentUuid: 'u1' })
    expect(map.get('t1')).toEqual({ indent: 2, isChild: true, parentUuid: 'a1' })
    expect(map.get('r1')).toEqual({ indent: 3, isChild: true, parentUuid: 't1' })
    expect(map.get('a2')).toEqual({ indent: 4, isChild: true, parentUuid: 'r1' })
  })

  it('handles compact mode (only user + assistant, orphaned from parents)', () => {
    // In compact mode, tool_use/tool_result are filtered out.
    // Assistant's parent_uuid points to a filtered-out message → treated as root.
    const compactMessages = [
      { uuid: 'u1', parent_uuid: null, role: 'user', content: 'Fix the bug' },
      { uuid: 'a2', parent_uuid: 'r1', role: 'assistant', content: 'Fixed.' },
    ]
    const map = buildThreadMap(compactMessages)

    expect(map.get('u1')!.indent).toBe(0)
    // a2's parent 'r1' is not in the filtered list → orphaned → root
    expect(map.get('a2')!.indent).toBe(0)
    expect(map.get('a2')!.isChild).toBe(false)
  })

  it('handles messages with no uuid at all', () => {
    const messages = [
      { uuid: null, parent_uuid: null, role: 'system', content: '' },
      { uuid: 'a1', parent_uuid: null, role: 'assistant', content: 'hi' },
    ]
    const map = buildThreadMap(messages)
    expect(map.size).toBe(1) // only a1
  })
})
```

**Step 2: Run test**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view && npx vitest run src/components/ConversationView.thread.test.ts`
Expected: PASS (uses Task 1's utility)

**Step 3: Apply changes to ConversationView.tsx**

Add import after existing imports (around line 17):
```typescript
import { buildThreadMap } from '../lib/thread-map'
```

Add useMemo after the `filteredMessages` useMemo (after line 101):
```typescript
  const threadMap = useMemo(
    () => buildThreadMap(filteredMessages),
    [filteredMessages]
  )
```

Update the Virtuoso `itemContent` callback (lines 245-256) to:
```typescript
              itemContent={(index, message) => {
                const thread = message.uuid ? threadMap.get(message.uuid) : undefined
                return (
                  <div className="max-w-4xl mx-auto px-6 pb-4">
                    <MessageTyped
                      key={message.uuid || index}
                      message={message}
                      messageIndex={index}
                      messageType={message.role}
                      metadata={message.metadata}
                      parentUuid={thread?.parentUuid}
                      indent={thread?.indent ?? 0}
                      isChildMessage={thread?.isChild ?? false}
                    />
                  </div>
                )
              }}
```

**Step 4: Run all component tests**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view && npx vitest run src/components/`
Expected: All existing tests still pass (MessageTyped tests don't depend on ConversationView)

**Step 5: Manual verification**

Open browser → navigate to a conversation with tool calls → verify:
- Child messages show 12px left indent per level
- Child messages have dashed gray left border
- Root messages have no indent
- Compact mode: orphaned messages (parent filtered out) render as root with no indent

**Step 6: Commit**

```bash
git add src/components/ConversationView.tsx src/components/ConversationView.thread.test.ts
git commit -m "feat: wire thread indent/isChild into ConversationView Virtuoso render

buildThreadMap computes indent levels from parent_uuid chains.
Compact mode gracefully orphans messages whose parents were filtered.

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

### Task 3: Thread hover highlighting

**Files:**
- Create: `src/contexts/ThreadHighlightContext.tsx`
- Create: `src/contexts/ThreadHighlightContext.test.tsx`
- Add: `getThreadChain()` to `src/lib/thread-map.ts`
- Add: tests to `src/lib/thread-map.test.ts`
- Modify: `src/components/ConversationView.tsx` (wrap in provider)
- Modify: `src/components/MessageTyped.tsx` (hover handlers + highlight style)

**Context:** When a user hovers a message, the entire thread chain (ancestors + descendants) should get a subtle background highlight. This uses a React context (matching the ExpandContext pattern) and a `getThreadChain()` helper.

**Performance concern:** Virtuoso renders ~50-100 visible items. Each mouse enter triggers a Set lookup per visible item. The chain computation itself only runs on hover (not on render), so it's fine. The context update triggers re-render of visible items only because Virtuoso virtualizes the rest. Use `useCallback` for the hover handlers to avoid closure churn.

**Step 1: Write failing tests for ThreadHighlightContext**

```typescript
// src/contexts/ThreadHighlightContext.test.tsx
import { describe, it, expect } from 'vitest'
import { renderHook, act } from '@testing-library/react'
import { ThreadHighlightProvider, useThreadHighlight } from './ThreadHighlightContext'
import type { ReactNode } from 'react'

const wrapper = ({ children }: { children: ReactNode }) => (
  <ThreadHighlightProvider>{children}</ThreadHighlightProvider>
)

describe('ThreadHighlightContext', () => {
  it('starts with empty highlighted set', () => {
    const { result } = renderHook(() => useThreadHighlight(), { wrapper })
    expect(result.current.highlightedUuids.size).toBe(0)
  })

  it('highlights a set of uuids', () => {
    const { result } = renderHook(() => useThreadHighlight(), { wrapper })
    act(() => result.current.setHighlightedUuids(new Set(['a', 'b', 'c'])))
    expect(result.current.highlightedUuids).toEqual(new Set(['a', 'b', 'c']))
  })

  it('clears highlight', () => {
    const { result } = renderHook(() => useThreadHighlight(), { wrapper })
    act(() => result.current.setHighlightedUuids(new Set(['a'])))
    act(() => result.current.clearHighlight())
    expect(result.current.highlightedUuids.size).toBe(0)
  })

  it('replaces previous highlight set entirely', () => {
    const { result } = renderHook(() => useThreadHighlight(), { wrapper })
    act(() => result.current.setHighlightedUuids(new Set(['a'])))
    act(() => result.current.setHighlightedUuids(new Set(['b', 'c'])))
    expect(result.current.highlightedUuids).toEqual(new Set(['b', 'c']))
  })

  it('throws when used outside provider', () => {
    // Suppress console.error for expected error
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {})
    expect(() => {
      renderHook(() => useThreadHighlight())
    }).toThrow('useThreadHighlight must be used within ThreadHighlightProvider')
    spy.mockRestore()
  })
})
```

**Step 2: Run test to verify it fails**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view && npx vitest run src/contexts/ThreadHighlightContext.test.tsx`
Expected: FAIL — module not found

**Step 3: Implement ThreadHighlightContext**

```typescript
// src/contexts/ThreadHighlightContext.tsx
import { createContext, useContext, useState, useCallback, type ReactNode } from 'react'

interface ThreadHighlightState {
  highlightedUuids: Set<string>
  setHighlightedUuids: (uuids: Set<string>) => void
  clearHighlight: () => void
}

const EMPTY_SET: ReadonlySet<string> = new Set<string>()

const ThreadHighlightContext = createContext<ThreadHighlightState | null>(null)

export function ThreadHighlightProvider({ children }: { children: ReactNode }) {
  const [highlightedUuids, setHighlightedUuids] = useState<Set<string>>(EMPTY_SET as Set<string>)
  const clearHighlight = useCallback(() => setHighlightedUuids(EMPTY_SET as Set<string>), [])

  return (
    <ThreadHighlightContext.Provider value={{ highlightedUuids, setHighlightedUuids, clearHighlight }}>
      {children}
    </ThreadHighlightContext.Provider>
  )
}

export function useThreadHighlight(): ThreadHighlightState {
  const ctx = useContext(ThreadHighlightContext)
  if (!ctx) throw new Error('useThreadHighlight must be used within ThreadHighlightProvider')
  return ctx
}
```

**Step 4: Run ThreadHighlightContext tests**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view && npx vitest run src/contexts/ThreadHighlightContext.test.tsx`
Expected: PASS — all 5 tests green

**Step 5: Add getThreadChain to thread-map.ts**

Append to `src/lib/thread-map.ts`:

```typescript
/**
 * Returns the full ancestor + descendant chain for a given uuid.
 * Used for hover-highlighting an entire thread.
 */
export function getThreadChain(uuid: string, messages: HasUuids[]): Set<string> {
  const chain = new Set<string>()
  const childrenOf = new Map<string, string[]>()
  const parentOf = new Map<string, string>()

  for (const msg of messages) {
    if (!msg.uuid) continue
    if (msg.parent_uuid && msg.parent_uuid !== msg.uuid) {
      parentOf.set(msg.uuid, msg.parent_uuid)
      const siblings = childrenOf.get(msg.parent_uuid) || []
      siblings.push(msg.uuid)
      childrenOf.set(msg.parent_uuid, siblings)
    }
  }

  // Walk up (ancestors)
  let current: string | undefined = uuid
  const visited = new Set<string>()
  while (current && !visited.has(current)) {
    visited.add(current)
    chain.add(current)
    current = parentOf.get(current)
  }

  // Walk down (descendants) via BFS
  const queue = [uuid]
  while (queue.length > 0) {
    const node = queue.shift()!
    chain.add(node)
    for (const child of childrenOf.get(node) || []) {
      if (!chain.has(child)) queue.push(child)
    }
  }

  return chain
}
```

**Step 6: Add getThreadChain tests**

Append to `src/lib/thread-map.test.ts`:

```typescript
import { getThreadChain } from './thread-map'

describe('getThreadChain', () => {
  it('returns ancestors and descendants for a mid-chain node', () => {
    const msgs = [
      { uuid: 'a', parent_uuid: null },
      { uuid: 'b', parent_uuid: 'a' },
      { uuid: 'c', parent_uuid: 'b' },
      { uuid: 'd', parent_uuid: 'b' },  // sibling of c
    ]
    const chain = getThreadChain('b', msgs)
    expect(chain).toEqual(new Set(['a', 'b', 'c', 'd']))
  })

  it('returns just self for isolated root', () => {
    const msgs = [
      { uuid: 'a', parent_uuid: null },
      { uuid: 'x', parent_uuid: null },  // unrelated
    ]
    expect(getThreadChain('a', msgs)).toEqual(new Set(['a']))
  })

  it('returns full linear chain from leaf', () => {
    const msgs = [
      { uuid: 'a', parent_uuid: null },
      { uuid: 'b', parent_uuid: 'a' },
      { uuid: 'c', parent_uuid: 'b' },
    ]
    expect(getThreadChain('c', msgs)).toEqual(new Set(['a', 'b', 'c']))
  })

  it('handles circular references without infinite loop', () => {
    const msgs = [
      { uuid: 'a', parent_uuid: 'b' },
      { uuid: 'b', parent_uuid: 'a' },
    ]
    const chain = getThreadChain('a', msgs)
    expect(chain.has('a')).toBe(true)
    expect(chain.has('b')).toBe(true)
    // Just verify it terminates — no hang
  })
})
```

**Step 7: Run all thread-map tests**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view && npx vitest run src/lib/thread-map.test.ts`
Expected: PASS — all 16 tests green

**Step 8: Wire provider into ConversationView**

In `ConversationView.tsx`, add import:
```typescript
import { ThreadHighlightProvider } from '../contexts/ThreadHighlightContext'
```

Wrap `<ExpandProvider>` with `<ThreadHighlightProvider>` (around line 242):
```tsx
<ThreadHighlightProvider>
  <ExpandProvider>
    <Virtuoso ... />
  </ExpandProvider>
</ThreadHighlightProvider>
```

Also pass `filteredMessages` as a data attribute or via a ref so MessageTyped can access the message list for chain computation. **Better approach:** pre-compute a `getChainForUuid` callback via useMemo and pass it through context or as a prop.

Add to ConversationView after the threadMap useMemo:
```typescript
  const getThreadChainForUuid = useCallback(
    (uuid: string) => getThreadChain(uuid, filteredMessages),
    [filteredMessages]
  )
```

Add import:
```typescript
import { buildThreadMap, getThreadChain } from '../lib/thread-map'
```

Pass as prop to MessageTyped:
```tsx
<MessageTyped
  ...
  onGetThreadChain={getThreadChainForUuid}
/>
```

**Step 9: Update MessageTyped for hover highlighting**

In `MessageTyped.tsx`:

Add to props interface:
```typescript
  /** Callback to get the full thread chain for highlighting */
  onGetThreadChain?: (uuid: string) => Set<string>
```

Add imports:
```typescript
import { useThreadHighlight } from '../contexts/ThreadHighlightContext'
```

Inside the component function, after the `handleCopyMessage` callback:
```typescript
  const { highlightedUuids, setHighlightedUuids, clearHighlight } = useThreadHighlight()
  const isHighlighted = message.uuid ? highlightedUuids.has(message.uuid) : false

  const handleMouseEnter = useCallback(() => {
    if (message.uuid && onGetThreadChain) {
      setHighlightedUuids(onGetThreadChain(message.uuid))
    }
  }, [message.uuid, onGetThreadChain, setHighlightedUuids])

  const handleMouseLeave = useCallback(() => {
    clearHighlight()
  }, [clearHighlight])
```

Add to the outer div:
```tsx
onMouseEnter={handleMouseEnter}
onMouseLeave={handleMouseLeave}
```

Add highlight class (alongside existing classes):
```typescript
isHighlighted && 'bg-indigo-50/60 dark:bg-indigo-950/30'
```

**Important:** Since `useThreadHighlight` will now be called in MessageTyped, it must be inside the `<ThreadHighlightProvider>`. It already is — ConversationView wraps the Virtuoso in the provider.

**Step 10: Handle MessageTyped used outside provider (tests)**

The existing MessageTyped tests render the component standalone without a provider. Two options:
1. Make `useThreadHighlight` return a no-op default when outside provider
2. Wrap test renders in the provider

Option 1 is safer — change `useThreadHighlight` to return a safe default:
```typescript
export function useThreadHighlight(): ThreadHighlightState {
  const ctx = useContext(ThreadHighlightContext)
  if (!ctx) {
    return {
      highlightedUuids: EMPTY_SET as Set<string>,
      setHighlightedUuids: () => {},
      clearHighlight: () => {},
    }
  }
  return ctx
}
```

Update the test that checks for the throw — remove it or change to verify graceful fallback.

**Step 11: Run all tests**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view && npx vitest run src/`
Expected: All tests pass

**Step 12: Manual verification**

Open browser → hover over a message → verify:
- The hovered message + all ancestors + all descendants get `bg-indigo-50/60` tint
- Moving mouse away clears the highlight
- No visible jank or lag when hovering rapidly
- Works in both compact and full view modes

**Step 13: Commit**

```bash
git add src/contexts/ThreadHighlightContext.tsx src/contexts/ThreadHighlightContext.test.tsx src/lib/thread-map.ts src/lib/thread-map.test.ts src/components/MessageTyped.tsx src/components/ConversationView.tsx
git commit -m "feat: add thread hover highlighting across ancestor/descendant chain

Hovering a message highlights the full thread chain. Uses a lightweight
context (ThreadHighlightProvider) and getThreadChain() utility.
Falls back gracefully when used outside provider (tests).

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

### Task 4: Dark mode for MessageTyped

**Files:**
- Modify: `src/components/MessageTyped.tsx`

**Context:** ConversationView already uses `dark:` variants (lines 167-196). MessageTyped has zero `dark:` classes — it's hardcoded to light mode (`bg-white`, `text-gray-900`, etc.). Follow the same `dark:bg-gray-*` / `dark:text-gray-*` pattern as ConversationView.

**Step 1: Apply dark mode classes**

These are the exact replacements in `MessageTyped.tsx`:

**Outer card container** (line ~293-298):
```
Old: 'bg-white hover:bg-gray-50/50'
New: 'bg-white dark:bg-gray-900 hover:bg-gray-50/50 dark:hover:bg-gray-800/50'
```

**Name label** (line ~320):
```
Old: 'font-semibold text-gray-900 text-sm'
New: 'font-semibold text-gray-900 dark:text-gray-100 text-sm'
```

**Copy button** (line ~329-333):
```
Old: 'text-gray-400 hover:text-gray-600'
New: 'text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300'
```

**Timestamp** (line ~336):
```
Old: 'text-xs text-gray-500 whitespace-nowrap'
New: 'text-xs text-gray-500 dark:text-gray-400 whitespace-nowrap'
```

**Tool calls divider** (line ~470):
```
Old: 'mt-2 pt-3 border-t border-gray-200'
New: 'mt-2 pt-3 border-t border-gray-200 dark:border-gray-700'
```

**Tool calls label** (line ~471):
```
Old: 'text-xs font-semibold text-gray-600 mb-2'
New: 'text-xs font-semibold text-gray-600 dark:text-gray-400 mb-2'
```

**Tool call badges** (line ~478):
```
Old: 'px-2 py-1 bg-gray-100 border border-gray-200 rounded text-xs font-mono text-gray-700'
New: 'px-2 py-1 bg-gray-100 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded text-xs font-mono text-gray-700 dark:text-gray-300'
```

**SystemMetadataCard** — system variant (line ~172-174):
```
Old: 'bg-amber-100/30 border border-amber-200/50'
New: 'bg-amber-100/30 dark:bg-amber-900/20 border border-amber-200/50 dark:border-amber-700/30'
```

**SystemMetadataCard** — progress variant (line ~172-174):
```
Old: 'bg-indigo-100/30 border border-indigo-200/50'
New: 'bg-indigo-100/30 dark:bg-indigo-900/20 border border-indigo-200/50 dark:border-indigo-700/30'
```

**SystemMetadataCard key labels** (line ~182):
```
Old (system): 'text-amber-700'
New (system): 'text-amber-700 dark:text-amber-300'

Old (progress): 'text-indigo-700'
New (progress): 'text-indigo-700 dark:text-indigo-300'
```

**SystemMetadataCard values** (line ~187):
```
Old: 'text-gray-700 break-all'
New: 'text-gray-700 dark:text-gray-300 break-all'
```

**Inline code in markdown** (line ~377):
```
Old: 'px-1.5 py-0.5 bg-gray-100 rounded text-xs font-mono'
New: 'px-1.5 py-0.5 bg-gray-100 dark:bg-gray-800 rounded text-xs font-mono'
```

**Step 2: Run existing tests**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view && npx vitest run src/components/MessageTyped.test.tsx`
Expected: All 13 tests still pass (class changes don't affect test assertions)

**Step 3: Manual verification**

- Toggle system dark mode (or add `class="dark"` to `<html>`)
- Verify: card backgrounds are dark, text is light, borders are subtle, badges readable
- Check SystemMetadataCard in both system (amber) and progress (indigo) variants
- Check tool call badges
- Compare side-by-side with ConversationView header (should feel consistent)

**Step 4: Commit**

```bash
git add src/components/MessageTyped.tsx
git commit -m "fix: add dark mode support to MessageTyped and SystemMetadataCard

Adds dark: Tailwind variants to all hardcoded light-mode classes.
Follows the same pattern as ConversationView's existing dark mode.

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

### Task 5: Update plan statuses

**Files:**
- Modify: `docs/plans/PROGRESS.md`
- Modify: `docs/plans/2026-01-29-CONVERSATION-UI-COMPREHENSIVE-REDESIGN.md` (line 4)
- Modify: `docs/plans/2026-01-29-HARDENING-IMPLEMENTATION-PLAN-V2-FINAL.md` (YAML status)

**Step 1: Mark comprehensive redesign as superseded**

In `2026-01-29-CONVERSATION-UI-COMPREHENSIVE-REDESIGN.md`, change line 4:
```
Old: **Status:** Design Phase (Brainstorming Output)
New: **Status:** Superseded — substance implemented, remaining gaps covered by 2026-02-02-thread-visualization-polish.md
```

**Step 2: Mark hardening plan as done**

In `2026-01-29-HARDENING-IMPLEMENTATION-PLAN-V2-FINAL.md`, update YAML frontmatter status to `done`.

**Step 3: Update PROGRESS.md Plan File Index**

In the Active Plans table:
- Hardening plan: change status `pending` → `done`
- Comprehensive redesign: change status `pending` → `superseded`
- Add new row: `2026-02-02-thread-visualization-polish.md` | `pending` | Thread hover highlighting + dark mode polish

**Step 4: Commit**

```bash
git add docs/plans/PROGRESS.md docs/plans/2026-01-29-CONVERSATION-UI-COMPREHENSIVE-REDESIGN.md docs/plans/2026-01-29-HARDENING-IMPLEMENTATION-PLAN-V2-FINAL.md
git commit -m "docs: update plan statuses — hardening done, redesign superseded, thread polish pending

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```
