# Phase F: Implementation Plan Audit Results & Corrected Reference

> **Purpose:** Portable session context. Paste this into a new chat to resume Phase F work with full audit knowledge.
>
> **Date:** 2026-02-27
> **Plan file:** `docs/plans/mission-control/phase-f-impl.md`
> **Design doc:** `docs/plans/mission-control/phase-f-interactive.md`

---

## TL;DR

The Phase F implementation plan has **10 compile blockers, 10 high-severity behavioral bugs, and 7 warnings**. The plan's architecture is sound (Node.js sidecar + Unix socket + WS relay), but three areas need complete rewrites before execution:

1. **Agent SDK API is fictional** — `session-manager.ts` (Task 6) uses function names, parameter shapes, and event types that don't exist in the real SDK
2. **hyperlocal is incompatible** — `hyperlocal 0.9` requires `hyper 0.14`, but the plan adds `hyper 1.x`. Dependency conflict.
3. **Hono Unix socket pattern is broken** — `serve()+close()+emit('connection')` hack doesn't work. Use `createAdaptorServer()`.

**Verdict: FAIL — Do not execute without rework.** The fixes below make it executable.

---

## What Phase F Does

Allows users to **resume and interact with Claude Code sessions from the web dashboard**. Architecture:

```
Browser ←WS→ Axum (Rust) ←WS-over-Unix-socket→ Node.js Sidecar ←Agent SDK→ Claude Code subprocess
```

- Rust server manages a lazy-started Node.js sidecar child process
- Sidecar uses `@anthropic-ai/claude-agent-sdk` to spawn/resume Claude Code sessions
- Communication: Axum ↔ Unix domain socket HTTP/WS ↔ Sidecar ↔ Agent SDK
- Frontend connects via Axum WebSocket proxy

---

## Blocker Fixes Required (10 items)

### B1: `db.get_session_by_id()` does not exist

**Task 5, `routes/control.rs`**

The `Database` struct has no `get_session_by_id()` method. Must be added to `crates/db/src/queries/sessions.rs`.

```rust
// Add to crates/db/src/queries/sessions.rs
pub async fn get_session_by_id(&self, id: &str) -> DbResult<Option<SessionInfo>> {
    // Query sessions table by ID, map to SessionInfo
}
```

### B2: Wrong SessionInfo field names

**Task 5, `routes/control.rs`**

The plan uses fictional field names. Correct mapping:

| Plan uses | Actual field | Type |
|-----------|-------------|------|
| `session.model` | `session.primary_model` | `Option<String>` |
| `session.updated_at` | `session.modified_at` | `i64` |
| `session.title` | Does not exist. Use `session.longest_task_preview` or omit | `Option<String>` |
| `session.project_dir` | `session.project_path` | `String` |
| `session.files_modified` | `session.files_edited_count` | `u32` (not Option) |
| `session.turn_count` | `session.turn_count_api` | `Option<u64>` (not usize) |
| `session.total_input_tokens` | Same name, but type is `Option<u64>` | — |

Reference: `crates/core/src/types.rs` lines 232-360.

### B3: hyperlocal 0.9 incompatible with hyper 1.x

**Task 3, `Cargo.toml`**

`hyperlocal = "0.9"` depends on `hyper 0.14`. The plan also adds `hyper = "1"`. These cannot coexist.

**Fix:** Replace `hyperlocal` with one of:
- `hyper-client-sockets` (crates.io — supports hyper 1.x)
- Raw `tokio::net::UnixStream` + manual HTTP/1.1 via `hyper::client::conn::http1`

Also add `http-body-util = "0.1"` to workspace deps (needed for body types in hyper 1.x).

### B4 + B5: hyper 1.x body type API changes

**Task 8, `routes/control.rs`**

In hyper 1.x, `Body` is a trait, not a struct. `hyper::body::Body::from()` and `::empty()` don't exist.

**Fix:**
```rust
// Instead of:
req.body(hyper::body::Body::from(body))      // WRONG
req.body(hyper::body::Body::empty())          // WRONG
Body::from(resp.into_body())                  // WRONG

// Use:
use http_body_util::{Full, Empty, BodyExt};
req.body(Full::new(bytes::Bytes::from(body))) // request body
req.body(Empty::<bytes::Bytes>::new())        // empty body

// For response conversion:
let bytes = resp.into_body().collect().await?.to_bytes();
axum::body::Body::from(bytes)
```

### B6: Axum 0.8 WS `Message::Text` uses `Utf8Bytes`, not `String`

