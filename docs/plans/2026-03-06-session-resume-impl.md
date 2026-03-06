# Lazy Session Resume — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace ad-hoc session resume state with a unified `useSessionControl` hook that owns phase transitions, message lifecycle, and connection health.

**Architecture:** New `useSessionControl(sessionId)` hook wraps the existing `useControlSession` internally. It adds a phase state machine (`idle → resuming → connecting → ready`), optimistic message queue, and connection health tracking. Components consume it instead of calling `useControlSession` directly.

**Tech Stack:** React 19, TypeScript, WebSocket, Vitest

**Design Doc:** `docs/plans/2026-03-06-session-resume-design.md`

---

### Task 1: Add `ChatMessageWithStatus` type

**Files:**
- Modify: `apps/web/src/types/control.ts:133-144`

**Step 1: Add the status type and extended message interface**

After the existing `ChatMessage` interface (line 144), add:

```typescript
/** Message lifecycle status for optimistic rendering */
export type MessageStatus = 'optimistic' | 'sending' | 'sent' | 'failed'

/** ChatMessage with lifecycle tracking */
export interface ChatMessageWithStatus extends ChatMessage {
  /** Unique ID for this message instance (used for retry/status updates) */
  localId: string
  /** Lifecycle status */
  status: MessageStatus
  /** Timestamp when the message was created locally */
  createdAt: number
}
```

**Step 2: Verify TypeScript compiles**

Run: `cd apps/web && bunx tsc --noEmit --pretty 2>&1 | head -20`
Expected: No errors related to `ChatMessageWithStatus`

**Step 3: Commit**

```bash
git add apps/web/src/types/control.ts
git commit -m "feat(control): add ChatMessageWithStatus type for message lifecycle"
```

---

### Task 2: Create `useSessionControl` hook

This is the core of the design — a single hook that owns the entire session control lifecycle.

**Files:**
- Create: `apps/web/src/hooks/use-session-control.ts`

**Step 1: Write the failing test**

Create `apps/web/src/hooks/use-session-control.test.ts`:

```typescript
import { renderHook, act } from '@testing-library/react'
import { describe, it, expect, vi, beforeEach } from 'vitest'

// We'll mock useControlSession to avoid real WebSocket connections
vi.mock('./use-control-session', () => ({
  useControlSession: vi.fn(() => ({
    status: 'disconnected',
    messages: [],
    streamingContent: '',
    streamingMessageId: '',
    contextUsage: 0,
    turnCount: 0,
    sessionCost: null,
    lastTurnCost: null,
    permissionRequest: null,
    askQuestion: null,
    planApproval: null,
    elicitation: null,
    error: null,
    sendMessage: vi.fn(),
    sendRaw: vi.fn(),
    respondPermission: vi.fn(),
    answerQuestion: vi.fn(),
    approvePlan: vi.fn(),
    submitElicitation: vi.fn(),
  })),
}))

describe('useSessionControl', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    globalThis.fetch = vi.fn()
  })

  it('starts in idle phase with dormant inputBarState', async () => {
    const { useSessionControl } = await import('./use-session-control')
    const { result } = renderHook(() => useSessionControl('session-123'))
    expect(result.current.phase).toBe('idle')
    expect(result.current.inputBarState).toBe('dormant')
    expect(result.current.messages).toEqual([])
  })

  it('transitions to resuming on send() when idle', async () => {
    const mockFetch = vi.fn().mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ controlId: 'ctrl-1', sessionId: 'session-123' }),
    })
    globalThis.fetch = mockFetch

    const { useSessionControl } = await import('./use-session-control')
    const { result } = renderHook(() => useSessionControl('session-123'))

    await act(async () => {
      result.current.send('hello')
    })

    // After send, should have created an optimistic message
    expect(result.current.messages).toHaveLength(1)
    expect(result.current.messages[0].content).toBe('hello')
    expect(result.current.messages[0].status).toBe('optimistic')
    expect(result.current.messages[0].role).toBe('user')
  })

  it('marks message as failed when resume POST fails', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue({
      ok: false,
      status: 500,
    })

    const { useSessionControl } = await import('./use-session-control')
    const { result } = renderHook(() => useSessionControl('session-123'))

    await act(async () => {
      result.current.send('hello')
    })

    expect(result.current.messages[0].status).toBe('failed')
    expect(result.current.phase).toBe('error')
  })
})
```

**Step 2: Run test to verify it fails**

Run: `cd apps/web && bunx vitest run src/hooks/use-session-control.test.ts 2>&1 | tail -20`
Expected: FAIL — module `./use-session-control` not found

**Step 3: Write the hook implementation**

