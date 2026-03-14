# V1 Frontend Control Wiring — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire all 13 V1 control methods from the sidecar WS handler into the frontend via a SessionChannel transport abstraction, with capability negotiation and per-request correlation.

**Architecture:** `SessionChannel` (send/request with requestId) sits between the WS connection and `useSessionActions`. The sidecar echoes `requestId` on responses and advertises `capabilities` in `session_init`. UI components gate on capabilities and use `useWsQuery` for reactive data.

**Tech Stack:** TypeScript (React, Vitest), existing sidecar (small modifications)

**Spec:** `docs/superpowers/specs/2026-03-14-v1-frontend-control-wiring-design.md`

---

## File Structure

### New files
| File | Responsibility |
|------|----------------|
| `apps/web/src/lib/session-channel.ts` | Promise-based WS transport: `send()` + `request()` with requestId |
| `apps/web/src/lib/session-channel.test.ts` | Unit tests for SessionChannel |
| `apps/web/src/hooks/use-ws-query.ts` | Generic reactive hook wrapping `channel.request()` |
| `apps/web/src/components/chat/ThinkingBudgetControl.tsx` | Thinking token budget dropdown |
| `apps/web/src/components/chat/McpPanel.tsx` | MCP server management panel |
| `apps/web/src/components/conversation/RewindButton.tsx` | Per-message undo with dry-run preview |
| `apps/web/src/components/chat/AccountInfoPanel.tsx` | Account usage/plan info display |

### Modified files
| File | Change |
|------|--------|
| `sidecar/src/protocol.ts` | Add `requestId?` to response types, `capabilities?` to SessionInit |
| `sidecar/src/event-mapper.ts` | Add `capabilities` array to session_init |
| `sidecar/src/ws-handler.ts` | Echo `requestId` in 7 response-bearing cases |
| `packages/shared/src/types/sidecar-protocol.ts` | Mirror protocol.ts changes |
| `apps/web/src/hooks/use-session-actions.ts` | Add 13 methods via channel |
| `apps/web/src/hooks/use-session-source.ts` | Route response events to channel |
| `apps/web/src/pages/ChatPage.tsx` | Pass onStop, wire model/thinking controls |
| `apps/web/src/components/chat/ModelSelector.tsx` | Use set_model when live |

---

## Chunk 1: Sidecar protocol changes (requestId + capabilities)

Small sidecar modifications that enable the frontend wiring. Must be done first.

### Task 1: Protocol types — requestId + capabilities

**Files:**
- Modify: `sidecar/src/protocol.ts`
- Modify: `packages/shared/src/types/sidecar-protocol.ts`

- [ ] **Step 1: Add `requestId` to response types in sidecar protocol.ts**

Find `QueryResult`, `RewindResult`, `McpSetResult` interfaces (~line 475-487). Add `requestId?: string` to each:

```typescript
export interface QueryResult { type: 'query_result'; queryType: string; data: unknown; requestId?: string }
export interface RewindResult { type: 'rewind_result'; result: unknown; requestId?: string }
export interface McpSetResult { type: 'mcp_set_result'; result: unknown; requestId?: string }
```

- [ ] **Step 2: Add `capabilities` to SessionInit in sidecar protocol.ts**

Find `SessionInit` interface (~line 120). Add after `outputStyle`:

```typescript
capabilities?: string[]
```

- [ ] **Step 3: Add `requestId` to client message types that need request/response**

Find `QueryModelsMsg`, `QueryCommandsMsg`, `QueryAgentsMsg`, `QueryMcpStatusMsg`, `QueryAccountInfoMsg`, `SetMcpServersMsg`, `RewindFilesMsg`. Add `requestId?: string` to each.

- [ ] **Step 4: Mirror changes in shared sidecar-protocol.ts**

Apply the same 3 changes to `packages/shared/src/types/sidecar-protocol.ts`: `requestId` on response types, `capabilities` on SessionInit, `requestId` on client message types.

