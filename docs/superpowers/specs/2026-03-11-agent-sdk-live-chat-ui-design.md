# Agent SDK Live Chat UI — Design Spec

**Date:** 2026-03-11
**Status:** Done (implemented 2026-03-11)
**Scope:** Rich message rendering, interactive UI, mode selection, context gauge for live Agent SDK sessions

## Overview

Four improvements to the live chat UI for Agent SDK sessions:

1. **Rich message rendering** — reuse existing RichPane components inside chat bubbles
2. **Interactive UI polish** — notifications, countdown visibility, timeout states
3. **Mode selection** — expose all 5 SDK permission modes
4. **Context gauge** — show real token usage from SDK session data

## 1. Rich Message Rendering (Hybrid Approach)

### Problem

Live chat renders all messages as plain text via `LiveMessageBubble.tsx`. The history viewer has rich rendering (tool cards, thinking blocks, syntax highlighting, JSON trees) via `RichPane.tsx` and friends — but these aren't used in the live path.

### Design

**Reuse, don't rebuild.** `LiveMessageBubble` becomes a dispatcher that embeds existing rich components based on message type.

#### Message type → Component mapping

| Message type | Chat mode (compact) | Debug mode (verbose) |
|-------------|-------------------|---------------------|
| `user` | Markdown bubble | Markdown bubble |
| `assistant_chunk` | Streaming markdown with cursor | Streaming markdown with cursor |
| `tool_use_start` | Collapsed summary line | `PairedToolCard` (full) |
| `tool_use_result` | Hidden (paired with tool_use) | `PairedToolCard` (full) |
| `thinking` | Hidden | `ThinkingMessage` (collapsible) |
| `error` | `ErrorMessage` inline | `ErrorMessage` inline |
| `assistant_done` | Token/cost badge | Token/cost badge + details |
| `permission_request` | `PermissionCard` | `PermissionCard` |
| `ask_user_question` | `AskUserQuestionCard` | `AskUserQuestionCard` |
| `plan_approval` | `PlanApprovalCard` | `PlanApprovalCard` |
| `elicitation` | `ElicitationCard` | `ElicitationCard` |

#### Tool pairing

Add `toolPairMap: Map<string, { input: ToolUseStart, result?: ToolUseResult, startTime: number }>` to the `useControlSession` reducer. When `tool_use_result` arrives, pair it with the matching `tool_use_start` by `toolUseId` and compute duration.

`LiveMessageBubble` checks the map: if a result exists, render `PairedToolCard`; if pending, render with a spinner.

#### Chat vs Debug toggle

Reuse existing `ViewModeToggle` component and `verboseMode` store flag. Same toggle controls both history and live views.

#### Components reused

- `PairedToolCard` (from `apps/web/src/components/live/PairedToolCard.tsx`) — no changes
- `ThinkingMessage` — currently internal to `RichPane.tsx`, **must be extracted and exported** as a standalone component
- `ErrorMessage` — currently internal to `RichPane.tsx`, **must be extracted and exported** as a standalone component
- `CompactCodeBlock` (syntax highlighting) — no changes
- `JsonTree` (expandable JSON) — no changes
- `ViewModeToggle` (from `packages/shared`) — no changes

#### Sidecar protocol additions for rich rendering

The sidecar currently does NOT emit these message types — they must be added:

1. **`tool_use_result`** — The SDK's `'user'` message type contains tool results as content blocks. The sidecar's `'user'` case is currently a no-op (`break`). Must extract `tool_result` content blocks from user messages and emit `tool_use_result` events with `toolUseId` for pairing.

2. **`thinking`** — The SDK's `'assistant'` message contains `thinking` content blocks alongside `text` and `tool_use`. The sidecar currently ignores thinking blocks. Must emit `thinking` events for thinking content blocks.

Add both to `ServerMessage` union in `sidecar/src/types.ts`.

#### Message ordering

Expected sequence per assistant turn: `thinking` (optional, 0-1) → `assistant_chunk` (1+) → `tool_use_start` (0+) → `tool_use_result` (0+, paired) → `assistant_done` (1).

## 2. Interactive UI Polish

### Problem

Interactive cards exist and work end-to-end. Missing: notifications when cards appear off-screen, countdown visibility, timeout states.

### Design

#### 2a. Notification layer

When a `permission_request`, `ask_user_question`, or `plan_approval` arrives:

1. **Auto-scroll** to the card if user hasn't manually scrolled up
2. **Persistent toast** at bottom of chat if user IS scrolled up: "Action requires your approval" + scroll-to button
3. **Browser Notification API** (if permitted): "Claude View — Permission requested: {toolName}"
4. Toast dismisses automatically when user scrolls to the card or responds

