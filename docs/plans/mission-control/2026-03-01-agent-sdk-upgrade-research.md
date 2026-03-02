# Agent SDK Upgrade Research — Session Transfer Document

> **Date:** 2026-03-01
> **Status:** Done (implemented 2026-03-02)
> **Context:** This document captures the full findings from a research session analyzing whether the claude-view sidecar needs the Agent SDK, what version to use, and what to change.

---

## Questions Answered

### Q1: Do we need `/agent-sdk-dev:new-sdk-app` to scaffold a new app?

**Answer: No.**

The sidecar (`sidecar/`) already exists as a fully built application:
- `sidecar/package.json` — deps: `@anthropic-ai/claude-agent-sdk`, `hono`, `@hono/node-server`, `ws`
- `sidecar/src/index.ts` — Hono HTTP server on Unix socket
- `sidecar/src/session-manager.ts` — multi-session state machine using SDK
- `sidecar/src/control.ts` — REST endpoints (resume, send, sessions, terminate)
- `sidecar/src/ws-handler.ts` — WebSocket bidirectional relay
- `sidecar/src/health.ts` — health check handler
- `sidecar/src/types.ts` — shared IPC message types

The `/agent-sdk-dev:new-sdk-app` skill scaffolds a **new** Agent SDK application from scratch. Our sidecar is past that stage.

### Q2: Would replacing the sidecar with a "new SDK app" help?

**Answer: No, it would hurt.**

The sidecar and an "Agent SDK app" are fundamentally different things:

| | Agent SDK App | Our Sidecar |
|---|---|---|
| **Role** | IS the AI agent (uses Claude as its brain) | PROXY between Rust server + React frontend and Claude Code |
| **SDK usage** | Core architecture — tools, prompts, conversation loop | Library — calls `unstable_v2_resumeSession()` and `.stream()` |
| **HTTP server** | Not typically needed | Essential (Hono on Unix socket, proxied by Axum) |
| **WebSocket** | Not typically needed | Essential (bidirectional streaming to React frontend) |
| **Multi-session** | Usually single session | Manages N concurrent control sessions |

The SDK is a **library** here, not the architecture. The actual SDK touchpoints are only 4 calls in `session-manager.ts`:
1. `unstable_v2_resumeSession(sessionId, { model, env })` (line 93)
2. `cs.sdkSession.send(content)` (line 131)
3. `for await (const msg of cs.sdkSession.stream())` (line 136)
4. `cs.sdkSession.close()` (line 220)

Everything else is custom server logic (HTTP routes, WebSocket relay, permission infrastructure, health checks).

### Q3: What is the latest Agent SDK version?

**Answer: `0.2.63`** (published 2026-02-28)

The sidecar pins `"@anthropic-ai/claude-agent-sdk": "^0.1.0"` — that's ~60 releases behind.

Release cadence is fast — nearly daily publishes in late Feb 2026:
- 0.2.63 — 2026-02-28
- 0.2.62 — 2026-02-27
- 0.2.61 — 2026-02-26
- 0.2.59 — 2026-02-26
- 0.2.58 — 2026-02-25

---

## SDK v0.2.63 API Surface (from inspecting `sdk.d.ts`)

### Two APIs: V1 (stable) and V2 (unstable preview)

**V1 — `query()` (stable, recommended):**
```ts
import { query } from "@anthropic-ai/claude-agent-sdk";

for await (const msg of query({
  prompt: "Fix the auth bug",
  options: {
    resume: sessionId,           // resume existing session
    cwd: "/path/to/project",     // first-class option
    model: "claude-sonnet-4-20250514",
    canUseTool: async (toolName, input, { signal, suggestions }) => {
      // Permission routing — return allow or deny
      return { behavior: "allow", updatedInput: input };
    },
    allowedTools: ["Read", "Edit", "Bash"],
    disallowedTools: ["Write"],
    permissionMode: "default",   // "default" | "acceptEdits" | "plan" | "dontAsk" | "bypassPermissions"
    hooks: {
      PostToolUse: [{ matcher: "Edit|Write", hooks: [logFileChange] }],
      Elicitation: [...],
      PermissionRequest: [...],
    },
    agents: {
      "code-reviewer": {
        description: "Reviews code",
        prompt: "You are a code reviewer...",
        tools: ["Read", "Glob", "Grep"],
      }
    },
    forkSession: false,          // true = branch without mutating original
    tools: ["Read", "Edit", "Bash", "AskUserQuestion"],
    mcpServers: { ... },
    settingSources: ["project"],
    enableFileCheckpointing: true,
  }
})) {
  // Process messages...
}
```

