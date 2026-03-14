# V1 Frontend Control Wiring — Full WS Surface

**Date:** 2026-03-14
**Status:** Draft
**Depends on:** V1 Streaming Sidecar Migration (completed 2026-03-14)

## Problem

The V1 sidecar migration added 15 WS control method handlers (`ws-handler.ts`) and shared protocol types, but the frontend only wires 6 of the existing methods (user_message, permission_response, question_response, plan_response, elicitation_response, set_mode). 13 new V1 control methods have no frontend integration:

- **Stop button is non-functional** — ChatInputBar renders it but `onStop` is never passed from ChatPage
- **Model switching** uses a full session resume instead of live `set_model` WS message
- **MCP management** (reconnect, toggle, configure) has no UI
- **File rewind** has no UI
- **Query methods** (models, commands, agents, mcp_status, account_info) exist but aren't called
- **Thinking budget control** has no UI

## Decision

Wire all 13 methods through a single `SessionChannel` transport abstraction with `send()` (fire-and-forget) and `request()` (Promise-based correlated response). WS is the primary data source; HTTP endpoints (`GET /api/models`) remain for data the SDK doesn't have (pricing, context windows, usage stats).

## Architecture

### SessionChannel — Transport Abstraction

New file: `apps/web/src/lib/session-channel.ts` (~80 lines)

A thin wrapper over WebSocket that provides two operations:

```typescript
class SessionChannel {
  private ws: WebSocket | null
  private pending = new Map<string, { resolve: (data: unknown) => void; reject: (err: Error) => void; timer: ReturnType<typeof setTimeout> }>()

  /** Fire-and-forget. Queues if WS not open (existing lazy-send pattern). */
  send(msg: ClientMessage): void

  /**
   * Request/response. Sends msg, returns Promise that resolves when a matching
   * response arrives. Rejects on timeout (default 10s) or WS disconnect.
   *
   * Deduplicates: if a request with the same responseKey is already in-flight,
   * returns the existing Promise instead of sending a duplicate message.
   */
  request<T>(msg: ClientMessage, responseKey: string, timeoutMs?: number): Promise<T>

  /** Called by use-session-source.ts when a response event arrives. */
  handleResponse(responseKey: string, data: unknown): void

  /** Called on WS disconnect. Rejects all pending promises. */
  handleDisconnect(): void
}
```

Response routing in `use-session-source.ts`:
- `query_result` → `channel.handleResponse(event.queryType, event.data)`
- `rewind_result` → `channel.handleResponse('rewind', event.result)`
- `mcp_set_result` → `channel.handleResponse('mcp_set', event.result)`

### Method Classification

#### Fire-and-forget (6 methods) — `channel.send()`

| Method | Message | UI Trigger |
|--------|---------|------------|
| `interrupt` | `{ type: 'interrupt' }` | Stop button in ChatInputBar |
| `setModel` | `{ type: 'set_model', model }` | ModelSelector mid-session (when `isLive`) |
| `setMaxThinkingTokens` | `{ type: 'set_max_thinking_tokens', maxThinkingTokens }` | ThinkingBudgetControl dropdown |
| `stopTask` | `{ type: 'stop_task', taskId }` | Cancel button on task progress cards |
| `reconnectMcp` | `{ type: 'reconnect_mcp', serverName }` | MCP panel reconnect button (refresh status after) |
| `toggleMcp` | `{ type: 'toggle_mcp', serverName, enabled }` | MCP panel toggle switch (refresh status after) |

#### Request/response queries (5 methods) — `channel.request()`

| Method | Message | Response key | Replaces |
|--------|---------|-------------|----------|
| `queryModels` | `{ type: 'query_models' }` | `'models'` | Supplements `GET /api/models` (SDK display names only — unified catalog stays for pricing/context) |
| `queryCommands` | `{ type: 'query_commands' }` | `'commands'` | `session_init.slashCommands` as primary |
| `queryAgents` | `{ type: 'query_agents' }` | `'agents'` | `session_init.agents` as primary |
| `queryMcpStatus` | `{ type: 'query_mcp_status' }` | `'mcp_status'` | `session_init.mcpServers` as primary |
| `queryAccountInfo` | `{ type: 'query_account_info' }` | `'account_info'` | Nothing (new capability) |

#### Request/response mutations (2 methods) — `channel.request()`

| Method | Message | Response key | UI Flow |
|--------|---------|-------------|---------|
| `setMcpServers` | `{ type: 'set_mcp_servers', servers }` | `'mcp_set'` | MCP panel config editor → submit → result |
| `rewindFiles` | `{ type: 'rewind_files', userMessageId, dryRun? }` | `'rewind'` | Undo button → dry-run preview → confirm → execute |

