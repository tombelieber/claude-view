# Sidecar Revamp — Complete Agent SDK UI

**Date:** 2026-03-11
**Status:** Approved
**Scope:** Ground-up rewrite of `sidecar/src/` to handle ALL Agent SDK v0.2.72 capabilities

## Context

An audit of the current sidecar found 14 fundamental issues: 5 CRITICAL (silent drops, dead code, fabricated data), 5 HIGH (missing handlers, no cancellation), 4 MEDIUM (wrong data, lost context). Root cause: the sidecar handles 4 of 22 SDK message types and gets 3 wrong. The current code is ~500 lines — rewriting is faster than patching.

## Architecture

### What stays unchanged

- **Rust proxy** (`crates/server/src/routes/control.rs`) — transparent bidirectional WS relay via Unix socket. Zero changes.
- **Sidecar process model** — Rust spawns Node.js sidecar, communicates via Unix socket.
- **Ring buffer replay** — reconnect mechanism (RingBuffer class).
- **Frontend hook layering** — `use-control-session` (WS protocol) → `use-session-control` (lifecycle state machine).

### What gets rewritten

- **`session-manager.ts`** → split into 5 focused files
- **`types.ts`** → expanded protocol with 27 event types
- **`ws-handler.ts`** → simplified, more message types
- **`control.ts`** → new endpoints (create, list available, one-shot prompt)

### Stream processing model

**Current (broken):** Each `sendMessage()` creates a new `stream()` generator per turn.

**New:** One `stream()` call per session lifetime. Started on create/resume, runs until `close()`. `send()` is called concurrently — the SDK handles internal queuing. No `isStreaming` guard, no per-turn generators, no race conditions.

```
resume/create → stream() starts → RUNS FOR SESSION LIFETIME
                    ↕ (concurrent)
sendMessage() → send() pushes into session
                    ↓
               stream yields all events across all turns
               stream runs until session.close()
```

## File Structure

```
sidecar/src/
├── index.ts              # Server setup, shutdown
├── routes.ts             # HTTP: create, resume, list, list-available, prompt, terminate
├── ws-handler.ts         # WS message routing
├── session-registry.ts   # Map<controlId, ControlSession>, lookup, list, cleanup
├── sdk-session.ts        # Create/resume SDK session, long-lived stream loop, send()
├── event-mapper.ts       # SDKMessage → ProtocolEvent[]. Pure translation, exhaustive switch.
├── permission-handler.ts # canUseTool factory, pending maps (permission/question/plan/elicitation)
├── protocol.ts           # All 27 server events + 7 client messages, fully typed
├── ring-buffer.ts        # Keep as-is
├── health.ts             # Keep as-is
└── *.test.ts             # Tests for event-mapper, permission-handler, ring-buffer
```

Each file: one responsibility, under ~150 lines. `event-mapper.ts` is the only file that changes when the SDK evolves.

## SDK Message → Protocol Event Mapping

### 22 SDK message types → 27 protocol events. Zero silent drops.

#### Assistant Output

| SDK Type | type/subtype | Protocol Event(s) |
|---|---|---|
| `SDKAssistantMessage` | `assistant` | `assistant_text` per text block, `tool_use_start` per tool_use block, `assistant_thinking` per thinking block |
| `SDKAssistantMessage.error` | `assistant` (error set) | `assistant_error` with error type |
| `SDKPartialAssistantMessage` | `stream_event` | `stream_delta` (forward-compat for future V2 streaming) |

#### User Messages (Tool Results)

| SDK Type | type | Protocol Event(s) |
|---|---|---|
| `SDKUserMessage` | `user` | `tool_use_result` per tool_result block |
| `SDKUserMessageReplay` | `user` | Same, tagged `isReplay: true` |

#### Turn Lifecycle

| SDK Type | type/subtype | Protocol Event |
|---|---|---|
| `SDKResultSuccess` | `result`/`success` | `turn_complete` |
| `SDKResultError` | `result`/`error_*` | `turn_error` |

`turn_complete` forwards: `total_cost_usd`, `num_turns`, `duration_ms`, `duration_api_ms`, `usage` (NonNullableUsage), `modelUsage` (Record<string, ModelUsage> with contextWindow, maxOutputTokens, costUSD, webSearchRequests), `permission_denials`, `result` text, `structured_output`, `stop_reason`, `fast_mode_state`.