- [ ] **Step 5: Run typecheck**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view/.worktrees/v1-streaming-migration && npx turbo typecheck`
Expected: ALL PASS

- [ ] **Step 6: Commit**

```bash
cd /Users/TBGor/dev/@vicky-ai/claude-view/.worktrees/v1-streaming-migration
git add sidecar/src/protocol.ts packages/shared/src/types/sidecar-protocol.ts
git commit -m "feat: add requestId to response types + capabilities to SessionInit"
```

### Task 2: Sidecar — echo requestId in WS handler

**Files:**
- Modify: `sidecar/src/ws-handler.ts`

- [ ] **Step 1: Read ws-handler.ts**

Read `sidecar/src/ws-handler.ts` to find all 7 response-bearing cases.

- [ ] **Step 2: Echo requestId in all query_result responses**

For each of the 5 `query_*` cases, add `requestId: msg.requestId` to the response JSON. Example for `query_models` (~line 142-148):

```typescript
case 'query_models':
  try {
    const models = await session.query.supportedModels()
    ws.send(JSON.stringify({ type: 'query_result', queryType: 'models', data: models, requestId: msg.requestId }))
  } catch (err) { sendError(ws, err) }
  break
```

Apply same pattern to: `query_commands`, `query_agents`, `query_mcp_status`, `query_account_info`.

- [ ] **Step 3: Echo requestId in mutation responses**

For `set_mcp_servers` (~line 203-209) and `rewind_files` (~line 212-220):

```typescript
ws.send(JSON.stringify({ type: 'mcp_set_result', result, requestId: msg.requestId }))
ws.send(JSON.stringify({ type: 'rewind_result', result, requestId: msg.requestId }))
```

- [ ] **Step 4: Run typecheck**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view/.worktrees/v1-streaming-migration/sidecar && npx tsc --noEmit`

- [ ] **Step 5: Commit**

```bash
git add sidecar/src/ws-handler.ts
git commit -m "feat: echo requestId in all query/mutation WS responses"
```

### Task 3: Sidecar — capabilities in session_init

**Files:**
- Modify: `sidecar/src/event-mapper.ts`

- [ ] **Step 1: Read event-mapper.ts case 'init'**

Find the `case 'init':` block in `mapSystem()` (~line 241-257).

- [ ] **Step 2: Add capabilities array**

Add `capabilities` field to the returned SessionInit object, after `outputStyle`:

```typescript
capabilities: [
  'interrupt', 'set_model', 'set_max_thinking_tokens', 'stop_task',
  'query_models', 'query_commands', 'query_agents', 'query_mcp_status',
  'query_account_info', 'reconnect_mcp', 'toggle_mcp', 'set_mcp_servers', 'rewind_files',
],
```

- [ ] **Step 3: Run typecheck + existing tests**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view/.worktrees/v1-streaming-migration/sidecar && npx tsc --noEmit && npx vitest run src/event-mapper.test.ts`

- [ ] **Step 4: Add test for capabilities in session_init**

Add to `sidecar/src/event-mapper.test.ts`:

```typescript
it('session_init includes capabilities array', () => {
  const msg = {
    type: 'system',
    subtype: 'init',
    tools: [],
    model: 'claude-sonnet-4-20250514',
    mcp_servers: [],
    permissionMode: 'default',
    slash_commands: [],
    claude_code_version: '1.0.0',
    cwd: '/tmp',
    agents: [],
    skills: [],
    output_style: '',
    session_id: 'sess-1',
  }
  const events = mapSdkMessage(msg as any)
  const init = events[0] as any
  expect(init.capabilities).toContain('interrupt')
  expect(init.capabilities).toContain('rewind_files')
  expect(init.capabilities).toHaveLength(13)
})
```

- [ ] **Step 5: Run tests**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view/.worktrees/v1-streaming-migration/sidecar && npx vitest run src/event-mapper.test.ts`
Expected: ALL PASS

- [ ] **Step 6: Commit**

```bash
git add sidecar/src/event-mapper.ts sidecar/src/event-mapper.test.ts
git commit -m "feat: add capabilities array to session_init event"
```

---

## Chunk 2: SessionChannel transport + response routing

### Task 4: SessionChannel — failing tests

**Files:**
- Create: `apps/web/src/lib/session-channel.test.ts`