New component: `InteractionToast.tsx` in `apps/web/src/components/chat/`.

#### 2b. Permission countdown

Enhance `PermissionCard`:

- Circular countdown ring (60s → 0s) using CSS `conic-gradient` animation
- Pulse animation when < 10s remaining
- On timeout: card becomes disabled, shows "Timed out — denied automatically" with muted styling
- Countdown timer value displayed as text (e.g., "42s remaining")

#### 2c. Notification permission UX

Request browser notification permission on the first interactive card arrival via a small inline prompt: "Enable desktop notifications for permission requests?" with Allow/Dismiss. This is triggered by a user gesture (clicking "Enable") per browser security policy. Persist the choice in `localStorage`.

#### 2d. Elicitation verification

The sidecar has `pendingElicitations` map and `resolveElicitation()` method, but `handleCanUseTool` doesn't route any tool name to elicitation handling — only `AskUserQuestion` and `ExitPlanMode` are explicitly handled. Check the SDK for the `Elicit` tool name (see `SDKElicitationCompleteMessage` in SDK types). If the SDK supports it, add routing in `handleCanUseTool`. If not yet, add a TODO but keep the card component ready.

## 3. Mode Selection (All SDK Permission Modes)

### Problem

`ModeSwitch` shows plan/code/ask (system prompt modes) but never transmits to sidecar. SDK supports 5 permission modes that control actual tool approval behavior.

### Design

#### 3a. UI — Replace ModeSwitch

Replace the plan/code/ask selector with all 5 SDK permission modes:

| Mode | Label | Icon | Color | Description |
|------|-------|------|-------|-------------|
| `default` | Default | Shield | blue | Prompts for dangerous operations |
| `acceptEdits` | Accept Edits | FileEdit | teal | Auto-approves file edits, prompts for rest |
| `plan` | Plan | ClipboardList | amber | Planning mode, no actual tool execution |
| `dontAsk` | Skip Dangerous | SkipForward | gray | Skips tools that need permission (deny if not pre-approved) |
| `bypassPermissions` | Trust All | ShieldOff | red | Bypasses ALL permission checks (dangerous) |

Default: `default`. Persist to `localStorage` keyed by session ID.

Show a small mode badge in the input bar area (left side, next to mode dropdown) so users always see the active mode.

**`bypassPermissions` safety gate:** Selecting this mode shows a confirmation dialog: "This mode auto-approves ALL tool executions including destructive operations. Are you sure?" Only activates on explicit confirm.

#### 3b. Protocol — new `set_mode` message

```typescript
// Client → Sidecar
{ type: 'set_mode', mode: 'default' | 'acceptEdits' | 'bypassPermissions' | 'plan' | 'dontAsk' }
```

Separate from `user_message` so mode can change mid-session without sending text.

#### 3c. Sidecar — handle `set_mode`

In `ws-handler.ts`:
- Receive `set_mode`, store `cs.permissionMode = msg.mode`
- Call `cs.sdkSession.setPermissionMode(msg.mode)` immediately (SDK method exists for mid-session changes)
- When `bypassPermissions` is set, also pass `allowDangerouslySkipPermissions: true` in SDK options (required by SDK safety gate)
- On initial session resume, pass `permissionMode` + `allowDangerouslySkipPermissions` in `SDKSessionOptions`

#### 3d. Rust relay — no changes

Already forwards all WS message types transparently.

#### 3e. Initial mode on session resume

When resuming a session via `POST /api/control/resume`, accept optional `mode` query param or body field. Pass through to sidecar on session creation.

## 4. Context Gauge (Real Token Usage)

### Problem

Sidecar hardcodes `contextUsage: 0` in `session_status`. The `ChatContextGauge` component works but shows stale data.

### Design

#### 4a. Sidecar — accumulate tokens from two sources

**Per-turn tokens** from each `SDKAssistantMessage.message.usage` (the `BetaMessage.usage` field on every assistant message in the stream). This provides per-turn granularity during the conversation:

```typescript
interface TokenAccumulator {
  totalInputTokens: number
  totalOutputTokens: number
  cacheReadTokens: number
  cacheCreationTokens: number
  lastTurnInputTokens: number  // context pressure = this / contextWindow
}
```

Accumulate in the `'assistant'` branch of the stream handler (where we already process text/tool_use blocks). Each assistant message has `msg.message.usage` with `input_tokens`, `output_tokens`, `cache_read_input_tokens`, `cache_creation_input_tokens`.

**Final totals + cost** from `SDKResultSuccess` (arrives at end of stream):
- `result.usage` — cumulative `NonNullableUsage` (use as authoritative final total)
- `result.modelUsage` — `Record<string, ModelUsage>` where each `ModelUsage` has `costUSD` and `contextWindow`
- `result.total_cost_usd` — total cost across all models

