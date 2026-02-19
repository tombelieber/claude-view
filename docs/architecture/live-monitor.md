# Live Monitor Architecture

> How Mission Control monitors active Claude Code sessions in real-time.
>
> **Date:** 2026-02-18
> **Status:** Current implementation (hook-primary state, JSONL metadata-only)

---

## 1. System Overview

The Live Monitor tracks all active Claude Code sessions across all terminals on the user's machine, showing their state in a real-time dashboard. The operator sees which sessions need attention ("Needs You") and which are working autonomously ("Autonomous").

```
                        ┌─────────────────────────────────────────────┐
                        │            Claude Code Sessions             │
                        │  (terminal 1)  (terminal 2)  (terminal 3)  │
                        └────┬──────────────┬──────────────┬─────────┘
                             │              │              │
              ┌──────────────┼──────────────┼──────────────┼──────────┐
              │              ▼              ▼              ▼          │
              │     ┌────────────────────────────────────────────┐    │
              │     │        Hook Events (curl POST)             │    │
              │     │  ALL 14 Claude Code hooks fire here.       │    │
              │     │  Hooks are the SOLE authority for agent    │    │
              │     │  state — every hook maps to exactly one    │    │
              │     │  AgentState via a simple FSM.              │    │
              │     └──────────────────┬─────────────────────────┘    │
              │                        │                              │
              │              ┌─────────▼──────────┐                   │
              │              │   Hook Handler      │                   │
              │              │   (POST /api/live/  │                   │
              │              │    hook)             │                   │
              │              │                     │                   │
              │              │   Directly mutates   │                   │
              │              │   session.agent_state │                  │
              │              │   + session.status    │                  │
              │              └─────────┬──────────┘                   │
              │                        │                              │
              │              ┌─────────▼──────────┐                   │
              │              │  LiveSessionManager │                   │
              │              │  (central           │                   │
              │              │   orchestrator)     │                   │
              │              └─────────┬──────────┘                   │
              │                        │                              │
              │         ┌──────────────┼──────────────┐               │
              │         ▼              ▼              ▼               │
              │       File          Process        Cleanup            │
              │       Watcher       Detector       Task               │
              │       (JSONL →      (5s poll,      (30s)              │
              │        metadata     crash-only)                       │
              │        only)                                          │
              │                                                       │
              └───────────────────────┬───────────────────────────────┘
                                      │
                            ┌─────────▼──────────┐
                            │  Broadcast Channel  │
                            │  (tokio broadcast)  │
                            └─────────┬──────────┘
                                      │
              Rust Server (Axum)      │
              ────────────────────────┼────────────────────────────────
                                      │
                            ┌─────────▼──────────┐
                            │    SSE Stream       │
                            │ GET /api/live/      │
                            │     stream          │
                            └─────────┬──────────┘
                                      │
              ┌───────────────────────┼───────────────────────────────┐
              │    React Frontend     │                               │
              │              ┌────────▼──────────┐                    │
              │              │  useLiveSessions() │                    │
              │              │  (EventSource SSE)  │                   │
              │              └────────┬──────────┘                    │
              │                       │                               │
              │        ┌──────────────┼──────────────┐                │
              │        ▼              ▼              ▼                │
              │     KanbanView    GridView      SessionCard           │
              │                                                       │
              └───────────────────────────────────────────────────────┘
```

---

## 2. Signal Sources

The system has two signal sources with strict separation of concerns:

| Source | Role | Latency | What it provides |
|--------|------|---------|-----------------|
| **Hooks** | **SOLE state authority** | ~0ms (synchronous) | ALL agent state transitions: every hook maps to exactly one AgentState. No merging, no expiry, no confidence scores. |
| **JSONL** | **Metadata-only enrichment** | 2-30s (file watcher) | Token counts, cost, context window fill, model ID, git branch, sub-agent details, task/todo progress items. JSONL never touches `agent_state` or `status`. |

**Key principle:** Hooks own ALL state. JSONL only enriches metadata. There is no StateResolver, no dual-source merge, no confidence scoring. The hook handler directly mutates `session.agent_state` and derives `session.status` from it.

---

## 3. Hook System

### 3.1 Hook Registration (`hook_registrar.rs`)

On server startup, `create_app_full()` calls `hook_registrar::register(port)` which injects **14 hook entries** into `~/.claude/settings.json` — one for every Claude Code hook event:

```json
{
  "hooks": {
    "SessionStart": [{ "hooks": [{ "type": "command", "command": "curl ... # claude-view-hook", "statusMessage": "Mission Control" }] }],
    "UserPromptSubmit": [{ "hooks": [{ ..., "async": true }] }],
    "PreToolUse": [{ "hooks": [{ ..., "async": true }] }],
    "PostToolUse": [{ "hooks": [{ ..., "async": true }] }],
    "PostToolUseFailure": [{ "hooks": [{ ..., "async": true }] }],
    "PermissionRequest": [{ "hooks": [{ ..., "async": true }] }],
    "Stop": [{ "hooks": [{ ..., "async": true }] }],
    "Notification": [{ "hooks": [{ ..., "async": true }] }],
    "SubagentStart": [{ "hooks": [{ ..., "async": true }] }],
    "SubagentStop": [{ "hooks": [{ ..., "async": true }] }],
    "TeammateIdle": [{ "hooks": [{ ..., "async": true }] }],
    "TaskCompleted": [{ "hooks": [{ ..., "async": true }] }],
    "PreCompact": [{ "hooks": [{ ..., "async": true }] }],
    "SessionEnd": [{ "hooks": [{ ..., "async": true }] }]
  }
}
```