- [ ] **Step 1: Write failing tests**

```typescript
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { SessionChannel } from './session-channel'

describe('SessionChannel', () => {
  let channel: SessionChannel
  let mockSend: ReturnType<typeof vi.fn>

  beforeEach(() => {
    mockSend = vi.fn()
    channel = new SessionChannel(mockSend)
  })

  describe('send()', () => {
    it('sends message via the send function', () => {
      channel.send({ type: 'interrupt' })
      expect(mockSend).toHaveBeenCalledWith({ type: 'interrupt' })
    })

    it('does nothing when send function is null', () => {
      const ch = new SessionChannel(null)
      expect(() => ch.send({ type: 'interrupt' })).not.toThrow()
    })
  })

  describe('request()', () => {
    it('sends message with requestId and resolves on matching response', async () => {
      const promise = channel.request<string[]>({ type: 'query_models' })

      // Extract the requestId from the sent message
      const sentMsg = mockSend.mock.calls[0][0]
      expect(sentMsg.type).toBe('query_models')
      expect(sentMsg.requestId).toBeDefined()

      // Simulate response
      channel.handleResponse(sentMsg.requestId, ['model-a', 'model-b'])
      const result = await promise
      expect(result).toEqual(['model-a', 'model-b'])
    })

    it('rejects on timeout', async () => {
      const promise = channel.request({ type: 'query_models' }, 50)
      await expect(promise).rejects.toThrow('timeout')
    })

    it('concurrent requests with same type get independent responses', async () => {
      const p1 = channel.request<string>({ type: 'query_mcp_status' })
      const p2 = channel.request<string>({ type: 'query_mcp_status' })

      const id1 = mockSend.mock.calls[0][0].requestId
      const id2 = mockSend.mock.calls[1][0].requestId
      expect(id1).not.toBe(id2)

      channel.handleResponse(id2, 'second')
      channel.handleResponse(id1, 'first')

      expect(await p1).toBe('first')
      expect(await p2).toBe('second')
    })

    it('rejects when send function is null', async () => {
      const ch = new SessionChannel(null)
      await expect(ch.request({ type: 'query_models' })).rejects.toThrow('not connected')
    })
  })

  describe('handleDisconnect()', () => {
    it('rejects all pending requests', async () => {
      const p1 = channel.request({ type: 'query_models' })
      const p2 = channel.request({ type: 'query_agents' })

      channel.handleDisconnect()

      await expect(p1).rejects.toThrow('disconnect')
      await expect(p2).rejects.toThrow('disconnect')
    })

    it('clears pending map after disconnect', () => {
      channel.request({ type: 'query_models' }).catch(() => {})
      channel.handleDisconnect()
      // No pending requests — handleResponse is a no-op
      expect(() => channel.handleResponse('nonexistent', {})).not.toThrow()
    })
  })

  describe('updateSend()', () => {
    it('updates the send function for reconnection', () => {
      const newSend = vi.fn()
      channel.updateSend(newSend)
      channel.send({ type: 'interrupt' })
      expect(newSend).toHaveBeenCalled()
      expect(mockSend).toHaveBeenCalledTimes(0)
    })
  })
})
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view/.worktrees/v1-streaming-migration/apps/web && npx vitest run src/lib/session-channel.test.ts`
Expected: FAIL — `Cannot find module './session-channel'`

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/lib/session-channel.test.ts
git commit -m "test: add SessionChannel unit tests (red)"
```

### Task 5: SessionChannel — implementation

**Files:**
- Create: `apps/web/src/lib/session-channel.ts`

- [ ] **Step 1: Implement SessionChannel**

```typescript
type SendFn = (msg: Record<string, unknown>) => void

interface PendingRequest {
  resolve: (data: unknown) => void
  reject: (err: Error) => void
  timer: ReturnType<typeof setTimeout>
}

export class SessionChannel {
  private sendFn: SendFn | null
  private pending = new Map<string, PendingRequest>()

  constructor(sendFn: SendFn | null) {
    this.sendFn = sendFn
  }