`turn_error` forwards: `subtype` (error_during_execution, error_max_turns, error_max_budget_usd, error_max_structured_output_retries), `errors[]`, `permission_denials[]`, `total_cost_usd`, `usage`, `fast_mode_state`.

#### System Messages (all use `type: 'system'`, routed by `subtype`)

| SDK Type | subtype | Protocol Event | Key data |
|---|---|---|---|
| `SDKSystemMessage` | `init` | `session_init` | `tools[]`, `model`, `mcp_servers[]`, `permissionMode`, `slash_commands[]`, `claude_code_version`, `cwd`, `agents[]`, `skills[]`, `output_style` |
| `SDKStatusMessage` | `status` | `session_status` | `status` (compacting/null), `permissionMode` |
| `SDKCompactBoundaryMessage` | `compact_boundary` | `context_compacted` | `trigger` (manual/auto), `pre_tokens` |
| `SDKElicitationCompleteMessage` | `elicitation_complete` | `elicitation_complete` | `mcp_server_name`, `elicitation_id` |
| `SDKTaskStartedMessage` | `task_started` | `task_started` | `task_id`, `tool_use_id?`, `description`, `task_type?`, `prompt?` |
| `SDKTaskProgressMessage` | `task_progress` | `task_progress` | `task_id`, `tool_use_id?`, `description`, `last_tool_name?`, `summary?`, `usage` |
| `SDKTaskNotificationMessage` | `task_notification` | `task_notification` | `task_id`, `tool_use_id?`, `status` (completed/failed/stopped), `output_file`, `summary`, `usage?` |
| `SDKHookStartedMessage` | `hook_started` | `hook_event` (phase=started) | `hook_id`, `hook_name`, `hook_event` |
| `SDKHookProgressMessage` | `hook_progress` | `hook_event` (phase=progress) | + `stdout`, `stderr`, `output` |
| `SDKHookResponseMessage` | `hook_response` | `hook_event` (phase=response) | + `exit_code?`, `outcome` (success/error/cancelled) |
| `SDKFilesPersistedEvent` | `files_persisted` | `files_saved` | `files[]` (filename, file_id), `failed[]` (filename, error), `processed_at` |
| `SDKLocalCommandOutputMessage` | `local_command_output` | `command_output` | `content` |

#### Other Top-Level Types

| SDK Type | type | Protocol Event | Key data |
|---|---|---|---|
| `SDKToolProgressMessage` | `tool_progress` | `tool_progress` | `tool_use_id`, `tool_name`, `elapsed_time_seconds`, `task_id?`, `parent_tool_use_id` |
| `SDKRateLimitEvent` | `rate_limit_event` | `rate_limit` | `status`, `resetsAt`, `utilization`, `rateLimitType`, overage info |
| `SDKAuthStatusMessage` | `auth_status` | `auth_status` | `isAuthenticating`, `output[]`, `error?` |
| `SDKToolUseSummaryMessage` | `tool_use_summary` | `tool_summary` | `summary`, `preceding_tool_use_ids[]` |
| `SDKPromptSuggestionMessage` | `prompt_suggestion` | `prompt_suggestion` | `suggestion` |

#### Catch-all

Any unrecognized SDK message type → `unknown_sdk_event` with full raw message. Logged as warning. Never silently dropped.

### Interactive Cards (from `canUseTool` callback)

| Tool Name | Protocol Event | Key data |
|---|---|---|
| `AskUserQuestion` | `ask_question` | `requestId`, `questions[]` (options, headers, multiSelect) |
| `ExitPlanMode` | `plan_approval` | `requestId`, `planData` |
| MCP elicitation tools | `elicitation` | `requestId`, `prompt`, MCP server context |
| Everything else | `permission_request` | `requestId`, `toolName`, `toolInput`, `toolUseID`, `suggestions[]`, `decisionReason`, `blockedPath`, `agentID`, `timeoutMs` |

### Infrastructure Events (unchanged pattern)

| Event | Direction | Notes |
|---|---|---|
| `heartbeat_config` | server→client | No seq, connection-scoped |
| `pong` | server→client | Heartbeat response |
| `error` | server→client | Fatal/non-fatal errors |
| `session_closed` | server→client | NEW: stream ended or explicit close |