**Task 10, `routes/control.rs`**

In Axum 0.8 + tungstenite 0.28, `Message::Text` wraps `Utf8Bytes`, not `String`. Cross-type relay needs conversions.

**Fix:** Follow the existing pattern in `crates/server/src/routes/terminal.rs`:
```rust
// Sending text:
socket.send(Message::Text(json_string.into())).await  // .into() converts String to Utf8Bytes

// Receiving text:
Message::Text(text) => {
    let s: &str = text.as_ref();  // or text.to_string()
    // ...
}
```

### B7: Adding `sidecar` field breaks test struct literals

**Task 4, `state.rs`**

Adding a new field to `AppState` breaks ALL places that construct it with struct literals, including test files. The plan mentions 3 constructors but there may be inline `AppState { ... }` in tests.

**Fix:** Search all files for `AppState {` and add `sidecar: Arc::new(SidecarManager::new())` to every occurrence.

### B8: WebSocket cleanup ordering bug

**Task 11, `use-control-session.ts`**

```typescript
// WRONG — stale guard fires in onclose, skipping state update
return () => {
  clearInterval(pingInterval)
  wsRef.current = null  // ← nulled BEFORE close
  ws.close()
}

// CORRECT — close first, then null
return () => {
  clearInterval(pingInterval)
  ws.close()
  wsRef.current = null
}
```

Better yet, follow the existing `useTerminalSocket` pattern with `intentionalCloseRef`.

### B9: Missing Vite WS proxy entry

**NOT IN PLAN — must be added**

The current Vite config has `ws: true` only for `/api/live/sessions`. The new control WS at `/api/control/sessions/:id/stream` will silently fail to upgrade in dev mode.

**Fix:** Add to `apps/web/vite.config.ts`, BEFORE the catch-all `/api`:
```typescript
proxy: {
  '/api/live/sessions': { target: 'http://localhost:47892', ws: true },
  '/api/control/sessions': { target: 'http://localhost:47892', ws: true },  // NEW
  '/api': 'http://localhost:47892',
}
```

---

## High-Severity Fixes Required (10 items)

### H1 + H2 + H3: Agent SDK API is completely wrong

**Task 6, `session-manager.ts`** — The entire file must be rewritten. See "Correct Agent SDK API" section below.

### H4: Hono Unix socket pattern is broken

**Task 2 + 9, `sidecar/src/index.ts`**

```typescript
// WRONG — undocumented hack, server.emit('connection') on closed server
const server = serve({ fetch: app.fetch, port: 0 })
server.close()
const unixServer = net.createServer((socket) => {
  server.emit('connection', socket)
})

// CORRECT — use createAdaptorServer
import { createAdaptorServer } from '@hono/node-server'

if (fs.existsSync(SOCKET_PATH)) fs.unlinkSync(SOCKET_PATH)
const server = createAdaptorServer(app)

// WS upgrade handler on the HTTP server (not a separate net.Server)
const wss = new WebSocketServer({ noServer: true })
server.on('upgrade', (req, socket, head) => {
  const match = req.url?.match(/\/control\/sessions\/([^/]+)\/stream/)
  if (!match) { socket.destroy(); return }
  wss.handleUpgrade(req, socket, head, (ws) => {
    handleWebSocket(ws, match[1], sessionManager)
  })
})

server.listen(SOCKET_PATH, () => {
  console.log(`[sidecar] Listening on ${SOCKET_PATH}`)
})
```

### H5: Hardcoded `localhost:47892` in frontend hook

**Task 11, `use-control-session.ts`**

Use existing utility instead:
```typescript
import { wsUrl } from '../lib/ws-url'
// ...
const ws = new WebSocket(wsUrl(`/api/control/sessions/${controlId}/stream`))
```

### H6: Route double-prefix `.nest("/api", control::router())`

**Task 5, `routes/mod.rs`**

Routes inside `api_routes()` already get the `/api` prefix. Using `.nest("/api", ...)` would create `/api/api/control/...`.

**Fix:** Use `.nest("/api", control::router())` at the TOP level (same as all other routes), OR use `.merge(control::router())` inside the existing `api_routes()` function. Check `routes/mod.rs` for the actual pattern — it uses `.nest("/api", ...)` at the router assembly level.

### H7: No reconnect logic in WS hook

**Task 11** — Follow `useTerminalSocket` pattern with exponential backoff, or at minimum expose a `reconnect()` callback.

### H8: Hand-rolled overlay violates CLAUDE.md