  /** Update the send function (e.g., on WS reconnect). */
  updateSend(sendFn: SendFn | null): void {
    this.sendFn = sendFn
  }

  /** Fire-and-forget. No-op if not connected. */
  send(msg: Record<string, unknown>): void {
    this.sendFn?.(msg)
  }

  /**
   * Request/response with per-request requestId correlation.
   * Rejects on timeout or disconnect. No deduplication.
   */
  request<T>(msg: Record<string, unknown>, timeoutMs = 10_000): Promise<T> {
    if (!this.sendFn) {
      return Promise.reject(new Error('SessionChannel not connected'))
    }

    const requestId = crypto.randomUUID()
    return new Promise<T>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(requestId)
        reject(new Error(`Request timeout after ${timeoutMs}ms`))
      }, timeoutMs)

      this.pending.set(requestId, {
        resolve: resolve as (data: unknown) => void,
        reject,
        timer,
      })

      this.sendFn!({ ...msg, requestId })
    })
  }

  /** Resolve a pending request by requestId. Called by the WS event router. */
  handleResponse(requestId: string, data: unknown): void {
    const entry = this.pending.get(requestId)
    if (!entry) return
    clearTimeout(entry.timer)
    this.pending.delete(requestId)
    entry.resolve(data)
  }

  /** Reject all pending requests. Called on WS disconnect. */
  handleDisconnect(): void {
    for (const [id, entry] of this.pending) {
      clearTimeout(entry.timer)
      entry.reject(new Error('SessionChannel disconnect'))
    }
    this.pending.clear()
  }
}
```

- [ ] **Step 2: Run tests**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view/.worktrees/v1-streaming-migration/apps/web && npx vitest run src/lib/session-channel.test.ts`
Expected: ALL PASS

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/lib/session-channel.ts
git commit -m "feat: SessionChannel — request/response transport with requestId correlation"
```

### Task 6: Response routing in use-session-source.ts

**Files:**
- Modify: `apps/web/src/hooks/use-session-source.ts`

- [ ] **Step 1: Read use-session-source.ts fully**

Understand the WS event handler switch statement and where to add new cases.

- [ ] **Step 2: Import SessionChannel and create instance**

Add import at top of file. Create a `channelRef = useRef(new SessionChannel(null))` inside the hook. Update `channelRef.current.updateSend(send)` whenever the `send` function changes.

- [ ] **Step 3: Add response routing cases**

In the event handler switch statement, add:

```typescript
case 'query_result': {
  const evt = event as { requestId?: string; data: unknown }
  if (evt.requestId) channelRef.current.handleResponse(evt.requestId, evt.data)
  break
}
case 'rewind_result': {
  const evt = event as { requestId?: string; result: unknown }
  if (evt.requestId) channelRef.current.handleResponse(evt.requestId, evt.result)
  break
}
case 'mcp_set_result': {
  const evt = event as { requestId?: string; result: unknown }
  if (evt.requestId) channelRef.current.handleResponse(evt.requestId, evt.result)
  break
}
```

- [ ] **Step 4: Call handleDisconnect on WS close**

In the WS `onclose` handler, add: `channelRef.current.handleDisconnect()`

- [ ] **Step 5: Store capabilities from session_init**

In the `session_init` case, extract capabilities:

```typescript
case 'session_init': {
  // ...existing code...
  const caps = (event as any).capabilities as string[] | undefined
  setCapabilities(caps ?? [])
  break
}
```

Add `capabilities` to `SessionSourceResult` and the `useState`.

- [ ] **Step 6: Expose channel in SessionSourceResult**

Add `channel: channelRef.current` and `capabilities` to the returned object so `useSessionActions` can access it.

- [ ] **Step 7: Run typecheck**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view/.worktrees/v1-streaming-migration && npx turbo typecheck`

- [ ] **Step 8: Commit**

```bash
git add apps/web/src/hooks/use-session-source.ts
git commit -m "feat: route query_result/rewind_result/mcp_set_result to SessionChannel"
```

---

## Chunk 3: Session actions + interrupt wiring

### Task 7: Extend useSessionActions with all 13 methods

**Files:**
- Modify: `apps/web/src/hooks/use-session-actions.ts`