### Client → Sidecar Messages

| Message | Purpose |
|---|---|
| `user_message` | Send a message |
| `permission_response` | Allow/deny (optionally with `updatedPermissions` for "always allow") |
| `question_response` | Answer AskUserQuestion |
| `plan_response` | Approve/reject plan |
| `elicitation_response` | Respond to elicitation |
| `resume` | Replay from lastSeq |
| `ping` | Heartbeat |

## Permission System

### Full context forwarding

The `canUseTool` callback receives `toolUseID`, `suggestions`, `blockedPath`, `decisionReason`, `agentID` — all forwarded to the frontend.

### "Always allow" flow

When the frontend responds with `allowed: true` + `updatedPermissions` (echoing back the `suggestions` from the request), the SDK persists permission rules so the user isn't prompted again for the same tool pattern.

### Routing

1. `AskUserQuestion` → `ask_question` card with options
2. `ExitPlanMode` → `plan_approval` card
3. MCP elicitation → `elicitation` card
4. Everything else → `permission_request` with full context

### Abort handling

All pending maps (permission, question, plan, elicitation) respect the SDK's `AbortSignal`. On abort: deny with reason. On WS close: drain pending maps (deny permissions, reject plans, empty answers for questions, empty response for elicitations).

### Timeout

Generic permission requests auto-deny after 60 seconds (configurable). Questions, plans, and elicitations have no auto-timeout (they require human input).

## Session Lifecycle

### State machine

```
create/resume → initializing → ready (waiting_input) → active (streaming)
                                   ↑                         ↓
                                   ├── waiting_permission ←──┤
                                   ├── compacting ←──────────┤
                                   └─────────────────────────┘
                                        → error (fatal)
                                        → closed
```

### Endpoints

| Endpoint | Method | Purpose |
|---|---|---|
| `/control/sessions` | POST | Create new session |
| `/control/sessions/resume` | POST | Resume existing session |
| `/control/sessions` | GET | List active control sessions |
| `/control/available-sessions` | GET | List all Claude Code sessions (SDK listSessions) |
| `/control/sessions/:id` | DELETE | Terminate session |
| `/control/sessions/:id/stream` | WS | WebSocket (enhanced protocol) |
| `/control/prompt` | POST | One-shot prompt (no session lifecycle) |

### Session creation options

```typescript
interface CreateSessionRequest {
  model: string
  permissionMode?: 'default' | 'acceptEdits' | 'plan' | 'dontAsk'
  allowedTools?: string[]
  disallowedTools?: string[]
  projectPath?: string
  initialMessage?: string
}
```

Note: V2 `SDKSessionOptions` lacks `cwd`. Workaround: use `executableArgs` or process CWD. Verify at implementation time.

## Frontend Changes

### Rust proxy: ZERO changes

### `apps/web/src/types/control.ts`: expanded from 11 to 27 server event types

### `use-control-session.ts`: switch handles all 27 event types

### New UI state fields

```typescript
sessionInit: SessionInitData | null           // tools, model, mcp_servers, cwd, version
rateLimitStatus: RateLimitInfo | null          // warning/rejected state
activeTasks: Map<string, TaskInfo>             // subagent tracking
activeToolProgress: Map<string, ToolProgress>  // tool execution timers
contextCompaction: CompactionInfo | null       // compaction trigger + pre_tokens
fastModeState: FastModeState | null            // off/cooldown/on
hookEvents: HookEvent[]                        // recent hook lifecycle
promptSuggestion: string | null                // suggested next prompt
modelUsage: Record<string, ModelUsage>         // per-model token/cost/contextWindow
```

### New hooks

- `use-available-sessions.ts` — fetches session picker data from `/control/available-sessions`

### Context usage computation

`ModelUsage.contextWindow` provides the model's context window size. The frontend computes: `contextPercent = (totalTokensUsed / contextWindow) * 100`. No more hardcoded 0%.

## Non-goals

- Rust proxy changes — it's already correct
- Desktop/Tauri integration — deferred
- Multi-user session sharing — not in SDK
- Custom MCP server configuration UI — phase 2
- Custom agent definition UI — phase 2
