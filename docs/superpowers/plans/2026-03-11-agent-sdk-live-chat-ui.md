# Agent SDK Live Chat UI — Implementation Plan

> **Status:** DONE (2026-03-11) — all 12 tasks implemented, shippable audit passed (SHIP IT)

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

## Completion Summary

| Task | Commit | Description |
|------|--------|-------------|
| All 12 tasks (6 chunks) | `dae35d0d` | feat(live-chat): upgrade Agent SDK live chat UI with rich rendering, mode selection, and token usage |

Shippable audit: SHIP IT — 0 plan gaps, all 6 wiring flows verified, 0 blockers, 11/11 typecheck, build success. 15 files changed, 775 insertions.

---

**Goal:** Upgrade the live chat UI to render rich messages (tool cards, thinking, errors), wire mode selection to the SDK, show real context/token usage, and polish interactive cards.

**Architecture:** Hybrid approach — existing RichPane components (PairedToolCard, ThinkingMessage, ErrorMessage) are embedded inside chat-style LiveMessageBubble. Sidecar accumulates tokens from SDK assistant messages and emits real usage. Mode selection sends `set_mode` messages to sidecar which closes and re-resumes the SDK session with the new `permissionMode` (V2 SDK has no `setPermissionMode()` method).

**Tech Stack:** TypeScript (sidecar + React frontend), Agent SDK V2 (`@anthropic-ai/claude-agent-sdk`), Radix UI, Tailwind CSS, Lucide icons.

**Spec:** `docs/superpowers/specs/2026-03-11-agent-sdk-live-chat-ui-design.md`

---

> **Implementation note:** Line numbers in this plan are approximate. The working tree has unstaged changes that may shift lines. **Always use semantic anchors** (function names, interface names, `case` labels) to find the right location — never rely on line numbers alone.

## Chunk 1: Sidecar Protocol Extensions

### Task 1: Add `thinking` and `tool_use_result` message types to sidecar types

**Files:**
- Modify: `sidecar/src/types.ts`

- [ ] **Step 1: Add `ThinkingMessage` server type**

In `sidecar/src/types.ts`, add after the `ToolUseResult` interface:

```typescript
export interface ThinkingMessage {
  type: 'thinking'
  content: string
  messageId: string
}
```

- [ ] **Step 2: Add `SetModeMessage` client type**

Add after the `ResumeMsg` interface:

```typescript
export interface SetModeMessage {
  type: 'set_mode'
  mode: 'default' | 'acceptEdits' | 'bypassPermissions' | 'plan' | 'dontAsk'
}
```

- [ ] **Step 3: Update union types**

Add `ThinkingMessage` to `ServerMessage` union. Add `SetModeMessage` to `ClientMessage` union.

- [ ] **Step 4: Extend `SessionStatusMessage` with token/cost fields**

```typescript
export interface SessionStatusMessage {
  type: 'session_status'
  status: 'active' | 'waiting_input' | 'waiting_permission' | 'completed' | 'error'
  contextUsage: number
  turnCount: number
  tokenUsage?: {
    input: number
    output: number
    cacheRead: number
    cacheCreation: number
  }
  costUsd?: number
  model?: string
  contextWindow?: number
}
```

- [ ] **Step 5: Commit**

```bash
git add sidecar/src/types.ts
git commit -m "feat(sidecar): add thinking, set_mode, and token usage types to protocol"
```

### Task 2: Emit thinking blocks and tool results from session-manager

**Files:**
- Modify: `sidecar/src/session-manager.ts`

- [ ] **Step 1: Add token accumulator fields to `ControlSession` interface**

In `session-manager.ts`, add to the `ControlSession` interface (after the `contextUsage: number` field):

```typescript
  totalInputTokens: number
  totalOutputTokens: number
  cacheReadTokens: number
  cacheCreationTokens: number
  lastTurnInputTokens: number
  permissionMode: import('@anthropic-ai/claude-agent-sdk').PermissionMode | null // persisted for reconnect state
  model: string | null
  sessionContextWindow: number | null // from SDK result.modelUsage
```

- [ ] **Step 2: Initialize new fields in `resume()` method**

In the `cs = { ... }` assignment block inside `resume()`, add:

```typescript
  totalInputTokens: 0,
  totalOutputTokens: 0,
  cacheReadTokens: 0,
  cacheCreationTokens: 0,
  lastTurnInputTokens: 0,
  permissionMode: null, // updated to `permissionMode ?? null` in Task 3 Step 3
  model: null,
  sessionContextWindow: null,
```

- [ ] **Step 3: Import `ThinkingMessage` type**

Update the imports from `./types.js` to include `ThinkingMessage`:

```typescript
import type {
  ActiveSession,
  AskUserQuestionMessage,
  SequencedServerMessage,
  ServerMessage,
  ThinkingMessage,
} from './types.js'
```

- [ ] **Step 4: Emit `thinking` blocks in the `'assistant'` case**

In the `for (const block of assistantMsg.message.content)` loop inside the `case 'assistant'` branch of `sendMessage()`, add a `thinking` check before the `text` check:

```typescript
if (block.type === 'thinking' && 'thinking' in block && block.thinking) {
  this.emitSequenced(cs, {
    type: 'thinking',
    content: block.thinking as string,
    messageId,
  } satisfies ThinkingMessage)
} else if (block.type === 'text' && block.text) {
```

- [ ] **Step 5: Accumulate tokens from assistant message usage**

After the content block loop in the `case 'assistant'` branch, add token accumulation:

```typescript
// Accumulate per-turn tokens from BetaMessage.usage (BetaUsage uses snake_case)
// Note: BetaUsage comes from @anthropic-ai/sdk (API types, snake_case),
// NOT ModelUsage from @anthropic-ai/claude-agent-sdk (SDK types, camelCase).
const usage = assistantMsg.message.usage
if (usage) {
  cs.lastTurnInputTokens = usage.input_tokens ?? 0
  cs.totalInputTokens += usage.input_tokens ?? 0
  cs.totalOutputTokens += usage.output_tokens ?? 0
  cs.cacheReadTokens += usage.cache_read_input_tokens ?? 0
  cs.cacheCreationTokens += usage.cache_creation_input_tokens ?? 0
  // NOTE: BetaMessage does NOT have a .model field — model is only available
  // from SDKResultSuccess.modelUsage keys (extracted in Step 7 result handler).
}
```

- [ ] **Step 6: Emit `tool_use_result` from `'user'` messages**

Replace the no-op `break` in `case 'user'` with:

```typescript
case 'user': {
  // Tool results arrive as user messages with tool_result content blocks
  const userMsg = msg as SDKMessage & { type: 'user' }
  if (userMsg.message?.content && Array.isArray(userMsg.message.content)) {
    for (const block of userMsg.message.content) {
      if (block.type === 'tool_result') {
        this.emitSequenced(cs, {
          type: 'tool_use_result',
          toolUseId: block.tool_use_id ?? '',
          output: typeof block.content === 'string'
            ? block.content
            : JSON.stringify(block.content ?? ''),
          isError: block.is_error ?? false,
        })
      }
    }
  }
  break
}
```

- [ ] **Step 7: Wire real usage into `assistant_done` and `result` handler**

Replace the entire `case 'result'` block in `sendMessage()` with:

```typescript
case 'result': {
  const resultMsg = msg as SDKMessage & { type: 'result' }
  if (resultMsg.subtype === 'success') {
    cs.totalCost = resultMsg.total_cost_usd ?? null
    cs.turnCount = resultMsg.num_turns ?? 0

    // Extract context window from modelUsage (SDK type: Record<string, ModelUsage>)
    if (resultMsg.modelUsage) {
      for (const [model, mu] of Object.entries(resultMsg.modelUsage)) {
        if (mu.contextWindow) {
          cs.sessionContextWindow = mu.contextWindow
          cs.model = model
        }
      }
    }

    // Override accumulated totals with authoritative result usage
    // NonNullableUsage maps from BetaUsage — keys are snake_case
    if (resultMsg.usage) {
      cs.totalInputTokens = resultMsg.usage.input_tokens ?? cs.totalInputTokens
      cs.totalOutputTokens = resultMsg.usage.output_tokens ?? cs.totalOutputTokens
      cs.cacheReadTokens = resultMsg.usage.cache_read_input_tokens ?? cs.cacheReadTokens
      cs.cacheCreationTokens = resultMsg.usage.cache_creation_input_tokens ?? cs.cacheCreationTokens
    }
  }
  cs.status = 'waiting_input'

  // Compute context usage percentage
  const ctxWindow = cs.sessionContextWindow ?? 200_000
  cs.contextUsage = Math.round((cs.lastTurnInputTokens / ctxWindow) * 100)

  this.emitSequenced(cs, {
    type: 'assistant_done',
    messageId,
    usage: {
      inputTokens: cs.totalInputTokens,
      outputTokens: cs.totalOutputTokens,
      cacheReadTokens: cs.cacheReadTokens,
      cacheWriteTokens: cs.cacheCreationTokens,
    },
    cost: null, // SDK V2 does not provide per-turn cost — lastTurnCost will always be null; total cost via tokenUsage is the authoritative source
    totalCost: cs.totalCost,
  })

  this.emitSequenced(cs, {
    type: 'session_status',
    status: cs.status,
    contextUsage: cs.contextUsage,
    turnCount: cs.turnCount,
    tokenUsage: {
      input: cs.totalInputTokens,
      output: cs.totalOutputTokens,
      cacheRead: cs.cacheReadTokens,
      cacheCreation: cs.cacheCreationTokens,
    },
    costUsd: cs.totalCost ?? undefined,
    model: cs.model ?? undefined,
    contextWindow: cs.sessionContextWindow ?? undefined,
  })
  break
}
```

- [ ] **Step 8: Commit**

```bash
git add sidecar/src/session-manager.ts
git commit -m "feat(sidecar): emit thinking blocks, tool results, and real token usage"
```

### Task 3: Handle `set_mode` in ws-handler

**Files:**
- Modify: `sidecar/src/ws-handler.ts`
- Modify: `sidecar/src/session-manager.ts`

- [ ] **Step 1: Add `setMode()` method to SessionManager**

Add after the `resolveElicitation()` method:

```typescript
async setMode(controlId: string, mode: import('@anthropic-ai/claude-agent-sdk').PermissionMode): Promise<boolean> {
  const cs = this.sessions.get(controlId)
  if (!cs) return false

  // CRITICAL: Only allow mode changes when NOT actively streaming.
  // close() during an active for-await loop would trigger the catch block
  // in sendMessage(), emitting a fatal error to the frontend.
  // NOTE: isStreaming is set to true BEFORE the send() call in sendMessage().
  // If isStreaming is false here, no stream is active. The TOCTOU window between
  // isStreaming=false check and close() is safe because both setMode() and
  // sendMessage() run on the same JS event loop tick — there's no interleaving.
  if (cs.isStreaming) {
    this.emitSequenced(cs, {
      type: 'error',
      message: 'Cannot change mode while agent is processing. Wait for the current turn to complete.',
      fatal: false,
    })
    return false
  }

  try {
    // V2 SDK does NOT support mid-session setPermissionMode().
    // setPermissionMode() exists on Query (V1 API), not SDKSession (V2 API).
    // Strategy: close the current SDK session and re-resume with the new mode.
    // unstable_v2_resumeSession() reconnects to the same Claude process,
    // preserving conversation state while applying the new permission mode.
    cs.sdkSession.close()
    // unstable_v2_resumeSession is already imported at module top level
    // NOTE: allowDangerouslySkipPermissions is V1-only (Options type),
    // NOT available in SDKSessionOptions (V2). For V2, just pass
    // permissionMode: 'bypassPermissions' directly — the SDK handles it.
    cs.sdkSession = unstable_v2_resumeSession(cs.sessionId, {
      model: cs.model ?? 'claude-sonnet-4-20250514',
      permissionMode: mode,
      canUseTool: async (toolName, input, { signal }) => {
        return this.handleCanUseTool(cs, toolName, input, signal)
      },
    })
    cs.permissionMode = mode
    return true
  } catch (err) {
    // Emit error to frontend
    this.emitSequenced(cs, {
      type: 'error',
      message: `Failed to set mode: ${err instanceof Error ? err.message : String(err)}`,
      fatal: false,
    })
    return false
  }
}
```