- [ ] **Step 1: Read current use-session-actions.ts**

Current file has 6 methods, all fire-and-forget via `send()`.

- [ ] **Step 2: Add channel parameter**

Change the function signature to accept both `send` and `channel`:

```typescript
export function useSessionActions(
  send: ((msg: Record<string, unknown>) => void) | null,
  channel: SessionChannel | null,
): SessionActions {
```

Import `SessionChannel` from `../lib/session-channel`.

- [ ] **Step 3: Add 6 fire-and-forget methods**

```typescript
interrupt: () => send?.({ type: 'interrupt' }),
setModel: (model: string) => send?.({ type: 'set_model', model }),
setMaxThinkingTokens: (tokens: number | null) =>
  send?.({ type: 'set_max_thinking_tokens', maxThinkingTokens: tokens }),
stopTask: (taskId: string) => send?.({ type: 'stop_task', taskId }),
reconnectMcp: (serverName: string) => send?.({ type: 'reconnect_mcp', serverName }),
toggleMcp: (serverName: string, enabled: boolean) =>
  send?.({ type: 'toggle_mcp', serverName, enabled }),
```

- [ ] **Step 4: Add 7 request/response methods**

```typescript
queryModels: () => channel?.request({ type: 'query_models' }) ?? Promise.reject(new Error('No channel')),
queryCommands: () => channel?.request({ type: 'query_commands' }) ?? Promise.reject(new Error('No channel')),
queryAgents: () => channel?.request({ type: 'query_agents' }) ?? Promise.reject(new Error('No channel')),
queryMcpStatus: () => channel?.request({ type: 'query_mcp_status' }) ?? Promise.reject(new Error('No channel')),
queryAccountInfo: () => channel?.request({ type: 'query_account_info' }) ?? Promise.reject(new Error('No channel')),
setMcpServers: (servers: Record<string, unknown>) =>
  channel?.request({ type: 'set_mcp_servers', servers }) ?? Promise.reject(new Error('No channel')),
rewindFiles: (userMessageId: string, opts?: { dryRun?: boolean }) =>
  channel?.request({ type: 'rewind_files', userMessageId, dryRun: opts?.dryRun }) ?? Promise.reject(new Error('No channel')),
```

- [ ] **Step 5: Update SessionActions interface**

Add all 13 new methods to the interface (see spec lines 110-131).

- [ ] **Step 6: Update NOOP_ACTIONS**

Add no-op implementations for all 13 methods. Fire-and-forget methods get `() => {}`. Request/response methods get `() => Promise.reject(new Error('No session'))`.

- [ ] **Step 7: Update callers**

Find all calls to `useSessionActions(send)` and add the channel parameter: `useSessionActions(send, channel)`.

- [ ] **Step 8: Run typecheck**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view/.worktrees/v1-streaming-migration && npx turbo typecheck`

- [ ] **Step 9: Commit**

```bash
git add apps/web/src/hooks/use-session-actions.ts apps/web/src/pages/ChatPage.tsx
git commit -m "feat: extend useSessionActions with all 13 V1 control methods"
```

### Task 8: Wire interrupt — stop button

**Files:**
- Modify: `apps/web/src/pages/ChatPage.tsx`

- [ ] **Step 1: Read ChatPage.tsx**

Find where ChatInputBar is rendered (~line 270-283). Find the `onStop` prop.

- [ ] **Step 2: Pass onStop to ChatInputBar**

Add `onStop={actions.interrupt}` to the ChatInputBar props:

```tsx
<ChatInputBar
  onSend={handleSend}
  onStop={actions.interrupt}
  state={inputBarState}
  // ...rest unchanged