Create `apps/web/src/hooks/use-session-control.ts`:

```typescript
// apps/web/src/hooks/use-session-control.ts
// Unified hook for session control lifecycle — owns resume, WS, messages, health.
import { useCallback, useEffect, useRef, useState } from 'react'
import type {
  AskUserQuestionMsg,
  ChatMessageWithStatus,
  ElicitationMsg,
  MessageStatus,
  PermissionRequestMsg,
  PlanApprovalMsg,
} from '../types/control'
import type { InputBarState } from '../components/chat/ChatInputBar'
import { useControlSession, type ControlStatus } from './use-control-session'

// ---------------------------------------------------------------------------
// Phase state machine
// ---------------------------------------------------------------------------

export type SessionPhase = 'idle' | 'resuming' | 'connecting' | 'ready' | 'reconnecting' | 'completed' | 'error'

export type ConnectionHealth = 'ok' | 'degraded' | 'lost'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

let _localIdCounter = 0
function nextLocalId(): string {
  return `local-${Date.now()}-${++_localIdCounter}`
}

function phaseToInputBarState(phase: SessionPhase, controlStatus: ControlStatus): InputBarState {
  switch (phase) {
    case 'idle':
      return 'dormant'
    case 'resuming':
      return 'resuming'
    case 'connecting':
      return 'connecting'
    case 'reconnecting':
      return 'reconnecting'
    case 'completed':
      return 'completed'
    case 'error':
      return 'dormant' // allow retry
    case 'ready':
      // Derive from WS control status
      switch (controlStatus) {
        case 'active':
          return 'streaming'
        case 'waiting_input':
          return 'active'
        case 'waiting_permission':
          return 'waiting_permission'
        case 'completed':
          return 'completed'
        default:
          return 'active'
      }
  }
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export interface UseSessionControlReturn {
  phase: SessionPhase
  inputBarState: InputBarState
  messages: ChatMessageWithStatus[]
  connectionHealth: ConnectionHealth
  streamingContent: string
  contextPercent: number
  sessionCost: number | null
  lastTurnCost: number | null
  permissionRequest: PermissionRequestMsg | null
  askQuestion: AskUserQuestionMsg | null
  planApproval: PlanApprovalMsg | null
  elicitation: ElicitationMsg | null
  error: string | null
  send: (text: string) => void
  retry: (localId: string) => void
  respondPermission: (id: string, allowed: boolean) => void
  answerQuestion: (id: string, answers: Record<string, string>) => void
  approvePlan: (id: string, approved: boolean, feedback?: string) => void
  submitElicitation: (id: string, response: string) => void
}

export function useSessionControl(sessionId: string): UseSessionControlReturn {
  const [phase, setPhase] = useState<SessionPhase>('idle')
  const [controlId, setControlId] = useState<string | null>(null)
  const [messages, setMessages] = useState<ChatMessageWithStatus[]>([])
  const [error, setError] = useState<string | null>(null)
  const pendingQueueRef = useRef<string[]>([]) // localIds of messages waiting to be sent over WS

  // Internal WS hook — only connects when controlId is set
  const controlSession = useControlSession(controlId)

  // ---------------------------------------------------------------------------
  // Phase transitions driven by controlSession.status
  // ---------------------------------------------------------------------------
  useEffect(() => {
    if (!controlId) return

    const s = controlSession.status
    if (s === 'active' || s === 'waiting_input') {
      setPhase('ready')
    } else if (s === 'reconnecting') {
      setPhase('reconnecting')
    } else if (s === 'completed') {
      setPhase('completed')
    } else if (s === 'error') {
      setPhase('error')
      setError(controlSession.error ?? 'Session error')
    }
  }, [controlId, controlSession.status, controlSession.error])

  // ---------------------------------------------------------------------------
  // Drain pending messages when WS reaches waiting_input
  // ---------------------------------------------------------------------------
  useEffect(() => {
    if (phase !== 'ready' || controlSession.status !== 'waiting_input') return
    if (pendingQueueRef.current.length === 0) return

    const localId = pendingQueueRef.current.shift()!
    setMessages((prev) =>
      prev.map((m) => (m.localId === localId ? { ...m, status: 'sending' as MessageStatus } : m)),
    )

    // Find the message content and send it
    setMessages((prev) => {
      const msg = prev.find((m) => m.localId === localId)
      if (msg?.content) {
        controlSession.sendMessage(msg.content)
      }
      return prev
    })
  }, [phase, controlSession.status, controlSession.sendMessage])

  // ---------------------------------------------------------------------------
  // Mark optimistic/sending messages as "sent" when assistant starts responding
  // ---------------------------------------------------------------------------
  useEffect(() => {
    if (controlSession.streamingContent.length > 0) {
      setMessages((prev) =>
        prev.map((m) =>
          m.role === 'user' && (m.status === 'optimistic' || m.status === 'sending')
            ? { ...m, status: 'sent' as MessageStatus }
            : m,
        ),
      )
    }
  }, [controlSession.streamingContent])

  // ---------------------------------------------------------------------------
  // Connection health
  // ---------------------------------------------------------------------------
  const connectionHealth: ConnectionHealth =
    phase === 'reconnecting'
      ? 'degraded'
      : controlSession.status === 'error' || controlSession.status === 'disconnected'
        ? controlId
          ? 'lost'
          : 'ok' // no controlId = idle, not lost
        : 'ok'

  // ---------------------------------------------------------------------------
  // send()
  // ---------------------------------------------------------------------------
  const send = useCallback(
    (text: string) => {
      const localId = nextLocalId()
      const optimisticMsg: ChatMessageWithStatus = {
        role: 'user',
        content: text,
        localId,
        status: 'optimistic',
        createdAt: Date.now(),
      }
      setMessages((prev) => [...prev, optimisticMsg])
      setError(null)

      if (phase === 'idle' || phase === 'error') {
        // Need to resume first
        pendingQueueRef.current.push(localId)
        setPhase('resuming')

        fetch('/api/control/resume', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ sessionId }),
        })
          .then(async (res) => {
            if (!res.ok) throw new Error(`Resume failed: ${res.status}`)
            const data = await res.json()
            setControlId(data.controlId)
            setPhase('connecting')
          })
          .catch(() => {
            setPhase('error')
            setError('Failed to resume session')
            setMessages((prev) =>
              prev.map((m) => (m.localId === localId ? { ...m, status: 'failed' as MessageStatus } : m)),
            )
            pendingQueueRef.current = pendingQueueRef.current.filter((id) => id !== localId)
          })
      } else if (phase === 'ready' && controlSession.status === 'waiting_input') {
        // Already connected — send immediately
        setMessages((prev) =>
          prev.map((m) => (m.localId === localId ? { ...m, status: 'sending' as MessageStatus } : m)),
        )
        controlSession.sendMessage(text)
      } else {
        // Queue for later drain
        pendingQueueRef.current.push(localId)
      }
    },
    [phase, sessionId, controlSession.status, controlSession.sendMessage],
  )

  // ---------------------------------------------------------------------------
  // retry()
  // ---------------------------------------------------------------------------
  const retry = useCallback(
    (localId: string) => {
      setMessages((prev) => {
        const msg = prev.find((m) => m.localId === localId)
        if (!msg || msg.status !== 'failed') return prev
        // Re-send the message
        if (msg.content) {
          // This will trigger a new send flow
          setTimeout(() => send(msg.content!), 0)
        }
        // Remove the failed message — send() will create a new optimistic one
        return prev.filter((m) => m.localId !== localId)
      })
    },
    [send],
  )

  // ---------------------------------------------------------------------------
  // Derived state
  // ---------------------------------------------------------------------------
  const inputBarState = phaseToInputBarState(phase, controlSession.status)

  return {
    phase,
    inputBarState,
    messages,
    connectionHealth,
    streamingContent: controlSession.streamingContent,
    contextPercent: Math.round(controlSession.contextUsage),
    sessionCost: controlSession.sessionCost,
    lastTurnCost: controlSession.lastTurnCost,
    permissionRequest: controlSession.permissionRequest,
    askQuestion: controlSession.askQuestion,
    planApproval: controlSession.planApproval,
    elicitation: controlSession.elicitation,
    error,
    send,
    retry,
    respondPermission: controlSession.respondPermission,
    answerQuestion: controlSession.answerQuestion,
    approvePlan: controlSession.approvePlan,
    submitElicitation: controlSession.submitElicitation,
  }
}
```