**Task 14, `PermissionDialog.tsx`** — Use `@radix-ui/react-dialog` with Portal, not `z-50` fixed div.

### H9: DashboardChat needs both controlId and sessionId

**Task 13** — Component receives `controlId` (for WS) but history API needs `sessionId`. Props must include both. Also, the referenced `GET /api/session/:project/:id/messages` endpoint does not exist — check available session endpoints.

### H10: Pin SDK version

**Task 1, `sidecar/package.json`** — Change `"latest"` to a pinned version:
```json
"@anthropic-ai/claude-agent-sdk": "^0.1.0"
```
(Replace with actual current version from `npm info @anthropic-ai/claude-agent-sdk`)

---

## Correct Agent SDK API Reference

### Exported Functions

**V2 (the ones Phase F uses):**
```typescript
unstable_v2_createSession(options: { model: string; ... }): SDKSession  // synchronous
unstable_v2_resumeSession(sessionId: string, options: { model: string; ... }): SDKSession  // synchronous
unstable_v2_prompt(prompt: string, options: { model: string; ... }): Promise<SDKResultMessage>
```

**V1 (alternative):**
```typescript
query({ prompt, options }): Query  // async generator + rich control methods
```

### SDKSession Interface

```typescript
interface SDKSession {
  readonly sessionId: string
  send(message: string | SDKUserMessage): Promise<void>
  stream(): AsyncGenerator<SDKMessage, void>
  close(): void  // synchronous, NOT async
}
```

### Options Type (relevant subset)

```typescript
interface Options {
  model: string
  cwd?: string
  env?: Record<string, string | undefined>
  permissionMode?: PermissionMode  // 'default' | 'acceptEdits' | 'bypassPermissions' | ...
  hooks?: Partial<Record<HookEvent, HookCallbackMatcher[]>>
  canUseTool?: CanUseTool  // Alternative permission function
  includePartialMessages?: boolean  // Enable streaming chunks
  maxTurns?: number
  maxBudgetUsd?: number
  allowedTools?: string[]
  disallowedTools?: string[]
  systemPrompt?: string
  persistSession?: boolean  // default true
}
```

### Hooks Format

```typescript
// Hook events
type HookEvent = "PreToolUse" | "PostToolUse" | "PermissionRequest" | "Stop" | "SessionStart" | ...

// Configuration
hooks: {
  PermissionRequest: [{
    matcher?: "Write|Edit|Bash",  // regex for tool names (optional)
    hooks: [myPermissionHandler],
    timeout?: 60
  }]
}

// Callback signature
type HookCallback = (
  input: HookInput,
  toolUseID: string | undefined,
  options: { signal: AbortSignal }
) => Promise<HookJSONOutput>

// Permission hook return format
return {
  hookSpecificOutput: {
    hookEventName: "PermissionRequest",
    decision: {
      behavior: "allow",  // or "deny"
      updatedInput?: Record<string, unknown>,
    }
  }
}
// OR for deny:
return {
  hookSpecificOutput: {
    hookEventName: "PermissionRequest",
    decision: {
      behavior: "deny",
      message: "User denied permission",
    }
  }
}
// OR empty object to use default behavior:
return {}
```

### Alternative: `canUseTool` (simpler permission routing)

```typescript
// Passed in Options, simpler than hooks for permission-only use case
canUseTool: async (toolName, input, { signal, toolUseID }) => {
  // Route to frontend, wait for response
  const allowed = await askFrontendForPermission(toolName, input)
  if (allowed) {
    return { behavior: "allow" }
  }
  return { behavior: "deny", message: "User denied" }
}
```

### Streaming Event Types

```typescript
type SDKMessage =
  | SDKAssistantMessage     // type: "assistant" — contains message.content blocks
  | SDKUserMessage          // type: "user" — tool results come as user messages
  | SDKResultMessage        // type: "result" — terminal, always last
  | SDKSystemMessage        // type: "system", subtype: "init" — session metadata
  | SDKPartialAssistantMessage  // type: "stream_event" — real-time chunks (needs includePartialMessages)
  | SDKToolProgressMessage  // type: "tool_progress" — heartbeat during tool execution
  | SDKStatusMessage        // type: "system", subtype: "status"
  | ...
```

**Key: How text and tool calls appear in the stream:**

1. `SDKSystemMessage` (`type: "system"`, `subtype: "init"`) — once at session start
2. `SDKAssistantMessage` (`type: "assistant"`) — `message.content` is an array:
   - `{ type: "text", text: "..." }` — text response
   - `{ type: "tool_use", id: "toolu_xxx", name: "Bash", input: { command: "ls" } }` — tool call