/>
```

- [ ] **Step 3: Run typecheck**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view/.worktrees/v1-streaming-migration && npx turbo typecheck`

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/pages/ChatPage.tsx
git commit -m "feat: wire interrupt to ChatInputBar stop button"
```

---

## Chunk 4: Model + thinking controls

### Task 9: ModelSelector — live set_model

**Files:**
- Modify: `apps/web/src/components/chat/ModelSelector.tsx`
- Modify: `apps/web/src/pages/ChatPage.tsx`

- [ ] **Step 1: Read ModelSelector.tsx**

Understand the current `onModelChange` callback and how it's called from ChatPage.

- [ ] **Step 2: Add isLive prop to ModelSelector**

```typescript
interface ModelSelectorProps {
  // ...existing props
  isLive?: boolean
  onSetModel?: (model: string) => void  // V1 live model switch
}
```

- [ ] **Step 3: Use set_model when live**

In the model change handler inside ModelSelector, check `isLive`:

```typescript
const handleChange = (newModel: string) => {
  if (isLive && onSetModel) {
    onSetModel(newModel)
  } else {
    onModelChange(newModel)  // existing behavior (resume or set for next session)
  }
}
```

- [ ] **Step 4: Wire from ChatPage**

Pass `isLive` and `onSetModel` from ChatPage:

```tsx
<ModelSelector
  isLive={source.isLive}
  onSetModel={actions.setModel}
  // ...existing props
/>
```

- [ ] **Step 5: Run typecheck**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view/.worktrees/v1-streaming-migration && npx turbo typecheck`

- [ ] **Step 6: Commit**

```bash
git add apps/web/src/components/chat/ModelSelector.tsx apps/web/src/pages/ChatPage.tsx
git commit -m "feat: live model switch via V1 set_model when session is active"
```

### Task 10: ThinkingBudgetControl component

**Files:**
- Create: `apps/web/src/components/chat/ThinkingBudgetControl.tsx`

- [ ] **Step 1: Create component**

```tsx
import { useCallback } from 'react'

const PRESETS = [
  { label: 'Default', value: null },
  { label: '1K', value: 1024 },
  { label: '4K', value: 4096 },
  { label: '16K', value: 16384 },
  { label: '64K', value: 65536 },
  { label: 'Max', value: 0 }, // 0 = max available
] as const

interface ThinkingBudgetControlProps {
  value: number | null
  onChange: (tokens: number | null) => void
  disabled?: boolean
}

export function ThinkingBudgetControl({ value, onChange, disabled }: ThinkingBudgetControlProps) {
  const handleChange = useCallback(
    (e: React.ChangeEvent<HTMLSelectElement>) => {
      const v = e.target.value
      onChange(v === 'null' ? null : Number(v))
    },
    [onChange],
  )

  return (
    <select
      value={value === null ? 'null' : String(value)}
      onChange={handleChange}
      disabled={disabled}
      className="text-xs bg-transparent border border-border-secondary rounded px-1.5 py-0.5"
      title="Thinking budget"
    >
      {PRESETS.map((p) => (
        <option key={String(p.value)} value={p.value === null ? 'null' : String(p.value)}>
          {p.label}
        </option>
      ))}
    </select>
  )
}
```

- [ ] **Step 2: Wire from ChatPage**

Add ThinkingBudgetControl near ModelSelector in ChatPage. Only render when capabilities include `set_max_thinking_tokens`:

```tsx
{capabilities.includes('set_max_thinking_tokens') && (
  <ThinkingBudgetControl
    value={thinkingBudget}
    onChange={(tokens) => {
      setThinkingBudget(tokens)
      actions.setMaxThinkingTokens(tokens)
    }}
    disabled={!source.isLive}
  />
)}
```

- [ ] **Step 3: Run typecheck**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view/.worktrees/v1-streaming-migration && npx turbo typecheck`

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/components/chat/ThinkingBudgetControl.tsx apps/web/src/pages/ChatPage.tsx
git commit -m "feat: thinking budget control with V1 set_max_thinking_tokens"
```

---

## Chunk 5: useWsQuery + MCP panel + task cancel

### Task 11: useWsQuery reactive hook

**Files:**
- Create: `apps/web/src/hooks/use-ws-query.ts`

- [ ] **Step 1: Implement hook**

