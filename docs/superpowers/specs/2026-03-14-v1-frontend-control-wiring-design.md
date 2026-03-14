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
   * Request/response. Attaches a unique `requestId` (crypto.randomUUID()) to the
   * message, sends it, and returns a Promise that resolves when a response with
   * the matching `requestId` arrives. Rejects on timeout (default 10s) or WS disconnect.
   *
   * No deduplication — every call sends a fresh WS message and gets its own response.
   * Concurrent queries of the same type are allowed (each has a unique requestId).
   *
   * Pattern: JSON-RPC `id` field (LSP, Ethereum), Discord Gateway `nonce`,
   * Phoenix Channels `ref`. Per-request correlation is the industry standard.
   */
  request<T>(msg: ClientMessage, timeoutMs?: number): Promise<T>

  /** Called by use-session-source.ts when a response event arrives. */
  handleResponse(requestId: string, data: unknown): void

  /** Called on WS disconnect. Rejects all pending promises. */
  handleDisconnect(): void
}
```

**requestId protocol:** Every `request()` call attaches `requestId: crypto.randomUUID()` to the outgoing message. The sidecar echoes the `requestId` in the response. This requires a small sidecar change — see "Sidecar Changes" section below.

Response routing in `use-session-source.ts`:
- `query_result` → `channel.handleResponse(event.requestId, event.data)`
- `rewind_result` → `channel.handleResponse(event.requestId, event.result)`
- `mcp_set_result` → `channel.handleResponse(event.requestId, event.result)`

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
| Concurrent queries (same type) | Each gets a unique `requestId` — no dedup, no stale data risk. |
| Session not active | `send()` queues (existing lazy-send). `request()` rejects immediately. |
| `set_model` while streaming | Sidecar queues it — SDK applies on next turn. No frontend guard. |
| `interrupt` when idle | No-op at sidecar. No error. |
| Older CLI version | Frontend checks `capabilities` array from `session_init`. If method not in capabilities, UI is hidden (not rendered). No string-matching error messages. See "Capability Negotiation" section. |
| Fire-and-forget error | Sidecar `sendError()` includes method context in message (e.g., `"setModel failed: unknown model"`). Arrives as generic `{ type: 'error', fatal: false }` — displayed as toast. Caller has no direct error callback. |

State consistency:
- `set_model`: **Optimistic** update (instant UI), confirmed by next `turn_complete`
- MCP operations: **Confirmed** — wait for `queryMcpStatus()` refresh after toggle/reconnect
- `set_max_thinking_tokens`: **Optimistic** (setting stored locally, applied on next turn)

### Capability Negotiation

The sidecar adds a `capabilities` array to the `session_init` event listing all supported V1 control methods:

```typescript
// In session_init event (event-mapper.ts, case 'init'):
capabilities: ['interrupt', 'set_model', 'set_max_thinking_tokens', 'stop_task',
  'query_models', 'query_commands', 'query_agents', 'query_mcp_status',
  'query_account_info', 'reconnect_mcp', 'toggle_mcp', 'set_mcp_servers', 'rewind_files']
```

Frontend checks `capabilities.includes('interrupt')` before rendering the stop button, `capabilities.includes('rewind_files')` before rendering RewindButton, etc. This replaces fragile error-message parsing with structured feature detection.

**Precedent:** HTTP `Accept` headers, GraphQL introspection, WebSocket subprotocols, LSP `ServerCapabilities`. Capability negotiation is the industry standard for feature detection across versioned protocols.

If `capabilities` is absent (older sidecar without this field), the frontend treats all 13 methods as unavailable — graceful degradation with zero string matching.

### MCP Panel Refresh Timing

After `toggleMcp()` or `reconnectMcp()` (fire-and-forget), the MCP panel must refresh status. The sidecar `await`s the SDK call before returning, so by the time the WS message handler completes, the toggle has taken effect. However, the fire-and-forget `send()` provides no completion signal to the frontend.

**Solution:** The MCP panel uses a 500ms debounced refresh after any toggle/reconnect action. This gives the sidecar time to process the SDK call before the status query arrives. The debounce coalesces rapid toggles (user clicking multiple servers) into a single refresh.

```typescript
// McpPanel.tsx
const debouncedRefresh = useDebouncedCallback(() => {
  refresh() // calls queryMcpStatus()
}, 500)

