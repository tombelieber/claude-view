# Lossless RichMessage Conversion — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make `messagesToRichMessages()` lossless so ConversationView verbose mode shows all 7 JSONL message types through RichPane with specialized cards.

**Architecture:** Extend `RichMessage` type with `system|progress|summary` and `metadata` field. Update the conversion function to emit instead of skip. Add dispatch cards in RichPane that reuse existing specialized card components. Hide thinking in compact chat mode.

**Tech Stack:** React, TypeScript, Vitest

**Design doc:** `docs/plans/2026-02-21-lossless-rich-messages-design.md`

---

### Task 1: Test `messagesToRichMessages()` lossless conversion

**Files:**
- Create: `src/lib/message-to-rich.test.ts`

**Step 1: Write the failing tests**

```typescript
import { describe, it, expect } from 'vitest'
import { messagesToRichMessages } from './message-to-rich'
import type { Message } from '../types/generated'

function makeMsg(overrides: Partial<Message>): Message {
  return { role: 'user', content: '', ...overrides } as Message
}

describe('messagesToRichMessages', () => {
  describe('existing types (regression)', () => {
    it('converts user messages', () => {
      const result = messagesToRichMessages([makeMsg({ role: 'user', content: 'hello' })])
      expect(result).toHaveLength(1)
      expect(result[0]).toMatchObject({ type: 'user', content: 'hello' })
    })

    it('converts assistant messages with thinking', () => {
      const result = messagesToRichMessages([
        makeMsg({ role: 'assistant', content: 'reply', thinking: 'let me think' }),
      ])
      expect(result).toHaveLength(2)
      expect(result[0]).toMatchObject({ type: 'thinking', content: 'let me think' })
      expect(result[1]).toMatchObject({ type: 'assistant', content: 'reply' })
    })

    it('converts tool_use with tool_calls', () => {
      const result = messagesToRichMessages([
        makeMsg({
          role: 'tool_use',
          content: '',
          tool_calls: [{ name: 'Read', input: { file_path: '/foo' } }],
        }),
      ])
      expect(result).toHaveLength(1)
      expect(result[0]).toMatchObject({ type: 'tool_use', name: 'Read', category: 'builtin' })
    })

    it('converts tool_result', () => {
      const result = messagesToRichMessages([
        makeMsg({ role: 'tool_result', content: 'file contents here' }),
      ])
      expect(result).toHaveLength(1)
      expect(result[0]).toMatchObject({ type: 'tool_result', content: 'file contents here' })
    })
  })

  describe('system messages (NEW — was flattened to assistant)', () => {
    it('emits type system with metadata', () => {
      const result = messagesToRichMessages([
        makeMsg({
          role: 'system',
          content: 'turn ended',
          metadata: { type: 'turn_duration', durationMs: 1500 },
        }),
      ])
      expect(result).toHaveLength(1)
      expect(result[0]).toMatchObject({
        type: 'system',
        content: 'turn ended',
        metadata: { type: 'turn_duration', durationMs: 1500 },
      })
    })

    it('emits system even with empty content', () => {
      const result = messagesToRichMessages([
        makeMsg({
          role: 'system',
          content: '',
          metadata: { type: 'api_error', error: { code: 'overloaded' } },
        }),
      ])
      expect(result).toHaveLength(1)
      expect(result[0].type).toBe('system')
      expect(result[0].metadata).toEqual({ type: 'api_error', error: { code: 'overloaded' } })
    })
  })

  describe('progress messages (NEW — was skipped)', () => {
    it('emits type progress with metadata', () => {
      const result = messagesToRichMessages([
        makeMsg({
          role: 'progress',
          content: '',
          metadata: { type: 'agent_progress', agentId: 'abc', model: 'opus' },
        }),
      ])
      expect(result).toHaveLength(1)
      expect(result[0]).toMatchObject({
        type: 'progress',
        metadata: { type: 'agent_progress', agentId: 'abc', model: 'opus' },
      })
    })
  })

  describe('summary messages (NEW — was skipped)', () => {
    it('emits type summary with metadata', () => {
      const result = messagesToRichMessages([
        makeMsg({
          role: 'summary',
          content: 'Session summary text',
          metadata: { summary: 'Session summary text', leafUuid: 'uuid-123' },
        }),
      ])
      expect(result).toHaveLength(1)
      expect(result[0]).toMatchObject({
        type: 'summary',
        content: 'Session summary text',
        metadata: { summary: 'Session summary text', leafUuid: 'uuid-123' },
      })
    })
  })
})
```