```typescript
import { useCallback, useEffect, useRef, useState } from 'react'

interface WsQueryResult<T> {
  data: T | null
  loading: boolean
  error: Error | null
  refresh: () => void
}

export function useWsQuery<T>(
  queryFn: (() => Promise<T>) | null,
  options?: { autoFetch?: boolean },
): WsQueryResult<T> {
  const [data, setData] = useState<T | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<Error | null>(null)
  const mountedRef = useRef(true)

  useEffect(() => {
    mountedRef.current = true
    return () => { mountedRef.current = false }
  }, [])

  const refresh = useCallback(() => {
    if (!queryFn) return
    setLoading(true)
    setError(null)
    queryFn()
      .then((result) => {
        if (mountedRef.current) {
          setData(result)
          setLoading(false)
        }
      })
      .catch((err) => {
        if (mountedRef.current) {
          setError(err instanceof Error ? err : new Error(String(err)))
          setLoading(false)
        }
      })
  }, [queryFn])

  useEffect(() => {
    if (options?.autoFetch !== false && queryFn) {
      refresh()
    }
  }, [refresh, options?.autoFetch, queryFn])

  return { data, loading, error, refresh }
}
```

- [ ] **Step 2: Run typecheck**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view/.worktrees/v1-streaming-migration && npx turbo typecheck`

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/hooks/use-ws-query.ts
git commit -m "feat: useWsQuery — reactive hook for WS request/response queries"
```

### Task 12: McpPanel component

**Files:**
- Create: `apps/web/src/components/chat/McpPanel.tsx`

- [ ] **Step 1: Read existing MCP data structures**

Check `session_init.mcpServers` type: `{ name: string; status: string }[]`. Read the shared protocol types to understand what `queryMcpStatus` returns.

- [ ] **Step 2: Create McpPanel**

Build the MCP panel with:
- Server list with status badges
- Toggle switch per server
- Reconnect button per server
- 500ms debounced refresh after toggle/reconnect
- Use `useWsQuery(() => actions.queryMcpStatus(), { autoFetch: true })`

Use the existing UI patterns from the codebase (check other panels/dialogs for styling conventions).

- [ ] **Step 3: Wire from ChatPage**

Add a button in the toolbar area that opens McpPanel. Only render when `capabilities.includes('query_mcp_status')`.

- [ ] **Step 4: Run typecheck**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view/.worktrees/v1-streaming-migration && npx turbo typecheck`

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/components/chat/McpPanel.tsx apps/web/src/pages/ChatPage.tsx
git commit -m "feat: MCP server management panel with toggle, reconnect, status"
```

### Task 13: Task progress — stop_task button

**Files:**
- Modify: task progress block component (find in `apps/web/src/components/conversation/blocks/`)

- [ ] **Step 1: Find the task progress rendering component**

Search for where `task_started` or `task_progress` events are rendered. Check `SystemBlock.tsx` or similar.

- [ ] **Step 2: Verify taskId is available**

Check if `taskId` is threaded through from the event data to the rendering component. If not, add it.

- [ ] **Step 3: Add cancel button**

Add a small cancel/stop icon button next to task progress. Calls `actions.stopTask(taskId)`. Only show when `capabilities.includes('stop_task')`.

- [ ] **Step 4: Run typecheck**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view/.worktrees/v1-streaming-migration && npx turbo typecheck`

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/components/conversation/blocks/
git commit -m "feat: stop_task cancel button on task progress cards"
```

---

## Chunk 6: Rewind + account info + slash refresh

### Task 14: RewindButton component

**Files:**
- Create: `apps/web/src/components/conversation/RewindButton.tsx`

- [ ] **Step 1: Create RewindButton**

Hover-reveal undo icon on user message blocks. Two-step flow:

```tsx
import { useCallback, useState } from 'react'
import type { SessionActions } from '../../hooks/use-session-actions'

interface RewindButtonProps {
  userMessageId: string
  actions: SessionActions
}

export function RewindButton({ userMessageId, actions }: RewindButtonProps) {
  const [loading, setLoading] = useState(false)

  const handleRewind = useCallback(async () => {
    setLoading(true)
    try {
      const preview = await actions.rewindFiles(userMessageId, { dryRun: true })
      // Show confirmation with preview data
      const confirmed = window.confirm(`Revert files changed by this message?`)
      if (confirmed) {
        await actions.rewindFiles(userMessageId)
      }
    } catch (err) {
      console.error('Rewind failed:', err)
    } finally {
      setLoading(false)
    }
  }, [userMessageId, actions])

  return (
    <button
      onClick={handleRewind}
      disabled={loading}
      className="opacity-0 group-hover:opacity-100 transition-opacity p-1 rounded hover:bg-bg-secondary"
      title="Undo file changes from this message"
    >
      {loading ? '...' : '↩'}
    </button>
  )
}
```