**Step 4: Run test to verify it passes**

Run: `cd apps/web && bunx vitest run src/hooks/use-session-control.test.ts 2>&1 | tail -20`
Expected: 3 tests PASS

**Step 5: Verify TypeScript compiles**

Run: `cd apps/web && bunx tsc --noEmit --pretty 2>&1 | head -20`
Expected: No errors

**Step 6: Commit**

```bash
git add apps/web/src/hooks/use-session-control.ts apps/web/src/hooks/use-session-control.test.ts
git commit -m "feat: add useSessionControl hook with phase machine and message lifecycle"
```

---

### Task 3: Create `ConnectionBanner` component

**Files:**
- Create: `apps/web/src/components/chat/ConnectionBanner.tsx`

**Step 1: Write the component**

```typescript
// apps/web/src/components/chat/ConnectionBanner.tsx
import { RefreshCw, WifiOff } from 'lucide-react'
import type { ConnectionHealth } from '../../hooks/use-session-control'
import { cn } from '../../lib/utils'

interface ConnectionBannerProps {
  health: ConnectionHealth
  onRetry?: () => void
}

export function ConnectionBanner({ health, onRetry }: ConnectionBannerProps) {
  if (health === 'ok') return null

  const isDegraded = health === 'degraded'

  return (
    <div
      role="status"
      className={cn(
        'flex items-center gap-2 px-4 py-2 text-xs font-medium',
        isDegraded
          ? 'bg-amber-50 text-amber-800 dark:bg-amber-950/50 dark:text-amber-300'
          : 'bg-red-50 text-red-800 dark:bg-red-950/50 dark:text-red-300',
      )}
    >
      {isDegraded ? (
        <RefreshCw className="w-3.5 h-3.5 animate-spin" />
      ) : (
        <WifiOff className="w-3.5 h-3.5" />
      )}
      <span>{isDegraded ? 'Reconnecting...' : 'Connection lost'}</span>
      {!isDegraded && onRetry && (
        <button
          type="button"
          onClick={onRetry}
          className="ml-auto text-xs underline hover:no-underline cursor-pointer"
        >
          Retry
        </button>
      )}
    </div>
  )
}
```