**Step 2: Run tests to verify they fail**

Run: `bunx vitest run src/lib/message-to-rich.test.ts`

Expected: System test fails (gets `type: 'assistant'` instead of `type: 'system'`). Progress and summary tests fail (empty result array).

**Step 3: Commit test file**

```bash
git add src/lib/message-to-rich.test.ts
git commit -m "test: add messagesToRichMessages lossless conversion tests"
```

---

### Task 2: Extend RichMessage type

**Files:**
- Modify: `src/components/live/RichPane.tsx:29-37` (RichMessage interface)

**Step 1: Update the type union and add metadata field**

In `src/components/live/RichPane.tsx`, change the `RichMessage` interface:

```typescript
export interface RichMessage {
  type: 'user' | 'assistant' | 'tool_use' | 'tool_result' | 'thinking' | 'error' | 'hook'
      | 'system' | 'progress' | 'summary'
  content: string
  name?: string // tool name for tool_use
  input?: string // tool input summary for tool_use
  inputData?: unknown // raw parsed object for tool_use (avoids re-parsing)
  ts?: number // timestamp
  category?: ActionCategory // set for tool_use, tool_result, hook, error
  metadata?: Record<string, any> // system/progress/summary subtype data
}
```

**Step 2: Verify TypeScript compiles**

Run: `bunx tsc --noEmit`

Expected: No new errors (the type union is additive, existing code still satisfies it).

**Step 3: Commit**

```bash
git add src/components/live/RichPane.tsx
git commit -m "feat: extend RichMessage type with system/progress/summary and metadata"
```

---

### Task 3: Make `messagesToRichMessages()` lossless

**Files:**
- Modify: `src/lib/message-to-rich.ts:124-136` (system/default cases)

**Step 1: Replace the system case and default case**

In `src/lib/message-to-rich.ts`, replace the existing `case 'system'` block (lines 124-131) and the `default` block (lines 133-136) with:

```typescript
      case 'system': {
        const content = stripCommandTags(msg.content)
        result.push({
          type: 'system',
          content: content || '',
          ts,
          metadata: msg.metadata ?? undefined,
        })
        break
      }

      case 'progress': {
        const content = stripCommandTags(msg.content)
        result.push({
          type: 'progress',
          content: content || '',
          ts,
          metadata: msg.metadata ?? undefined,
        })
        break
      }

      case 'summary': {
        result.push({
          type: 'summary',
          content: msg.content || '',
          ts,
          metadata: msg.metadata ?? undefined,
        })
        break
      }

      default:
        break
```

**Step 2: Run the tests from Task 1**

Run: `bunx vitest run src/lib/message-to-rich.test.ts`

Expected: All tests PASS.

**Step 3: Verify TypeScript compiles**

Run: `bunx tsc --noEmit`

Expected: No errors.

**Step 4: Commit**

```bash
git add src/lib/message-to-rich.ts
git commit -m "feat: emit system/progress/summary in messagesToRichMessages instead of skipping"
```

---

### Task 4: Add system/progress/summary card renderers to RichPane

**Files:**
- Modify: `src/components/live/RichPane.tsx` (add imports, 3 new components, update MessageCard switch)

**Step 1: Add imports at the top of RichPane.tsx**

After the existing imports (around line 25), add:

```typescript
// System event cards (reused from MessageTyped)
import { TurnDurationCard } from '../TurnDurationCard'
import { ApiErrorCard } from '../ApiErrorCard'
import { CompactBoundaryCard } from '../CompactBoundaryCard'
import { HookSummaryCard } from '../HookSummaryCard'
import { LocalCommandEventCard } from '../LocalCommandEventCard'
import { MessageQueueEventCard } from '../MessageQueueEventCard'
import { FileSnapshotCard } from '../FileSnapshotCard'
// Progress event cards
import { AgentProgressCard } from '../AgentProgressCard'
import { BashProgressCard } from '../BashProgressCard'
import { HookProgressCard } from '../HookProgressCard'
import { McpProgressCard } from '../McpProgressCard'
import { TaskQueueCard } from '../TaskQueueCard'
// Summary card
import { SessionSummaryCard } from '../SessionSummaryCard'
```

