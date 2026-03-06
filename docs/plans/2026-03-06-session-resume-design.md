# Lazy Session Resume — Design Document

**Date:** 2026-03-06
**Status:** Done (implemented 2026-03-06)
**Scope:** History session resume flow, connection health, message lifecycle

## Problem Statement

The history session view has 5 structural problems that create a fragile, inconsistent UX:

1. **No single source of truth** — session lifecycle state is split across `ConversationView.tsx` (ad-hoc refs + effects), `useControlSession` (WS state), and `ChatInputBar` (input state machine). Each component holds a piece of the puzzle.
2. **No message lifecycle** — messages go from "typed" to "gone" with no intermediate states. Users get no feedback on whether their message was sent, is being processed, or failed.
3. **Two incompatible resume paths** — `ResumePreFlight` (modal with cost estimate) and inline chat resume (POST `/api/control/resume`) share zero code and have different error handling.
4. **Duplicated state mapping** — `controlStatusToInputState()` is copy-pasted identically in `ConversationView.tsx` and `SessionDetailPanel.tsx`.
5. **Scatter-shot error handling** — errors are swallowed silently, shown as toasts, or logged to console with no unified strategy.

### Trigger

User sends a message on a history session page. Nothing happens — no visual feedback, no error, no indication of what went wrong. The input bar was also permanently disabled due to `useControlSession` initializing as `'connecting'` when no control session exists.

## Design

### New Hook: `useSessionControl(sessionId: string)`

A single hook that owns the entire session control lifecycle. Replaces all ad-hoc state in `ConversationView` and the duplicated mapping in `SessionDetailPanel`.

#### Phase State Machine

```
idle → resuming → connecting → ready → (completed | error)
                                ↑              |
                                └── reconnecting ←┘ (on transient disconnect)
```

| Phase | Description | InputBarState |
|-------|-------------|---------------|
| `idle` | No control session. User can type. | `dormant` |
| `resuming` | POST `/api/control/resume` in flight | `resuming` |
| `connecting` | WebSocket handshake in progress | `connecting` |
| `ready` | WS open, sidecar streaming | derived from WS messages (`active`, `streaming`, `waiting_input`, `waiting_permission`) |
| `reconnecting` | WS dropped, auto-reconnecting | `reconnecting` |
| `completed` | Session ended normally | `completed` |
| `error` | Unrecoverable failure | shows error + retry button |

#### Message Queue with Lifecycle

Each message has a status:

| Status | Visual | Meaning |
|--------|--------|---------|
| `optimistic` | Faded + spinner | Shown immediately on send, before resume completes |
| `sending` | Faded + spinner | Resume done, WS open, message sent to sidecar |
| `sent` | Normal | Sidecar acknowledged (assistant starts responding) |
| `failed` | Red badge + "Retry" | Resume or WS send failed after retries |

**Pattern:** WhatsApp/Slack optimistic message rendering. Show the message immediately in the chat, update its status as the backend progresses.

#### Connection Health (Separate Concern)

| State | Trigger | UI |
|-------|---------|-----|
| `ok` | WS open, pongs received | No banner |
| `degraded` | 2+ missed pongs, or reconnecting | Amber banner: "Reconnecting..." |
| `lost` | Max reconnect attempts exhausted | Red banner: "Connection lost. Retrying..." + manual retry button |

**Pattern:** Slack/Google Docs persistent connection banner. This is independent of the phase state machine — a session can be `ready` with `degraded` connection.

#### Hook API Surface