**V2 — `unstable_v2_*` (preview, what sidecar currently uses):**
```ts
import { unstable_v2_resumeSession } from "@anthropic-ai/claude-agent-sdk";

const session = unstable_v2_resumeSession(sessionId, {
  model: "claude-sonnet-4-20250514",
  canUseTool: async (toolName, input, { signal, suggestions }) => { ... },  // NOW AVAILABLE
  allowedTools: ["Read", "Edit"],
  disallowedTools: [...],
  permissionMode: "default",
  hooks: { ... },
  env: { ... },
});

await session.send("Fix the auth bug");
for await (const msg of session.stream()) { ... }
session.close();
```

**Also available:**
```ts
unstable_v2_createSession(options)    // create new session
unstable_v2_prompt(message, options)  // single prompt, returns SDKResultMessage
```

### New Utility Functions (not in v0.1.0)
```ts
listSessions(options?)                              // v0.2.53 — discover past sessions
getSessionMessages(sessionId, { limit, offset })    // v0.2.59 — read session history with pagination
```

### Key Types

**`SDKSessionOptions` (V2)** — now includes:
- `canUseTool?: CanUseTool` — **was missing in v0.1.0, NOW AVAILABLE**
- `allowedTools?: string[]`
- `disallowedTools?: string[]`
- `permissionMode?: PermissionMode`
- `hooks?: Partial<Record<HookEvent, HookCallbackMatcher[]>>`
- `env?: Record<string, string | undefined>`

**`CanUseTool` type:**
```ts
type CanUseTool = (
  toolName: string,
  input: Record<string, unknown>,
  options: {
    signal: AbortSignal;
    suggestions?: PermissionUpdate[];
    filePath?: string;
  }
) => Promise<PermissionResult>;

// PermissionResult is either:
// { behavior: "allow", updatedInput: Record<string, unknown> }
// { behavior: "deny", message: string }
```

**`AskUserQuestion` handling:**
- Routed through the SAME `canUseTool` callback
- Check `toolName === "AskUserQuestion"`
- Input contains `{ questions: [...], ... }`
- Return `{ behavior: "allow", updatedInput: { questions, answers: { "question text": "selected label" } } }`

**`SDKMessage` union types:**
- `type: 'system'` with `subtype: 'init'` — contains `session_id`
- `type: 'assistant'` — complete assistant message with content blocks (text, tool_use)
- `type: 'user'` — tool results come back as user messages
- `type: 'result'` with `subtype: 'success' | 'error'` — contains `total_cost_usd`, `num_turns`, `stop_reason`
- `type: 'stream_event'` — real-time chunks (content_block_delta)

**`SDKSession` interface (V2):**
```ts
interface SDKSession {
  readonly sessionId: string;
  send(message: string | SDKUserMessage): Promise<void>;
  stream(): AsyncGenerator<SDKMessage, void>;
  close(): void;
  [Symbol.asyncDispose](): Promise<void>;
}
```

**Hook events available:**
`PreToolUse`, `PostToolUse`, `PostToolUseFailure`, `Notification`, `UserPromptSubmit`, `SessionStart`, `SessionEnd`, `Stop`, `SubagentStart`, `SubagentStop`, `PreCompact`, `PermissionRequest`, `Setup`, `TeammateIdle`, `TaskCompleted`, `Elicitation`, `ElicitationResult`, `ConfigChange`, `WorktreeCreate`, `WorktreeRemove`