**Step 2: Add SystemMessageCard component**

Place before the `MessageCard` function (around line 612):

```typescript
function SystemMessageCard({ message }: { message: RichMessage }) {
  const m = message.metadata
  const subtype = m?.type ?? m?.subtype

  const card = (() => {
    switch (subtype) {
      case 'turn_duration':
        return <TurnDurationCard durationMs={m.durationMs} startTime={m.startTime} endTime={m.endTime} />
      case 'api_error':
        return <ApiErrorCard error={m.error} retryAttempt={m.retryAttempt} maxRetries={m.maxRetries} retryInMs={m.retryInMs} />
      case 'compact_boundary':
        return <CompactBoundaryCard trigger={m.trigger} preTokens={m.preTokens} postTokens={m.postTokens} />
      case 'hook_summary':
        return <HookSummaryCard hookCount={m.hookCount} hookInfos={m.hookInfos} hookErrors={m.hookErrors} durationMs={m.durationMs} preventedContinuation={m.preventedContinuation} />
      case 'local_command':
        return <LocalCommandEventCard content={m.content ?? message.content} />
      case 'queue-operation':
        return <MessageQueueEventCard operation={m.operation} timestamp={m.timestamp || ''} content={m.content} />
      case 'file-history-snapshot': {
        const snapshot = m.snapshot || {}
        const files = Object.keys(snapshot.trackedFileBackups || {})
        return <FileSnapshotCard fileCount={files.length} timestamp={snapshot.timestamp || ''} files={files} isIncremental={m.isSnapshotUpdate || false} />
      }
      default:
        return null
    }
  })()

  return (
    <div className="border-l-2 border-amber-500/30 dark:border-amber-500/20 pl-2 py-0.5">
      <div className="flex items-center gap-1.5">
        <AlertTriangle className="w-3 h-3 text-amber-500/60 dark:text-amber-400/50 flex-shrink-0" />
        <span className="text-[10px] font-mono text-amber-600 dark:text-amber-400">system</span>
        {subtype && (
          <span className="text-[9px] font-mono text-gray-400 dark:text-gray-600">{subtype}</span>
        )}
        <div className="flex-1" />
        <Timestamp ts={message.ts} />
      </div>
      {card ? (
        <div className="mt-0.5 ml-5">{card}</div>
      ) : message.content ? (
        <div className="text-[10px] text-gray-600 dark:text-gray-500 mt-0.5 ml-5 font-mono">{message.content}</div>
      ) : m ? (
        <pre className="text-[10px] text-gray-500 dark:text-gray-600 mt-0.5 ml-5 font-mono whitespace-pre-wrap">{JSON.stringify(m, null, 2)}</pre>
      ) : null}
    </div>
  )
}
```

**Step 3: Add ProgressMessageCard component**

Place after SystemMessageCard:

```typescript
function ProgressMessageCard({ message }: { message: RichMessage }) {
  const m = message.metadata
  const subtype = m?.type

  const card = (() => {
    switch (subtype) {
      case 'agent_progress':
        return <AgentProgressCard agentId={m.agentId} prompt={m.prompt} model={m.model} tokens={m.tokens} normalizedMessages={m.normalizedMessages} indent={m.indent} />
      case 'bash_progress':
        return <BashProgressCard command={m.command} output={m.output} exitCode={m.exitCode} duration={m.duration} />
      case 'hook_progress':
        return <HookProgressCard hookEvent={m.hookEvent} hookName={m.hookName} command={m.command} output={m.output} />
      case 'mcp_progress':
        return <McpProgressCard server={m.server} method={m.method} params={m.params} result={m.result} />
      case 'waiting_for_task':
        return <TaskQueueCard waitDuration={m.waitDuration} position={m.position} queueLength={m.queueLength} />
      default:
        return null
    }
  })()

  return (
    <div className="border-l-2 border-indigo-500/30 dark:border-indigo-500/20 pl-2 py-0.5">
      <div className="flex items-center gap-1.5">
        <Zap className="w-3 h-3 text-indigo-500/60 dark:text-indigo-400/50 flex-shrink-0" />
        <span className="text-[10px] font-mono text-indigo-600 dark:text-indigo-400">progress</span>
        {subtype && (
          <span className="text-[9px] font-mono text-gray-400 dark:text-gray-600">{subtype}</span>
        )}
        <div className="flex-1" />
        <Timestamp ts={message.ts} />
      </div>
      {card ? (
        <div className="mt-0.5 ml-5">{card}</div>
      ) : message.content ? (
        <div className="text-[10px] text-gray-600 dark:text-gray-500 mt-0.5 ml-5 font-mono">{message.content}</div>
      ) : m ? (
        <pre className="text-[10px] text-gray-500 dark:text-gray-600 mt-0.5 ml-5 font-mono whitespace-pre-wrap">{JSON.stringify(m, null, 2)}</pre>
      ) : null}
    </div>
  )
}
```