**Step 2: Verify TypeScript compiles**

Run: `cd apps/web && bunx tsc --noEmit --pretty 2>&1 | head -20`
Expected: No errors

**Step 3: Commit**

```bash
git add apps/web/src/components/chat/ConnectionBanner.tsx
git commit -m "feat: add ConnectionBanner component for degraded/lost states"
```

---

### Task 4: Refactor `ConversationView.tsx` to use `useSessionControl`

This is the largest change. We remove the ad-hoc resume flow and replace it with the unified hook.

**Files:**
- Modify: `apps/web/src/components/ConversationView.tsx`

**Step 1: Replace imports**

Remove these imports (lines 17, 48):
```typescript
// REMOVE:
import { useControlSession } from '../hooks/use-control-session'
import { ChatInputBar, type InputBarState } from './chat/ChatInputBar'
```

Add these imports:
```typescript
import { useSessionControl } from '../hooks/use-session-control'
import { ChatInputBar } from './chat/ChatInputBar'
import { ConnectionBanner } from './chat/ConnectionBanner'
```

**Step 2: Delete the duplicated `controlStatusToInputState` function**

Remove lines 95-111 entirely (the `controlStatusToInputState` function).

**Step 3: Replace ad-hoc state with `useSessionControl`**

In the `ConversationView` component body (starting around line 126), replace:

```typescript
// REMOVE these lines (126-128):
const [controlId, setControlId] = useState<string | null>(null)
const pendingMessageRef = useRef<string | null>(null)
const controlSession = useControlSession(controlId)
```

With:

```typescript
const sessionControl = useSessionControl(sessionId || '')
```

**Step 4: Delete `handleChatSend` and drain effect**

Remove the `handleChatSend` callback (lines 254-276) and the drain effect (lines 278-285).

**Step 5: Update ChatInputBar usage**

Replace the ChatInputBar at line 847-852:

```typescript
// BEFORE:
<ChatInputBar
  onSend={handleChatSend}
  state={controlStatusToInputState(controlSession.status)}
  contextPercent={Math.round(controlSession.contextUsage)}
  placeholder={controlId ? 'Send a message...' : 'Resume this session...'}
/>
```

With:

```typescript
<ConnectionBanner health={sessionControl.connectionHealth} />
<ChatInputBar
  onSend={sessionControl.send}
  state={sessionControl.inputBarState}
  contextPercent={sessionControl.contextPercent}
  placeholder={sessionControl.phase === 'idle' ? 'Resume this session...' : 'Send a message...'}
/>
```

**Step 6: Also remove `controlId` from the `useState` import**

Check that `useState` is still needed (it is — other state uses it). Just verify no references to `controlId`, `pendingMessageRef`, or `controlSession` remain in the file. Search for them:

Run: `cd apps/web && grep -n 'controlId\|pendingMessageRef\|controlSession\|controlStatusToInputState' src/components/ConversationView.tsx`
Expected: No matches

**Step 7: Verify TypeScript compiles**

Run: `cd apps/web && bunx tsc --noEmit --pretty 2>&1 | head -20`
Expected: No errors

**Step 8: Commit**