#### 4b. Sidecar — emit in `session_status`

Extend `session_status` message:

```typescript
{
  type: 'session_status',
  status: string,
  contextUsage: number,        // Math.round((lastTurnInputTokens / contextWindow) * 100)
  turnCount: number,
  tokenUsage: {                // NEW — accumulated per-turn
    input: number,
    output: number,
    cacheRead: number,
    cacheCreation: number,
  },
  costUsd?: number,            // NEW — from result.total_cost_usd (only after first result)
  model?: string,              // NEW — from assistant message model field
  contextWindow?: number,      // NEW — from result.modelUsage[model].contextWindow
}
```

#### 4c. Frontend — wire to existing components

- `useControlSession` already reads `contextUsage` from `session_status` — now it's real data
- Also read `tokenUsage` for cost display, `costUsd` for cost badge
- `ChatContextGauge` — no changes needed (already handles 0-100% with color thresholds)
- Add collapsible token/cost summary in session info area, reusing `CostBreakdown` component
- Use `contextWindow` from `session_status` (from SDK's `ModelUsage.contextWindow`) instead of hardcoded lookup

#### 4d. Cost calculation

**Sidecar provides cost — frontend just renders.** The SDK's `result.total_cost_usd` and `result.modelUsage[model].costUSD` are the authoritative cost source. The sidecar emits `costUsd` in `session_status` after each `result` message. The frontend displays it — no client-side pricing logic needed.

This follows the "logic-clean UI" rule: backend sends the right data, frontend renders it.

## Changes by Layer

| Layer | File(s) | Changes |
|-------|---------|---------|
| **Sidecar** | `session-manager.ts` | Accumulate tokens from assistant messages + result. Emit `tool_use_result` and `thinking` events. Emit real `contextUsage` + `tokenUsage` + `costUsd` in `session_status`. Call `setPermissionMode()` on mode change. |
| **Sidecar** | `ws-handler.ts` | Handle `set_mode` message, call `sdkSession.setPermissionMode()`. Pass `allowDangerouslySkipPermissions` for bypass mode. |
| **Sidecar** | `types.ts` | Add `set_mode` client msg, `tool_use_result` + `thinking` server msgs, extend `session_status` with `tokenUsage`/`costUsd`/`model`/`contextWindow` |
| **Rust server** | `control.rs` | No changes (transparent relay) |
| **Frontend** | `use-control-session.ts` | Parse `tokenUsage`/`costUsd` from `session_status`, add `toolPairMap` to reducer |
| **Frontend** | `use-session-control.ts` | Add `setMode()` method |
| **Frontend** | `LiveMessageBubble.tsx` | Dispatch to rich components based on message type |
| **Frontend** | `ModeSwitch.tsx` | Replace plan/code/ask with 5 SDK permission modes + bypass confirmation dialog |
| **Frontend** | `ChatInputBar.tsx` | Wire new ModeSwitch, show mode badge (left side, next to dropdown) |
| **Frontend** | `PermissionCard.tsx` | Add countdown ring, pulse, timeout state |
| **Frontend** | `ConversationView.tsx` | Pass `verboseMode` to live message rendering |
| **Frontend** | `RichPane.tsx` | Extract `ThinkingMessage` and `ErrorMessage` as exported standalone components |
| **New** | `InteractionToast.tsx` | Toast notification for off-screen interactive cards |
| **Reused** | `PairedToolCard`, `ThinkingMessage`, `ErrorMessage`, `CompactCodeBlock`, `CostBreakdown` | Imported into live chat path |

## Dependencies & Order

1. **Section 4 (Context gauge)** — sidecar token accumulation is foundational, do first
2. **Section 3 (Mode selection)** — protocol change, independent of rendering
3. **Section 1 (Rich rendering)** — largest change, depends on tool pairing in reducer
4. **Section 2 (Interactive polish)** — cosmetic, can be done last

## Out of Scope

- Model selector wiring (UI exists, not connected — separate feature)
- Slash command execution (popover exists, not connected — separate feature)
- File attachment sending (UI exists, not connected — separate feature)
- Streaming token-by-token display (V2 SDK doesn't support it — complete messages only)

## Type Synchronization Note

The sidecar protocol types are hand-written in both `sidecar/src/types.ts` and `apps/web/src/types/control.ts`. These have no Rust equivalent (sidecar is pure Node.js), so the generated types rule doesn't apply. However, when extending `SessionStatusMsg` or `ServerMessage` with new fields/variants, **both files must be updated in lockstep**. Consider a future TODO to generate these from a shared schema.