---

## What the Sidecar Currently Does Wrong (v0.1.0 limitations)

From `sidecar/src/session-manager.ts` comments (lines 77-92):

```
SDK V2 LIMITATION (verified against installed `@anthropic-ai/claude-agent-sdk` types):

`SDKSessionOptions` only accepts: { model, pathToClaudeCodeExecutable?, executable?,
  executableArgs?, env? }

The following fields are NOT available in V2:
  - `cwd` -- pass via env: { CLAUDE_CWD: projectPath } if the SDK respects it, or omit
  - `canUseTool` -- NOT available in V2
  - `includePartialMessages` -- NOT available in V2
```

**Every single one of these limitations is fixed in v0.2.63:**

| Limitation in v0.1.0 | Status in v0.2.63 |
|---|---|
| `canUseTool` not in V2 SDKSessionOptions | **FIXED** — `canUseTool?: CanUseTool` is on SDKSessionOptions |
| `cwd` not available | **FIXED** — `cwd?: string` on V1 Options; V2 still uses env but V1 is recommended anyway |
| `includePartialMessages` not available | **N/A** — streaming comes via `stream()` events in V2 |
| No `allowedTools` | **FIXED** — `allowedTools?: string[]` on both V1 and V2 |
| No `disallowedTools` | **FIXED** — `disallowedTools?: string[]` on both V1 and V2 |
| No `permissionMode` | **FIXED** — `permissionMode?: PermissionMode` on both V1 and V2 |
| No hooks | **FIXED** — full hook system with 20+ event types |
| Phase F.2 TODO: permission routing | **UNBLOCKED** — canUseTool callback receives toolName + input |

---

## Recommended Action Plan

### Step 1: Bump dependency
```bash
cd sidecar && bun add @anthropic-ai/claude-agent-sdk@^0.2.63
```

### Step 2: Rewrite `session-manager.ts`

Key changes:
1. Add `canUseTool` callback to `SDKSessionOptions` in `resume()` method
2. Wire `canUseTool` to the existing `pendingPermissions` map + WebSocket relay
3. Handle `AskUserQuestion` inside the same `canUseTool` callback (check `toolName === "AskUserQuestion"`)
4. Add `allowedTools` / `permissionMode` support (pass through from frontend)
5. Remove the `CLAUDE_CWD` env hack comment, document that V1 query() supports `cwd` natively

### Step 3: Wire interactive cards (chat-input-bar-impl.md)

With `canUseTool` available:
- `PermissionCard` → wired via `canUseTool` callback → WebSocket → frontend → user clicks Allow/Deny → resolve promise
- `AskUserQuestionCard` → same `canUseTool` callback, `toolName === "AskUserQuestion"` → render card → user selects options → return answers
- `PlanApprovalCard` → needs investigation — may use `permissionMode: "plan"` + hooks
- `ElicitationCard` → `Elicitation` hook event → WebSocket → frontend → user responds

### Step 4: Consider V1 vs V2 API choice

**Option A: Stay on V2 (current approach, updated)**
- Pro: Multi-turn within single process (no re-spawn per message)
- Pro: Already coded this way
- Con: Still `unstable_` prefix, API may change
- Con: V2 `SDKSessionOptions` is a subset of V1 `Options` (no `cwd`, `forkSession`, `agents`, `tools`, `mcpServers`)

**Option B: Switch to V1 `query()` with `resume`**
- Pro: Stable API, fully featured
- Pro: Has `cwd`, `forkSession`, `agents`, `tools`, `mcpServers`, `settingSources`
- Pro: Simpler — one `query()` call per user message
- Con: Re-spawns Claude Code process per message (but `resume` handles context loading)
- Con: Would require rewriting the send/stream pattern

**Recommendation: Option A (stay V2, bump version) for now.** The V2 API is marked `@alpha` but is actively developed and matches the sidecar's multi-turn pattern. Switch to V1 if V2 is deprecated or if you need V1-only features like `forkSession` or `agents`.

---