> **V2 SDK Limitation:** `SDKSession` only exposes `send()`, `stream()`, `close()`, and `[Symbol.asyncDispose]()`. The `setPermissionMode()` method exists on `Query` (V1's `query()` API), not on `SDKSession`. The close-and-re-resume strategy works because `unstable_v2_resumeSession()` reconnects to the same running Claude process by session ID, so conversation state is preserved.

- [ ] **Step 2: Handle `set_mode` in ws-handler switch**

In `ws-handler.ts`, add a case in the `switch (msg.type)` block (after the `ping` case):

```typescript
case 'set_mode': {
  // Validate mode enum before passing to session manager — reject malformed WS messages
  const VALID_MODES = new Set(['default', 'acceptEdits', 'bypassPermissions', 'plan', 'dontAsk'])
  if (!VALID_MODES.has(msg.mode)) {
    ws.send(JSON.stringify({ type: 'error', message: `Invalid mode: ${msg.mode}`, fatal: false }))
    break
  }
  // setMode handles errors internally (emits to frontend via emitSequenced),
  // but catch here defensively in case of unexpected throws
  sessions.setMode(controlId, msg.mode).catch(() => {})
  break
}
```

- [ ] **Step 3: Add `permissionMode` to resume options**

In `session-manager.ts` `resume()` method signature, accept optional `permissionMode` parameter:

```typescript
async resume(
  sessionId: string,
  model?: string,
  projectPath?: string,
  permissionMode?: import('@anthropic-ai/claude-agent-sdk').PermissionMode,
): Promise<ControlSession> {
```

And in the `unstable_v2_resumeSession()` call, add:

```typescript
const sdkSession = unstable_v2_resumeSession(sessionId, {
  model: model ?? 'claude-sonnet-4-20250514',
  ...(projectPath ? { cwd: projectPath } : {}),
  ...(permissionMode ? { permissionMode } : {}),
  // allowDangerouslySkipPermissions is V1-only — not needed in SDKSessionOptions
  canUseTool: async (toolName, input, { signal }) => {
    return this.handleCanUseTool(cs, toolName, input, signal)
  },
})
```

Also update the `cs = { ... }` init block's `permissionMode` line (added as `null` in Task 2 Step 2) to use the new parameter:

```typescript
  permissionMode: permissionMode ?? null,
```

- [ ] **Step 4: Commit**

```bash
git add sidecar/src/ws-handler.ts sidecar/src/session-manager.ts
git commit -m "feat(sidecar): handle set_mode message and wire permissionMode to SDK"
```

## Chunk 2: Frontend Type & Hook Updates

### Task 4: Update frontend types to match sidecar protocol

**Files:**
- Modify: `apps/web/src/types/control.ts`

- [ ] **Step 1: Add `ThinkingMsg` type**

After the `ElicitationMsg` interface:

```typescript
export interface ThinkingMsg {
  type: 'thinking'
  content: string
  messageId: string
}
```

- [ ] **Step 2: Add `SetModeClientMsg` type**

After the `ResumeClientMsg` interface:

```typescript
export type PermissionMode = 'default' | 'acceptEdits' | 'bypassPermissions' | 'plan' | 'dontAsk'

export interface SetModeClientMsg {
  type: 'set_mode'
  mode: PermissionMode
}
```

- [ ] **Step 3: Extend `SessionStatusMsg` with token/cost fields**

Update the `SessionStatusMsg` interface:

```typescript
export interface SessionStatusMsg {
  type: 'session_status'
  status: 'active' | 'waiting_input' | 'waiting_permission' | 'completed' | 'error'
  contextUsage: number
  turnCount: number
  tokenUsage?: {
    input: number
    output: number
    cacheRead: number
    cacheCreation: number
  }
  costUsd?: number
  model?: string
  contextWindow?: number
}
```

- [ ] **Step 4: Add `ThinkingMsg` to `ServerMessage` union**

**INSERT** (do NOT replace the entire union) a `| ThinkingMsg` line into the existing `ServerMessage` union, between `ElicitationMsg` and `SessionStatusMsg`. The final union should include ALL existing members plus the new one:

```typescript
export type ServerMessage =
  | AssistantChunk
  | AssistantDone
  | ToolUseStartMsg
  | ToolUseResultMsg
  | PermissionRequestMsg
  | AskUserQuestionMsg
  | PlanApprovalMsg
  | ElicitationMsg
  | ThinkingMsg          // NEW — insert this line
  | SessionStatusMsg
  | ErrorMsg
  | PongMsg
  | HeartbeatConfigMsg   // KEEP — do NOT drop this existing member
```

> **WARNING:** If you replace the entire union instead of inserting one line, you will silently drop `HeartbeatConfigMsg` which breaks WebSocket heartbeat detection. Verify the union has the same member count as before + 1.

- [ ] **Step 5: Add `'thinking'` role to `ChatMessage`**

Update the `ChatMessage` interface:

```typescript
export interface ChatMessage {
  role: 'user' | 'assistant' | 'tool_use' | 'tool_result' | 'thinking'
  content?: string
  messageId?: string
  toolName?: string
  toolInput?: Record<string, unknown>
  toolUseId?: string
  output?: string
  isError?: boolean
  usage?: AssistantDone['usage']
}
```

- [ ] **Step 6: Commit**

```bash
git add apps/web/src/types/control.ts
git commit -m "feat(web): extend control types with thinking, set_mode, and token usage"
```

### Task 5: Wire token usage and thinking into useControlSession

**Files:**
- Modify: `apps/web/src/hooks/use-control-session.ts`

- [ ] **Step 1: Add token/cost fields to `ControlSessionState`**

Update the `ControlSessionState` interface:

```typescript
interface ControlSessionState {
  messages: ChatMessage[]
  streamingContent: string
  streamingMessageId: string
  contextUsage: number
  turnCount: number
  sessionCost: number | null
  lastTurnCost: number | null
  tokenUsage: { input: number; output: number; cacheRead: number; cacheCreation: number } | null
  model: string | null
  contextWindow: number | null
  toolPairMap: Map<string, { toolName: string; toolInput: Record<string, unknown>; result?: { output: string; isError: boolean }; startTime: number }>
  permissionRequest: PermissionRequestMsg | null
  askQuestion: AskUserQuestionMsg | null
  planApproval: PlanApprovalMsg | null
  elicitation: ElicitationMsg | null
  error: string | null
}
```

- [ ] **Step 2: Update `initialUIState` with new fields**

Replace the existing `const initialUIState` with a factory function (prevents shared `Map` reference across resets):

```typescript
function makeInitialUIState(): ControlSessionState {
  return {
    messages: [],
    streamingContent: '',
    streamingMessageId: '',
    contextUsage: 0,
    turnCount: 0,
    sessionCost: null,
    lastTurnCost: null,
    tokenUsage: null,
    model: null,
    contextWindow: null,
    toolPairMap: new Map(), // NOTE: grows unbounded within a session (no eviction). Acceptable for MVP — Map is cleared on session change via makeInitialUIState(). For very long sessions (500+ tool calls), consider capping to last N entries.
    permissionRequest: null,
    askQuestion: null,
    planApproval: null,
    elicitation: null,
    error: null,
  }
}
```

Then update ALL usages of `initialUIState` in the file to `makeInitialUIState()`:
- `useState<ControlSessionState>(initialUIState)` → `useState<ControlSessionState>(makeInitialUIState)`
- `setUI(initialUIState)` → `setUI(makeInitialUIState())`

- [ ] **Step 3: Handle `thinking` messages in `setUI` switch**

In the `setUI((prev) => { switch (msg.type) { ... }})` block, add a case before `session_status`:

```typescript
case 'thinking':
  return {
    ...prev,
    messages: [
      ...prev.messages,
      {
        role: 'thinking',
        content: msg.content,
        messageId: msg.messageId,
      },
    ],
  }
```

Add the same case in BOTH `setUI` blocks (initial connection handler at ~line 161 and reconnect handler at ~line 346).

- [ ] **Step 4: Update `session_status` handler to read new fields**

In **BOTH** `setUI` switch blocks (initial connection handler and reconnect handler — same pattern as Step 3), update the `session_status` case:

```typescript
case 'session_status':
  return {
    ...prev,
    contextUsage: msg.contextUsage,
    turnCount: msg.turnCount,
    tokenUsage: msg.tokenUsage ?? prev.tokenUsage,
    model: msg.model ?? prev.model,
    contextWindow: msg.contextWindow ?? prev.contextWindow,
    sessionCost: msg.costUsd ?? prev.sessionCost,
  }
```

- [ ] **Step 5: Update `tool_use_start` and `tool_use_result` cases to populate `toolPairMap`**

In **BOTH** `setUI` switch blocks (initial connection handler and reconnect handler — same pattern as Steps 3 and 4), replace the `tool_use_start` and `tool_use_result` cases:

```typescript
case 'tool_use_start': {
  const newMap = new Map(prev.toolPairMap)
  newMap.set(msg.toolUseId, { toolName: msg.toolName, toolInput: msg.toolInput, startTime: Date.now() })
  return {
    ...prev,
    toolPairMap: newMap,
    messages: [
      ...prev.messages,
      { role: 'tool_use', toolName: msg.toolName, toolInput: msg.toolInput, toolUseId: msg.toolUseId },
    ],
  }
}

case 'tool_use_result': {
  const newMap = new Map(prev.toolPairMap)
  const existing = newMap.get(msg.toolUseId)
  if (existing) {
    newMap.set(msg.toolUseId, { ...existing, result: { output: msg.output, isError: msg.isError } })
  }
  return {
    ...prev,
    toolPairMap: newMap,
    messages: [
      ...prev.messages,
      { role: 'tool_result', toolUseId: msg.toolUseId, output: msg.output, isError: msg.isError },
    ],
  }
}
```

- [ ] **Step 6: Add `setMode` callback**

First, add `PermissionMode` to the existing import at the top of `use-control-session.ts`:

```typescript
import type {
  AskUserQuestionMsg,
  ChatMessage,
  ElicitationMsg,
  PermissionMode,        // NEW
  PermissionRequestMsg,
  PlanApprovalMsg,
  ServerMessage,
} from '../types/control'
```

Then add after the `submitElicitation` callback:

```typescript
const setMode = useCallback((mode: PermissionMode) => {
  sendRaw({ type: 'set_mode', mode })
}, [sendRaw])
```

- [ ] **Step 7: Return new fields**

Add to the return object:

```typescript
return {
  status,
  messages: ui.messages,
  streamingContent: ui.streamingContent,
  streamingMessageId: ui.streamingMessageId,
  contextUsage: ui.contextUsage,
  turnCount: ui.turnCount,
  sessionCost: ui.sessionCost,
  lastTurnCost: ui.lastTurnCost,
  tokenUsage: ui.tokenUsage,
  model: ui.model,
  contextWindow: ui.contextWindow,
  permissionRequest: ui.permissionRequest,
  askQuestion: ui.askQuestion,
  planApproval: ui.planApproval,
  elicitation: ui.elicitation,
  error: ui.error,
  fatalCode: connState.phase === 'fatal' ? connState.code ?? null : null,
  sendMessage,
  sendRaw,
  respondPermission,
  answerQuestion,
  approvePlan,
  submitElicitation,
  setMode,
  toolPairMap: ui.toolPairMap,
}
```

- [ ] **Step 8: Commit**

```bash
git add apps/web/src/hooks/use-control-session.ts
git commit -m "feat(web): wire thinking, token usage, and setMode into useControlSession"
```

### Task 6: Add `setMode` to useSessionControl

**Files:**
- Modify: `apps/web/src/hooks/use-session-control.ts`

- [ ] **Step 1: Update `UseSessionControlReturn` interface and return object**

The hook accesses `useControlSession` via dot notation (e.g. `controlSession.setMode`), NOT destructuring. Update the `UseSessionControlReturn` interface (at ~line 90) to add:

First, add `PermissionMode` to the existing top-level import from `'../types/control'`:

```typescript
import type { ..., PermissionMode } from '../types/control'
```

Then **APPEND** (do NOT replace) these fields to the existing `UseSessionControlReturn` interface (after `submitElicitation`). Use semantic anchor `submitElicitation` to find the insertion point:

```typescript
  // Add these 5 fields AFTER the existing submitElicitation line:
  setMode: (mode: PermissionMode) => void
  tokenUsage: { input: number; output: number; cacheRead: number; cacheCreation: number } | null
  model: string | null
  contextWindow: number | null
  toolPairMap: Map<string, { toolName: string; toolInput: Record<string, unknown>; result?: { output: string; isError: boolean }; startTime: number }>
```

Then **APPEND** to the existing return object (at ~line 366, after `submitElicitation: controlSession.submitElicitation`):

```typescript
  setMode: controlSession.setMode,
  tokenUsage: controlSession.tokenUsage,
  model: controlSession.model,
  contextWindow: controlSession.contextWindow,
  toolPairMap: controlSession.toolPairMap,
```

- [ ] **Step 2: Commit**

```bash
git add apps/web/src/hooks/use-session-control.ts
git commit -m "feat(web): expose setMode and token fields from useSessionControl"
```

> **Task 6b was merged into Task 5** (Steps 1, 2, 4b, and 6 now include `toolPairMap`). No separate task needed.

## Chunk 3: ModeSwitch Replacement

### Task 7: Replace ModeSwitch with SDK permission modes

**Files:**
- Modify: `apps/web/src/components/chat/ModeSwitch.tsx`

- [ ] **Step 1: Rewrite ModeSwitch component**

Replace the entire file with a new implementation using the 5 SDK permission modes:

```typescript
import * as AlertDialog from '@radix-ui/react-alert-dialog'
import * as Popover from '@radix-ui/react-popover'
import {
  ChevronDown,
  ClipboardList,
  FileEdit,
  Shield,
  ShieldOff,
  SkipForward,
} from 'lucide-react'
import { useState } from 'react'
import { cn } from '../../lib/utils'
import type { PermissionMode } from '../../types/control'

interface ModeSwitchProps {
  mode: PermissionMode
  onModeChange: (mode: PermissionMode) => void
  disabled?: boolean
}

const MODE_CONFIG: Record<
  PermissionMode,
  {
    label: string
    icon: typeof Shield
    description: string
    pillBg: string
    pillText: string
    activeBg: string
    activeText: string
  }
> = {
  default: {
    label: 'Default',
    icon: Shield,
    description: 'Prompts for dangerous operations',
    pillBg: 'bg-blue-100 dark:bg-blue-950/40',
    pillText: 'text-blue-700 dark:text-blue-400',
    activeBg: 'bg-blue-50 dark:bg-blue-950/30',
    activeText: 'text-blue-700 dark:text-blue-300',
  },
  acceptEdits: {
    label: 'Accept Edits',
    icon: FileEdit,
    description: 'Auto-approves file edits',
    pillBg: 'bg-teal-100 dark:bg-teal-950/40',
    pillText: 'text-teal-700 dark:text-teal-400',
    activeBg: 'bg-teal-50 dark:bg-teal-950/30',
    activeText: 'text-teal-700 dark:text-teal-300',
  },
  plan: {
    label: 'Plan',
    icon: ClipboardList,
    description: 'Plan only, no tool execution',
    pillBg: 'bg-amber-100 dark:bg-amber-950/40',
    pillText: 'text-amber-700 dark:text-amber-400',
    activeBg: 'bg-amber-50 dark:bg-amber-950/30',
    activeText: 'text-amber-700 dark:text-amber-300',
  },
  dontAsk: {
    label: 'Skip Dangerous',
    icon: SkipForward,
    description: 'Skips tools that need permission',
    pillBg: 'bg-gray-100 dark:bg-gray-800/60',
    pillText: 'text-gray-600 dark:text-gray-400',
    activeBg: 'bg-gray-50 dark:bg-gray-800/30',
    activeText: 'text-gray-600 dark:text-gray-300',
  },
  bypassPermissions: {
    label: 'Trust All',
    icon: ShieldOff,
    description: 'Auto-approves everything (dangerous)',
    pillBg: 'bg-red-100 dark:bg-red-950/40',
    pillText: 'text-red-700 dark:text-red-400',
    activeBg: 'bg-red-50 dark:bg-red-950/30',
    activeText: 'text-red-700 dark:text-red-300',
  },
}

const MODES: PermissionMode[] = ['default', 'acceptEdits', 'plan', 'dontAsk', 'bypassPermissions']

export function ModeSwitch({ mode, onModeChange, disabled }: ModeSwitchProps) {
  const [open, setOpen] = useState(false)
  const [confirmBypass, setConfirmBypass] = useState(false)
  const config = MODE_CONFIG[mode]
  const Icon = config.icon

  const handleSelect = (m: PermissionMode) => {
    if (m === 'bypassPermissions') {
      setConfirmBypass(true)
      return
    }
    onModeChange(m)
  }

  const handleConfirmBypass = () => {
    setConfirmBypass(false)
    onModeChange('bypassPermissions')
  }

  return (
    <>
      <Popover.Root open={open} onOpenChange={setOpen}>
        <Popover.Trigger asChild>
          <button
            type="button"
            disabled={disabled}
            className={cn(
              'inline-flex items-center gap-1 px-2 py-1 rounded-full text-xs font-medium transition-colors duration-150',
              'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1',
              config.pillBg,
              config.pillText,
              disabled ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer hover:opacity-80',
            )}
            aria-label={`Mode: ${config.label}. Click to change.`}
          >
            <Icon className="w-3 h-3" aria-hidden="true" />
            <span>{config.label}</span>
            <ChevronDown className="w-3 h-3" aria-hidden="true" />
          </button>
        </Popover.Trigger>

        <Popover.Portal>
          <Popover.Content
            side="top"
            sideOffset={6}
            align="start"
            className="z-50 w-56 bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-lg shadow-lg p-1 animate-in fade-in-0 zoom-in-95"
          >
            {MODES.map((m) => {
              const mc = MODE_CONFIG[m]
              const ModeIcon = mc.icon
              const isActive = m === mode
              return (
                <Popover.Close key={m} asChild>
                  <button
                    type="button"
                    onClick={() => handleSelect(m)}
                    className={cn(
                      'flex items-center gap-2 w-full px-3 py-2 text-sm rounded-md transition-colors cursor-pointer',
                      isActive
                        ? `${mc.activeBg} ${mc.activeText} font-medium`
                        : 'text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800',
                    )}
                  >
                    <ModeIcon className="w-4 h-4 shrink-0" aria-hidden="true" />
                    <div className="text-left">
                      <div>{mc.label}</div>
                      <div className="text-[10px] opacity-70">{mc.description}</div>
                    </div>
                  </button>
                </Popover.Close>
              )
            })}
          </Popover.Content>
        </Popover.Portal>
      </Popover.Root>

      {/* Bypass confirmation dialog — uses Radix AlertDialog per CLAUDE.md overlay rule */}
      <AlertDialog.Root open={confirmBypass} onOpenChange={setConfirmBypass}>
        <AlertDialog.Portal>
          <AlertDialog.Overlay className="fixed inset-0 z-50 bg-black/40 animate-in fade-in-0" />
          <AlertDialog.Content className="fixed left-1/2 top-1/2 z-50 -translate-x-1/2 -translate-y-1/2 bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-xl shadow-2xl p-6 max-w-sm mx-4 animate-in fade-in-0 zoom-in-95">
            <AlertDialog.Title className="flex items-center gap-2 mb-3">
              <ShieldOff className="w-5 h-5 text-red-500" />
              <span className="text-sm font-semibold text-gray-900 dark:text-gray-100">
                Enable Trust All Mode?
              </span>
            </AlertDialog.Title>
            <AlertDialog.Description className="text-xs text-gray-600 dark:text-gray-400 mb-4">
              This mode auto-approves ALL tool executions including destructive operations
              like file deletion and command execution. Use only when you fully trust the session.
            </AlertDialog.Description>
            <div className="flex gap-2 justify-end">
              <AlertDialog.Cancel asChild>
                <button
                  type="button"
                  className="px-3 py-1.5 text-xs font-medium text-gray-700 dark:text-gray-300 bg-gray-100 dark:bg-gray-800 rounded-md hover:bg-gray-200 dark:hover:bg-gray-700 transition-colors"
                >
                  Cancel
                </button>
              </AlertDialog.Cancel>
              <AlertDialog.Action asChild>
                <button
                  type="button"
                  onClick={handleConfirmBypass}
                  className="px-3 py-1.5 text-xs font-medium text-white bg-red-600 rounded-md hover:bg-red-700 transition-colors"
                >
                  Enable Trust All
                </button>
              </AlertDialog.Action>
            </div>
          </AlertDialog.Content>
        </AlertDialog.Portal>
      </AlertDialog.Root>
    </>
  )
}
```

- [ ] **Step 2: Update ChatInputBar props type**

In `ChatInputBar.tsx`, update the props interface to use `PermissionMode` instead of `'plan' | 'code' | 'ask'`:

```typescript
import type { PermissionMode } from '../../types/control'

// In the props interface:
mode?: PermissionMode
onModeChange?: (mode: PermissionMode) => void
```

Also update `MODE_COMMANDS` to use the new mode names:
```typescript
const MODE_COMMANDS = new Set(['default', 'acceptEdits', 'plan', 'dontAsk', 'bypassPermissions'])
```

Update the default value in the props destructuring (find `mode = 'code'` in the function parameter list):
```typescript
// Change from:  mode = 'code',
// To:
mode = 'default',
```

Update the `handleSlashSelect` cast (find `onModeChange(cmd.name as` inside the `handleSlashSelect` function):
```typescript
// Change from:  onModeChange(cmd.name as 'plan' | 'code' | 'ask')
// To:
onModeChange(cmd.name as PermissionMode)
```

Also update `apps/web/src/components/chat/commands.ts` — replace the old mode commands with the new SDK permission modes:

```typescript
// Mode commands — replace the old plan/code/ask with SDK permission modes
{ name: 'default', description: 'Default mode — prompts for dangerous operations', category: 'mode' },
{ name: 'acceptEdits', description: 'Auto-approve file edits', category: 'mode' },
{ name: 'plan', description: 'Plan mode — think and plan, no tool execution', category: 'mode' },
{ name: 'dontAsk', description: 'Skip tools that need permission', category: 'mode' },
{ name: 'bypassPermissions', description: 'Trust all — auto-approve everything (dangerous)', category: 'mode' },
```

> Note: `ChatInputBar` does NOT have access to `sessionControl`. The `onModeChange` prop is wired in `ConversationView.tsx` (Step 3 below), which calls both `setChatMode` and `sessionControl.setMode`.

- [ ] **Step 3: Update ConversationView mode state type with localStorage persistence**

In `ConversationView.tsx`, change:

```typescript
const [chatMode, setChatMode] = useState<'plan' | 'code' | 'ask'>('code')
```

to:

```typescript
import type { PermissionMode } from '../types/control'

// Read initial mode from localStorage, default to 'default'
// Validates stored value — old 'code'/'ask' values from pre-SDK mode are rejected gracefully
const VALID_MODES: PermissionMode[] = ['default', 'acceptEdits', 'bypassPermissions', 'plan', 'dontAsk']
const [chatMode, setChatMode] = useState<PermissionMode>(() => {
  if (!sessionId) return 'default'
  try {
    const stored = localStorage.getItem(`claude-view:mode:${sessionId}`)
    return stored && VALID_MODES.includes(stored as PermissionMode)
      ? (stored as PermissionMode)
      : 'default'
  } catch {
    // SecurityError in private browsing — silently fall back
    return 'default'
  }
})

// When mode changes, persist and send to sidecar
const handleModeChange = useCallback((mode: PermissionMode) => {
  setChatMode(mode)
  if (sessionId) localStorage.setItem(`claude-view:mode:${sessionId}`, mode)
  sessionControl.setMode(mode)
}, [sessionId, sessionControl.setMode])

// On WS reconnect/connect, send persisted mode so sidecar initializes correctly.
// The sidecar's resume() already accepts permissionMode (Task 3 Step 3),
// but that only applies when the session is first created. For mode changes
// that happen while disconnected, re-send after WS opens:
// NOTE: `phase` is SessionPhase ('idle'|'connecting'|'ready'|...), NOT ControlStatus.
// `'ready'` = WS connected and session active.
// Uses lastSentModeRef to prevent sending the same mode on rapid reconnect cycles.
const lastSentModeRef = useRef<PermissionMode | null>(null)
useEffect(() => {
  if (sessionControl.phase === 'ready' && chatMode !== 'default' && lastSentModeRef.current !== chatMode) {
    lastSentModeRef.current = chatMode
    sessionControl.setMode(chatMode)
  }
}, [sessionControl.phase, chatMode, sessionControl.setMode])
```

In the JSX where `ChatInputBar` is rendered (at ~line 858), update:
```tsx
// Change from:
onModeChange={setChatMode}
// To:
onModeChange={handleModeChange}
```

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/components/chat/ModeSwitch.tsx apps/web/src/components/chat/ChatInputBar.tsx apps/web/src/components/chat/commands.ts apps/web/src/components/ConversationView.tsx
git commit -m "feat(web): replace plan/code/ask with 5 SDK permission modes"
```

## Chunk 4: Rich Message Rendering

### Task 8: Extract ThinkingMessage and ErrorMessage from RichPane

**Files:**
- Create: `apps/web/src/components/chat/ThinkingBlock.tsx`
- Create: `apps/web/src/components/chat/ErrorBlock.tsx`

> Note: `RichPane.tsx` is NOT modified — its internal `ThinkingMessage`/`ErrorMessage` remain unchanged. These new components are standalone equivalents for the live chat path.

- [ ] **Step 1: Create standalone ThinkingBlock component**

Extract the ThinkingMessage rendering logic from RichPane into a reusable component:

```typescript
// apps/web/src/components/chat/ThinkingBlock.tsx
import { Brain, ChevronDown, ChevronRight } from 'lucide-react'
import { useState } from 'react'

interface ThinkingBlockProps {
  content: string
  defaultExpanded?: boolean
}

export function ThinkingBlock({ content, defaultExpanded = false }: ThinkingBlockProps) {
  const [expanded, setExpanded] = useState(defaultExpanded)
  const preview = content.slice(0, 120).replace(/\n/g, ' ')

  return (
    <div className="border-l-2 border-purple-300 dark:border-purple-700 pl-3 py-1.5">
      <button
        type="button"
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-1.5 text-xs text-purple-600 dark:text-purple-400 hover:text-purple-700 dark:hover:text-purple-300 cursor-pointer"
      >
        <Brain className="w-3.5 h-3.5" />
        <span className="font-medium">Thinking</span>
        {expanded ? <ChevronDown className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />}
      </button>
      {expanded ? (
        <p className="mt-1.5 text-xs text-gray-600 dark:text-gray-400 italic whitespace-pre-wrap">
          {content}
        </p>
      ) : (
        <p className="mt-0.5 text-xs text-gray-500 dark:text-gray-500 truncate">
          {preview}...
        </p>
      )}
    </div>
  )
}
```

- [ ] **Step 2: Create standalone ErrorBlock component**

```typescript
// apps/web/src/components/chat/ErrorBlock.tsx
import { AlertCircle } from 'lucide-react'

interface ErrorBlockProps {
  message: string
}

export function ErrorBlock({ message }: ErrorBlockProps) {
  return (
    <div className="flex items-start gap-2 px-3 py-2 bg-red-50 dark:bg-red-950/20 border border-red-200 dark:border-red-800/40 rounded-md">
      <AlertCircle className="w-4 h-4 text-red-500 dark:text-red-400 shrink-0 mt-0.5" />
      <p className="text-xs text-red-700 dark:text-red-300">{message}</p>
    </div>
  )
}
```

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/components/chat/ThinkingBlock.tsx apps/web/src/components/chat/ErrorBlock.tsx
git commit -m "feat(web): extract ThinkingBlock and ErrorBlock as standalone components"
```

### Task 9: Upgrade LiveMessageBubble to dispatch rich components

**Files:**
- Modify: `apps/web/src/components/chat/LiveMessageBubble.tsx`

- [ ] **Step 1: Rewrite LiveMessageBubble as a dispatcher**

Replace the current implementation with one that dispatches to rich components based on `message.role`:

```typescript
import { AlertCircle, Check, Loader2, RefreshCw, Wrench } from 'lucide-react'
import type { ChatMessageWithStatus } from '../../types/control'
import { cn } from '../../lib/utils'
import { ThinkingBlock } from './ThinkingBlock'
import { ErrorBlock } from './ErrorBlock'

interface LiveMessageBubbleProps {
  message: ChatMessageWithStatus
  onRetry?: (localId: string) => void
  verbose?: boolean
  toolResult?: { output: string; isError: boolean; duration?: number } | null
}

function StatusIndicator({ message, onRetry }: LiveMessageBubbleProps) {
  switch (message.status) {
    case 'optimistic':
      return <span className="inline-block w-1.5 h-1.5 rounded-full bg-gray-400 dark:bg-gray-500 animate-pulse" />
    case 'sending':
      return <Loader2 className="w-3 h-3 text-gray-400 dark:text-gray-500 animate-spin" />
    case 'sent':
      return <Check className="w-3 h-3 text-gray-400 dark:text-gray-500" />
    case 'failed':
      return (
        <span className="inline-flex items-center gap-1.5">
          <span className="text-xs text-red-500 dark:text-red-400">Failed to send</span>
          {onRetry && (
            <button
              type="button"
              onClick={() => onRetry(message.localId)}
              className="inline-flex items-center gap-1 text-xs text-blue-500 hover:text-blue-600 dark:text-blue-400 dark:hover:text-blue-300"
            >
              <RefreshCw className="w-3 h-3" />
              Retry
            </button>
          )}
        </span>
      )
    default:
      return null
  }
}

function formatTime(ts: number | undefined): string {
  if (!ts || ts <= 0) return ''
  return new Date(ts).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
}

export function LiveMessageBubble({ message, onRetry, verbose, toolResult }: LiveMessageBubbleProps) {
  // --- Thinking blocks ---
  if (message.role === 'thinking') {
    if (!verbose) return null // hidden in chat mode
    return (
      <div className="px-4 py-2">
        <ThinkingBlock content={message.content} />
      </div>
    )
  }

  // --- Tool use ---
  if (message.role === 'tool_use') {
    if (!verbose) {
      // Compact: one-line summary
      return (
        <div className="flex items-center gap-2 px-4 py-2 text-xs text-gray-500 dark:text-gray-400">
          <Wrench className="w-3.5 h-3.5" />
          <span className="font-mono">{message.toolName ?? 'tool'}</span>
          {toolResult && (
            <span className={cn(
              'ml-auto',
              toolResult.isError ? 'text-red-500 dark:text-red-400' : 'text-green-500 dark:text-green-400'
            )}>
              {toolResult.isError ? 'error' : 'done'}
            </span>
          )}
        </div>
      )
    }
    // Verbose: show full tool input
    return (
      <div className="px-4 py-2 space-y-1">
        <div className="flex items-center gap-2 text-xs">
          <Wrench className="w-3.5 h-3.5 text-purple-500 dark:text-purple-400" />
          <span className="font-mono font-medium text-purple-700 dark:text-purple-300">
            {message.toolName ?? 'tool'}
          </span>
        </div>
        <pre className="text-[11px] text-gray-600 dark:text-gray-400 font-mono overflow-x-auto max-h-48 whitespace-pre-wrap bg-gray-50 dark:bg-gray-800/50 rounded p-2">
          {JSON.stringify(message.toolInput, null, 2)}
        </pre>
        {toolResult && (
          <div className={cn(
            'border-l-2 pl-2 mt-1',
            toolResult.isError
              ? 'border-red-300 dark:border-red-700'
              : 'border-green-300 dark:border-green-700',
          )}>
            <pre className="text-[11px] text-gray-600 dark:text-gray-400 font-mono overflow-x-auto max-h-48 whitespace-pre-wrap">
              {toolResult.output}
            </pre>
          </div>
        )}
      </div>
    )
  }

  // --- Tool result (standalone, only in verbose) ---
  if (message.role === 'tool_result') {
    if (!verbose) return null
    return (
      <div className="flex items-center gap-2 px-4 py-2 text-xs text-gray-500 dark:text-gray-400">
        {message.isError ? (
          <AlertCircle className="w-3.5 h-3.5 text-red-500 dark:text-red-400" />
        ) : (
          <Check className="w-3.5 h-3.5 text-green-500 dark:text-green-400" />
        )}
        <span className="font-mono truncate max-w-md">{message.output?.slice(0, 200)}</span>
      </div>
    )
  }

  // --- User / Assistant messages ---
  const isUser = message.role === 'user'
  const barColor = isUser
    ? 'bg-blue-500 dark:bg-blue-400'
    : 'bg-orange-500 dark:bg-orange-400'

  return (
    <div
      className={cn(
        'flex gap-3 px-4 py-3',
        message.status === 'failed' && 'border-l-2 border-red-500 dark:border-red-400',
      )}
    >
      <div className={cn('w-0.5 shrink-0 rounded-full', barColor)} />
      <div className="flex-1 min-w-0">
        <p className="text-sm text-gray-900 dark:text-gray-100 whitespace-pre-wrap break-words">
          {message.content}
        </p>
        <div className="flex items-center gap-2 mt-1.5">
          <span className="text-xs text-gray-400 dark:text-gray-500">
            {formatTime(message.createdAt)}
          </span>
          {isUser && <StatusIndicator message={message} onRetry={onRetry} />}
        </div>
      </div>
    </div>
  )
}
```

- [ ] **Step 2: Wire `verbose` and `toolResult` props from ConversationView**

In `ConversationView.tsx`, update the `LiveMessageBubble` render inside the Virtuoso `itemContent` callback (in the `item.kind === 'live'` branch, currently at ~line 786):

```tsx
if (item.kind === 'live') {
  // Skip rendering wrapper for hidden items (thinking/tool_result when !verbose)
  // to avoid empty padding divs in the Virtuoso list
  const isHiddenThinking = !verboseMode && item.message.role === 'thinking'
  const isHiddenToolResult = !verboseMode && item.message.role === 'tool_result'
  if (isHiddenThinking || isHiddenToolResult) return <div />

  const toolResult = item.message.role === 'tool_use' && item.message.toolUseId
    ? sessionControl.toolPairMap.get(item.message.toolUseId)?.result ?? null
    : null
  return (
    <div className="max-w-4xl mx-auto px-6 pb-4">
      <LiveMessageBubble
        message={item.message}
        onRetry={sessionControl.retry}
        verbose={verboseMode}
        toolResult={toolResult ? { output: toolResult.output, isError: toolResult.isError } : null}
      />
    </div>
  )
}
```

This requires `sessionControl.toolPairMap` to be available — which it is via the `UseSessionControlReturn` update in Task 6 Step 1.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/components/chat/LiveMessageBubble.tsx apps/web/src/components/ConversationView.tsx
git commit -m "feat(web): upgrade LiveMessageBubble with rich rendering dispatch"
```

## Chunk 5: Interactive UI Polish

### Task 10: Add InteractionToast for off-screen cards

**Files:**
- Create: `apps/web/src/components/chat/InteractionToast.tsx`
- Modify: `apps/web/src/components/ConversationView.tsx`

- [ ] **Step 1: Create InteractionToast component**

```typescript
// apps/web/src/components/chat/InteractionToast.tsx
import { Bell } from 'lucide-react'

interface InteractionToastProps {
  visible: boolean
  label: string
  onScrollTo: () => void
}

export function InteractionToast({ visible, label, onScrollTo }: InteractionToastProps) {
  if (!visible) return null

  return (
    <div className="absolute bottom-20 left-1/2 -translate-x-1/2 z-40 animate-in slide-in-from-bottom-2 fade-in-0">
      <button
        type="button"
        onClick={onScrollTo}
        className="inline-flex items-center gap-2 px-4 py-2 bg-amber-500 dark:bg-amber-600 text-white text-xs font-medium rounded-full shadow-lg hover:bg-amber-600 dark:hover:bg-amber-700 transition-colors cursor-pointer"
      >
        <Bell className="w-3.5 h-3.5 animate-bounce" />
        {label}
      </button>
    </div>
  )
}
```

- [ ] **Step 2: Wire toast into ConversationView**

In `ConversationView.tsx`:

1. Add state, ref, and import:
```typescript
import { InteractionToast } from './chat/InteractionToast'
import type { VirtuosoHandle } from 'react-virtuoso'

const [isAtBottom, setIsAtBottom] = useState(true)
const virtuosoRef = useRef<VirtuosoHandle>(null)
```

2. Add `ref` and `atBottomStateChange` to the Virtuoso component:
```tsx
<Virtuoso
  ref={virtuosoRef}
  // ... existing props ...
  atBottomStateChange={(atBottom) => setIsAtBottom(atBottom)}
>
```

3. Derive toast visibility and label:
```typescript
const hasInteraction = !!(sessionControl.permissionRequest || sessionControl.askQuestion || sessionControl.planApproval || sessionControl.elicitation)
const showToast = hasInteraction && !isAtBottom
const toastLabel = sessionControl.permissionRequest
  ? 'Permission required — click to view'
  : sessionControl.askQuestion
    ? 'Question requires your answer'
    : sessionControl.planApproval
      ? 'Plan needs approval'
      : 'Input needed'
```

4. Render `InteractionToast` just before `<ConnectionBanner>`:
```tsx
<InteractionToast
  visible={showToast}
  label={toastLabel}
  onScrollTo={() => {
    virtuosoRef.current?.scrollToIndex({
      index: conversationItems.length - 1,
      behavior: 'smooth',
    })
    setIsAtBottom(true)
  }}
/>
<ConnectionBanner ... />
```

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/components/chat/InteractionToast.tsx apps/web/src/components/ConversationView.tsx
git commit -m "feat(web): add InteractionToast for off-screen approval cards"
```

### Task 11: Enhance PermissionCard countdown

**Files:**
- Modify: `apps/web/src/components/chat/cards/PermissionCard.tsx`

- [ ] **Step 1: Add pulse animation at < 10s**

Update the countdown bar section (lines 153-167). Add a CSS class that pulses when `countdown < 10`:

```typescript
{!resolved && (
  <div className="flex items-center gap-2">
    <div className="flex-1 h-1 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
      <div
        className={cn(
          'h-full rounded-full transition-all duration-1000 ease-linear',
          countdown < 10
            ? 'bg-red-500 animate-pulse'
            : 'bg-amber-500',
        )}
        style={{
          width: `${(countdown / totalSeconds) * 100}%`,
        }}
      />
    </div>
    <span className={cn(
      'text-[10px] font-mono tabular-nums w-6 text-right',
      countdown < 10
        ? 'text-red-500 dark:text-red-400 font-bold'
        : 'text-gray-500 dark:text-gray-400',
    )}>
      {countdown}s
    </span>
  </div>
)}
```

- [ ] **Step 2: Import `cn` utility**

Add `import { cn } from '../../../lib/utils'` at the top of the file.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/components/chat/cards/PermissionCard.tsx
git commit -m "feat(web): add pulse animation to permission countdown at <10s"
```

## Chunk 6: End-to-End Verification

### Task 12: Build and verify

- [ ] **Step 1: Run TypeScript type checking**

```bash
cd /Users/TBGor/dev/@vicky-ai/claude-view && bunx turbo typecheck
```

Expected: All workspaces pass.

- [ ] **Step 2: Run frontend tests**

```bash
cd /Users/TBGor/dev/@vicky-ai/claude-view/apps/web && bunx vitest run
```

Expected: All tests pass.

- [ ] **Step 3: Build the frontend**

```bash
cd /Users/TBGor/dev/@vicky-ai/claude-view && bun run build
```

Expected: Build succeeds.

- [ ] **Step 4: Verify sidecar types compile**

```bash
cd /Users/TBGor/dev/@vicky-ai/claude-view/sidecar && npx tsc --noEmit
```

Expected: No type errors.

- [ ] **Step 5: Fix any issues found**

Address any type errors or test failures.

- [ ] **Step 6: Final commit**

```bash
# Stage ONLY files touched by this plan — NEVER use `git add -A` (CLAUDE.md rule)
git add sidecar/src/types.ts sidecar/src/session-manager.ts sidecar/src/ws-handler.ts \
  apps/web/src/types/control.ts \
  apps/web/src/hooks/use-control-session.ts apps/web/src/hooks/use-session-control.ts \
  apps/web/src/components/chat/ModeSwitch.tsx apps/web/src/components/chat/ChatInputBar.tsx \
  apps/web/src/components/chat/commands.ts apps/web/src/components/ConversationView.tsx \
  apps/web/src/components/chat/ThinkingBlock.tsx apps/web/src/components/chat/ErrorBlock.tsx \
  apps/web/src/components/chat/LiveMessageBubble.tsx \
  apps/web/src/components/chat/InteractionToast.tsx \
  apps/web/src/components/chat/cards/PermissionCard.tsx
git commit -m "fix: address build/test issues from live chat UI implementation"
```

## Changelog of Fixes Applied (Audit Round 1)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | Task 2 Step 2: `permissionMode: permissionMode ?? null` references param not yet added to `resume()` | Blocker | Changed to `permissionMode: null` with comment; Task 3 Step 3 updates to use param |
| 2 | Task 6 Step 1: `UseSessionControlReturn` interface never updated with new fields | Blocker | Added explicit interface extension + return object code with `setMode`, `tokenUsage`, `model`, `contextWindow`, `toolPairMap` |
| 3 | Task 7 Step 2: Default `mode = 'code'` invalid in `PermissionMode`; `handleSlashSelect` cast stale | Blocker | Added explicit instructions to change default to `'default'` and update cast to `PermissionMode` |
| 4 | Task 9 Step 2: No code for `verbose` prop or `toolResult` lookup in ConversationView | Blocker | Added full JSX code with `toolPairMap.get()` lookup and both props wired |
| 5 | Task 6b: `toolPairMap` never forwarded through `useSessionControl` | Blocker | Merged into Task 6 Step 1 — `toolPairMap` now in `UseSessionControlReturn` |
| 6 | Task 6b Step 1: `toolPairMap` added after `initialUIState` already finalized | Blocker | Merged Task 6b into Task 5 — `toolPairMap` now in Steps 1, 2, 4b, and 6 |
| 7 | Task 5 Step 5: `setMode` param typed `string` instead of `PermissionMode` | Warning | Changed to `PermissionMode` with import |
| 8 | Task 10 Step 2: Toast wiring vague — no actual code | Warning | Added explicit state, Virtuoso callback, derivation logic, and JSX placement |

## Changelog of Fixes Applied (Audit Round 2 — Adversarial Review Score: 72)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | SDK uses camelCase (`inputTokens`, `outputTokens`) not snake_case | Critical | Replaced all `input_tokens`→`inputTokens` etc. throughout plan (NOTE: partially reverted in Round 3 — see below) |
| 2 | `import type` placed inside callback body (Task 2 Step 7) | Critical | Moved `import type { ThinkingMessage }` to module-level import block, added `ThinkingMsg` to frontend imports |
| 3 | `new Map()` in module-level `const initialUIState` creates shared mutable reference | Important | Changed to `makeInitialUIState()` factory function; all call sites updated |
| 4 | `// ... existing fields ...` placeholder in `ControlSessionState` extension (Task 5 Step 1) | Important | Changed to explicit APPEND instruction — keep all existing fields, add new ones after `error` |
| 5 | `lastTurnCost` silently becomes always-null with no documentation | Important | Added inline comment on `cost: null` line documenting that SDK V2 does not provide per-turn cost |

## Changelog of Fixes Applied (Audit Round 3 — Adversarial Review Score: 58→pending)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | `setPermissionMode()` does not exist on `SDKSession` (V2) — only on `Query` (V1) | Blocker | Rewrote `setMode()` to close SDK session and re-resume with new `permissionMode` via `unstable_v2_resumeSession()`. Added V2 limitation documentation. |
| 2 | `BetaUsage` (from `@anthropic-ai/sdk`) uses snake_case (`input_tokens`), not camelCase | Blocker | Reverted Round 2 fix #1: changed all `usage.inputTokens` → `usage.input_tokens`, `usage.outputTokens` → `usage.output_tokens`, `usage.cacheReadInputTokens` → `usage.cache_read_input_tokens`, `usage.cacheCreationInputTokens` → `usage.cache_creation_input_tokens` in Task 2 Steps 5 and 7. Added clarifying comments distinguishing `BetaUsage` (snake_case, from `@anthropic-ai/sdk`) vs `ModelUsage` (camelCase, from `@anthropic-ai/claude-agent-sdk`). |
| 3 | `InteractionToast.onScrollTo` was a no-op | Warning | Added `virtuosoRef` with `VirtuosoHandle` type, wired `ref={virtuosoRef}` to Virtuoso, implemented `scrollToIndex` + `setIsAtBottom(true)` in handler |
| 4 | Dead `ThinkingMsg` import (discriminated union narrowing doesn't need it) | Minor | Removed `ThinkingMsg` from import in Task 5 Step 6 |
| 5 | `resume()` param `permissionMode?: string` too loose | Minor | Changed to `permissionMode?: import('@anthropic-ai/claude-agent-sdk').PermissionMode`, removed cast at call site |
| 6 | `setMode` ws-handler `.catch()` was dead code (errors handled internally) | Warning | Changed to `.catch(() => {})` with comment explaining internal error handling |
| 7 | Task 5 step numbering: `Step 4b` breaks sequential order | Minor | Renumbered: 4b→5, 5→6, 6→7, 7→8 |

## Changelog of Fixes Applied (Audit Round 4 — Adversarial Review Score: 78→pending)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | `commands.ts` not updated — slash commands `/code` and `/ask` break, new modes have no slash commands | Blocker | Added `commands.ts` update in Task 7 Step 2: replace plan/code/ask mode entries with default/acceptEdits/plan/dontAsk/bypassPermissions. Added to commit. |
| 2 | Race condition: `setMode()` closes SDK session while `sendMessage()` has active stream loop | Blocker | Added `if (cs.isStreaming) return false` guard with user-facing error message at top of `setMode()` |
| 3 | ConversationView import path `../../types/control` wrong (should be `../types/control`) | Warning | Fixed to `../types/control` — ConversationView is at `components/`, not `components/chat/` |
| 4 | Mode not forwarded to sidecar on reconnect — localStorage mode silently ignored | Warning | Added `useEffect` that sends `setMode(chatMode)` when `sessionControl.phase` transitions to `'ready'` (corrected from `'active'` in Round 5 fix #1) |
| 5 | Unnecessary dynamic `import()` in `setMode()` — `unstable_v2_resumeSession` already imported at top | Warning | Replaced with comment noting existing top-level import |
| 6 | `ControlSession.permissionMode: string | null` — should use SDK's `PermissionMode` type | Minor | Changed to `import('@anthropic-ai/claude-agent-sdk').PermissionMode | null` |
| 7 | Hidden thinking/tool_result messages create empty 24px wrapper divs in Virtuoso | Minor | Added early return `<div />` in `itemContent` callback for hidden items before the wrapper div |

## Changelog of Fixes Applied (Audit Round 5 — Adversarial Review Score: 88→92+)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | `sessionControl.phase === 'active'` wrong — `SessionPhase` has no `'active'` value (compile error) | Critical | Changed to `'ready'` with explanatory comment about SessionPhase vs ControlStatus |
| 2 | `useCallback` dependency on entire `sessionControl` object (new object every render) | Important | Changed to `sessionControl.setMode` (stable useCallback ref) |
| 3 | `useEffect` deps incomplete — `chatMode` captured stale via closure | Important | Changed deps from `[sessionControl.phase]` to `[sessionControl.phase, chatMode, sessionControl.setMode]`, removed eslint-disable-line |
| 4 | Toast `hasInteraction` missing `elicitation` case | Minor | Added `sessionControl.elicitation` to check, added `'Input needed'` label |

## Changelog of Fixes Applied (Round 6 — Cross-agent findings)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | `assistantMsg.message.model` does not exist on `BetaMessage` — Anthropic API doesn't echo model per-message | Blocker | Removed `if (assistantMsg.message.model)` block from Task 2 Step 5. Model is only extracted from `SDKResultSuccess.modelUsage` keys in Step 7 (already correct). |
| 2 | `allowDangerouslySkipPermissions` is V1-only (`Options` type), not in `SDKSessionOptions` (V2) | Blocker | Removed from both `setMode()` (Task 3 Step 1) and `resume()` (Task 3 Step 3). V2 only needs `permissionMode: 'bypassPermissions'`. |
| 3 | JSX diff for `onModeChange` prop not shown explicitly | Warning | Added explicit diff: `onModeChange={setChatMode}` → `onModeChange={handleModeChange}` in Task 7 Step 3 |

## Changelog of Fixes Applied (Round 7 — 4-agent parallel audit, adversarial review)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | Architecture description says `setPermissionMode()` but V2 uses close-and-re-resume | Minor | Updated plan header to describe close-and-re-resume strategy |
| 2 | ws-handler `set_mode` case has no mode enum validation — malformed WS messages pass through | Blocker | Added `VALID_MODES` Set check + error response before calling `sessions.setMode()` in Task 3 Step 2 |
| 3 | Task 4 Step 4: `ServerMessage` union block replaces entire union, silently drops `HeartbeatConfigMsg` | Blocker | Changed to INSERT instruction with `// NEW` and `// KEEP` annotations + warning comment |
| 4 | Task 7 Step 1: hand-rolled `fixed inset-0` modal violates CLAUDE.md Radix overlay rule | Blocker | Replaced with `@radix-ui/react-alert-dialog` (AlertDialog.Root/Portal/Overlay/Content/Title/Description/Cancel/Action) |
| 5 | Task 8 Step 1: `ThinkingBlock.tsx` imports `cn` but never uses it — fails Biome lint | Blocker | Removed unused `cn` import |
| 6 | Task 6 Step 1: inline `import('../types/control').PermissionMode` non-idiomatic | Warning | Changed to top-level `import type { PermissionMode }` with explicit instruction |
| 7 | Task 7 Step 3: localStorage reads unvalidated mode — old `'code'`/`'ask'` values crash sidecar | Warning | Added `VALID_MODES.includes()` check + try/catch for SecurityError |
| 8 | Rapid reconnect sends duplicate `set_mode` messages (3 close/reopen cycles) | Warning | Added `lastSentModeRef` to deduplicate, only sends when mode actually changes |
| 9 | Task 5 Steps 4-5 don't say "update BOTH setUI blocks" (only Step 3 says it) | Warning | Added explicit "BOTH" + "(same pattern as Step 3)" to Steps 4 and 5 |
| 10 | Task 8 "Files" lists `RichPane.tsx` as Modify but no step edits it — orphaned entry | Minor | Removed from Files list, added clarifying note |
| 11 | Round 4 changelog entry #4 says `'active'` but body uses `'ready'` | Minor | Updated changelog entry to note Round 5 correction |
| 12 | Task 12 Step 6 uses `git add -A` — violates CLAUDE.md git discipline | Minor | Replaced with explicit file list of all 15 plan-touched files |
| 13 | Task 7 Step 2 uses approximate line numbers (~88, ~133) | Minor | Replaced with semantic anchor descriptions ("find `mode = 'code'`", "find `onModeChange(cmd.name as`") |
| 14 | `ThinkingMessage` import in session-manager.ts technically unused (no explicit type annotation) | Minor | Added `satisfies ThinkingMessage` to emitted object in Task 2 Step 4 |
| 15 | `toolPairMap` grows unbounded within session — no eviction | Minor | Added comment in `makeInitialUIState()` acknowledging MVP-acceptable growth with future cap suggestion |
| 16 | `setMode()` TOCTOU claim debunked — `isStreaming` set BEFORE `send()` at line 290 | Info | Added explanatory comment in Task 3 Step 1 confirming single-threaded JS safety |