- [ ] **Step 2: Wire into user message blocks**

Find where user messages are rendered. Add RewindButton as a hover overlay. Only render when `capabilities.includes('rewind_files')`.

- [ ] **Step 3: Run typecheck**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view/.worktrees/v1-streaming-migration && npx turbo typecheck`

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/components/conversation/RewindButton.tsx apps/web/src/components/conversation/blocks/
git commit -m "feat: rewind button with dry-run preview on user messages"
```

### Task 15: AccountInfoPanel

**Files:**
- Create: `apps/web/src/components/chat/AccountInfoPanel.tsx`

- [ ] **Step 1: Create AccountInfoPanel**

Simple panel that fetches account info on open:

```tsx
import { useWsQuery } from '../../hooks/use-ws-query'
import type { SessionActions } from '../../hooks/use-session-actions'

interface AccountInfoPanelProps {
  actions: SessionActions
}

export function AccountInfoPanel({ actions }: AccountInfoPanelProps) {
  const { data, loading, error } = useWsQuery(
    () => actions.queryAccountInfo(),
    { autoFetch: true },
  )

  if (loading) return <div className="p-4 text-text-secondary">Loading...</div>
  if (error) return <div className="p-4 text-text-error">Failed to load account info</div>
  if (!data) return null

  return (
    <div className="p-4 space-y-2 text-sm">
      <h3 className="font-medium">Account Info</h3>
      <pre className="text-xs bg-bg-secondary p-2 rounded overflow-auto max-h-48">
        {JSON.stringify(data, null, 2)}
      </pre>
    </div>
  )
}
```

- [ ] **Step 2: Wire from settings or session info area**

Add AccountInfoPanel to an appropriate location. Only render when `capabilities.includes('query_account_info')`.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/components/chat/AccountInfoPanel.tsx
git commit -m "feat: account info panel via V1 query_account_info"
```

### Task 16: Slash menu — live refresh for commands/agents

**Files:**
- Modify: `apps/web/src/components/chat/ChatPalette.tsx` (or equivalent slash menu)

- [ ] **Step 1: Find the slash menu component**

Read `ChatPalette.tsx` or wherever slash commands are rendered.

- [ ] **Step 2: Add stale data refresh**

On menu open, if commands/agents data is older than 60s, fire `actions.queryCommands()` / `actions.queryAgents()` in background. Store the timestamp of last fetch. Update the autocomplete options when response arrives.

- [ ] **Step 3: Run typecheck**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view/.worktrees/v1-streaming-migration && npx turbo typecheck`

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/components/chat/
git commit -m "feat: live refresh slash commands and agents via WS queries"
```

---

## Chunk 7: Full build verification

### Task 17: Full typecheck + build + tests

- [ ] **Step 1: Run full typecheck**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view/.worktrees/v1-streaming-migration && npx turbo typecheck`
Expected: ALL PASS

- [ ] **Step 2: Run sidecar tests**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view/.worktrees/v1-streaming-migration/sidecar && npx vitest run --exclude src/integration.test.ts`
Expected: ALL PASS

- [ ] **Step 3: Run web tests**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view/.worktrees/v1-streaming-migration/apps/web && npx vitest run`
Expected: ALL PASS

- [ ] **Step 4: Run shared tests**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view/.worktrees/v1-streaming-migration/packages/shared && npx vitest run`
Expected: ALL PASS

- [ ] **Step 5: Run full build**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view/.worktrees/v1-streaming-migration && npx turbo build`
Expected: ALL PASS

- [ ] **Step 6: Final commit (if needed)**

```bash
git add -u && git commit -m "feat: V1 frontend control wiring complete"
```