3. `SDKUserMessage` (`type: "user"`) — tool result (injected back as user message)
4. More `SDKAssistantMessage`s as Claude processes tool results
5. `SDKResultMessage` (`type: "result"`, `subtype: "success"`) — final, includes `total_cost_usd`, `num_turns`

**For real-time streaming chunks** (set `includePartialMessages: true`):
- `SDKPartialAssistantMessage` (`type: "stream_event"`) wraps `BetaRawMessageStreamEvent`:
  - `content_block_start`, `content_block_delta` (with `delta.text`), `content_block_stop`
  - `message_start`, `message_delta`, `message_stop`

### Corrected session-manager.ts Resume Pattern

```typescript
// WRONG (what the plan says):
const sdkSession = await unstable_v2_resumeSession({
  sessionId,
  env: { ...process.env },
  hooks: { async onPermissionRequest(req) { return { allowed } } }
})

// CORRECT:
import { unstable_v2_resumeSession, type SDKMessage } from '@anthropic-ai/claude-agent-sdk'

const sdkSession = unstable_v2_resumeSession(sessionId, {
  model: model ?? 'claude-sonnet-4-20250514',
  includePartialMessages: true,  // for streaming chunks
  canUseTool: async (toolName, input, { signal }) => {
    const allowed = await cs.requestPermission(toolName, input)
    return allowed
      ? { behavior: 'allow' }
      : { behavior: 'deny', message: 'User denied permission' }
  },
})
```

### Corrected sendMessage Pattern

```typescript
// WRONG (what the plan says):
const query = this.sdkSession.unstable_v2_prompt(content)
for await (const event of query.stream()) {
  switch (event.type) {
    case 'text': ...      // doesn't exist
    case 'tool_use': ...  // doesn't exist
  }
}

// CORRECT:
await this.sdkSession.send(content)
for await (const msg of this.sdkSession.stream()) {
  switch (msg.type) {
    case 'stream_event': {
      // Real-time text chunks (needs includePartialMessages: true)
      const event = msg.event
      if (event.type === 'content_block_delta' && event.delta?.type === 'text_delta') {
        this.emit({ type: 'assistant_chunk', content: event.delta.text, messageId })
      }
      break
    }
    case 'assistant': {
      // Complete assistant message with all content blocks
      for (const block of msg.message.content) {
        if (block.type === 'text') {
          // Full text (if not using streaming chunks)
        }
        if (block.type === 'tool_use') {
          this.emit({ type: 'tool_use_start', toolName: block.name, toolInput: block.input, toolUseId: block.id })
        }
      }
      break
    }
    case 'user': {
      // Tool results come back as user messages
      // Extract tool_use_result if present
      break
    }
    case 'result': {
      // Session complete
      if (msg.subtype === 'success') {
        this.totalCost = msg.total_cost_usd
        this.turnCount = msg.num_turns
      }
      break
    }
  }
}
```

---

## Codebase Patterns to Follow

### WebSocket Message Types (from `terminal.rs`)

```rust
// Sending text — .into() converts String to Utf8Bytes
socket.send(Message::Text(json_string.into())).await;

// Receiving text
Some(Ok(Message::Text(text))) => {
    let parsed: ClientMessage = serde_json::from_str(&text)?;
}

// Heartbeat
socket.send(Message::Ping(vec![].into())).await;
```

### AppState Constructor Pattern

```rust
// All 3 constructors must include the new field:
pub fn new(db: Database) -> Arc<Self> {
    Arc::new(Self {
        // ... existing fields ...
        sidecar: Arc::new(SidecarManager::new()),  // ADD THIS
    })
}
```

### Route Registration Pattern

```rust
// In routes/mod.rs — all routes use .nest("/api", ...) at the assembly level
pub fn api_routes(state: Arc<AppState>) -> Router {
    Router::new()
        .nest("/api", sessions::router())
        .nest("/api", projects::router())
        .nest("/api", control::router())    // ADD THIS
        .nest("/api/live", terminal::router())
        // ...
        .with_state(state)
}
```

### Frontend WS URL Pattern

```typescript
import { wsUrl } from '../lib/ws-url'
const ws = new WebSocket(wsUrl('/api/control/sessions/${controlId}/stream'))
// Dev: ws://localhost:47892/api/control/sessions/xxx/stream
// Prod: wss://hostname/api/control/sessions/xxx/stream
```

---

## Warnings (7 items, non-blocking but should fix)