### React Hook Layer

#### `use-session-channel.ts` (NEW)

Wraps `SessionChannel` in a React context. Provides the channel instance to all components via `useSessionChannel()`.

#### `use-session-actions.ts` (MODIFY)

Extend `SessionActions` interface with all 13 new methods. Each method delegates to `channel.send()` or `channel.request()`:

```typescript
export interface SessionActions {
  // Existing (unchanged)
  sendMessage: (text: string) => void
  respondPermission: (requestId: string, allowed: boolean, updatedPermissions?: unknown[]) => void
  answerQuestion: (requestId: string, answers: Record<string, string>) => void
  approvePlan: (requestId: string, approved: boolean, feedback?: string) => void
  submitElicitation: (requestId: string, response: string) => void
  setPermissionMode: (mode: string) => void

  // NEW — fire-and-forget
  interrupt: () => void
  setModel: (model: string) => void
  setMaxThinkingTokens: (tokens: number | null) => void
  stopTask: (taskId: string) => void
  reconnectMcp: (serverName: string) => void
  toggleMcp: (serverName: string, enabled: boolean) => void

  // NEW — request/response
  queryModels: () => Promise<unknown>
  queryCommands: () => Promise<unknown>
  queryAgents: () => Promise<unknown>
  queryMcpStatus: () => Promise<unknown>
  queryAccountInfo: () => Promise<unknown>
  setMcpServers: (servers: Record<string, unknown>) => Promise<unknown>
  rewindFiles: (userMessageId: string, opts?: { dryRun?: boolean }) => Promise<unknown>
}
```

#### `useWsQuery` (NEW generic hook)

Reactive wrapper for query methods. Components that want auto-refresh use this instead of raw `channel.request()`:

```typescript
function useWsQuery<T>(queryFn: () => Promise<T>, deps?: unknown[]): {
  data: T | null
  loading: boolean
  error: Error | null
  refresh: () => void
}
```

### UI Components

#### ChatInputBar — Interrupt wiring

`ChatPage.tsx` passes `onStop={actions.interrupt}`. The existing stop button UI in ChatInputBar already handles the rest (renders when `state === 'streaming'`, Escape key binding).

#### ModelSelector — Live model switch

ModelSelector receives a new `isLive: boolean` prop from ChatPage. When `isLive === true`, it calls `actions.setModel(model)` instead of `actions.resume(mode, newModel)`. The model change takes effect on the next turn without reconnection. When `isLive === false` or no session is active, keeps the existing behavior (stores model for next session creation).

State update: **optimistic** — update local `model` state immediately on send. The next `turn_complete` confirms the model in use.

#### ThinkingBudgetControl (NEW)

Small dropdown near ModelSelector. Options: Default (null), 1K, 4K, 16K, 64K, Max. Calls `actions.setMaxThinkingTokens(value)`. Renders only when the active model supports extended thinking.

File: `apps/web/src/components/chat/ThinkingBudgetControl.tsx` (~40 lines)

#### McpPanel (NEW)

Accessible from session settings toolbar button. Sections:

1. **Server list** — rows with name, status badge, toggle switch, reconnect button
   - Data: `useWsQuery(() => actions.queryMcpStatus())`
   - Toggle: `actions.toggleMcp(name, enabled)` → auto-refresh status
   - Reconnect: `actions.reconnectMcp(name)` → auto-refresh status
2. **Configure** — JSON editor for server config
   - Submit: `await actions.setMcpServers(config)` → show result toast

File: `apps/web/src/components/chat/McpPanel.tsx` (~120 lines)

#### RewindButton (NEW)

Hover-reveal undo icon on each user message block. Flow:

1. Click → `const preview = await actions.rewindFiles(msgId, { dryRun: true })`
2. Show confirmation dialog listing files to revert
3. Confirm → `await actions.rewindFiles(msgId)`
4. Show success/error toast

File: `apps/web/src/components/conversation/RewindButton.tsx` (~60 lines)

#### AccountInfoPanel (NEW)

Displayed in a session info popover or settings drawer. Shows account usage, plan info, rate limit status. Fetched on panel open via `useWsQuery(() => actions.queryAccountInfo())`.

File: `apps/web/src/components/chat/AccountInfoPanel.tsx` (~50 lines)

#### Task progress cards — stop_task

Existing task progress blocks (`TaskProgressBlock` or similar in `SystemBlock.tsx`) get a cancel icon button. Calls `actions.stopTask(taskId)` where `taskId` comes from the `task_started` event. **Note:** Verify that `TaskStarted` and `TaskProgressEvent` types expose `taskId` to the rendering component — the current `SystemBlock` may need the `taskId` field threaded through from the event data.