## Key Files Referenced

| File | What |
|------|------|
| `sidecar/package.json` | Current dep: `@anthropic-ai/claude-agent-sdk: ^0.1.0` |
| `sidecar/src/session-manager.ts` | Main file to rewrite — 235 lines, SDK integration |
| `sidecar/src/types.ts` | WebSocket message types (ServerMessage union) |
| `sidecar/src/control.ts` | REST endpoints — resume, send, sessions |
| `sidecar/src/ws-handler.ts` | WebSocket relay handler |
| `docs/plans/mission-control/phase-f-interactive.md` | Phase F design doc |
| `docs/plans/web/2026-02-28-chat-input-bar-design.md` | ChatInputBar + interactive cards design |
| `docs/plans/web/2026-02-28-chat-input-bar-impl.md` | ChatInputBar implementation plan |
| `apps/web/src/hooks/use-control-session.ts` | Frontend WebSocket state hook |
| `apps/web/src/types/control.ts` | Frontend TypeScript types for control messages |
| `apps/web/src/components/live/ResumePreFlight.tsx` | Pre-flight cost estimate modal |
| `apps/web/src/components/live/DashboardChat.tsx` | Dashboard chat view (to be deleted per chat-input-bar plan) |
| `apps/web/src/pages/ControlPage.tsx` | Control page route (to be deleted per chat-input-bar plan) |

## External References

| Resource | Location |
|----------|----------|
| Agent SDK repo (cloned) | `/Users/TBGor/dev/@vicky-ai/claude-agent-sdk-typescript/` |
| SDK npm page | https://www.npmjs.com/package/@anthropic-ai/claude-agent-sdk |
| SDK docs — overview | https://platform.claude.com/docs/en/agent-sdk/overview |
| SDK docs — sessions | https://platform.claude.com/docs/en/agent-sdk/sessions |
| SDK docs — user input | https://platform.claude.com/docs/en/agent-sdk/user-input |
| SDK docs — TypeScript API | https://platform.claude.com/docs/en/agent-sdk/typescript |
| SDK docs — V2 preview | https://platform.claude.com/docs/en/agent-sdk/typescript-v2-preview |
| SDK docs — permissions | https://platform.claude.com/docs/en/agent-sdk/permissions |
| SDK docs — hooks | https://platform.claude.com/docs/en/agent-sdk/hooks |
| SDK docs — migration guide | https://platform.claude.com/docs/en/agent-sdk/migration-guide |
| SDK changelog | `/Users/TBGor/dev/@vicky-ai/claude-agent-sdk-typescript/CHANGELOG.md` |
| SDK type definitions (v0.2.63) | `/tmp/package/sdk.d.ts` (from `npm pack`) |

---

## Notable Changelog Entries (v0.1.0 → v0.2.63)

| Version | Change |
|---------|--------|
| 0.2.63 | Fixed `pathToClaudeCodeExecutable` PATH resolution; added `supportedAgents()` method |
| 0.2.59 | **`getSessionMessages()`** — read session history with pagination |
| 0.2.53 | **`listSessions()`** — discover and list past sessions |
| 0.2.51 | Fixed `session.close()` killing subprocess before persisting data (broke `resumeSession`); added `task_progress` events; fixed unbounded memory growth |
| 0.2.49 | Permission suggestions populated for safety checks; added `ConfigChange` hook |
| 0.2.47 | Added `promptSuggestion()` method; `tool_use_id` in `task_notification` events |
| 0.2.45 | Sonnet 4.6 support; `task_started` system message; fixed `Session.stream()` returning early with background subagents |
| 0.2.33 | `TeammateIdle` and `TaskCompleted` hook events; `sessionId` option for custom UUIDs |
| 0.2.31 | `stop_reason` field on `SDKResultSuccess` and `SDKResultError` |
| 0.2.21 | `reconnectMcpServer()`, `toggleMcpServer()` methods; fixed PermissionRequest hooks in SDK mode |
| 0.2.15 | Notification hook support; `close()` method on Query interface |