| # | Task | Issue | Fix |
|---|------|-------|-----|
| W1 | T1,2,7 | `npm install`/`npm run` in sidecar. CLAUDE.md says use Bun for dev | Change to `bun install`/`bun run` |
| W2 | T4 | "Step 11" reference in main.rs is ambiguous | Show actual insertion point near `with_graceful_shutdown` |
| W3 | T10 | WS relay only handles Text+Close. Binary/Ping/Pong dropped | Add Ping/Pong passthrough |
| W4 | T12 | Radix Dialog has no built-in bottom sheet. Plan says "mobile = bottom sheet" | Use centered dialog on all viewports, or CSS overrides for mobile |
| W5 | T15 | `CacheCountdownBar.tsx` already exists in `live/`. May duplicate `ChatStatusBar.tsx` | Review before creating |
| W6 | T16 | Forward-references design doc Step 9 for CI YAML without inlining | Read actual `.github/workflows/release.yml` first |
| W7 | T6 | `await session.close()` — close() is sync in V2 | Remove `await` |

---

## Task Execution Order (unchanged from plan)

```
Layer 1: Sidecar Foundation
  Task 1 → Task 2 → Task 4 (scaffold → server → wire into Axum)
                └──→ Task 3 (Rust SidecarManager, parallel with Task 2)

Layer 2: Session Control
  Task 5 (cost estimation — Rust only, independent)
  Task 6 → Task 7 → Task 8 (session manager → control routes → Rust proxy)

Layer 3: WebSocket
  Task 9 → Task 10 → Task 11 (sidecar WS → Rust WS proxy → frontend hook)

Layer 4: Frontend UI
  Task 12 (pre-flight modal, needs Task 5 + Task 8)
  Task 13 (dashboard chat, needs Task 11)
  Task 14 (permission dialog, needs Task 11)
  Task 15 (cost status bar, needs Task 13)

Layer 5: Integration
  Task 16 (distribution)
  Task 17 (end-to-end verification)
```

---

## Files You Must Read Before Starting

| File | Why |
|------|-----|
| `crates/core/src/types.rs:232` | `SessionInfo` struct — actual field names |
| `crates/server/src/state.rs` | `AppState` struct + 3 constructors |
| `crates/server/src/routes/mod.rs` | Route registration pattern |
| `crates/server/src/routes/terminal.rs` | Existing WS handler — copy patterns for Utf8Bytes, Message types |
| `crates/server/Cargo.toml` | Already has `tokio-tungstenite`, `futures-util` |
| `Cargo.toml` (root) | Workspace deps — axum 0.8, tokio-tungstenite 0.28 |
| `apps/web/src/lib/ws-url.ts` | Existing WS URL utility — use instead of hardcoding |
| `apps/web/vite.config.ts` | Must add `ws: true` entry for control WS |
| `apps/web/src/hooks/use-terminal-socket.ts` | Reference WS hook pattern (stale guard, reconnect, intentionalCloseRef) |
| `apps/web/tsconfig.json` | Strict mode: `noUnusedLocals`, `noUnusedParameters`, `erasableSyntaxOnly` |
| `crates/db/src/queries/sessions.rs` | Must add `get_session_by_id()` method |

---

## Status

- **Phase F status in PROGRESS.md:** `ready` (impl plan done, not yet started)
- **Phases A-D.2:** All `done`
- **Phase E:** `pending` (Custom Layout — independent of F)
- **Phases G-J:** `pending`

---

## Instruction for New Session

> Execute Phase F implementation plan at `docs/plans/mission-control/phase-f-impl.md` using `superpowers:executing-plans`.
>
> **CRITICAL:** Before executing, apply ALL fixes from `docs/plans/mission-control/phase-f-audit-results.md`. The plan has 10 compile blockers and 10 high-severity bugs that must be fixed first.
>
> Key corrections:
> 1. Replace `hyperlocal` with `hyper-client-sockets` or raw `UnixStream` (hyper 1.x compat)
> 2. Rewrite `session-manager.ts` against real Agent SDK API (see audit doc for correct patterns)
> 3. Use `createAdaptorServer()` for Hono Unix socket (not `serve()+close()` hack)
> 4. Fix all SessionInfo field names (see B2 mapping table)
> 5. Add `get_session_by_id()` to `crates/db/`
> 6. Add Vite WS proxy entry for `/api/control/sessions`
> 7. Use existing `wsUrl()` utility, not hardcoded `localhost:47892`
> 8. Follow `terminal.rs` pattern for WS Message types (`Utf8Bytes`, `.into()`)