```bash
git add apps/web/src/components/ConversationView.tsx
git commit -m "refactor: replace ad-hoc resume state with useSessionControl in ConversationView"
```

---

### Task 5: Refactor `SessionDetailPanel.tsx` to use `useSessionControl`

**Files:**
- Modify: `apps/web/src/components/live/SessionDetailPanel.tsx`

**Step 1: Replace imports**

Remove (line 18):
```typescript
import { useControlSession } from '../../hooks/use-control-session'
```

Add:
```typescript
import { useSessionControl } from '../../hooks/use-session-control'
import { ConnectionBanner } from '../chat/ConnectionBanner'
```

**Step 2: Delete the duplicated `controlStatusToInputState` function**

Remove lines 82-98 entirely.

**Step 3: Replace `useControlSession` usage in the component**

In the component body (line 154), replace:

```typescript
// REMOVE:
const controlSession = useControlSession(controlId ?? null)
```

With:

```typescript
const sessionControl = useSessionControl(data.id)
```

Note: `useControlCallbacks` (line 155-158) still needs `sendRaw` and `respondPermission`. These come from `controlSession` which is now internal to `useSessionControl`. We need to either:
- Expose `sendRaw` from `useSessionControl`, OR
- Move `controlCallbacks` into the hook

The cleanest approach: add `controlCallbacks` to the hook's return value. But that's scope creep for now. Instead, since `SessionDetailPanel` uses `controlId` prop (only set for live sessions), and live sessions are already managed differently, we can keep `useControlCallbacks` using the exposed action methods.

Actually, looking at the code more carefully: `SessionDetailPanel` uses `controlId` prop for **live monitor** sessions (not history resume). The live monitor already has a `controlId` from the server. So for this panel, we should keep `useControlSession(controlId)` as-is for the live case, and only use `useSessionControl` for the history case.

**Revised approach:** Don't change `SessionDetailPanel`'s live path. Only delete the duplicated `controlStatusToInputState` and replace it with the one from the hook:

Replace the `controlStatusToInputState` call at line 637:

```typescript
// BEFORE:
state={controlStatusToInputState(controlSession.status)}

// AFTER (import phaseToInputBarState or just inline the mapping):
```

Actually, the simplest correct approach: extract `controlStatusToInputState` into a shared util and import it in both places. This eliminates duplication without requiring the full hook refactor for the live panel.

**Revised Step 2:** Create shared util instead.

Create `apps/web/src/lib/control-status-map.ts`:

```typescript
import type { InputBarState } from '../components/chat/ChatInputBar'

/** Map control session status string to InputBarState. Shared by all consumers. */
export function controlStatusToInputState(status: string | undefined): InputBarState {
  switch (status) {
    case 'active':
    case 'waiting_input':
      return 'active'
    case 'waiting_permission':
      return 'waiting_permission'
    case 'connecting':
      return 'connecting'
    case 'reconnecting':
      return 'reconnecting'
    case 'completed':
      return 'completed'
    default:
      return 'dormant'
  }
}
```

Then in `SessionDetailPanel.tsx`:
- Remove the local `controlStatusToInputState` (lines 82-98)
- Add import: `import { controlStatusToInputState } from '../../lib/control-status-map'`

**Step 3: Verify TypeScript compiles**

Run: `cd apps/web && bunx tsc --noEmit --pretty 2>&1 | head -20`
Expected: No errors

**Step 4: Commit**

```bash
git add apps/web/src/lib/control-status-map.ts apps/web/src/components/live/SessionDetailPanel.tsx
git commit -m "refactor: extract controlStatusToInputState into shared util, remove duplication"
```

---

### Task 6: Run full test suite and verify

**Step 1: Run all web tests**

Run: `cd apps/web && bunx vitest run 2>&1 | tail -30`
Expected: All tests pass

**Step 2: TypeScript check**

Run: `cd apps/web && bunx tsc --noEmit --pretty 2>&1 | head -20`
Expected: No errors

**Step 3: Build frontend**

Run: `cd apps/web && bun run build 2>&1 | tail -10`
Expected: Build succeeds

**Step 4: Manual smoke test**

1. Start dev server: `bun dev` (in repo root)
2. Open a history session in browser: `http://localhost:5173/sessions/<any-session-id>`
3. Verify input bar shows "Resume this session..." and is enabled (not disabled)
4. Type "hello" — verify message appears immediately as optimistic (faded)
5. If sidecar is running, verify resume completes and assistant responds
6. If sidecar is NOT running, verify message shows as "failed" (not silently lost)

**Step 5: Final commit if any fixes needed**

```bash
git add -A && git commit -m "fix: address test/build issues from session control refactor"
```