```typescript
interface UseSessionControl {
  // Phase
  phase: SessionPhase
  inputBarState: InputBarState

  // Messages
  messages: ChatMessageWithStatus[]

  // Connection
  connectionHealth: 'ok' | 'degraded' | 'lost'

  // Streaming
  streamingContent: string

  // Context
  contextPercent: number

  // Costs
  sessionCost: number | null
  lastTurnCost: number | null

  // Permission/elicitation (passthrough from useControlSession)
  permissionRequest: PermissionRequestMsg | null
  askQuestion: AskUserQuestionMsg | null
  planApproval: PlanApprovalMsg | null
  elicitation: ElicitationMsg | null

  // Actions
  send: (text: string) => void          // handles resume + queue + WS send
  retry: (messageId: string) => void     // retry a failed message
  respondPermission: (id: string, allowed: boolean) => void
  answerQuestion: (id: string, answers: Record<string, string>) => void
  approvePlan: (id: string, approved: boolean, feedback?: string) => void
  submitElicitation: (id: string, response: string) => void
}
```

#### `send(text)` Flow

1. Create optimistic message, append to messages array
2. If `phase === 'idle'`: transition to `resuming`, POST `/api/control/resume`
3. On resume success: store `controlId`, transition to `connecting`, open WS
4. On WS `session_status` received: transition to `ready`
5. Send queued message over WS, update status to `sending`
6. On first `assistant_chunk`: update message status to `sent`
7. On failure at any step: update message status to `failed`, show retry affordance

### New Component: `ConnectionBanner`

Persistent banner shown above the chat when `connectionHealth !== 'ok'`.

- **Amber** (`degraded`): "Reconnecting..." with subtle pulse animation
- **Red** (`lost`): "Connection lost" with manual "Retry" button

### Refactored Components

#### `ConversationView.tsx`

- Remove: `controlId` state, `pendingMessageRef`, drain effect, `controlStatusToInputState()`, ad-hoc resume POST
- Replace with: `const session = useSessionControl(sessionId)`
- ChatInputBar receives `state={session.inputBarState}`, `onSend={session.send}`

#### `SessionDetailPanel.tsx`

- Remove: duplicated `controlStatusToInputState()`
- Replace with: `const session = useSessionControl(sessionId)`
- Same interface as ConversationView

#### `ChatInputBar.tsx`

- No changes needed. The 8-state machine is correct — the problem was that nobody was mapping to `dormant`, `resuming`, or `streaming`. The new hook fixes this.

### `use-control-session.ts`

- Becomes an internal implementation detail of `useSessionControl`
- No API changes, but consumed only by the new hook (not directly by components)

## Anti-Patterns to Avoid

- **No "just add a retry" loops** — retries are bounded (3 attempts with exponential backoff)
- **No silent error swallowing** — every error either shows in the connection banner or on the failed message
- **No optimistic-only rendering** — every optimistic message MUST resolve to `sent` or `failed` within a timeout (30s)
- **No duplicated state mapping** — `inputBarState` is derived inside the hook, not in each consumer

## Industry Precedent

| Pattern | Used By |
|---------|---------|
| Optimistic messages with status | WhatsApp, Slack, Telegram, iMessage |
| Connection health banner | Slack, Google Docs, Figma, Notion |
| Lazy resume on first interaction | VS Code remote sessions, JupyterHub |
| Single hook for complex lifecycle | React Query (`useQuery`), Apollo (`useSubscription`) |

## Files to Create/Modify

| Action | File |
|--------|------|
| **Create** | `apps/web/src/hooks/use-session-control.ts` |
| **Create** | `apps/web/src/components/chat/ConnectionBanner.tsx` |
| **Modify** | `apps/web/src/components/ConversationView.tsx` |
| **Modify** | `apps/web/src/components/live/SessionDetailPanel.tsx` |
| **Internal** | `apps/web/src/hooks/use-control-session.ts` (no API change, just consumed differently) |
| **Modify** | `apps/web/src/types/control.ts` (add `ChatMessageWithStatus` type) |

## Testing Strategy

- Unit test the phase state machine transitions in isolation
- Unit test message lifecycle: optimistic → sending → sent, and optimistic → failed → retry → sent
- Integration test: mock WS, verify full send flow from idle through ready
- Manual E2E: open a history session, type a message, verify optimistic rendering → resume → WS → response