const handleToggle = (name: string, enabled: boolean) => {
  actions.toggleMcp(name, enabled)
  debouncedRefresh()
}
```

**Why not make toggle/reconnect request/response?** The sidecar WS handler doesn't send a response for these methods — it only `await`s the SDK call and catches errors. Adding responses would require sidecar changes for minimal benefit. The debounced refresh is simpler and handles the batching case (multiple toggles) naturally.

### Unified Model Catalog Interaction

The `query_models` WS method returns SDK model info (display names, descriptions). The unified model catalog (`GET /api/models`) returns richer data (pricing, context windows, usage stats). These are complementary:

- **ModelSelector** continues to use `GET /api/models` as its primary data source
- `query_models` is used to refresh SDK display names mid-session if needed (e.g., after CLI update)
- No replacement of the unified catalog — it has data the SDK doesn't provide

## Sidecar Changes (Small — requestId echo + capabilities)

Despite the V1 migration being "complete," this spec requires two small sidecar modifications:

### 1. Echo requestId on query/mutation responses (~10 lines)

The WS handler must read `requestId` from incoming messages and include it in responses:

```typescript
// ws-handler.ts — for each query/mutation case:
case 'query_models':
  try {
    const models = await session.query.supportedModels()
    ws.send(JSON.stringify({ type: 'query_result', queryType: 'models', data: models, requestId: msg.requestId }))
  } catch (err) { sendError(ws, err) }
  break
```

Same pattern for all 7 response-bearing methods. The `requestId` field is optional — if absent (older frontend), the response still works (frontend without SessionChannel ignores it).

### 2. Add capabilities to session_init (~5 lines)

In `event-mapper.ts`, the `case 'init':` handler adds a static `capabilities` array:

```typescript
capabilities: ['interrupt', 'set_model', 'set_max_thinking_tokens', 'stop_task',
  'query_models', 'query_commands', 'query_agents', 'query_mcp_status',
  'query_account_info', 'reconnect_mcp', 'toggle_mcp', 'set_mcp_servers', 'rewind_files'],
```

This is a static list (not dynamically computed from the SDK) because the sidecar WS handler is the layer that supports these methods — it always supports all of them. Future versions can conditionally omit methods if needed.

### 3. Protocol types update (~3 lines)

Add to `protocol.ts`:
- `requestId?: string` on `QueryResult`, `RewindResult`, `McpSetResult`
- `capabilities?: string[]` on `SessionInit`
- `requestId?: string` on all `ClientMessage` types that use `request()`

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

### Small sidecar modifications
| File | Change |
|------|--------|
| `sidecar/src/ws-handler.ts` | Echo `requestId` in query/mutation responses (~10 lines) |
| `sidecar/src/event-mapper.ts` | Add `capabilities` array to session_init (~3 lines) |
| `sidecar/src/protocol.ts` | Add `requestId?` to response types, `capabilities?` to SessionInit (~5 lines) |
| `packages/shared/src/types/sidecar-protocol.ts` | Mirror protocol.ts changes (~5 lines) |

### No changes
| File | Reason |
|------|--------|
| All Rust server code | Not involved in WS control surface |
| `sidecar/src/sdk-session.ts` | Session lifecycle unchanged |
| `sidecar/src/message-bridge.ts` | Transport unchanged |

## Testing Strategy

| Layer | Test Type | Coverage |
|-------|-----------|----------|
| `SessionChannel` | Unit (Vitest) | send, request with requestId correlation, timeout, concurrent queries, disconnect cleanup |
| `use-session-channel` | Hook test (RTL) | mount/unmount cleanup, re-render on response |
| `useWsQuery` | Hook test (RTL) | loading states, refresh, error handling |
| `useSessionActions` (new methods) | Unit | Verify correct message shape per method |
| `McpPanel` | Component test | Toggle, reconnect, config editor flow |
| `RewindButton` | Component test | Dry-run → preview → confirm → execute flow |
| `ThinkingBudgetControl` | Component test | Preset selection, null default |
| `ModelSelector` (live switch) | Component test | set_model when live, resume when not |
| Capability gating | Component test | Methods hidden when not in capabilities, shown when present |
| requestId echo | Unit (sidecar) | Verify all 7 response types include requestId from request |
| Integration | E2E (sidecar infra) | Send query with requestId → verify requestId echoed in response |