**Step 4: Add SummaryMessageCard component**

Place after ProgressMessageCard:

```typescript
function SummaryMessageCard({ message }: { message: RichMessage }) {
  const m = message.metadata
  const summary = m?.summary || message.content
  const leafUuid = m?.leafUuid || ''
  const wordCount = (summary || '').split(/\s+/).filter(Boolean).length

  return (
    <div className="border-l-2 border-rose-500/30 dark:border-rose-500/20 pl-2 py-0.5">
      <div className="flex items-center gap-1.5">
        <BookOpen className="w-3 h-3 text-rose-500/60 dark:text-rose-400/50 flex-shrink-0" />
        <span className="text-[10px] font-mono text-rose-600 dark:text-rose-400">summary</span>
        <div className="flex-1" />
        <Timestamp ts={message.ts} />
      </div>
      <div className="mt-0.5 ml-5">
        <SessionSummaryCard summary={summary} leafUuid={leafUuid} wordCount={wordCount} />
      </div>
    </div>
  )
}
```

**Step 5: Add Zap and BookOpen to lucide-react imports**

In the existing lucide-react import at the top of `RichPane.tsx` (line 8-14), add `Zap` and `BookOpen`:

```typescript
import {
  User,
  Bot,
  Wrench,
  Brain,
  AlertTriangle,
  ArrowDown,
  Zap,
  BookOpen,
} from 'lucide-react'
```

**Step 6: Update MessageCard switch statement**

In the `MessageCard` function (~line 615), add three new cases before `default`:

```typescript
    case 'system':
      return <SystemMessageCard message={message} />
    case 'progress':
      return <ProgressMessageCard message={message} />
    case 'summary':
      return <SummaryMessageCard message={message} />
```

**Step 7: Verify TypeScript compiles**

Run: `bunx tsc --noEmit`

Expected: No errors.

**Step 8: Commit**

```bash
git add src/components/live/RichPane.tsx
git commit -m "feat: add system/progress/summary card renderers to RichPane"
```

---

### Task 5: Update RichPane verbose filter to include new types

**Files:**
- Modify: `src/components/live/RichPane.tsx` (displayMessages useMemo)

**Step 1: Update the verbose filter in displayMessages**

In the `displayMessages` useMemo (~line 654), update the verbose category filter to always show structural types:

Change this block (the verbose filter around line 670-676):

```typescript
    // Verbose mode: apply category filter
    if (verboseFilter === 'all') return messages
    return messages.filter((m) => {
      // Always show conversation backbone
      if (m.type === 'user' || m.type === 'assistant' || m.type === 'thinking') return true
      // Filter by category
      return m.category === verboseFilter
    })
```

To:

```typescript
    // Verbose mode: apply category filter
    if (verboseFilter === 'all') return messages
    return messages.filter((m) => {
      // Always show conversation backbone + structural types
      if (m.type === 'user' || m.type === 'assistant' || m.type === 'thinking') return true
      if (m.type === 'system' || m.type === 'progress' || m.type === 'summary') return true
      // Filter by category
      return m.category === verboseFilter
    })
```

**Step 2: Verify TypeScript compiles**

Run: `bunx tsc --noEmit`

Expected: No errors.

**Step 3: Commit**

```bash
git add src/components/live/RichPane.tsx
git commit -m "feat: include system/progress/summary in verbose filter"
```

