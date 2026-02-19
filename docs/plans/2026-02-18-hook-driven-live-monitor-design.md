---
status: draft
date: 2026-02-18
---

# Hook-Driven Live Monitor Redesign

## Problem

A power user has 3 repos, each with 3 worktrees = 9 projects. Each project runs up to 6 Claude Code sessions. That's **54 concurrent sessions** across dozens of terminal tabs. The user frantically switches screens trying to find which sessions finished, which need permission, which errored.

Mission Control solves this: a single web dashboard showing ALL active sessions in a Kanban board (Needs You | Running). Cards show state, cost, activity, and context usage at a glance.

## Current Architecture (JSONL-Primary)

Today, the live monitor works by:

1. **File watcher** scans `~/.claude/projects/` for JSONL files (1-2s latency)
2. **JSONL tail parser** reads new lines, derives everything: state, tokens, messages, sub-agents
3. **Process detector** polls every 2s for PIDs to determine if sessions are alive
4. **Hooks** (10 events) override agent state temporarily via StateResolver

This works but has fundamental limitations:
- Session discovery waits for the JSONL file to appear (1-2s after session starts)
- State derivation from JSONL is **inference** — we guess "idle" from `end_turn` + time elapsed
- User interrupts are invisible (the `Stop` hook doesn't fire on interrupt)
- User messages arrive 1-2s late (after JSONL write + file watcher + parse)
- Complex classifier heuristics try to guess what Claude Code already knows

## New Architecture (Hooks-Primary)

**Core principle: hooks own state, JSONL owns data. Separate concerns, no dual-path fallback.**

Claude Code hooks are real-time events from Claude Code's own runtime. They are authoritative — when Claude Code says "Stop" fired, that's ground truth. When we parse JSONL and guess "end_turn means idle," that's inference. Don't mix the two.

```
┌──────────────────────────────────┐
│  Claude Code Hooks (real-time)   │
│  HTTP POST to /api/live/hook     │
│                                  │
│  Source of truth for:            │
│  - Session lifecycle (start/end) │
│  - Agent state (idle, working,   │
│    needs_permission, interrupted)│
│  - User messages (prompt text)   │
│  - Current activity (tool use)   │
└──────────────┬───────────────────┘
               │
┌──────────────▼───────────────────┐
│  Live Session Store (in-memory)  │
│  State: from hooks               │
│  Data: from JSONL enrichment     │
└──────────────┬───────────────────┘
               │
┌──────────────▼───────────────────┐
│  JSONL Tail Parser (background)  │
│  Data enrichment only:           │
│  - Token counts + cost           │
│  - Context window fill           │
│  - Sub-agent details             │
│  - Task/todo progress items      │
│  - Git branch                    │
│  - Model (confirmed)             │
└──────────────────────────────────┘
```

**No JSONL state fallback box.** The existing `derive_status()` and `derive_agent_state()` still run and feed `jsonl_states` in the StateResolver, which is only consulted when hooks haven't fired yet (server restart catch-up, user disabled hooks). We don't add new JSONL-based state detection paths — that's hooks' job.

### What Hooks Own vs What JSONL Owns

| Concern | Owner | Why |
|---------|-------|-----|
| Session exists/doesn't exist | **Hooks** (SessionStart/SessionEnd) | Instant. JSONL file may not exist yet. |
| Agent state (idle, working, etc.) | **Hooks** | Authoritative. No guessing. |
| Last user message text | **Hooks** (UserPromptSubmit) | Instant. Drives kanban card title + sort. |
| Last user message timestamp | **Hooks** (UserPromptSubmit) | Drives stack-sort in Running column. |
| Token counts + cost | **JSONL** | Hooks don't carry token data. |
| Context window fill | **JSONL** | Derived from last assistant turn tokens. |
| Sub-agent spawn details | **JSONL** (hooks supplement) | JSONL has prompt text, tool_use_id, type. Hooks add real-time start/stop. |
| Task/todo progress items | **JSONL** | Parsed from TodoWrite, TaskCreate, TaskUpdate lines. |
| Git branch | **JSONL** | Extracted from user-type JSONL lines. |
| Model | **Both** | Hook provides on SessionStart. JSONL confirms/updates if model switches mid-session. |

## Hook Events

### Registration

8 hooks, auto-injected into `~/.claude/settings.json` on server start.

| Hook | Sync/Async | Purpose |
|------|-----------|---------|
| `SessionStart` | sync (default) | Create session in live map; dedup with file watcher |
| `UserPromptSubmit` | async | Instant `lastUserMessage` + sort timestamp |
| `Stop` | async | Instant flip to NeedsYou / idle |
| `Notification` | async | `permission_prompt` / `idle_prompt` detection |
| `PostToolUseFailure` | async | `is_interrupt: true` → NeedsYou / interrupted |
| `SubagentStart` | async | Instant sub-agent pill appears |
| `SubagentStop` | async | Instant sub-agent completion |
| `SessionEnd` | async | Clean removal from live map |

**Why these 8, not all 14:**

| Skipped Hook | Why |
|---|---|
| `PreToolUse` | We don't need pre-execution signals. PostToolUse/Failure covers outcomes. |
| `PostToolUse` | Already handled for AskUserQuestion/ExitPlanMode. Generic tool tracking not needed — JSONL `derive_activity()` handles this. |
| `PermissionRequest` | `Notification(permission_prompt)` covers this with better UX context. |
| `PreCompact` | Not relevant to session state. |
| `TeammateIdle` | Team features are future scope. |
| `TaskCompleted` | Already parsed from JSONL (TaskUpdate lines). |

**Sync vs async rationale:**
- `SessionStart` uses **sync** (the default for command hooks). With sync execution, the curl completes before Claude Code proceeds, so the hook *usually* fires before the first JSONL write. However, this is a **timing advantage, not a hard guarantee** — network latency or a slow server response could let the JSONL file appear first. Therefore, **dedup is co-primary**: both the hook path and file watcher path must handle the other's prior existence (see "Dedup and State Buffering" below).
- Everything else is **async** (`"async": true` in the hook JSON) — we never want to slow down Claude's execution.

### Auto-Injection into settings.json

**⚠️ Critical: Claude Code skips files with JSON errors entirely** (not just the invalid settings). Malformed JSON in `settings.json` = all user settings lost for that session. Our hook registration must validate output JSON before writing.

Claude Code hooks use a **matcher-based nested format** (3 levels of nesting):

```
hooks[event_name] → [matcher_group] → hooks → [handler]
```

Each event array contains **matcher group** objects. A matcher group has:
- `matcher` (optional regex string) — filters when the hook fires. Omit to match all.
- `hooks` (array) — the hook handlers to run when matched.

On server startup, the Rust server:

1. Reads `~/.claude/settings.json` (create `{"hooks": {}}` if missing)
2. Parses existing hooks
3. Removes any previous Mission Control hooks (identified by sentinel in nested handler commands)
4. Also removes any old-format flat hooks (legacy cleanup from pre-matcher versions)
5. Appends 8 new matcher groups to the appropriate event arrays
6. Validates the output JSON is parseable
7. Writes back atomically (write to temp file, rename)

**Hook command format:**

```bash
# Each hook pipes stdin (Claude Code's JSON payload) directly to our endpoint.
# --data-binary @- reads stdin without shell interpolation (safe for arbitrary JSON).
# $(cat) is NOT used — it corrupts payloads containing $, ", or \ characters.
curl -s -X POST http://localhost:{PORT}/api/live/hook \
  -H 'Content-Type: application/json' \
  --data-binary @- \
  2>/dev/null || true  # claude-view-hook
```

The `|| true` ensures hook failure never blocks Claude Code. The `2>/dev/null` suppresses curl errors. The `# claude-view-hook` bash comment at the end is the sentinel for identification.

**Matcher group format (sync — SessionStart only):**

```json
{
  "hooks": [
    {
      "type": "command",
      "command": "curl -s -X POST http://localhost:47892/api/live/hook -H 'Content-Type: application/json' --data-binary @- 2>/dev/null || true # claude-view-hook",
      "statusMessage": "Mission Control"
    }
  ]
}
```

**Matcher group format (async — all other 7 hooks):**

```json
{
  "hooks": [
    {
      "type": "command",
      "command": "curl -s -X POST http://localhost:47892/api/live/hook -H 'Content-Type: application/json' --data-binary @- 2>/dev/null || true # claude-view-hook",
      "async": true,
      "statusMessage": "Mission Control"
    }
  ]
}
```

**Why no `matcher` field on our groups:** We want all occurrences of each event. Omitting `matcher` matches everything. For events that support matchers (SessionStart matches on source, Notification matches on notification_type, etc.), we still want all values — the event-specific dispatch happens in our `handle_hook` handler, not at the Claude Code matcher level.

We identify our hooks by the `# claude-view-hook` sentinel in the handler's `command` string (inside the nested `hooks` array). On next startup, we find matcher groups whose inner handlers contain this sentinel and remove the entire matcher group.

**Cleanup:** On server shutdown (graceful), remove our hooks from settings.json. If the server crashes, the next startup's "remove previous" step handles stale entries. The hooks will harmlessly fail (curl to dead server) in the meantime. Cleanup also handles the old flat format for hooks left over from pre-matcher versions.

**Graceful shutdown (currently missing — must be added in Phase 1):**

`main.rs` currently calls `axum::serve(listener, app).await?` (line 473) with no shutdown handler. Add:

```rust
// main.rs — replace line 473
let port = get_port(); // already exists at line 177
axum::serve(listener, app)
    .with_graceful_shutdown(async move {
        tokio::signal::ctrl_c().await.ok();
        tracing::info!("Shutting down, cleaning up hooks...");
        // File I/O is blocking but we're shutting down — acceptable.
        vibe_recall_server::live::hook_registrar::cleanup(port);
    })
    .await?;
```

**Port handling:** The port is baked into the curl command. `get_port()` resolves the port in `main()` (line 177) but it is a local variable not stored in `AppState`. The hook registrar receives the port as a parameter. It should use `CLAUDE_VIEW_PORT` exclusively (not the `PORT` fallback) to avoid accidentally embedding a port from a different tool.

**Server restart state regression (accepted trade-off):** When the server restarts, all in-memory hook state is lost. Sessions re-appear with JSONL-derived state only. A session in `NeedsYou/"needs_permission"` (from a hook) would temporarily revert to whatever JSONL derives (likely `Autonomous/"thinking"`). The session will self-correct when the next hook event fires. This is an accepted trade-off of in-memory hook state — persisting hook state to disk would add complexity with minimal benefit since hooks fire frequently during active sessions.

### State Transitions

Each hook event maps to a state transition on the LiveSession:

#### SessionStart

```
Payload: { session_id, transcript_path, cwd, source, model }
```

**Required HookPayload fields:** `model: Option<String>` (currently missing — must be added in Phase 2).

**Actions:**
1. Parse `transcript_path` → extract project dir, session ID, JSONL file path
2. Parse `cwd` → derive `project_display_name`, `project_path`
3. Create skeleton `LiveSession`:
   - `id` from `session_id`
   - `model` from payload (or `null` if absent)
   - `status: Working`
   - `agent_state: Autonomous / "thinking" / "Starting up..."`
   - All data fields (tokens, cost, context) zeroed
4. Create `SessionAccumulator` keyed by `session_id` (this is the existing keying convention — NOT file path)
5. Broadcast `SessionDiscovered`

**Prerequisite:** The hook handler must have access to the manager's accumulators. Currently `AppState` has no `Arc<LiveSessionManager>`. **Fix:** Add `Arc<LiveSessionManager>` to `AppState` (see Phase 2 Step 2).

**Source handling:**
- `startup` → fresh session, create new
- `resume` → check if session already exists in map (from prior JSONL scan). If yes, just update agent_state. If no, create new.
- `clear` → reset session state but keep same entry
- `compact` → no state change (internal operation)

**Dedup and State Buffering (co-primary with hook):**

Both the hook path and file watcher path must handle each other's prior existence:

| Scenario | What happens |
|----------|-------------|
| Hook fires first, file watcher discovers later | File watcher finds session already in map → **merge** (enrich with JSONL data, don't create duplicate) |
| File watcher discovers first, hook fires later | Hook finds session already in map → update `agent_state` and `model`, don't overwrite JSONL-derived data |
| `UserPromptSubmit` arrives before session exists | StateResolver **buffers** the hook state. When file watcher later creates the session and calls `resolve()`, it gets the buffered hook state. This is the existing StateResolver behavior — no new code needed. |
| Neither path creates session (both race) | Impossible — at least one wins the write lock |

#### UserPromptSubmit

```
Payload: { session_id, prompt }
```

**Required HookPayload fields:** `prompt: Option<String>` (currently missing — must be added in Phase 2).

**Actions (data mutations in `handle_hook`, NOT in `resolve_state_from_hook`):**
1. Find session by `session_id` (if not found, StateResolver buffers the state — see State Buffering above)
2. Set `session.last_user_message` = `payload.prompt` (truncated to 500 chars)
3. Set `session.current_turn_started_at` = `Some(unix_now)`
4. If `session.title` is empty, set `session.title` = `payload.prompt` (truncated). Note: the backend field is `title`, not `first_user_message`.
5. Increment `session.turn_count`
6. Set `agent_state: Autonomous / "thinking" / "Processing prompt..."`
7. Clear any stale NeedsYou hook state via `state_resolver.clear_hook_state(session_id)`
8. Broadcast `SessionUpdated`

**Why this is special:** Unlike other hooks that only set `agent_state`, `UserPromptSubmit` mutates 4+ fields on `LiveSession`. The `handle_hook` function must match on `hook_event_name` for event-specific data mutations BEFORE calling `resolve_state_from_hook` for agent state. See Phase 2 implementation details.

**This is the stack-sort signal:** The Running column sorts by `current_turn_started_at`. The instant the user hits Enter, this hook fires, the timestamp updates, and the card jumps to the top.

#### Stop

```
Payload: { session_id, stop_hook_active }
```

**Actions:**
1. Set `agent_state: NeedsYou / "idle" / "Waiting for your next prompt"`
2. Freeze `last_turn_task_seconds` (elapsed since `current_turn_started_at`)
3. Broadcast `SessionUpdated`

**Note:** `Stop` does NOT fire on user interrupt. That's handled by `PostToolUseFailure` with `is_interrupt: true`.

#### Notification

```
Payload: { session_id, notification_type, message }
```

**Required HookPayload fields:** `notification_type: Option<String>` and `message: Option<String>` (both currently missing — must be added in Phase 2).

**Actions (event-specific logic in `handle_hook`):** Match on `payload.notification_type` to dispatch:

| notification_type | Agent State |
|---|---|
| `permission_prompt` | NeedsYou / "needs_permission" / "Needs permission" |
| `idle_prompt` | NeedsYou / "idle" / "Session idle" |
| `elicitation_dialog` | NeedsYou / "awaiting_input" / message text (truncated) |
| `auth_success` | No change — return early from `handle_hook` before updating StateResolver |
| unknown / None | NeedsYou / "awaiting_input" / "Notification" (safe fallback) |

#### PostToolUseFailure

```
Payload: { session_id, tool_name, error, is_interrupt }
```

**Current bug:** `resolve_state_from_hook` ignores `payload.is_interrupt` and always returns `"error"`. Phase 2 must fix this with an explicit branch:

```rust
"PostToolUseFailure" => {
    if payload.is_interrupt.unwrap_or(false) {
        AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "interrupted".into(),
            label: format!("You interrupted {}", payload.tool_name.as_deref().unwrap_or("tool")),
            confidence: 0.95,
            source: SignalSource::Hook,
            context: None,
        }
    } else {
        AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "error".into(),
            label: format!("Failed: {}", payload.tool_name.as_deref().unwrap_or("tool")),
            confidence: 0.95,
            source: SignalSource::Hook,
            context: payload.error.as_ref().map(|e| serde_json::json!({"error": e})),
        }
    }
}
```

| is_interrupt | Agent State |
|---|---|
| `true` | NeedsYou / **"interrupted"** / "You interrupted {tool_name}" |
| `false` | NeedsYou / "error" / "Failed: {tool_name}" |

**New state: `interrupted`** is distinct from `error`. Different icon (`CirclePause` vs `AlertTriangle`), different color (orange vs red). Treated as **blocking** in the StateResolver (never expires — session stays in NeedsYou until next `UserPromptSubmit` clears it).

#### SubagentStart

```
Payload: { session_id, agent_type, agent_id }
```

Agent state: **Autonomous / "delegating" / "Running {agent_type} subagent"**

Already handled in hooks.rs (line 120-127). No changes needed.

#### SubagentStop

```
Payload: { session_id, agent_type, agent_id }
```

Agent state: **Autonomous / "acting" / "Subagent {agent_type} finished"**

Already handled (line 128-135). No changes needed.

#### SessionEnd

```
Payload: { session_id, reason }
```

**Actions:**
1. Set `agent_state: NeedsYou / "session_ended" / "Session closed"`
2. Mark session as Done
3. Broadcast `SessionUpdated` (status: Done) — card immediately moves to Done column
4. After brief delay (10s), remove from live map
5. Broadcast `SessionCompleted` (removes card from UI)
6. Clean up hook state and accumulator

## Interrupt Detection (Hooks Only)

User interrupts have a coverage gap: `Stop` doesn't fire on interrupt.

| Interrupt scenario | Hook signal | Detected? |
|---|---|---|
| Escape during tool execution | `PostToolUseFailure` + `is_interrupt: true` | **Yes** — instant |
| Escape during text generation | No hook fires | **No** — known gap |

**Design decision:** We only detect interrupts via hooks. The text-generation interrupt gap is an accepted limitation. When Claude Code adds a hook event for text-generation interrupts, we'll get it for free. We do NOT add JSONL-based interrupt detection — that's mixing concerns.

The existing JSONL `derive_status()` will eventually move the session to `Paused` via staleness detection (>30s since last write), which is good enough for the text-gen case.

## StateResolver Changes

The StateResolver's resolution order stays the same (hook wins if fresh), but the mental model inverts:

**Before (JSONL-primary):**
> "JSONL derives state. Hooks temporarily override."

**After (hooks-primary):**
> "Hooks define state. JSONL enriches data. Existing JSONL state derivation remains as passive fallback (server restart, hooks disabled)."

**Concrete change:** The existing StateResolver already implements the correct priority (hook wins if fresh, JSONL as fallback) with nuanced expiry by state category. **Do NOT replace this with a blanket 120s window.** The only change needed is adding `"interrupted"` to the blocking states list.

The existing expiry logic in `state_resolver.rs` (`state_category()` at line 85):
- **Terminal** states (`task_complete`, `session_ended`, `work_delivered`): never expire
- **Blocking** states (`awaiting_input`, `awaiting_approval`, `needs_permission`, `error`, `idle`): never expire
- **Transient** states (everything else: `acting`, `thinking`, `delegating`): expire after 60s (`TRANSIENT_EXPIRY_SECS`)

**The one-line fix:** Add `"interrupted"` to the blocking match arm in `state_category()`:

```rust
fn state_category(state: &str) -> StateCategory {
    match state {
        "task_complete" | "session_ended" | "work_delivered" => StateCategory::Terminal,
        "awaiting_input" | "awaiting_approval" | "needs_permission" | "error" | "idle"
        | "interrupted"  // ← NEW: interrupted never expires
            => StateCategory::Blocking,
        _ => StateCategory::Transient,
    }
}
```

This ensures an interrupted session stays in NeedsYou until the user responds (triggering `UserPromptSubmit` which calls `clear_hook_state`). Transient states like `acting` still expire after 60s, falling back to JSONL — this is correct behavior.

## JSONL Parser Role (Unchanged)

The JSONL tail parser **keeps running exactly as-is**. Its role is data enrichment only:

- Accumulate tokens, cost, context window fill
- Track sub-agent spawns and completions (detailed data hooks don't carry)
- Track task/todo progress items
- Extract git branch
- Update model if it changes mid-session
- Provide `last_user_message` confirmation (JSONL confirms what UserPromptSubmit already set)

The existing `derive_status()` and `derive_agent_state()` still run and feed `jsonl_states` in the StateResolver. They're consulted as a passive fallback when hooks haven't fired (server restart, hooks disabled). **No new JSONL detection paths are added.**

## Session Lifecycle (Complete)

```
                    SessionStart hook
                         │
                         ▼
               ┌─────────────────┐
               │   DISCOVERED    │  Skeleton session in map.
               │   Autonomous    │  Waiting for first JSONL or
               │   "Starting..." │  UserPromptSubmit.
               └────────┬────────┘
                         │
                  UserPromptSubmit
                         │
                         ▼
               ┌─────────────────┐
            ┌─▶│    WORKING      │◀─── UserPromptSubmit (new turn)
            │  │   Autonomous    │
            │  │  "Processing..."│
            │  └────────┬────────┘
            │            │
            │     PostToolUse / PostToolUseFailure / Stop / Notification
            │            │
            │            ▼
            │  ┌─────────────────┐
            │  │   NEEDS YOU     │  idle / needs_permission /
            │  │   NeedsYou      │  awaiting_input / interrupted /
            │  │   (various)     │  error / awaiting_approval
            │  └────────┬────────┘
            │            │
            │     UserPromptSubmit (user responds)
            └────────────┘
                         │
                    SessionEnd hook
                         │
                         ▼
               ┌─────────────────┐
               │     ENDED       │  Removed from map after 10s.
               │   "Session      │
               │    closed"      │
               └─────────────────┘
```

## Kanban Sort Rules

### Running Column (Autonomous)
Sort by `current_turn_started_at` descending (most recent user prompt on top).
This is the "stack" behavior — when user sends a message, that card jumps to top.
Falls back to `last_activity_at` if `current_turn_started_at` is null.

### Needs You Column
1. Cache status: warm > unknown > cold
2. Within same cache tier: urgency rank (needs_permission > awaiting_input > interrupted > error > awaiting_approval > idle)
3. Within same urgency: recency (`last_activity_at` descending)

## Implementation Plan

### Phase 1: Hook Registration (auto-inject)

**New file:** `crates/server/src/live/hook_registrar.rs`

**Wire-up steps:**
1. Add `pub mod hook_registrar;` to `crates/server/src/live/mod.rs`
2. Call `hook_registrar::register(port)` from `create_app_full()` in `lib.rs` (NOT from `AppState::new()`)
3. Add graceful shutdown to `main.rs` with `with_graceful_shutdown()` (see Cleanup section above)

**Implementation:** See `crates/server/src/live/hook_registrar.rs` (already implemented).

Key design decisions in the implementation:
- `make_matcher_group()` builds the nested format: `{ "hooks": [handler] }` (no `matcher` = match all)
- `make_hook_handler()` builds the inner handler with `type`, `command`, `async`, `statusMessage`
- `remove_our_hooks()` handles **both** the new matcher-group format and old flat format (legacy cleanup)
- `matcher_group_has_sentinel()` checks the nested `hooks[].command` path for the sentinel
- Output JSON is validated before writing (Claude Code skips entire files with errors)
- Includes unit tests for format correctness and cleanup of both old/new formats

### Phase 2: Expand Hook Handler

**File:** `crates/server/src/routes/hooks.rs`

**Step 1: Add missing fields to `HookPayload`:**
```rust
pub prompt: Option<String>,           // UserPromptSubmit
pub notification_type: Option<String>, // Notification
pub message: Option<String>,          // Notification
pub model: Option<String>,            // SessionStart
```

**Step 2: Add `Arc<LiveSessionManager>` to `AppState`:**

`SessionAccumulator` (at `manager.rs:38`) is private with 23 fields — promoting it into `AppState` would expose too much internal coupling. Instead, keep accumulators inside the manager and add two public methods:

```rust
// In manager.rs — add to impl LiveSessionManager:

/// Called by hook handler when SessionStart creates a new session.
pub async fn create_accumulator_for_hook(&self, session_id: &str) {
    self.accumulators.write().await
        .entry(session_id.to_string())
        .or_insert_with(SessionAccumulator::new);
}

/// Called by hook handler when SessionEnd removes a session after delay.
pub async fn remove_accumulator(&self, session_id: &str) {
    self.accumulators.write().await.remove(session_id);
}
```

**Why `async` not `blocking_write`:** These methods are called from `handle_hook` which is an async Axum handler. Using `blocking_write()` in an async context can deadlock if the lock is held by another async task on the same runtime. Always use `.write().await` for `tokio::sync::RwLock`.

Then add `Arc<LiveSessionManager>` to `AppState`:
```rust
// In state.rs (crates/server/src/state.rs), add to AppState struct:
pub live_manager: Option<Arc<crate::live::manager::LiveSessionManager>>,
```

Use `Option` because `AppState::new()` and `AppState::new_with_indexing()` (used in tests) don't start the manager. Only `create_app_full()` in `lib.rs` sets it to `Some(manager)`.

**Update these construction sites (add `live_manager: None`):**
- `AppState::new()` (`state.rs:68`) — inside the `Arc::new(Self { ... })`
- `AppState::new_with_indexing()` (`state.rs:92`) — inside the `Arc::new(Self { ... })`
- `AppState::new_with_indexing_and_registry()` (`state.rs:119`) — inside the `Arc::new(Self { ... })`
- `create_app_with_git_sync()` (`lib.rs:103`) — inside `state::AppState { ... }`

**Update `create_app_full()` (`lib.rs:130`):**
```rust
// Change line 142 from:
let (_manager, live_sessions, live_tx) =
    live::manager::LiveSessionManager::start(pricing.clone(), resolver.clone());
// To:
let (manager, live_sessions, live_tx) =
    live::manager::LiveSessionManager::start(pricing.clone(), resolver.clone());

// Register hooks AFTER manager starts, BEFORE building AppState
live::hook_registrar::register(
    std::env::var("CLAUDE_VIEW_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(47892)
);

// Then in the AppState construction, add:
live_manager: Some(manager),
```

**Step 3: Restructure `handle_hook` for event-specific mutations:**

The current handler only sets `agent_state`. The new structure must:
1. Early-return for no-op events (Notification/auth_success)
2. Resolve agent state
3. Update StateResolver (state buffering — even if session doesn't exist yet)
4. Match on `hook_event_name` for event-specific **data mutations**
5. Broadcast the appropriate event

**Key compilation constraints verified against actual codebase:**
- Return type is `Json<serde_json::Value>` (NOT `StatusCode`) — must match current signature at `hooks.rs:41`
- `AgentState` derives `Clone` (confirmed at `state.rs:11`) — clone before consuming in match arms
- Timestamps use `std::time::{SystemTime, UNIX_EPOCH}` pattern (no `unix_now()` function exists)
- Must NOT hold `live_sessions` write lock across `.await` (deadlock risk) — release lock before async calls

```rust
use std::time::{SystemTime, UNIX_EPOCH};

async fn handle_hook(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<HookPayload>,
) -> Json<serde_json::Value> {                    // ← matches current return type
    // Early return for no-op events (auth_success notification)
    if payload.hook_event_name == "Notification"
        && payload.notification_type.as_deref() == Some("auth_success")
    {
        return Json(serde_json::json!({ "ok": true }));
    }

    let agent_state = resolve_state_from_hook(&payload);

    tracing::info!(
        session_id = %payload.session_id,
        event = %payload.hook_event_name,
        state = %agent_state.state,
        group = ?agent_state.group,
        "Hook event received"
    );

    // Always update StateResolver FIRST (state buffering for pre-discovery hooks)
    state.state_resolver.update_from_hook(&payload.session_id, agent_state.clone()).await;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    match payload.hook_event_name.as_str() {
        "SessionStart" => {
            let mut sessions = state.live_sessions.write().await;

            if let Some(existing) = sessions.get_mut(&payload.session_id) {
                // Session already exists (file watcher got there first, OR resume)
                existing.agent_state = agent_state.clone();
                if let Some(m) = &payload.model { existing.model = Some(m.clone()); }
                if payload.source.as_deref() == Some("clear") {
                    existing.turn_count = 0;
                    existing.current_turn_started_at = None;
                }
                let _ = state.live_tx.send(SessionEvent::SessionUpdated {
                    session: existing.clone(),
                });
            } else {
                // Session doesn't exist — create skeleton (any source: startup, resume, clear)
                let session = LiveSession {
                    id: payload.session_id.clone(),
                    project: String::new(),               // enriched by file watcher later
                    project_display_name: extract_project_name(payload.cwd.as_deref()),
                    project_path: payload.cwd.clone().unwrap_or_default(),
                    file_path: payload.transcript_path.clone().unwrap_or_default(),
                    status: SessionStatus::Working,
                    agent_state: agent_state.clone(),
                    git_branch: None,
                    pid: None,
                    title: String::new(),
                    last_user_message: String::new(),
                    current_activity: "Starting up...".into(),
                    turn_count: 0,
                    started_at: Some(now),
                    last_activity_at: now,
                    model: payload.model.clone(),
                    tokens: TokenUsage::default(),
                    context_window_tokens: 0,
                    cost: CostBreakdown::default(),
                    cache_status: CacheStatus::Unknown,
                    current_turn_started_at: None,
                    last_turn_task_seconds: None,
                    sub_agents: Vec::new(),
                    progress_items: Vec::new(),
                };
                sessions.insert(session.id.clone(), session.clone());
                drop(sessions); // release lock before async manager call
                if let Some(mgr) = &state.live_manager {
                    mgr.create_accumulator_for_hook(&payload.session_id).await;
                }
                let _ = state.live_tx.send(SessionEvent::SessionDiscovered { session });
            }
        }
        "UserPromptSubmit" => {
            let mut sessions = state.live_sessions.write().await;
            if let Some(session) = sessions.get_mut(&payload.session_id) {
                if let Some(prompt) = &payload.prompt {
                    session.last_user_message = prompt.chars().take(500).collect();
                    if session.title.is_empty() {
                        session.title = session.last_user_message.clone();
                    }
                }
                session.current_turn_started_at = Some(now);
                session.turn_count += 1;
                session.agent_state = agent_state.clone();
                let _ = state.live_tx.send(SessionEvent::SessionUpdated {
                    session: session.clone(),
                });
            }
        }
        "SessionEnd" => {
            let session_id = payload.session_id.clone();
            {
                let mut sessions = state.live_sessions.write().await;
                if let Some(session) = sessions.get_mut(&session_id) {
                    session.agent_state = agent_state.clone();
                    session.status = SessionStatus::Done;
                    let _ = state.live_tx.send(SessionEvent::SessionUpdated {
                        session: session.clone(),
                    });
                }
            }
            // Remove from live map after 10s delay.
            let state_clone = state.clone();
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                state_clone.live_sessions.write().await.remove(&session_id);
                if let Some(mgr) = &state_clone.live_manager {
                    mgr.remove_accumulator(&session_id).await;
                }
                state_clone.state_resolver.clear_hook_state(&session_id).await;
                let _ = state_clone.live_tx.send(SessionEvent::SessionCompleted {
                    session_id,
                });
            });
        }
        _ => {
            // Generic: Stop, PostToolUseFailure, Notification, SubagentStart/Stop
            let mut sessions = state.live_sessions.write().await;
            if let Some(session) = sessions.get_mut(&payload.session_id) {
                session.agent_state = agent_state.clone();
                let _ = state.live_tx.send(SessionEvent::SessionUpdated {
                    session: session.clone(),
                });
            }
        }
    }

    Json(serde_json::json!({ "ok": true }))
}

/// Extract project name from cwd path (last component).
fn extract_project_name(cwd: Option<&str>) -> String {
    cwd.and_then(|p| std::path::Path::new(p).file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("Unknown Project")
        .to_string()
}
```

**Required imports to add at top of hooks.rs:**
```rust
use crate::live::state::{
    AgentState, AgentStateGroup, SignalSource,
    LiveSession, SessionEvent, SessionStatus,
};
use vibe_recall_core::cost::{CacheStatus, CostBreakdown, TokenUsage};
```

**Step 4: Fix `resolve_state_from_hook` match arms:**

Add these match arms to `resolve_state_from_hook` in `hooks.rs`:

```rust
"SessionStart" => AgentState {
    group: AgentStateGroup::Autonomous,
    state: "thinking".into(),
    label: "Starting up...".into(),
    confidence: 0.9,
    source: SignalSource::Hook,
    context: None,
},
"UserPromptSubmit" => AgentState {
    group: AgentStateGroup::Autonomous,
    state: "thinking".into(),
    label: "Processing prompt...".into(),
    confidence: 0.9,
    source: SignalSource::Hook,
    context: None,
},
"Notification" => {
    // auth_success is handled by early return in handle_hook — won't reach here.
    // But if it does (defensive), low confidence won't override anything.
    match payload.notification_type.as_deref() {
        Some("permission_prompt") => AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "needs_permission".into(),
            label: "Needs permission".into(),
            confidence: 0.95,
            source: SignalSource::Hook,
            context: None,
        },
        Some("idle_prompt") => AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "idle".into(),
            label: "Session idle".into(),
            confidence: 0.95,
            source: SignalSource::Hook,
            context: None,
        },
        Some("elicitation_dialog") => AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "awaiting_input".into(),
            label: payload.message.as_deref()
                .map(|m| m.chars().take(100).collect::<String>())
                .unwrap_or_else(|| "Awaiting input".into()),
            confidence: 0.95,
            source: SignalSource::Hook,
            context: None,
        },
        _ => AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "awaiting_input".into(),
            label: "Notification".into(),
            confidence: 0.5,
            source: SignalSource::Hook,
            context: None,
        },
    }
},
```

Fix `"PostToolUseFailure"` → branch on `payload.is_interrupt` (see PostToolUseFailure section above for full code).

**Keep existing `"PermissionRequest"` match arm** (hooks.rs:104-111) as a defensive fallback — even though we don't register this hook, users may have it configured manually. The existing arm correctly returns `NeedsYou/"needs_permission"`.

### Phase 3: StateResolver Update

**File:** `crates/server/src/live/state_resolver.rs`

One-line change — add `"interrupted"` to the blocking states in `state_category()` (line 88):

```rust
"awaiting_input" | "awaiting_approval" | "needs_permission" | "error" | "idle"
| "interrupted"  // ← NEW
    => StateCategory::Blocking,
```

**No other StateResolver changes needed.** The existing resolve priority (hook wins if fresh, JSONL as fallback) and expiry logic (transient=60s, blocking=never, terminal=never) are already correct for the hooks-primary architecture. Do NOT add a blanket 120s window.

### Phase 4: Frontend — Interrupted State

**File:** `src/components/live/SessionCard.tsx`

1. Add `CirclePause` to lucide-react imports (`PauseCircle` is a deprecated alias in v0.562.0 — the canonical named export is `CirclePause`)
2. Add `CirclePause` to `ICON_MAP`
3. Add `orange` to `COLOR_MAP`:
   ```typescript
   orange: 'bg-orange-100 text-orange-700 dark:bg-orange-900/30 dark:text-orange-400',
   ```

**File:** `src/components/live/types.ts`

4. Add `interrupted` to `KNOWN_STATES`:
   ```typescript
   interrupted: { icon: 'CirclePause', color: 'orange' },
   ```

**Note on Tailwind orange classes:** This project uses `@tailwindcss/vite` (Tailwind v4) with automatic content detection. Orange palette classes (`bg-orange-100`, etc.) are already used elsewhere in the codebase and will be picked up by JIT from the `COLOR_MAP` string literal. No config changes needed.

**File:** `src/components/live/KanbanView.tsx`

Update `needsYouSortKey()` to insert `interrupted` at rank 2:
```typescript
case 'needs_permission': return 0
case 'awaiting_input': return 1
case 'interrupted': return 2    // ← NEW
case 'error': return 3          // was 2
case 'awaiting_approval': return 4  // was 3
case 'idle': return 5           // was 4
default: return 6               // was 5
```

**Test:** interrupt a session, verify card moves to Needs You instantly with orange badge

### Phase 5: Testing + Verification

**Unit tests in `hooks.rs` (`#[cfg(test)] mod tests`):**

Add a `#[cfg(test)]` module directly in `hooks.rs` (private functions are accessible here — no visibility change needed):

- `test_session_start_returns_thinking_state`
- `test_user_prompt_submit_returns_thinking_state`
- `test_stop_returns_idle_state`
- `test_notification_permission_prompt_returns_needs_permission`
- `test_notification_idle_prompt_returns_idle`
- `test_notification_elicitation_returns_awaiting_input`
- `test_notification_auth_success_is_catchall` (low confidence fallback)
- `test_post_tool_use_failure_returns_error`
- `test_post_tool_use_failure_with_interrupt_returns_interrupted`
- `test_session_end_returns_session_ended`

**Unit test in `state_resolver.rs` (extend existing `#[cfg(test)]` module):**

- `test_interrupted_state_never_expires` — assert `!StateResolver::is_expired("interrupted", Duration::from_secs(7200))`

**Integration tests in `crates/server/tests/hook_integration.rs` (new file):**

Add `create_app_for_hook_testing() -> (Router, Arc<AppState>)` to `lib.rs` (needed because existing factories discard `Arc<AppState>`):

```rust
// In lib.rs — new public async function for integration tests.
// Must be async because Database::new_in_memory() is async.
// Called from #[tokio::test] contexts.
pub async fn create_app_for_hook_testing() -> (Router, Arc<state::AppState>) {
    let pricing = vibe_recall_db::default_pricing();
    let resolver = StateResolver::new();
    let (manager, live_sessions, live_tx) =
        live::manager::LiveSessionManager::start(pricing.clone(), resolver.clone());

    let db = vibe_recall_db::Database::new_in_memory()
        .await
        .expect("test DB");

    let state = Arc::new(state::AppState {
        start_time: std::time::Instant::now(),
        db,
        indexing: Arc::new(IndexingState::new()),
        git_sync: Arc::new(GitSyncState::new()),
        registry: Arc::new(std::sync::RwLock::new(None)),
        jobs: Arc::new(jobs::JobRunner::new()),
        classify: Arc::new(classify_state::ClassifyState::new()),
        facet_ingest: Arc::new(FacetIngestState::new()),
        pricing,
        live_sessions,
        live_tx,
        state_resolver: resolver,
        rules_dir: std::env::temp_dir().join("test-rules"),
        terminal_connections: Arc::new(terminal_state::TerminalConnectionManager::new()),
        live_manager: Some(manager),
    });

    (api_routes(state.clone()), state)
}
```

**Usage from integration tests:**
```rust
#[tokio::test]
async fn test_session_start_hook_creates_session() {
    let (app, state) = create_app_for_hook_testing().await;
    // ... test body
}
```

Integration test list:
- `test_session_start_hook_creates_session` — POST SessionStart, assert `live_sessions` contains new session
- `test_user_prompt_submit_updates_sort_key` — POST SessionStart then UserPromptSubmit, assert `current_turn_started_at` is set
- `test_interrupt_detection_hook_path` — POST PostToolUseFailure with `is_interrupt: true`, assert state is "interrupted"
- `test_dedup_hook_then_file_watcher` — POST SessionStart, then simulate file watcher discovering same session, assert no duplicate

**Test helper in `crates/server/src/live/state.rs`:**

Add `make_minimal_live_session(id: &str, project: &str) -> LiveSession` to reduce 30-line inline construction.

**Manual verification:**
- Run 3+ sessions, interrupt one (Escape during tool use), verify dashboard updates instantly
- Restart server mid-session, verify sessions reappear (with JSONL-derived state)
- Kill server ungracefully, verify hooks are cleaned up on next startup

## Known Limitations

| Gap | Impact | Mitigation |
|-----|--------|-----------|
| Text-generation interrupt (Escape during streaming) has no hook | Session stays in "working" until staleness timeout (30s→Paused) | When Claude Code adds a hook for this, we get it for free |
| Server restart loses hook state | Sessions revert to JSONL-derived state until next hook fires | Self-corrects on next hook event; accepted trade-off |

---

## Changelog of Fixes Applied

### Rev 1-3 (Historical — 22 issues fixed)

5 Blockers, 12 Warnings, 5 Minor issues identified and fixed across 3 audit rounds. Key fixes: HookPayload missing fields, PostToolUseFailure ignoring is_interrupt, hook_registrar contract, graceful shutdown, sync guarantee rewritten, curl command format, return type correction, ownership/cloning fixes, lock ordering, missing match arms. See git history for full rev 1-3 changelog.

### Rev 4 (Simplification + Final Audit)

**Directive:** Cut complexity. Hooks own state, JSONL owns data. No dual-path fallback.

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| S1 | Phase 3 (JSONL Interrupt Detection) adds complexity for marginal gain | Simplification | **Removed Phase 3 entirely.** No `is_interrupt` field on LiveLine. No changes to `crates/core/`. Hooks detect interrupts via PostToolUseFailure; text-gen interrupt is accepted gap. |
| S2 | "Dual Path" architecture mixes concerns | Simplification | Replaced "Interrupt Detection (Dual Path)" section with "Interrupt Detection (Hooks Only)". Added "Known Limitations" table. |
| S3 | Architecture diagram had "JSONL State Deriver (fallback)" box implying new JSONL detection paths | Simplification | Removed the box. Existing derive_status()/derive_agent_state() remain as passive fallback, but no new JSONL detection paths added. |
| B1 | `remove_accumulator()` doesn't exist on LiveSessionManager — plan references `mgr.remove_accumulator(&session_id)` in SessionEnd handler | Blocker | Added `pub async fn remove_accumulator(&self, session_id: &str)` to manager.rs. |
| B2 | `!is_resume \|\| true` on line 590 — always evaluates to true, making is_resume check dead code | Blocker | Fixed: changed to `else {` — always create session if not in map, regardless of source. |
| B3 | `blocking_write()` in async context — `create_accumulator_for_hook` used `self.accumulators.blocking_write()` which can deadlock in tokio | Blocker | Fixed: method is now `pub async fn` using `.write().await`. |
| W1 | Notification `auth_success` mismatch — table says "return early", code sends NeedsYou state | Warning | Fixed: early return in `handle_hook` before calling `resolve_state_from_hook` for auth_success notifications. |
| W2 | `_manager` discarded in create_app_full — plan says add to AppState but doesn't fix destructuring | Warning | Fixed: `_manager` → `manager`, stored as `live_manager: Some(manager)`. Full code shown. |
| W3 | hook_registrar.rs has no implementation (only contract) | Warning | Added complete implementation skeleton with register() and cleanup() functions. |
| W4 | Missing `"async": true` in hook JSON for non-SessionStart hooks | Warning | Added separate JSON format examples for sync (SessionStart) and async (all others). |
| W5 | `cleanup(port)` called as `.await` but is sync file I/O | Warning | Fixed: `cleanup()` is sync (no `.await`). Acceptable in shutdown path — server is exiting anyway. |
| M1 | `create_app_for_hook_testing()` hand-waved | Minor | Added full implementation with all AppState fields including `live_manager`. |
| M2 | Phase renumbering (old Phase 3 removed, 4→3, 5→4, 6→5) | Minor | Renumbered. 5 phases total. |

**All Blockers, Warnings, and Minor issues from rev 4 audit resolved. 0 issues remain.**

### Rev 5 (Adversarial Review)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| B4 | `create_app_for_hook_testing()` uses `futures::executor::block_on` but `futures` crate is not in dev-deps | Blocker | Made function `async`, replaced `block_on` with `.await`. Added usage example showing `#[tokio::test]` calling pattern. |
| M3 | SessionEnd action list omitted the immediate `SessionUpdated` broadcast before the 10s-delayed `SessionCompleted` | Minor | Added step 3 "Broadcast SessionUpdated (status: Done)" to action list, renumbered steps. |

**All issues from rev 5 adversarial review resolved. 0 issues remain.**

### Rev 6 (Frontend Audit — Icon Name)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| B5 | `PauseCircle` is a deprecated default-only alias in lucide-react v0.562.0; named import is `CirclePause` | Blocker | Replaced all `PauseCircle` references with `CirclePause` (Phase 4 + PostToolUseFailure section). |
| M4 | `types.ts` `KNOWN_STATES` update for `interrupted` not explicitly listed as a Phase 4 step | Minor | Added `types.ts` as a file to update in Phase 4 with `interrupted: { icon: 'CirclePause', color: 'orange' }`. |

**All issues from rev 6 frontend audit resolved. 0 issues remain.**

### Rev 7 (Hook Format Migration — Matcher-Based Structure)

Claude Code hooks changed to a **matcher-based nested format**. The old flat format (`event → [handler]`) was replaced with a 3-level structure (`event → [matcher_group] → hooks → [handler]`). Files with JSON errors are now skipped entirely (not just the invalid settings).

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| B6 | Hook registrar writes old flat format that Claude Code no longer recognizes — hooks silently don't fire | Blocker | Rewrote `make_hook_entry()` → `make_matcher_group()` + `make_hook_handler()`. Each event now gets a matcher group: `{ "hooks": [handler] }`. |
| B7 | Cleanup logic only checks top-level `command` field — can't find sentinel in nested matcher-group format | Blocker | Added `matcher_group_has_sentinel()` helper that checks `group.hooks[].command`. `remove_our_hooks()` now handles both old flat format (legacy cleanup) and new matcher format. |
| W6 | Claude Code skips entire settings file on JSON error, but registrar doesn't validate output | Warning | Added JSON round-trip validation before atomic write. Aborts registration (logs error) if output is malformed. |
| M5 | Design plan showed old flat JSON examples for hook format | Minor | Updated "Auto-Injection into settings.json" section with matcher-group format examples and explanation of the 3-level nesting. |
| M6 | Plan contained full implementation skeleton that was now outdated | Minor | Replaced skeleton with reference to actual `hook_registrar.rs` file + summary of key design decisions. |

**All issues from rev 7 hook format migration resolved. 0 issues remain.**