#### Slash menu — live refresh

On slash menu open, if commands/agents data is older than 60s, fire `actions.queryCommands()` / `actions.queryAgents()` in background. Update autocomplete options reactively when response arrives.

### Error Handling

| Scenario | Behavior |
|----------|----------|
| WS disconnects mid-request | All pending promises reject with `ChannelDisconnected`. Components show toast. |
| Request timeout (10s default) | Promise rejects with `QueryTimeout`. Caller decides retry vs error UI. |
| Duplicate in-flight query (same responseKey) | Returns existing pending promise. No duplicate WS message. |
| Session not active | `send()` queues (existing lazy-send). `request()` rejects immediately. |
| `set_model` while streaming | Sidecar queues it — SDK applies on next turn. No frontend guard. |
| `interrupt` when idle | No-op at sidecar. No error. |
| Older CLI version | Sidecar returns `{ type: 'error', fatal: false }`. Channel catches, toast shows "Feature requires Claude Code v0.19+". |

State consistency:
- `set_model`: **Optimistic** update (instant UI), confirmed by next `turn_complete`
- MCP operations: **Confirmed** — wait for `queryMcpStatus()` refresh after toggle/reconnect
- `set_max_thinking_tokens`: **Optimistic** (setting stored locally, applied on next turn)

### Unified Model Catalog Interaction

The `query_models` WS method returns SDK model info (display names, descriptions). The unified model catalog (`GET /api/models`) returns richer data (pricing, context windows, usage stats). These are complementary:

- **ModelSelector** continues to use `GET /api/models` as its primary data source
- `query_models` is used to refresh SDK display names mid-session if needed (e.g., after CLI update)
- No replacement of the unified catalog — it has data the SDK doesn't provide

## File Impact

### New files
| File | Lines | Purpose |
|------|-------|---------|
| `apps/web/src/lib/session-channel.ts` | ~80 | Promise-based WS transport with send/request |
| `apps/web/src/hooks/use-session-channel.ts` | ~30 | React context + hook for SessionChannel |
| `apps/web/src/hooks/use-ws-query.ts` | ~40 | Generic reactive wrapper for WS queries |
| `apps/web/src/components/chat/ThinkingBudgetControl.tsx` | ~40 | Thinking token budget dropdown |
| `apps/web/src/components/chat/McpPanel.tsx` | ~120 | MCP server management panel |
| `apps/web/src/components/conversation/RewindButton.tsx` | ~60 | Per-message undo with dry-run preview |
| `apps/web/src/components/chat/AccountInfoPanel.tsx` | ~50 | Account usage/plan info display |

### Modified files
| File | Change |
|------|--------|
| `apps/web/src/hooks/use-session-actions.ts` | Add 13 methods (6 send, 5 query, 2 mutation) |
| `apps/web/src/hooks/use-session-source.ts` | Route query_result/rewind_result/mcp_set_result to channel |
| `apps/web/src/pages/ChatPage.tsx` | Pass onStop, wire model/thinking controls, add MCP button |
| `apps/web/src/components/chat/ChatInputBar.tsx` | Wire onStop prop (UI already exists) |
| `apps/web/src/components/chat/ModelSelector.tsx` | Use set_model when live, keep resume when no session |
| Task progress block component | Add cancel button with stopTask action |
| User message block component | Add RewindButton hover overlay |

### No changes
| File | Reason |
|------|--------|
| All sidecar code | V1 handlers already complete from migration |
| All Rust server code | Not involved in WS control surface |
| `packages/shared/src/types/sidecar-protocol.ts` | Types already added in migration |

## Testing Strategy

| Layer | Test Type | Coverage |
|-------|-----------|----------|
| `SessionChannel` | Unit (Vitest) | send, request, timeout, dedup, disconnect cleanup |
| `use-session-channel` | Hook test (RTL) | mount/unmount cleanup, re-render on response |
| `useWsQuery` | Hook test (RTL) | loading states, refresh, error handling |
| `useSessionActions` (new methods) | Unit | Verify correct message shape per method |
| `McpPanel` | Component test | Toggle, reconnect, config editor flow |
| `RewindButton` | Component test | Dry-run → preview → confirm → execute flow |
| `ThinkingBudgetControl` | Component test | Preset selection, null default |
| `ModelSelector` (live switch) | Component test | set_model when live, resume when not |
| Integration | E2E (sidecar infra) | Send query via WS → verify response roundtrip |