---

### Task 6: Hide thinking in ConversationView chat mode

**Files:**
- Modify: `src/components/MessageTyped.tsx:51-64` (props interface)
- Modify: `src/components/MessageTyped.tsx:370-371` (thinking block render)
- Modify: `src/components/ConversationView.tsx:559` (MessageTyped usage)

**Step 1: Add `showThinking` prop to MessageTyped**

In `src/components/MessageTyped.tsx`, add to `MessageTypedProps` interface (~line 51):

```typescript
interface MessageTypedProps {
  message: MessageType
  messageIndex?: number
  messageType?: 'user' | 'assistant' | 'tool_use' | 'tool_result' | 'system' | 'progress' | 'summary'
  metadata?: Record<string, any>
  /** Parent message UUID for threading */
  parentUuid?: string
  /** Nesting level (0 = root, 1 = child, 2 = grandchild, etc.). Capped at MAX_INDENT_LEVEL. */
  indent?: number
  /** Whether this message is a child in a thread (shows connector line) */
  isChildMessage?: boolean
  /** Callback to get the full thread chain for highlighting */
  onGetThreadChain?: (uuid: string) => Set<string>
  /** Whether to show thinking blocks. Default true. */
  showThinking?: boolean
}
```

**Step 2: Destructure the prop and gate the thinking block**

In the `MessageTyped` function signature (~line 255), add `showThinking = true`:

```typescript
export function MessageTyped({
  message,
  messageIndex,
  messageType = message.role as any,
  metadata,
  parentUuid,
  indent = 0,
  isChildMessage = false,
  onGetThreadChain,
  showThinking = true,
}: MessageTypedProps) {
```

Change line 370-371 from:

```tsx
          {message.thinking && (
            <ThinkingBlock thinking={message.thinking} />
          )}
```

To:

```tsx
          {showThinking && message.thinking && (
            <ThinkingBlock thinking={message.thinking} />
          )}
```

**Step 3: Pass `showThinking={false}` in ConversationView**

In `src/components/ConversationView.tsx`, in the compact mode Virtuoso itemContent (~line 559), add the prop:

```tsx
                        <MessageTyped
                          message={message}
                          messageIndex={index}
                          messageType={message.role}
                          metadata={message.metadata}
                          parentUuid={thread?.parentUuid}
                          indent={thread?.indent ?? 0}
                          isChildMessage={thread?.isChild ?? false}
                          onGetThreadChain={getThreadChainForUuid}
                          showThinking={false}
                        />
```

**Step 4: Run existing MessageTyped tests**

Run: `bunx vitest run src/components/MessageTyped.test.tsx`

Expected: All existing tests pass (the default `showThinking=true` preserves backward compat).

**Step 5: Verify TypeScript compiles**

Run: `bunx tsc --noEmit`

Expected: No errors.

**Step 6: Commit**

```bash
git add src/components/MessageTyped.tsx src/components/ConversationView.tsx
git commit -m "feat: hide thinking blocks in ConversationView chat mode"
```

---

### Task 7: Run all frontend tests

**Files:** None (verification only)

**Step 1: Run full test suite**

Run: `bunx vitest run`

Expected: All tests pass.

**Step 2: Fix any failures**

If any tests fail due to the type changes, update them accordingly.

---

### Task 8: Manual verification

**Files:** None (verification only)

**Step 1: Start the dev server**

Run: `bun run dev`

**Step 2: Verify chat mode**

Open a past session in ConversationView with compact mode. Confirm:
- Only user and assistant messages show
- No thinking blocks visible
- No tool_use, tool_result, system, progress, or summary cards visible
- Clean ChatGPT-style layout

**Step 3: Verify verbose mode**

Toggle verbose mode. Confirm:
- ALL message types appear in the RichPane terminal view
- System events render as specialized cards (turn duration, API error, compact boundary, etc.)
- Progress events render as specialized cards (agent progress, bash progress, etc.)
- Summary messages render as SessionSummaryCard
- Unknown subtypes fall back to raw JSON display
- Category filter chips still work for tool types

**Step 4: Verify live monitor**

Open a live session in the monitor. Confirm:
- RichPane still works with WebSocket messages (no regression)
- Verbose/compact toggle works as before