Key design:
- **SessionStart is sync** (blocks Claude Code startup until curl completes) so the server creates the session skeleton before any JSONL is written.
- **All other 13 hooks are async** (`"async": true` inside the handler object) so they don't block Claude Code's workflow.
- **`statusMessage: "Mission Control"`** — all hooks show "Mission Control" in the Claude Code spinner while firing.
- **Sentinel comment** (`# claude-view-hook`) enables idempotent registration: old hooks are removed before new ones are added.
- **Graceful cleanup** on server shutdown (`Ctrl+C`) removes all hooks from settings.json via the `with_graceful_shutdown` handler.
- **Atomic writes** (temp file + rename) prevent Claude Code from seeing a partially-written settings.json.

### 3.2 Hook Event Flow (`routes/hooks.rs`)

All 14 hook events POST to a single endpoint: `POST /api/live/hook`.

Claude Code pipes the hook payload as JSON via stdin to `curl --data-binary @-`. The payload contains:

```rust
struct HookPayload {
    session_id: String,
    hook_event_name: String,     // "SessionStart", "PreToolUse", etc.
    cwd: Option<String>,         // Working directory
    transcript_path: Option<String>,
    tool_name: Option<String>,   // PreToolUse, PostToolUse, PostToolUseFailure, PermissionRequest
    tool_input: Option<Value>,   // PreToolUse, PostToolUse, PermissionRequest — rich tool context
    tool_use_id: Option<String>, // PreToolUse, PostToolUse, PostToolUseFailure
    tool_response: Option<Value>,// PostToolUse
    is_interrupt: Option<bool>,  // PostToolUseFailure: user interrupted?
    error: Option<String>,       // PostToolUseFailure: error description
    source: Option<String>,      // SessionStart: "startup"|"resume"|"clear"|"compact"
    prompt: Option<String>,      // UserPromptSubmit: the user's message
    notification_type: Option<String>, // Notification: "permission_prompt"|"idle_prompt"|"elicitation_dialog"
    message: Option<String>,     // Notification: message text
    model: Option<String>,       // SessionStart: initial model
    agent_id: Option<String>,    // SubagentStart/Stop
    agent_type: Option<String>,  // SubagentStart/Stop, SessionStart (--agent flag)
    teammate_name: Option<String>, // TeammateIdle
    team_name: Option<String>,   // TeammateIdle, TaskCompleted
    task_id: Option<String>,     // TaskCompleted
    task_subject: Option<String>,// TaskCompleted
    task_description: Option<String>, // TaskCompleted
    trigger: Option<String>,     // PreCompact: "manual"|"auto"
    reason: Option<String>,      // SessionEnd: "clear"|"logout"|"prompt_input_exit"|etc.
    permission_suggestions: Option<Value>, // PermissionRequest
    // ... more fields
}
```

### 3.3 Hook → AgentState Mapping (Simple FSM)

Every hook event maps to exactly **one** AgentState. No merging, no expiry, no confidence scores.

| Hook Event | State | Group | Label |
|-----------|-------|-------|-------|
| SessionStart (startup/resume/clear) | `idle` | NeedsYou | "Waiting for first prompt" |
| SessionStart (compact) | `thinking` | Autonomous | "Compacting context..." |
| UserPromptSubmit | `thinking` | Autonomous | "Processing prompt..." |
| PreToolUse (AskUserQuestion) | `awaiting_input` | NeedsYou | "Asked you a question" |
| PreToolUse (ExitPlanMode) | `awaiting_approval` | NeedsYou | "Plan ready for review" |
| PreToolUse (EnterPlanMode) | `thinking` | Autonomous | "Entering plan mode..." |
| PreToolUse (Bash) | `acting` | Autonomous | "Running: git status" (from tool_input) |
| PreToolUse (Read) | `acting` | Autonomous | "Reading lib.rs" (from tool_input) |
| PreToolUse (Edit/Write) | `acting` | Autonomous | "Editing lib.rs" (from tool_input) |
| PreToolUse (Grep) | `acting` | Autonomous | "Searching: pattern" (from tool_input) |
| PreToolUse (Glob) | `acting` | Autonomous | "Finding files" |
| PreToolUse (Task) | `acting` | Autonomous | "Agent: description" (from tool_input) |
| PreToolUse (WebFetch) | `acting` | Autonomous | "Fetching web page" |
| PreToolUse (WebSearch) | `acting` | Autonomous | "Searching: query" (from tool_input) |
| PreToolUse (mcp__*) | `acting` | Autonomous | "MCP: server__tool" |
| PreToolUse (other) | `acting` | Autonomous | "Using tool_name" |
| PostToolUse | `thinking` | Autonomous | "Thinking..." |
| PostToolUseFailure (interrupt) | `interrupted` | NeedsYou | "You interrupted tool_name" |
| PostToolUseFailure (error) | `error` | NeedsYou | "Failed: tool_name" |
| PermissionRequest | `needs_permission` | NeedsYou | "Needs permission: tool_name" |
| Stop | `idle` | NeedsYou | "Waiting for your next prompt" |
| Notification (permission_prompt) | `needs_permission` | NeedsYou | "Needs permission" |
| Notification (idle_prompt) | `idle` | NeedsYou | "Session idle" |
| Notification (elicitation_dialog) | `awaiting_input` | NeedsYou | (message text, truncated) |
| SubagentStart | `delegating` | Autonomous | "Running agent_type agent" |
| SubagentStop | `acting` | Autonomous | "agent_type agent finished" |
| TeammateIdle | `delegating` | Autonomous | "Teammate name idle" (metadata update) |
| TaskCompleted | `task_complete` | NeedsYou | task_subject |
| PreCompact (manual) | `thinking` | Autonomous | "Compacting context..." |
| PreCompact (auto) | `thinking` | Autonomous | "Auto-compacting context..." |
| SessionEnd | `session_ended` | NeedsYou | "Session closed" |

### 3.4 PreToolUse: Instant Activity Labels

PreToolUse is the most impactful hook in the new design. It fires **before** every tool call, providing the `tool_name` and full `tool_input` object. This gives instant, rich activity labels with zero JSONL delay:

```rust
fn activity_from_pre_tool(tool_name: &str, tool_input: &Option<Value>) -> String {
    match tool_name {
        "Bash" => /* extract command */ format!("Running: {}", &cmd[..60]),
        "Read" => /* extract file_path */ format!("Reading {}", short_path(path)),
        "Edit" | "Write" => format!("Editing {}", short_path(path)),
        "Grep" => /* extract pattern */ format!("Searching: {}", pattern),
        "Glob" => "Finding files".into(),
        "Task" => /* extract description */ format!("Agent: {}", desc),
        "WebFetch" => "Fetching web page".into(),
        "WebSearch" => /* extract query */ format!("Searching: {}", query),
        _ if tool_name.starts_with("mcp__") => format!("MCP: {}", short_name),
        _ => format!("Using {}", tool_name),
    }
}
```

This replaces the old JSONL-derived `currentActivity` which had 2-30s delay and less context.

### 3.5 Hook Handler Logic

The `handle_hook` function does event-specific processing. The hook handler **directly mutates** `session.agent_state` and derives `session.status` via `status_from_agent_state()`. There is no StateResolver intermediary.

- **SessionStart**: Creates a skeleton `LiveSession` in the session map if it doesn't exist. Creates an accumulator for JSONL parsing. If the session already exists (file watcher got there first), updates the existing entry.
- **UserPromptSubmit**: Updates the session title (from first prompt), sets agent_state to Autonomous/thinking, increments turn count.
- **PreToolUse**: Sets agent_state based on tool_name (AskUserQuestion/ExitPlanMode are NeedsYou, everything else is Autonomous/acting). Updates `current_activity` with rich label from `tool_input`.
- **PostToolUse**: Sets agent_state to Autonomous/thinking (Claude is between tools, deciding next action).
- **PermissionRequest**: Sets agent_state to NeedsYou/needs_permission with tool context.
- **TaskCompleted**: Sets agent_state to NeedsYou/task_complete, marks the matching progress item as completed.
- **TeammateIdle**: Updates sub-agent idle status in the sub_agents list, keeps parent session as delegating.
- **PreCompact**: Sets agent_state to Autonomous/thinking with compaction label.
- **SessionEnd**: Marks the session as Done, then spawns a 10-second delayed removal (grace period for UI animation).
- **Generic** (Stop, Notification, SubagentStart/Stop, PostToolUseFailure): Updates the session's `agent_state` and broadcasts an update.

---

## 4. LiveSessionManager (Central Orchestrator)

`LiveSessionManager::start()` creates the manager and spawns 3 background tasks:

```
LiveSessionManager
├── spawn_file_watcher()       — JSONL file watching + metadata extraction (no state derivation)
├── spawn_process_detector()   — 5s process table scan (crash detection only)
└── spawn_cleanup_task()       — 30s orphan accumulator cleanup
```

### 4.1 Shared State

| Structure | Type | Purpose |
|-----------|------|---------|
| `sessions` | `Arc<RwLock<HashMap<String, LiveSession>>>` | In-memory map of all live sessions. Shared with route handlers. |
| `tx` | `broadcast::Sender<SessionEvent>` | Channel for SSE event broadcasting. |
| `accumulators` | `Arc<RwLock<HashMap<String, SessionAccumulator>>>` | Per-session parsing state (byte offsets, token totals, message history). |
| `processes` | `Arc<RwLock<HashMap<PathBuf, ClaudeProcess>>>` | Detected Claude processes keyed by working directory. |
| `finders` | `Arc<TailFinders>` | Pre-compiled SIMD substring finders for JSONL parsing. |
| `pricing` | `Arc<HashMap<String, cost::ModelPricing>>` | Per-model pricing table for live cost calculation. |

Note: `StateResolver` and `SessionStateClassifier` have been deleted. There is no dual-source merge logic — hooks are the sole state authority.

### 4.2 File Watcher (`spawn_file_watcher`)

1. **Initial scan**: Scans `~/.claude/projects/` for recently-modified JSONL files. Processes each file to populate initial session metadata.
2. **Continuous watching**: Uses `notify` crate to watch for file system events.
3. **On Modified**: Calls `process_jsonl_update()` to incrementally parse new bytes and extract metadata.
4. **On Removed**: Removes the session from the map and broadcasts `SessionCompleted`.
5. **Drop recovery**: Tracks `notify` channel drops and triggers a catch-up full scan when drops are detected.

### 4.3 JSONL Processing (`process_jsonl_update`)

The JSONL processing pipeline extracts **metadata only**. It never touches `agent_state` or `status`.

```
1. Extract session_id from file path
2. Read byte offset from accumulator (0 on first read)
3. Call parse_tail(path, offset, finders) → new lines + new offset
4. For each new line:
   - Accumulate token counts (input, output, cache_read, cache_creation)
   - Track context window fill (input tokens from latest assistant turn)
   - Track model ID
   - Track user messages (first = title, latest = last_user_message)
   - Track current turn start time
   - Track sub-agent spawns, completions, and progress
   - Track TodoWrite (full replacement) and TaskCreate/TaskUpdate (incremental)
5. Calculate cost from accumulated tokens
6. Update session metadata in shared map:
   - session.tokens, session.cost, session.model
   - session.context_window_tokens, session.cache_status
   - session.git_branch, session.sub_agents, session.progress_items
   - session.title (only if hook hasn't set it)
   - session.last_user_message, session.last_activity_at
7. NEVER touch session.agent_state or session.status
8. Broadcast session_updated event
```

For JSONL-discovered sessions (server restart recovery, before any hook arrives), a fallback state is used:

```rust
let fallback_state = AgentState {
    group: AgentStateGroup::Autonomous,
    state: "unknown".into(),
    label: "Connecting...".into(),
    context: None,
};
```

The next hook event from that session will correct the state.

### 4.4 Process Detector (`spawn_process_detector`)

The process detector is **crash-only**. It does exactly two things:

1. **Update PIDs**: Scans the process table every 5 seconds for running `claude` processes and updates `session.pid` for each session.
2. **Mark dead sessions**: If no process is found AND no activity for 5 minutes (300s) AND session is not already Done, mark as `session_ended`.

```
Every 5 seconds:
  1. Scan process table for claude processes
  2. For each session:
     a. Match process by working directory → update session.pid
     b. If no process + stale >300s + not Done:
        - Set agent_state = NeedsYou/session_ended
        - Set status = Done
        - Broadcast session_updated, then session_completed
        - Remove from session map
```

The process detector does **no state derivation**. It never re-classifies agent state based on JSONL patterns or process presence. Its only state mutation is the crash-detection path above.

### 4.5 Cleanup Task (`spawn_cleanup_task`)

Every 30 seconds:
- Removes orphaned accumulators (accumulator exists but session doesn't).

---

## 5. State Model

### 5.1 AgentState (Simple FSM)

The `AgentState` is the core user-facing classification, driven entirely by hooks:

```rust
struct AgentState {
    group: AgentStateGroup,  // NeedsYou | Autonomous
    state: String,           // Open string (new states added freely)
    label: String,           // Human-readable text
    context: Option<Value>,  // Raw tool input, error details, etc.
}
```

There are no confidence scores, no `SignalSource` enum, and no expiry rules. Every hook maps to exactly one state — the most recent hook wins.

**NeedsYou states** (operator attention required):
| State | Meaning | Triggered by |
|-------|---------|-------------|
| `awaiting_input` | AskUserQuestion or elicitation dialog | PreToolUse(AskUserQuestion), Notification(elicitation_dialog) |
| `awaiting_approval` | ExitPlanMode — plan ready for review | PreToolUse(ExitPlanMode) |
| `needs_permission` | Permission prompt for tool use | PermissionRequest, Notification(permission_prompt) |
| `error` | Tool failure | PostToolUseFailure (non-interrupt) |
| `interrupted` | User interrupted a tool | PostToolUseFailure (interrupt) |
| `idle` | Session waiting for next prompt | Stop, SessionStart(startup/resume/clear), Notification(idle_prompt) |
| `task_complete` | Task finished | TaskCompleted |
| `session_ended` | Session closed | SessionEnd, Process Detector (crash) |

**Autonomous states** (agent working, no action needed):
| State | Meaning | Triggered by |
|-------|---------|-------------|
| `thinking` | Processing prompt or between tools | UserPromptSubmit, PostToolUse, SessionStart(compact), PreCompact |
| `acting` | Actively using tools | PreToolUse (non-blocking tools), SubagentStop |
| `delegating` | Running sub-agents | SubagentStart, TeammateIdle |

### 5.2 Session Status (3 states, derived from AgentState)

SessionStatus is **derived purely from AgentState** — it is never computed from JSONL patterns, file staleness, or process presence.

```rust
fn status_from_agent_state(agent_state: &AgentState) -> SessionStatus {
    match agent_state.state.as_str() {
        "session_ended" => SessionStatus::Done,
        _ => match agent_state.group {
            AgentStateGroup::Autonomous => SessionStatus::Working,
            AgentStateGroup::NeedsYou => SessionStatus::Paused,
        }
    }
}
```

```
                    ┌──────────┐
              ┌────►│ Working  │◄────┐
              │     └────┬─────┘     │
              │          │           │
         Any hook     Any hook    Any hook
         → Autonomous → NeedsYou  → Autonomous
              │          │           │
              │     ┌────▼─────┐     │
              └─────┤  Paused  ├─────┘
                    └────┬─────┘
                         │
                   SessionEnd hook
                   OR crash detection
                         │
                    ┌────▼─────┐
                    │   Done   │
                    └──────────┘
```

---

## 6. SSE Real-Time Streaming

### 6.1 Server Side (`routes/live.rs`)

`GET /api/live/stream` returns an SSE stream:

1. **On connect**: Sends a `summary` event with aggregate counts, then sends `session_discovered` for every active session (full state hydration).
2. **Ongoing**: Subscribes to the `broadcast::channel(256)` and forwards events.
3. **Lag recovery**: If the client falls behind (broadcast buffer full), re-sends the full state (summary + all sessions).
4. **Heartbeat**: Every 15 seconds to keep the connection alive.

Event types:

| SSE Event | Payload | Trigger |
|-----------|---------|---------|
| `summary` | `{ needsYouCount, autonomousCount, deliveredCount, totalCostTodayUsd, totalTokensToday }` | On connect, on lag recovery |
| `session_discovered` | Full `LiveSession` JSON | New session detected |
| `session_updated` | Full `LiveSession` JSON | Session state changed |
| `session_completed` | `{ sessionId }` | Session removed |
| `heartbeat` | `{}` | Every 15s |

### 6.2 Client Side (`use-live-sessions.ts`)

The `useLiveSessions()` React hook:

1. Opens an `EventSource` to `/api/live/stream` (bypasses Vite proxy in dev mode via `sseUrl()`).
2. Maintains an internal `Map<string, LiveSession>`. Returns `{ sessions: LiveSession[], summary, isConnected, lastUpdate, stalledSessions, currentTime }` — sessions are sorted by `lastActivityAt`.
3. Handles 4 event types:
   - `session_discovered`: Adds/updates session in map.
   - `session_updated`: Updates session in map.
   - `session_completed`: Removes session from map.
   - `summary`: Updates aggregate counts + triggers resync pruning (removes sessions that no longer exist server-side).
4. **Stall detection**: Tracks per-session last-event time. Sessions with no event for >3s are flagged as "stalled" (UI shows loading indicator).
5. **Clock tick**: Updates `currentTime` every second for live duration computation in cards.
6. **Reconnection**: Exponential backoff (1s → 30s max) on connection loss.

---

## 7. What JSONL Provides (Metadata-Only Enrichment)

JSONL parsing provides all the rich metadata that hooks don't carry. It **never** touches `agent_state` or `status`:

| Data | Extracted from | Used for |
|------|---------------|----------|
| Token counts (input, output, cache_read, cache_creation) | Assistant line `usage` | Cost computation |
| Context window fill | Latest assistant turn total input tokens | Context usage bar |
| Model ID | Assistant line `model` | Model badge, pricing lookup |
| Session title | First non-meta user message `content_preview` | Card title (fallback if hook hasn't set it) |
| Last user message | Latest user message `content_preview` | Card subtitle |
| Git branch | User message with branch info | Branch badge |
| Sub-agent spawns | Task tool_use blocks | Sub-agent visualization |
| Sub-agent progress | Progress lines with `agent_progress` | Sub-agent current activity |
| Sub-agent completion | User line `toolUseResult` | Sub-agent status + cost |
| Todo items | TodoWrite tool_use (full replacement) | Task progress list |
| Task items | TaskCreate/TaskUpdate tool_use (incremental) | Task progress list |

---

## 8. Startup Sequence

```
main.rs
  │
  ├── 1. Open database
  ├── 2. Create IndexingState + RegistryHolder
  │
  ├── 3. create_app_full()
  │     │
  │     ├── a. LiveSessionManager::start()
  │     │     ├── Creates broadcast channel (256 buffer)
  │     │     ├── Creates shared session map
  │     │     ├── spawn_file_watcher()     ← Initial scan of ~/.claude/projects/ (metadata only)
  │     │     ├── spawn_process_detector()  ← 5s polling loop (crash detection only)
  │     │     └── spawn_cleanup_task()      ← 30s cleanup loop
  │     │
  │     ├── b. hook_registrar::register(port)
  │     │     └── Injects 14 hooks into ~/.claude/settings.json
  │     │
  │     └── c. Build AppState with shared:
  │           - live_sessions (same Arc<RwLock<HashMap>>)
  │           - live_tx (same broadcast::Sender)
  │           - live_manager (for hook handler accumulator management)
  │
  ├── 4. Bind TCP listener on port 47892
  ├── 5. Spawn background indexing (unrelated to live monitor)
  ├── 6. axum::serve() with graceful shutdown
  │
  └── On Ctrl+C:
        └── hook_registrar::cleanup(port) ← Removes hooks from settings.json
```

---

## 9. Data Flow Examples

### 9.1 User Opens Claude Code

```
Claude Code starts
  → SessionStart hook fires (sync)
  → curl POST /api/live/hook { session_id, event: "SessionStart", source: "startup" }
  → Hook handler:
      1. resolve_state_from_hook → NeedsYou/idle "Waiting for first prompt"
      2. Directly set session.agent_state and session.status
      3. Create skeleton LiveSession
      4. Insert into live_sessions map
      5. Create accumulator for JSONL parsing
      6. Broadcast SessionDiscovered
  → Frontend receives session_discovered via SSE
  → Card appears in Kanban "Needs You" column
```

### 9.2 User Sends a Prompt

```
User types prompt and presses Enter
  → UserPromptSubmit hook fires (async)
  → curl POST /api/live/hook { session_id, event: "UserPromptSubmit", prompt: "..." }
  → Hook handler:
      1. Set agent_state = Autonomous/thinking, status = Working
      2. Update title from prompt text
      3. Broadcast SessionUpdated
  → Frontend: card moves to "Autonomous" column, shows "Processing prompt..."

Claude starts using tools
  → PreToolUse hook fires (async) for EACH tool call
  → curl POST /api/live/hook { session_id, event: "PreToolUse", tool_name: "Bash", tool_input: {"command": "git status"} }
  → Hook handler:
      1. Set agent_state = Autonomous/acting, label = "Running: git status"
      2. Update session.current_activity
      3. Broadcast SessionUpdated
  → Frontend: card shows "Running: git status" instantly

  → PostToolUse hook fires (async) after tool completes
  → Hook handler:
      1. Set agent_state = Autonomous/thinking
      2. Broadcast SessionUpdated
  → Frontend: card shows "Thinking..."

Meanwhile, JSONL file watcher picks up new bytes
  → process_jsonl_update():
      1. Parse new lines (assistant tokens, tool calls)
      2. Update session.tokens, session.cost, session.model, session.context_window_tokens
      3. NEVER touch session.agent_state or session.status
      4. Broadcast SessionUpdated (metadata only)
  → Frontend: card updates with cost, token count, model badge
```

### 9.3 Claude Asks for Permission

```
Claude tries to run a tool that needs permission
  → PermissionRequest hook fires (async)
  → curl POST /api/live/hook { session_id, event: "PermissionRequest", tool_name: "Bash", tool_input: {...} }
  → Hook handler:
      1. Set agent_state = NeedsYou/needs_permission, label = "Needs permission: Bash"
      2. Set status = Paused (derived from NeedsYou group)
      3. Broadcast SessionUpdated
  → Frontend: card moves to "Needs You", shows "Needs permission: Bash"
```

### 9.4 Claude Asks a Question

```
Claude calls AskUserQuestion tool
  → PreToolUse hook fires (async) with tool_name = "AskUserQuestion"
  → Hook handler:
      1. Set agent_state = NeedsYou/awaiting_input, label = "Asked you a question"
      2. Set status = Paused
      3. Broadcast SessionUpdated
  → Frontend: card moves to "Needs You", shows "Asked you a question"
```

### 9.5 Session Ends

```
User exits Claude Code (Ctrl+D or /exit)
  → SessionEnd hook fires (async)
  → Hook handler:
      1. Set agent_state = NeedsYou/session_ended
      2. Set status = Done
      3. Broadcast SessionUpdated (with Done status)
      4. Spawn 10s delayed removal:
          - Remove from live_sessions map
          - Remove accumulator
          - Broadcast SessionCompleted
  → Frontend: card shows "Session closed" briefly, then removed after SessionCompleted
```

### 9.6 Session Crashes (No Hook)

```
Claude Code process dies unexpectedly (kill -9, terminal closed)
  → No SessionEnd hook fires
  → Process detector (5s poll) notices:
      1. No matching process in process table
      2. session.last_activity_at is >300s stale
      3. Set agent_state = NeedsYou/session_ended, label = "Session ended (no process)"
      4. Set status = Done
      5. Broadcast SessionUpdated, then SessionCompleted
      6. Remove from session map
  → Frontend: card shows "Session ended (no process)", then removed
```

---

## 10. Frontend Architecture

### 10.1 Component Tree

```
App
└── MissionControl (page)
    ├── ViewModeSelector (Grid | List | Board | Monitor)
    ├── useLiveSessions() hook
    │   └── EventSource → /api/live/stream
    │
    ├── KanbanView (Board mode)
    │   ├── Column: "Needs You" (group = needs_you)
    │   │   └── SessionCard[]
    │   └── Column: "Autonomous" (group = autonomous)
    │       └── SessionCard[]
    │
    └── SessionDetailPanel (slide-over on card click)
        ├── Session metadata (model, cost, tokens, context)
        ├── TaskProgressList (todo + task items)
        └── Sub-agent visualization
```

### 10.2 Types

The frontend `LiveSession` interface mirrors the Rust `LiveSession` struct (camelCase):

```typescript
interface AgentState {
  group: AgentStateGroup    // 'needs_you' | 'autonomous'
  state: string             // open string (new states added freely)
  label: string             // human-readable text
  context?: Record<string, unknown>  // tool input, error details, etc.
}

interface LiveSession {
  id: string
  status: 'working' | 'paused' | 'done'
  agentState: AgentState
  projectDisplayName: string
  title: string
  model: string | null
  tokens: { inputTokens, outputTokens, cacheReadTokens, ... }
  cost: { totalUsd, inputCostUsd, outputCostUsd, ... }
  contextWindowTokens: number
  currentActivity: string
  subAgents?: SubAgentInfo[]
  progressItems?: ProgressItem[]
  // ... more fields
}
```

### 10.3 State Icons & Colors

The UI maps `agentState.state` to visual indicators:

| State | Icon | Color |
|-------|------|-------|
| `awaiting_input` | MessageCircle | amber |
| `awaiting_approval` | FileCheck | amber |
| `needs_permission` | Shield | red |
| `error` | AlertTriangle | red |
| `interrupted` | CirclePause | orange |
| `idle` | Clock | gray |
| `thinking` | Sparkles | green |
| `acting` | Terminal | green |
| `delegating` | GitBranch | green |
| unknown → group default | Bell / Loader | amber / green |

---

## 11. Key Invariants

1. **Hooks own ALL state transitions.** The hook handler is the only code path that mutates `session.agent_state`. JSONL processing and the process detector never touch agent state (except the crash-detection path in the process detector).

2. **JSONL only enriches metadata.** The `process_jsonl_update()` function updates tokens, cost, model, context window, git branch, sub-agents, and progress items. It never touches `agent_state` or `status`.

3. **No confidence scores, no SignalSource enum.** AgentState has exactly 4 fields: `group`, `state`, `label`, `context`. Every hook is definitive — the most recent hook wins unconditionally.

4. **SessionStatus is derived from AgentState.** `status_from_agent_state()` maps `session_ended` to Done, Autonomous group to Working, NeedsYou group to Paused. There is no independent status derivation from JSONL patterns, file staleness, or process presence.

5. **PreToolUse provides instant activity labels.** The `tool_input` field gives rich context (command text, file paths, search patterns) for real-time activity display with zero JSONL delay.

6. **Process detector is crash-only.** It updates PIDs and marks dead sessions (no process + stale 5 minutes → session_ended). It does no state re-derivation, no 3-phase lock pattern, no JSONL re-classification.

7. **Done sessions are removed with a grace period.** When SessionEnd fires, the session is marked Done and a 10-second delayed removal is spawned for UI animation. Crash-detected sessions are removed immediately.

8. **File watcher drop recovery.** If the notify channel drops events, a full catch-up scan is triggered automatically.

9. **JSONL-discovered sessions use a fallback state.** If the file watcher discovers a session before any hook arrives (e.g., server restart), it creates the session with `Autonomous/unknown "Connecting..."` as a placeholder. The next hook event from that session corrects the state.

---

## Appendix A. Complete Claude Code Hook Spec

> **Reference:** [https://code.claude.com/docs/en/hooks](https://code.claude.com/docs/en/hooks)
>
> All 14 hooks are now registered by Mission Control.

### A.1 Hook Event Overview

| Event | When it fires | Sync/Async | State mapping |
|-------|--------------|------------|---------------|
| `SessionStart` | Session begins or resumes | **Sync** (blocks startup) | idle or thinking(compact) |
| `UserPromptSubmit` | User submits a prompt | Async | thinking |
| `PreToolUse` | Before a tool call executes | Async | acting (or awaiting_input/awaiting_approval for blocking tools) |
| `PostToolUse` | After a tool call succeeds | Async | thinking |
| `PostToolUseFailure` | After a tool call fails | Async | error or interrupted |
| `PermissionRequest` | When a permission dialog appears | Async | needs_permission |
| `Stop` | When Claude finishes responding | Async | idle |
| `Notification` | When Claude Code sends a notification | Async | needs_permission, idle, or awaiting_input |
| `SubagentStart` | When a subagent is spawned | Async | delegating |
| `SubagentStop` | When a subagent finishes | Async | acting |
| `TeammateIdle` | When a teammate is about to go idle | Async | delegating (metadata update) |
| `TaskCompleted` | When a task is marked completed | Async | task_complete |
| `PreCompact` | Before context compaction | Async | thinking |
| `SessionEnd` | When a session terminates | Async | session_ended |

### A.2 Common Input Fields (All Events)

Every hook event receives these fields via stdin as JSON:

| Field | Type | Description |
|-------|------|-------------|
| `session_id` | `string` | Current session identifier |
| `transcript_path` | `string` | Path to conversation JSONL file |
| `cwd` | `string` | Current working directory when the hook is invoked |
| `permission_mode` | `string` | `"default"`, `"plan"`, `"acceptEdits"`, `"dontAsk"`, or `"bypassPermissions"` |
| `hook_event_name` | `string` | Name of the event that fired |

### A.3 Matcher Patterns (Per Event)

The `matcher` field is a regex that filters when hooks fire. Each event matches on a different field:

| Event | What the matcher filters | Example values |
|-------|------------------------|----------------|
| `PreToolUse`, `PostToolUse`, `PostToolUseFailure`, `PermissionRequest` | Tool name | `Bash`, `Edit\|Write`, `mcp__.*` |
| `SessionStart` | How the session started | `startup`, `resume`, `clear`, `compact` |
| `SessionEnd` | Why the session ended | `clear`, `logout`, `prompt_input_exit`, `bypass_permissions_disabled`, `other` |
| `Notification` | Notification type | `permission_prompt`, `idle_prompt`, `auth_success`, `elicitation_dialog` |
| `SubagentStart`, `SubagentStop` | Agent type | `Bash`, `Explore`, `Plan`, or custom agent names |
| `PreCompact` | What triggered compaction | `manual`, `auto` |
| `UserPromptSubmit`, `Stop`, `TeammateIdle`, `TaskCompleted` | No matcher support | Always fires on every occurrence |

### A.4 Per-Event Input Schemas

#### SessionStart

Additional fields beyond common:

| Field | Type | Description |
|-------|------|-------------|
| `source` | `string` | `"startup"`, `"resume"`, `"clear"`, or `"compact"` |
| `model` | `string` | Model identifier (e.g. `"claude-sonnet-4-6"`) |
| `agent_type` | `string?` | Agent name if started with `claude --agent <name>` |

**Decision control:** stdout text or `additionalContext` is added to Claude's context. Has access to `CLAUDE_ENV_FILE` for persisting environment variables.

#### UserPromptSubmit

| Field | Type | Description |
|-------|------|-------------|
| `prompt` | `string` | The user's submitted prompt text |

**Decision control:** `{ "decision": "block", "reason": "..." }` prevents prompt processing. `additionalContext` adds context.

#### PreToolUse

| Field | Type | Description |
|-------|------|-------------|
| `tool_name` | `string` | Name of the tool (e.g. `Bash`, `Edit`, `Write`, `Read`, `Glob`, `Grep`, `Task`, `WebFetch`, `WebSearch`, or MCP tools `mcp__<server>__<tool>`) |
| `tool_input` | `object` | Tool-specific input parameters (see below) |
| `tool_use_id` | `string` | Unique identifier for this tool call |

**Tool input schemas:**

- **Bash:** `{ command, description?, timeout?, run_in_background? }`
- **Write:** `{ file_path, content }`
- **Edit:** `{ file_path, old_string, new_string, replace_all? }`
- **Read:** `{ file_path, offset?, limit? }`
- **Glob:** `{ pattern, path? }`
- **Grep:** `{ pattern, path?, glob?, output_mode?, -i?, multiline? }`
- **WebFetch:** `{ url, prompt }`
- **WebSearch:** `{ query, allowed_domains?, blocked_domains? }`
- **Task:** `{ prompt, description, subagent_type, model? }`

**Decision control (via `hookSpecificOutput`):**

| Field | Description |
|-------|-------------|
| `permissionDecision` | `"allow"` (bypass permissions), `"deny"` (block), `"ask"` (prompt user) |
| `permissionDecisionReason` | Shown to user (allow/ask) or Claude (deny) |
| `updatedInput` | Modifies tool input before execution |
| `additionalContext` | Added to Claude's context before tool executes |

#### PermissionRequest

| Field | Type | Description |
|-------|------|-------------|
| `tool_name` | `string` | Tool requesting permission |
| `tool_input` | `object` | Tool-specific input |
| `permission_suggestions` | `array?` | "Always allow" options from the permission dialog |

**Decision control (via `hookSpecificOutput`):**

| Field | Description |
|-------|-------------|
| `decision.behavior` | `"allow"` or `"deny"` |
| `decision.updatedInput` | For allow: modifies tool input |
| `decision.updatedPermissions` | For allow: applies permission rules |
| `decision.message` | For deny: tells Claude why |
| `decision.interrupt` | For deny: if `true`, stops Claude |

#### PostToolUse

| Field | Type | Description |
|-------|------|-------------|
| `tool_name` | `string` | Tool that executed |
| `tool_input` | `object` | Tool input arguments |
| `tool_response` | `object` | Tool result |
| `tool_use_id` | `string` | Unique tool call identifier |

**Decision control:** `{ "decision": "block", "reason": "..." }` prompts Claude with the reason. `additionalContext` adds context. `updatedMCPToolOutput` replaces MCP tool output.

#### PostToolUseFailure

| Field | Type | Description |
|-------|------|-------------|
| `tool_name` | `string` | Tool that failed |
| `tool_input` | `object` | Tool input arguments |
| `tool_use_id` | `string` | Unique tool call identifier |
| `error` | `string` | Error description |
| `is_interrupt` | `bool?` | Whether failure was caused by user interruption |

**Decision control:** `additionalContext` adds context alongside the error.

#### Notification

| Field | Type | Description |
|-------|------|-------------|
| `message` | `string` | Notification text |
| `title` | `string?` | Notification title |
| `notification_type` | `string` | `"permission_prompt"`, `"idle_prompt"`, `"auth_success"`, `"elicitation_dialog"` |

**Decision control:** `additionalContext` adds context. Cannot block notifications.

#### SubagentStart

| Field | Type | Description |
|-------|------|-------------|
| `agent_id` | `string` | Unique subagent identifier |
| `agent_type` | `string` | Agent type (e.g. `"Explore"`, `"Plan"`, `"Bash"`, or custom) |

**Decision control:** `additionalContext` injected into the subagent's context. Cannot block subagent creation.

#### SubagentStop

| Field | Type | Description |
|-------|------|-------------|
| `agent_id` | `string` | Unique subagent identifier |
| `agent_type` | `string` | Agent type |
| `agent_transcript_path` | `string` | Path to the subagent's own transcript JSONL |
| `stop_hook_active` | `bool` | Whether a stop hook is already active |

**Decision control:** Same as Stop — `{ "decision": "block", "reason": "..." }` prevents subagent from stopping.

#### Stop

| Field | Type | Description |
|-------|------|-------------|
| `stop_hook_active` | `bool` | `true` when Claude is already continuing due to a prior stop hook. Check this to prevent infinite loops. |

**Decision control:** `{ "decision": "block", "reason": "..." }` prevents Claude from stopping and continues the conversation.

#### TeammateIdle

| Field | Type | Description |
|-------|------|-------------|
| `teammate_name` | `string` | Name of the teammate about to go idle |
| `team_name` | `string` | Name of the team |

**Decision control:** Exit code 2 only (no JSON). stderr is fed back as feedback and the teammate continues working.

#### TaskCompleted

| Field | Type | Description |
|-------|------|-------------|
| `task_id` | `string` | Identifier of the task being completed |
| `task_subject` | `string` | Task title |
| `task_description` | `string?` | Detailed description |
| `teammate_name` | `string?` | Teammate completing the task |
| `team_name` | `string?` | Team name |

**Decision control:** Exit code 2 only (no JSON). stderr prevents completion and is fed back as feedback.

#### PreCompact

| Field | Type | Description |
|-------|------|-------------|
| `trigger` | `string` | `"manual"` (user ran `/compact`) or `"auto"` (context window full) |
| `custom_instructions` | `string` | For manual: what the user passed to `/compact`. For auto: empty string. |

**Decision control:** None. Cannot block compaction.

#### SessionEnd

| Field | Type | Description |
|-------|------|-------------|
| `reason` | `string` | `"clear"`, `"logout"`, `"prompt_input_exit"`, `"bypass_permissions_disabled"`, `"other"` |

**Decision control:** None. Cannot block session termination. Used for cleanup tasks.

### A.5 Hook Handler Types

Claude Code supports three handler types:

| Type | Field | Description | Default timeout |
|------|-------|-------------|----------------|
| `command` | `command: "..."` | Shell command, receives JSON on stdin | 600s |
| `prompt` | `prompt: "..."` | Single-turn LLM evaluation, returns `{ ok, reason }` | 30s |
| `agent` | `prompt: "..."` | Multi-turn subagent with tool access (Read, Grep, Glob), returns `{ ok, reason }` | 60s |

**Common handler fields:**

| Field | Required | Description |
|-------|----------|-------------|
| `type` | Yes | `"command"`, `"prompt"`, or `"agent"` |
| `timeout` | No | Seconds before canceling |
| `statusMessage` | No | Custom spinner message while running |
| `once` | No | If `true`, runs only once per session (skills only) |
| `async` | No | Command hooks only. If `true`, runs in background without blocking |

### A.6 Exit Code Semantics

| Exit code | Meaning | JSON processed? |
|-----------|---------|----------------|
| **0** | Success / allow | Yes — stdout parsed for JSON |
| **2** | Blocking error | No — stderr fed back to Claude or user |
| **Other** | Non-blocking error | No — stderr shown in verbose mode |

**Exit code 2 behavior per event:**

| Event | Can block? | Effect |
|-------|-----------|--------|
| `PreToolUse` | Yes | Blocks the tool call |
| `PermissionRequest` | Yes | Denies the permission |
| `UserPromptSubmit` | Yes | Blocks prompt processing, erases prompt |
| `Stop` | Yes | Prevents Claude from stopping |
| `SubagentStop` | Yes | Prevents subagent from stopping |
| `TeammateIdle` | Yes | Prevents teammate from going idle |
| `TaskCompleted` | Yes | Prevents task completion |
| `PostToolUse` | No | stderr shown to Claude |
| `PostToolUseFailure` | No | stderr shown to Claude |
| `Notification` | No | stderr shown to user only |
| `SubagentStart` | No | stderr shown to user only |
| `SessionStart` | No | stderr shown to user only |
| `SessionEnd` | No | stderr shown to user only |
| `PreCompact` | No | stderr shown to user only |

### A.7 Universal JSON Output Fields

These fields work across all events when exit code is 0:

| Field | Default | Description |
|-------|---------|-------------|
| `continue` | `true` | If `false`, Claude stops entirely (overrides event-specific decisions) |
| `stopReason` | — | Message shown to user when `continue` is `false` |
| `suppressOutput` | `false` | If `true`, hides stdout from verbose mode |
| `systemMessage` | — | Warning message shown to the user |

### A.8 Hook Configuration Locations

| Location | Scope | Shareable |
|----------|-------|-----------|
| `~/.claude/settings.json` | All projects | No (local to machine) |
| `.claude/settings.json` | Single project | Yes (committable) |
| `.claude/settings.local.json` | Single project | No (gitignored) |
| Managed policy settings | Organization-wide | Yes (admin-controlled) |
| Plugin `hooks/hooks.json` | When plugin is enabled | Yes (bundled) |
| Skill/agent frontmatter | While component is active | Yes (in component file) |

### A.9 Environment Variables Available to Hooks

| Variable | Available in | Description |
|----------|-------------|-------------|
| `CLAUDE_PROJECT_DIR` | All hooks | Project root directory |
| `CLAUDE_PLUGIN_ROOT` | Plugin hooks | Plugin's root directory |
| `CLAUDE_ENV_FILE` | SessionStart only | File path for persisting env vars for subsequent Bash commands |
| `CLAUDE_CODE_REMOTE` | All hooks | Set to `"true"` in remote web environments |
