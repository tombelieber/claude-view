---
status: approved
date: 2026-02-10
feature: mission-control
---

# Mission Control — Design Specification

> **Source of truth** for the Mission Control feature. All phase implementation plans reference this document for architecture, data model, and UX decisions.

---

## 1. Problem Statement

### Who

Power users of Claude Code who run **10-20+ concurrent sessions** across multiple VSCode windows, terminal tabs, and tmux panes. Typical profile: senior engineers, tech leads, and AI-native developers who use Claude Code as their primary coding tool.

### What hurts

1. **No unified visibility.** Each Claude Code session is an isolated TUI. To check what session #14 is doing, you must Cmd-Tab through windows or scroll through terminal history. There is no "Activity Monitor for Claude."

2. **No cost awareness.** Users have no idea how much they are spending across concurrent sessions. Token usage is buried in individual session outputs. A user running 15 sessions simultaneously might burn through $50-100/hour without realizing it.

3. **No context health monitoring.** Context window usage, cache status (warm/cold), and auto-compaction events are invisible. Users discover "my context got compacted" only when Claude forgets earlier instructions.

4. **Sub-agent opacity.** Claude Code's multi-agent orchestration (Task tool spawning sub-agents) is invisible from the outside. A user sees "thinking..." but cannot tell that 3 sub-agents are working in parallel on different files.

5. **No way to resume from a dashboard.** If a session is waiting for user input, you must find the correct terminal window. There is no "answer from here" capability.

### Why existing tools fall short

| Tool | What it does | What it lacks |
|------|-------------|---------------|
| Claude Code TUI | Excellent for single-session coding | Zero multi-session visibility |
| `claude-code-ui` | XState-based web UI for single session | No multi-session monitoring |
| `claude-code-monitor` | Hooks into session events | No dashboard, no cost tracking |
| `clog` | Web viewer for conversation logs | Read-only, no live sessions |
| Our existing claude-view | Historical session browser + analytics | Only sees completed sessions, not live ones |

**Mission Control fills the gap:** real-time monitoring and management of all active Claude Code sessions from a single browser tab.

---

## 2. Solution Overview

### Core concept

A new **Mission Control** page within claude-view that discovers all active Claude Code sessions on the local machine, displays their real-time status in a unified dashboard, and optionally allows interactive control via the Claude Agent SDK.

### Key capabilities

| Capability | Description | Phase |
|-----------|-------------|-------|
| **Session discovery** | Find all active JSONL files + correlate with OS processes | A |
| **Real-time status** | Streaming status, current activity, last messages via SSE | A |
| **Cost tracking** | Per-session and aggregate token/cost calculation | A |
| **Context monitoring** | Context window usage %, cache warm/cold status | A |
| **Grid view** | Responsive card grid showing all sessions at a glance | A |
| **List view** | Sortable, dense table for many sessions | B |
| **Kanban view** | Sessions grouped by status columns (Active/Waiting/Idle) | B |
| **Monitor mode** | Live session chat grid via RichPane (HTML) | C |
| **Sub-agent visualization** | Swim lanes showing parallel agent activity | D |
| **Custom layout** | Drag-and-drop pane arrangement via react-mosaic | E |
| **Resume in Dashboard** | Take over a waiting session via Agent SDK | F |
| **Dashboard chat** | Send messages to resumed sessions via WebSocket | F |

### Architecture at a glance

```
┌─────────────────────────────────────────────────────────────┐
│                    Browser (React SPA)                       │
│  ┌──────────┬──────────┬──────────┬──────────┐              │
│  │ Grid     │ List     │ Kanban   │ Monitor  │ ← View modes │
│  └────┬─────┴────┬─────┴────┬─────┴────┬─────┘              │
│       │ SSE      │ REST     │ SSE      │ WebSocket           │
└───────┼──────────┼──────────┼──────────┼────────────────────┘
        │          │          │          │
┌───────┴──────────┴──────────┴──────────┴────────────────────┐
│               MONITOR Layer (Rust / Axum)                    │
│  ┌─────────────┐ ┌──────────┐ ┌───────────┐ ┌───────────┐  │
│  │ JSONL       │ │ Session  │ │ Cost      │ │ SSE/WS    │  │
│  │ File Watcher│→│ State    │→│ Calculator│→│ Endpoints │  │
│  │ (notify)    │ │ Machine  │ │           │ │           │  │
│  └─────────────┘ └──────────┘ └───────────┘ └───────────┘  │
│  ┌─────────────┐ ┌──────────┐                               │
│  │ Process     │ │ In-Memory│                               │
│  │ Detector    │→│ State    │                               │
│  │ (sysinfo)   │ │ (DashMap)│                               │
│  └─────────────┘ └──────────┘                               │
└─────────────────────────────────────────────────────────────┘
        │ (Phase F only)
┌───────┴─────────────────────────────────────────────────────┐
│              CONTROL Layer (Node.js Sidecar)                 │
│  ┌──────────────┐ ┌────────────────┐                        │
│  │ Agent SDK    │ │ IPC (stdio)    │                        │
│  │ Session Mgr  │←│ with Rust      │                        │
│  └──────────────┘ └────────────────┘                        │
└─────────────────────────────────────────────────────────────┘
```

### What this is NOT

- **Not a replacement for the Claude Code TUI.** Mission Control monitors and occasionally interacts with sessions. The TUI remains the primary coding interface.
- **Not a cloud service.** Everything runs locally on `localhost:47892`. Mobile access is via user-provided tunnels (Tailscale, Cloudflare).
- **Not a process manager.** We do not start/stop/restart Claude Code processes. We observe them and optionally spawn new ones via Agent SDK.

---

## 3. Architecture

### 3.1 MONITOR Layer (Rust)

The MONITOR layer is the core of Mission Control. It runs inside the existing `vibe-recall-server` Axum process alongside the historical session browser.

#### Components

| Component | Crate | Responsibility |
|-----------|-------|---------------|
| **JSONL File Watcher** | `server` | Uses `notify` crate to watch `~/.claude/projects/` for file modifications. Debounced at 500ms. Only watches files modified in the last 24h. |
| **Incremental Parser** | `core` | Parses only **new lines** appended to JSONL files since last read. Uses `memmem::Finder` for SIMD pre-filter before `serde_json` parse. Tracks file offset per session. |
| **Session State Machine** | `server` | Derives session status from the last N JSONL entries. Transitions: `DISCOVERED → ACTIVE → WAITING_FOR_USER → IDLE → COMPLETE`. |
| **Process Detector** | `server` | Uses `sysinfo` crate to enumerate running processes. Matches Claude Code processes to JSONL files via PID → cwd → project path correlation. Runs every 10s. |
| **Cost Calculator** | `core` | Computes per-session and aggregate costs from token counts × model-specific pricing. Pure math, no API calls. |
| **In-Memory State** | `server` | `DashMap<SessionId, LiveSession>` for lock-free concurrent reads/writes. No SQLite for live data (latency concern at 1-5s update frequency with ~20-50 sessions). |
| **SSE Endpoint** | `server` | Streams `SessionEvent` objects to connected frontends. One SSE connection per browser tab. Server-side fan-out from state changes. |
| **WebSocket Endpoint** | `server` | Streams structured JSONL messages for Monitor mode. One WebSocket per monitored pane. Rich mode parses messages into user/assistant/tool/thinking types. |

#### File Watcher design

```rust
// Pseudocode for the file watcher loop
async fn watch_live_sessions(state: Arc<LiveState>) {
    let (tx, rx) = tokio::sync::mpsc::channel(256);

    // notify watcher sends file events to channel
    let mut watcher = notify::recommended_watcher(move |event| {
        let _ = tx.blocking_send(event);
    })?;
    watcher.watch(claude_projects_dir(), RecursiveMode::Recursive)?;

    // Process events with debouncing
    let mut pending: HashMap<PathBuf, Instant> = HashMap::new();
    let debounce = Duration::from_millis(500);

    loop {
        tokio::select! {
            Some(event) = rx.recv() => {
                for path in event.paths {
                    if path.extension() == Some("jsonl") {
                        pending.insert(path, Instant::now());
                    }
                }
            }
            _ = tokio::time::sleep(debounce) => {
                let now = Instant::now();
                let ready: Vec<_> = pending.drain()
                    .filter(|(_, t)| now.duration_since(*t) >= debounce)
                    .map(|(p, _)| p)
                    .collect();
                for path in ready {
                    process_jsonl_update(&state, &path).await;
                }
            }
        }
    }
}
```

#### Incremental parsing

Per the project's Rust Performance Rules:

1. **mmap the file, never copy.** Use `memmap2::Mmap`, parse directly from mapped memory.
2. **Track file offset.** Store `last_read_offset: u64` per session. On update, seek to offset, read only new bytes.
3. **SIMD pre-filter before JSON parse.** Create `memmem::Finder` instances once at watcher startup:

```rust
// Created ONCE at startup, reused for all file reads
struct LiveParsers {
    role_finder: memmem::Finder<'static>,
    tool_use_finder: memmem::Finder<'static>,
    tool_result_finder: memmem::Finder<'static>,
    model_finder: memmem::Finder<'static>,
    usage_finder: memmem::Finder<'static>,
}

impl LiveParsers {
    fn new() -> Self {
        Self {
            role_finder: memmem::Finder::new(b"\"role\""),
            tool_use_finder: memmem::Finder::new(b"\"tool_use\""),
            tool_result_finder: memmem::Finder::new(b"\"tool_result\""),
            model_finder: memmem::Finder::new(b"\"model\""),
            usage_finder: memmem::Finder::new(b"\"usage\""),
        }
    }
}
```

4. **Parse only lines that match.** Most JSONL lines are large assistant content blocks. We only need the last few entries with `role`, `usage`, or `tool_use` fields to derive state.

### 3.2 CONTROL Layer (Node.js Sidecar)

The CONTROL layer handles interactive session management via Anthropic's Agent SDK. It is a **separate Node.js process** spawned by the Rust server on demand (Phase F only).

#### Why a sidecar?

The Claude Agent SDK (`@anthropic-ai/agent-sdk`) is an npm package. It cannot run in Rust. Rather than rewriting the SDK in Rust (massive scope), we spawn a thin Node.js process that:

1. Receives commands from the Rust server via **stdin/stdout JSON-RPC**
2. Uses the Agent SDK to spawn/resume Claude Code sessions
3. Streams session output back to Rust via the same IPC channel
4. The Rust server then fans out to WebSocket-connected frontends

```
Browser ←WebSocket→ Rust Server ←JSON-RPC/stdio→ Node.js Sidecar ←Agent SDK→ Claude API
```

#### IPC Protocol (JSON-RPC over stdio)

```jsonc
// Rust → Node: Resume a session
{"jsonrpc": "2.0", "method": "resume", "params": {"session_id": "abc123", "jsonl_path": "/path/to/session.jsonl"}, "id": 1}

// Node → Rust: Session output chunk
{"jsonrpc": "2.0", "method": "output", "params": {"session_id": "abc123", "content": "I'll update the auth middleware..."}}

// Node → Rust: Session completed
{"jsonrpc": "2.0", "method": "completed", "params": {"session_id": "abc123", "usage": {"input_tokens": 12400, "output_tokens": 3200}}}

// Rust → Node: Send user message
{"jsonrpc": "2.0", "method": "send", "params": {"session_id": "abc123", "message": "Also add rate limiting"}, "id": 2}
```

#### Lifecycle

- Sidecar is **not started at server boot.** It is spawned on first "Resume" action and kept alive for subsequent interactions.
- If the sidecar crashes, the Rust server detects EOF on stdin and marks all controlled sessions as disconnected.
- Sidecar auto-exits after 5 minutes of inactivity (no active sessions).

### 3.3 Frontend (React SPA)

The frontend adds a new `/mission-control` route to the existing React SPA. It uses the same design system, component library, and routing infrastructure as the rest of claude-view.

#### New dependencies

| Package | Version | Size | Purpose |
|---------|---------|------|---------|
| `xterm` | 5.x | ~150KB | Terminal emulator -- **deferred to Phase F (Interactive Control)** |
| `@xterm/addon-fit` | 0.10.x | ~5KB | Auto-resize terminal to container -- **deferred to Phase F** |
| `@xterm/addon-webgl` | 0.18.x | ~80KB | GPU-accelerated rendering -- **deferred to Phase F** |
| `react-mosaic-component` | 6.x | ~8KB | Drag-and-drop tiling layout (Phase E) |

#### Route structure

```
/mission-control                → MissionControlPage (default: Grid view)
/mission-control?view=list      → List view
/mission-control?view=kanban    → Kanban view
/mission-control?view=monitor   → Monitor mode
```

View mode is stored in URL search params for deep-linking and browser back/forward support.

---

## 4. Session Discovery Pipeline

Session discovery is a 5-step pipeline that runs continuously while Mission Control is active.

### Step 1: Find candidate JSONL files

Scan `~/.claude/projects/` recursively for `.jsonl` files with `mtime` in the last 24 hours. This is the initial discovery sweep, running once at Mission Control page load and then relying on the `notify` file watcher for subsequent updates.

```
~/.claude/projects/
├── -Users-alice-dev-myapp/
│   ├── 0191a2b3-c4d5-6e7f-8a9b-0c1d2e3f4a5b.jsonl  ← modified 2 min ago ✓
│   └── 0191a2b3-dead-beef-8a9b-0c1d2e3f4a5b.jsonl  ← modified 3 days ago ✗
├── -Users-alice-dev-backend/
│   └── 0191b4c5-d6e7-8f9a-0b1c-2d3e4f5a6b7c.jsonl  ← modified 30 sec ago ✓
└── ...
```

**Performance:** Typical user has 50-200 JSONL files total. The 24h filter reduces this to 5-30 candidates. Cost: one `readdir` + `stat` per file, ~1ms total.

### Step 2: Detect actively-writing files

Among the 24h candidates, identify files currently being written to:

| Recency | Classification | Rationale |
|---------|---------------|-----------|
| Modified < 5s ago | **ACTIVE** (high confidence) | Claude Code writes to JSONL in real-time during streaming |
| Modified 5-30s ago | **POSSIBLY_ACTIVE** | Could be between turns, could be idle |
| Modified 30s-5min ago | **RECENTLY_ACTIVE** | Session may still be open but waiting for user |
| Modified 5min-24h ago | **STALE** | Session likely closed but not yet cleaned up |

The `notify` file watcher provides sub-second detection of new writes, so Step 2 becomes implicit after initial startup.

### Step 3: Correlate with OS processes

Use the `sysinfo` crate to enumerate running processes and match them to JSONL files:

```rust
// Pseudocode for process correlation
fn correlate_processes(sessions: &mut HashMap<String, LiveSession>) {
    let sys = System::new_with_specifics(
        RefreshKind::new().with_processes(ProcessRefreshKind::everything())
    );

    for (pid, process) in sys.processes() {
        // Match Claude Code processes by name
        let name = process.name();
        if !matches_claude_process(name) {
            continue;
        }

        // Get working directory
        let cwd = process.cwd();

        // Find the JSONL session that matches this cwd
        for session in sessions.values_mut() {
            if session.project_path_matches(cwd) {
                session.pid = Some(pid.as_u32());
                session.status = SessionStatus::Active;
                break;
            }
        }
    }
}

fn matches_claude_process(name: &str) -> bool {
    // Claude Code runs as a Node.js process
    // The process tree: node → claude (CLI) → node (agent)
    name.contains("claude") || {
        // Check command line args for claude-related paths
        // e.g., /Users/alice/.nvm/versions/node/v22/bin/node /usr/local/bin/claude
        false // simplified
    }
}
```

**Frequency:** Process scan runs every 10 seconds. It is the most expensive step (~5-10ms on macOS with sysinfo) but infrequent enough to be negligible.

**Fallback:** If process correlation fails (e.g., user runs Claude Code via a wrapper script), we still have file-modification-based status from Step 2. The session appears as ACTIVE without a PID.

### Step 4: Parse tail of JSONL incrementally

For each actively-writing file, read only the **new bytes** since last read:

```
File: session.jsonl (2.4 MB total)
┌─────────────────────────────────────────────┐
│  Already parsed (offset: 2,391,040)         │  ← skip
├─────────────────────────────────────────────┤
│  New lines (9,216 bytes since last read)    │  ← parse these
└─────────────────────────────────────────────┘
```

From the new lines, extract:

| Field | JSONL key | Used for |
|-------|-----------|----------|
| Role | `message.role` | State machine input |
| Content type | `content[].type` | Detect `tool_use` vs `text` |
| Tool name | `content[].name` (when type=tool_use) | Current activity label |
| Tool input | `content[].input` (partial) | Activity description |
| Usage | `usage.input_tokens`, `usage.output_tokens`, `usage.cache_read_input_tokens`, `usage.cache_creation_input_tokens` | Cost calculation |
| Model | `model` | Cost rate selection |
| Stop reason | `stop_reason` | State machine input (`end_turn`, `tool_use`, `max_tokens`) |

**Optimization:** SIMD pre-filter with `memmem::Finder`. Only ~10-20% of lines contain `usage` data. Only ~5% contain `tool_use`. Skip full JSON parse for lines that cannot contribute to state updates.

### Step 5: Derive session state via state machine

Feed the extracted data into the session state machine (see Section 5). Update the `LiveSession` struct in the `DashMap`. If the state changed, emit an SSE event to all connected frontends.

### Pipeline timing

```
Step 1 (initial scan):     ~1ms    (runs once)
Step 2 (classify recency): ~0ms    (piggybacks on notify events)
Step 3 (process scan):     ~5-10ms (runs every 10s)
Step 4 (incremental parse): ~0.1ms per file (only new bytes, SIMD filtered)
Step 5 (state derivation):  ~0ms   (pure enum matching)

Total per-update cycle: < 1ms for file watcher events
Total for periodic scan: ~10ms every 10s
```

---

## 5. Session State Machine

### States

```
                    ┌──────────────┐
        ┌──────────│  DISCOVERED  │──────────┐
        │          └──────┬───────┘          │
        │                 │                  │
        │          file actively             │  no process found
        │          being written             │  AND mtime > 5min
        │                 │                  │
        │          ┌──────▼───────┐   ┌──────▼───────┐
        │          │    ACTIVE    │   │   COMPLETE   │
        │          │              │   └──────────────┘
        │          ├──────────────┤
        │          │  STREAMING   │ ← assistant is generating output
        │          │  TOOL_USE    │ ← tool call in progress (Read, Write, Bash, etc.)
        │          └──────┬───────┘
        │                 │
        │          stop_reason = end_turn
        │          AND last role = assistant
        │                 │
        │          ┌──────▼───────────────┐
        │          │  WAITING_FOR_USER    │ ← assistant finished, waiting for human input
        │          └──────┬───────────────┘
        │                 │
        │          no new writes for > 2min
        │          AND process still running
        │                 │
        │          ┌──────▼───────┐
        │          │     IDLE     │ ← session open but inactive
        │          └──────┬───────┘
        │                 │
        │          process exited
        │          OR mtime > 30min
        │                 │
        │          ┌──────▼───────┐
        └─────────►│   COMPLETE   │
                   └──────────────┘
```

### Transition rules

| From | To | Trigger |
|------|----|---------|
| `DISCOVERED` | `ACTIVE.STREAMING` | New bytes written to JSONL AND last entry has `role: "assistant"` with no `stop_reason` |
| `DISCOVERED` | `ACTIVE.TOOL_USE` | Last entry contains `tool_use` content block |
| `DISCOVERED` | `WAITING_FOR_USER` | Last entry has `role: "assistant"` with `stop_reason: "end_turn"` AND mtime < 5min |
| `DISCOVERED` | `IDLE` | Process found but mtime > 2min |
| `DISCOVERED` | `COMPLETE` | No process found AND mtime > 5min |
| `ACTIVE.STREAMING` | `ACTIVE.TOOL_USE` | New entry contains `tool_use` content block |
| `ACTIVE.STREAMING` | `WAITING_FOR_USER` | `stop_reason: "end_turn"` emitted |
| `ACTIVE.TOOL_USE` | `ACTIVE.STREAMING` | `tool_result` received, new assistant content streaming |
| `ACTIVE.TOOL_USE` | `WAITING_FOR_USER` | Tool completes AND `stop_reason: "end_turn"` |
| `WAITING_FOR_USER` | `ACTIVE.STREAMING` | New `role: "user"` entry, followed by `role: "assistant"` streaming |
| `WAITING_FOR_USER` | `IDLE` | No new writes for > 2 minutes |
| `IDLE` | `ACTIVE.STREAMING` | New bytes written (user resumed the session in terminal) |
| `IDLE` | `COMPLETE` | Process exited OR mtime > 30 minutes |
| Any | `COMPLETE` | Process exited AND no new writes for 60 seconds |

### Sub-state: Current activity

Within `ACTIVE` states, we derive a human-readable activity label:

| Tool name | Activity label |
|-----------|---------------|
| `Read` | "Reading `{filename}`" |
| `Write` | "Writing `{filename}`" |
| `Edit` | "Editing `{filename}`" |
| `Bash` | "Running command" |
| `Glob` | "Searching files" |
| `Grep` | "Searching code" |
| `WebFetch` | "Fetching URL" |
| `WebSearch` | "Searching web" |
| `Task` | "Spawning sub-agent: `{description}`" |
| `mcp__*` | "Using MCP tool: `{server}:{tool}`" |
| (streaming text) | "Generating response..." |

---

## 6. Data Model

### Primary types

```rust
/// A live Claude Code session being monitored in real-time.
/// Stored in DashMap<String, LiveSession> — never persisted to SQLite.
#[derive(Debug, Clone, Serialize)]
pub struct LiveSession {
    /// Unique session identifier (UUID from JSONL filename)
    pub session_id: String,

    /// Absolute path to the project directory
    /// e.g., "/Users/alice/dev/myapp"
    pub project_path: String,

    /// Human-readable project name (last path component)
    /// e.g., "myapp"
    pub project_name: String,

    /// Current git branch (if detectable from JSONL content)
    pub branch: Option<String>,

    /// Absolute path to the JSONL file
    pub jsonl_path: String,

    /// Current session status
    pub status: SessionStatus,

    /// OS process ID (None if process detection failed)
    pub pid: Option<u32>,

    /// Preview of the last user message (truncated to 200 chars)
    pub last_user_message: Option<String>,

    /// Preview of the last assistant message (truncated to 200 chars)
    pub last_assistant_message: Option<String>,

    /// What the session is currently doing
    /// e.g., "Editing auth.ts", "Running tests", "Generating response..."
    pub current_activity: Option<String>,

    /// Timestamp of last activity (Unix seconds)
    /// INVARIANT: Always > 0. Sessions with unknown times are not surfaced.
    pub last_activity_at: i64,

    /// Number of user↔assistant turn pairs
    pub turn_count: u32,

    /// Cumulative token usage
    pub token_usage: TokenUsage,

    /// Estimated cost in USD
    pub estimated_cost_usd: f64,

    /// Context window usage as percentage (0.0 - 1.0)
    /// Based on model's max context length vs accumulated tokens
    pub context_usage_pct: f32,

    /// Active sub-agents (from Task tool invocations)
    pub sub_agents: Vec<SubAgentInfo>,

    /// Prompt cache status
    pub cache_status: CacheStatus,

    /// Model being used (e.g., "claude-opus-4-6")
    pub model: Option<String>,

    /// JSONL file byte offset — tracks how far we've parsed
    /// Internal use only, not serialized to API responses.
    #[serde(skip)]
    pub last_read_offset: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    /// Just found the file, haven't determined activity yet
    Discovered,
    /// Claude is actively generating output
    Streaming,
    /// A tool call is in progress
    ToolUse,
    /// Assistant finished, waiting for human input
    WaitingForUser,
    /// Session open but no recent activity
    Idle,
    /// Session has ended (process exited or long inactivity)
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheStatus {
    /// Cache was used in the last API call (cache_read_tokens > 0)
    /// Tokens are being served at 90% discount
    Warm,
    /// No cache hits — full price on all input tokens
    /// Happens after 5+ minutes of inactivity (TTL expired)
    Cold,
    /// Unknown (not enough data to determine)
    Unknown,
}
```

### Token and cost types

```rust
/// Cumulative token counts for a session.
/// Updated incrementally from JSONL `usage` blocks.
#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
}

/// Breakdown of costs for transparency/education.
#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct CostBreakdown {
    /// Cost of non-cached input tokens
    pub input_cost: f64,
    /// Cost of output tokens
    pub output_cost: f64,
    /// Cost of cache-read tokens (discounted)
    pub cache_read_cost: f64,
    /// Cost of cache-write tokens (premium)
    pub cache_write_cost: f64,
    /// Total cost
    pub total: f64,
    /// How much the user saved via caching vs full-price input
    /// Formula: cache_read_tokens × (base_input_rate - cache_read_rate)
    pub saved: f64,
}
```

### Sub-agent tracking

```rust
/// Information about a sub-agent spawned by the Task tool.
#[derive(Debug, Clone, Serialize)]
pub struct SubAgentInfo {
    /// Type of agent (from tool input description or auto-detected)
    /// e.g., "code_review", "test_writer", "researcher"
    pub agent_type: String,

    /// Current status of the sub-agent
    pub status: SubAgentStatus,

    /// Human-readable description of what the sub-agent is doing
    pub description: String,

    /// When the sub-agent was spawned (Unix seconds)
    pub started_at: i64,

    /// When the sub-agent completed (Unix seconds, None if still running)
    pub completed_at: Option<i64>,

    /// Cost incurred by this sub-agent
    pub cost_usd: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SubAgentStatus {
    Running,
    Completed,
    Failed,
}
```

### Aggregate summary

```rust
/// Dashboard-level aggregate across all live sessions.
/// Recomputed on every state change (cheap: iterate DashMap, sum fields).
#[derive(Debug, Clone, Serialize)]
pub struct LiveSummary {
    pub total_sessions: u32,
    pub active_sessions: u32,
    pub waiting_sessions: u32,
    pub idle_sessions: u32,
    pub total_cost_usd: f64,
    pub total_cost_saved_usd: f64,
    pub total_tokens: TokenUsage,
    pub sessions_by_project: Vec<ProjectSessionCount>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectSessionCount {
    pub project_name: String,
    pub count: u32,
}
```

---

## 7. Cost Calculation

### Pricing table (as of February 2026)

Costs are calculated locally from token counts. No Anthropic API calls needed — the JSONL `usage` block contains all required data.

| Model | Input ($/1M tokens) | Output ($/1M tokens) | Cache Read ($/1M) | Cache Write ($/1M) |
|-------|---------------------|---------------------|--------------------|---------------------|
| Claude Opus 4.6 (`claude-opus-4-6`) | $15.00 | $75.00 | $1.50 (10% of input) | $18.75 (125% of input) |
| Claude Sonnet 4.5 (`claude-sonnet-4-5-20241022`) | $3.00 | $15.00 | $0.30 (10% of input) | $3.75 (125% of input) |
| Claude Haiku 4.5 (`claude-haiku-4-5-20241022`) | $1.00 | $5.00 | $0.10 (10% of input) | $1.25 (125% of input) |

### Calculation formula

```rust
fn calculate_cost(usage: &TokenUsage, model: &str) -> CostBreakdown {
    let rates = get_model_rates(model);

    // Non-cached input tokens = total input - cache_read - cache_write
    let fresh_input = usage.input_tokens
        .saturating_sub(usage.cache_read_tokens)
        .saturating_sub(usage.cache_write_tokens);

    let input_cost = (fresh_input as f64) * rates.input / 1_000_000.0;
    let output_cost = (usage.output_tokens as f64) * rates.output / 1_000_000.0;
    let cache_read_cost = (usage.cache_read_tokens as f64) * rates.cache_read / 1_000_000.0;
    let cache_write_cost = (usage.cache_write_tokens as f64) * rates.cache_write / 1_000_000.0;

    let total = input_cost + output_cost + cache_read_cost + cache_write_cost;

    // "Saved" = what cache_read tokens WOULD have cost at full input rate
    // minus what they actually cost at cache_read rate
    let saved = (usage.cache_read_tokens as f64)
        * (rates.input - rates.cache_read) / 1_000_000.0;

    CostBreakdown {
        input_cost,
        output_cost,
        cache_read_cost,
        cache_write_cost,
        total,
        saved,
    }
}
```

### "Saved you $X" messaging

This is the primary cost education mechanism. On every session card, next to the cost:

```
$2.34 total · Saved you $18.72 via caching
```

The "saved" number is computed as the difference between what cache-read tokens would have cost at full input price vs their discounted cache-read price. For Opus 4.6, this is a **90% discount** on cached input tokens — the savings compound quickly across long sessions.

### Pricing updates

Model pricing is stored in a `HashMap<&str, ModelRates>` initialized at compile time. When Anthropic changes pricing:

1. Update the `MODEL_PRICING` constant in `crates/core/src/cost.rs`
2. Add an `/api/pricing` endpoint that returns current rates (allows UI to validate)

Future enhancement: load pricing from a JSON config file at startup for user overrides (e.g., enterprise discount tiers).

---

## 8. UI/UX Design

### 8.1 Design System

Mission Control uses a dark OLED theme optimized for dashboard monitoring. Status colors provide instant visual scanning across dozens of sessions.

| Token | Value | Usage |
|-------|-------|-------|
| `--bg-primary` | `#020617` (slate-950) | Page background |
| `--bg-card` | `#0F172A` (slate-900) | Card / panel background |
| `--bg-card-hover` | `#1E293B` (slate-800) | Card hover state |
| `--text-primary` | `#F8FAFC` (slate-50) | Primary text |
| `--text-secondary` | `#94A3B8` (slate-400) | Secondary text, labels |
| `--text-muted` | `#64748B` (slate-500) | Timestamps, IDs |
| `--border` | `#1E293B` (slate-800) | Card borders |
| `--status-active` | `#22C55E` (green-500) | STREAMING, TOOL_USE |
| `--status-waiting` | `#F59E0B` (amber-500) | WAITING_FOR_USER |
| `--status-idle` | `#64748B` (slate-500) | IDLE |
| `--status-complete` | `#3B82F6` (blue-500) | COMPLETE |
| `--accent` | `#8B5CF6` (violet-500) | Interactive elements, links |
| `--cost-saved` | `#22C55E` (green-500) | "Saved $X" text |
| `--context-cached` | `#3B82F6` (blue-500) | Cached tokens segment |
| `--context-new` | `#F59E0B` (amber-500) | Fresh tokens segment |
| `--context-available` | `#1E293B` (slate-800) | Remaining context capacity |

### 8.2 Session Card (Grid View)

The session card is the fundamental UI unit. It appears in Grid, List (as a row), and Kanban (as a card in a column).

```
┌─────────────────────────────────────────────────┐
│  ● myapp                                  ⋮     │ ← green dot = active, project name, overflow menu
│  feature/auth-flow                              │ ← branch name (muted)
│                                                  │
│  "Add JWT authentication to login endpoint..."   │ ← last user message (truncated)
│  → Editing auth.ts                               │ ← current activity (green)
│                                                  │
│  ┌─────────────────────────────────────────┐    │
│  │ ████████████████████░░░░░░░░░░░░░░░░░░ │    │ ← context gauge
│  │ ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓░░░░░░░░░░░░░░░░░░ │    │   blue=cached, amber=new, dark=available
│  │ 72% context used · Cache: Warm          │    │
│  └─────────────────────────────────────────┘    │
│                                                  │
│  12 turns · $2.34         Saved $18.72     i    │ ← cost + savings + info tooltip
│                                                  │
│  ┌─ Sub-agents ────────────────────────────┐    │ ← only shown when sub_agents.len() > 0
│  │  🔵 test_writer: Writing unit tests     │    │
│  │  🔵 reviewer: Reviewing auth.ts         │    │
│  │  ✅ researcher: Done (12s, $0.08)       │    │
│  └─────────────────────────────────────────┘    │
│                                                  │
│  2 min ago                            [Resume]   │ ← relative time + resume button (if waiting)
└─────────────────────────────────────────────────┘
```

#### Status dot behavior

| Status | Dot color | Animation |
|--------|-----------|-----------|
| `STREAMING` | `#22C55E` green | Pulse animation (opacity 0.4 → 1.0, 2s cycle) |
| `TOOL_USE` | `#22C55E` green | Pulse animation (faster: 1s cycle) |
| `WAITING_FOR_USER` | `#F59E0B` amber | Solid (no animation) |
| `IDLE` | `#64748B` slate | Solid (no animation) |
| `COMPLETE` | `#3B82F6` blue | None (static dot) |
| `DISCOVERED` | `#94A3B8` slate-400 | Fade-in animation |

#### Context gauge

A horizontal bar with three segments showing context window utilization:

```
┌────────────────────────────────────────────────┐
│ ████████████████▓▓▓▓▓▓▓▓░░░░░░░░░░░░░░░░░░░░ │
│  cached (blue)  new (amber)  available (dark)  │
└────────────────────────────────────────────────┘
```

- **Blue segment:** `cache_read_tokens / model_context_limit` — tokens served from cache
- **Amber segment:** `(input_tokens - cache_read_tokens) / model_context_limit` — fresh tokens this turn
- **Dark segment:** remaining capacity

When context usage exceeds 75%, show a warning icon with tooltip: "Context is 75% full. Claude may auto-compact soon, which resets cached context."

### 8.3 View Modes

#### Grid View (default)

Responsive card grid. Default view when user has < 12 sessions.

```
Breakpoints:
  ≥1920px:  4 columns
  ≥1440px:  3 columns
  ≥1024px:  2 columns
  <1024px:  1 column (mobile)

Card min-width: 340px
Card max-width: 480px
Gap: 16px
```

#### List View

Dense sortable table. Preferred when user has > 12 sessions. Each row is a condensed session card.

```
┌──────┬───────────────┬──────────┬───────────────────────────┬──────────┬─────────┬──────────┬──────────┐
│ ●    │ Project       │ Branch   │ Activity                  │ Context  │ Turns   │ Cost     │ Updated  │
├──────┼───────────────┼──────────┼───────────────────────────┼──────────┼─────────┼──────────┼──────────┤
│ ● ▌  │ myapp         │ feat/auth│ Editing auth.ts           │ ████░ 72%│ 12      │ $2.34    │ 2m ago   │
│ ● ▌  │ backend       │ main     │ Running tests             │ ███░░ 58%│ 8       │ $1.12    │ 30s ago  │
│ ◉ ▌  │ docs          │ main     │ Waiting for input         │ ██░░░ 41%│ 3       │ $0.28    │ 5m ago   │
│ ○ ▌  │ infra         │ deploy   │ Idle                      │ █░░░░ 23%│ 1       │ $0.05    │ 12m ago  │
└──────┴───────────────┴──────────┴───────────────────────────┴──────────┴─────────┴──────────┴──────────┘

Sortable columns: Project, Status, Context%, Turns, Cost, Updated
Click row → expand to show full card details inline
```

#### Kanban View

Sessions grouped by status in swim-lane columns. Cards move between columns in real-time as state changes.

```
┌──────────────────┬──────────────────┬──────────────────┬──────────────────┐
│  Active (4)      │  Waiting (2)     │  Idle (3)        │  Complete (1)    │
├──────────────────┼──────────────────┼──────────────────┼──────────────────┤
│ ┌──────────────┐ │ ┌──────────────┐ │ ┌──────────────┐ │ ┌──────────────┐ │
│ │ myapp        │ │ │ docs         │ │ │ infra        │ │ │ scripts      │ │
│ │ Editing...   │ │ │ 5m waiting   │ │ │ 12m idle     │ │ │ Done 1h ago  │ │
│ │ 72% · $2.34  │ │ │ 41% · $0.28  │ │ │ 23% · $0.05  │ │ │ $0.02        │ │
│ └──────────────┘ │ └──────────────┘ │ └──────────────┘ │ └──────────────┘ │
│ ┌──────────────┐ │ ┌──────────────┐ │ ┌──────────────┐ │                  │
│ │ backend      │ │ │ frontend     │ │ │ ml-pipeline  │ │                  │
│ │ Running tests│ │ │ 8m waiting   │ │ │ 20m idle     │ │                  │
│ │ 58% · $1.12  │ │ │ 35% · $0.15  │ │ │ 45% · $0.89  │ │                  │
│ └──────────────┘ │ └──────────────┘ │ └──────────────┘ │                  │
│ ...              │                  │                  │                  │
└──────────────────┴──────────────────┴──────────────────┴──────────────────┘
```

Column order: Active → Waiting → Idle → Complete (left-to-right priority).

Cards animate smoothly (300ms ease) when transitioning between columns. New sessions slide in from the top of their column.

#### Monitor Mode

Live session chat grid. Each pane shows a read-only chat view (RichPane) displaying the conversation for one session, with markdown rendering for tables, code blocks, etc.

```
┌─────────────────────────────────┬─────────────────────────────────┐
│  myapp · feature/auth           │  backend · main                 │
│  ● Editing auth.ts              │  ● Running `cargo test`         │
│  ───────────────────────────    │  ───────────────────────────    │
│  I'll add the JWT middleware    │  running 142 tests...           │
│  to the Express app. Let me     │  test auth::verify ... ok       │
│  first update the auth.ts       │  test auth::refresh ... ok      │
│  file:                          │  test db::migrate ... ok        │
│                                 │                                 │
│  ```typescript                  │  test result: ok. 142 passed;   │
│  import jwt from 'jsonwebtoken' │  0 failed; 0 ignored            │
│  ...                            │                                 │
│  $2.34 · 72% ctx · Cache: Warm  │  $1.12 · 58% ctx · Cache: Warm  │
├─────────────────────────────────┼─────────────────────────────────┤
│  docs · main                    │  frontend · feat/dashboard      │
│  ◉ Waiting for input            │  ● Searching files              │
│  ───────────────────────────    │  ───────────────────────────    │
│  I've updated the README with   │  Let me find all components     │
│  the new API documentation.     │  that import the old DatePicker │
│  Would you like me to also      │  ...                            │
│  update the changelog?          │                                 │
│              [Resume ▸]         │                                 │
│  $0.28 · 41% ctx · Cache: Cold  │  $0.45 · 33% ctx · Cache: Warm  │
└─────────────────────────────────┴─────────────────────────────────┘
```

**Monitor mode sub-modes:**

| Mode | Behavior |
|------|----------|
| **Auto Grid** (default) | Automatically arranges panes in a grid based on active session count and screen size |
| **Custom Layout** (Phase E) | User drags panes with react-mosaic to create custom tiling arrangement |

**Verbose toggle (per-pane):**

| Mode | What is shown |
|------|---------------|
| **Chat** (default) | User prompts + assistant text responses only. Clean, scannable view. |
| **Verbose** | All message types: user, assistant, tool calls, tool results, thinking blocks. Full detail. |

**Auto Grid responsive sizing:**

| Sessions | Screen ≥1920px | Screen ≥1440px | Screen ≥1024px |
|----------|---------------|----------------|----------------|
| 1 | 1x1 | 1x1 | 1x1 |
| 2 | 2x1 | 2x1 | 1x2 (stacked) |
| 3-4 | 2x2 | 2x2 | 2x2 |
| 5-6 | 3x2 | 2x3 | 2x3 |
| 7-8 | 4x2 | 4x2 | 2x4 |
| 9+ | 4x2 + scroll | 3x3 + scroll | 2x4 + scroll |

### 8.4 Sub-Agent Visualization

#### Swim Lanes (real-time, inside session detail panel)

When a session has active sub-agents (detected via `Task` tool invocations), show swim lanes:

```
┌─ Session: myapp · feature/auth ──────────────────────────────────────────────┐
│                                                                               │
│  Orchestrator  ━━━━━━━▶ Editing auth.ts ━━━━━▶ Review results ━━━▶           │
│                 │                                      ▲                      │
│  Sub-agent 1    └━━━━▶ Writing tests ━━━━━━━━━━━━━━━━━┘                      │
│                 │       test_writer · 12s · $0.08                              │
│  Sub-agent 2    └━━━━▶ Reviewing auth.ts ━━━━━━━━━━━━━━━━━━━▶ (still running)│
│                         reviewer · 18s · $0.12                                │
│  Sub-agent 3    └━━━━▶ Research: OAuth2 best practices ━━━━━━━┛ Done          │
│                         researcher · 8s · $0.04                               │
│                                                                               │
│  Timeline: 0s ─────────── 10s ─────────── 20s ─────────── 30s ──────► now    │
└───────────────────────────────────────────────────────────────────────────────┘
```

#### Compact Pills (inside Grid/Kanban session cards)

When space is limited, show sub-agents as inline pills:

```
Sub-agents: [● test_writer] [● reviewer] [✓ researcher]
```

- Blue dot = running
- Green check = completed
- Red x = failed
- Hover for details (description, duration, cost)

#### Timeline (historical, inside session detail view)

After a session completes, the swim lane becomes a static timeline showing when each sub-agent started, ran, and completed. Useful for understanding orchestration patterns and finding bottlenecks.

### 8.5 Mobile Design

Mission Control is accessible via Tailscale or Cloudflare Tunnel on mobile devices. The responsive design adapts:

| Element | Desktop | Mobile (<768px) |
|---------|---------|-----------------|
| View switcher | Horizontal tab bar | Bottom tab bar (fixed) |
| Session cards | Grid/List/Kanban | Vertical card stack |
| Card interaction | Hover → expand | Tap → full-screen detail |
| Monitor mode | Multi-pane grid | Single pane + swipe to switch |
| Navigation | Sidebar | Bottom sheet |
| Touch targets | N/A | Minimum 44px × 44px |
| Modals | Centered dialog | Bottom sheet (slide up) |
| Resume panel | Side panel | Full-screen overlay |

**Card stack with swipe:**

```
┌─────────────────────┐
│  ● myapp            │
│  feature/auth       │
│  Editing auth.ts    │ ← swipe left for next session
│  72% · $2.34        │    swipe right for previous
│  ─────────────────  │
│  [Resume]           │
└─────────────────────┘
   ○ ● ○ ○ ○          ← dot indicators
```

### 8.6 Resume Flow

When a session is in `WAITING_FOR_USER` state, the Resume button triggers a pre-flight panel:

```
┌─ Resume Session ─────────────────────────────────────────────┐
│                                                               │
│  myapp · feature/auth-flow                                    │
│  12 turns · 45K tokens · Last active 5 minutes ago            │
│                                                               │
│  ┌─ Pre-flight Check ──────────────────────────────────────┐ │
│  │                                                          │ │
│  │  Cache status: ● Warm (last API call 2 min ago)          │ │
│  │  Estimated resume cost: ~$0.15 (cached context)          │ │
│  │                                                          │ │
│  │  ⚠ Note: This spawns a NEW Claude Code process using    │ │
│  │  the Agent SDK. The original terminal session will       │ │
│  │  continue independently. Your messages go to the new     │ │
│  │  process only.                                           │ │
│  │                                                          │ │
│  └──────────────────────────────────────────────────────────┘ │
│                                                               │
│  Your message:                                                │
│  ┌──────────────────────────────────────────────────────────┐ │
│  │ Yes, also add rate limiting to the auth endpoint         │ │
│  └──────────────────────────────────────────────────────────┘ │
│                                                               │
│                              [Cancel]  [Resume & Send ▸]      │
└───────────────────────────────────────────────────────────────┘
```

**Why pre-flight?**

1. **Cost transparency.** Users know what they will spend before committing.
2. **Cache awareness.** If cache is cold (TTL expired), the resume costs ~10x more. Users can decide to wait or go to the terminal instead.
3. **Expectation setting.** The "spawns a NEW process" warning prevents confusion about why the original terminal doesn't show the new messages.

---

## 9. Cost Education UX

### Philosophy

Cost education happens at **decision points**, not documentation pages. Users learn about caching, context, and pricing naturally through contextual tooltips and visual indicators.

### Touchpoints

| Location | What the user sees | Educational content |
|----------|-------------------|---------------------|
| **Session card: cost** | `$2.34` | Tooltip: breakdown table (input $0.42, output $1.68, cache read $0.06, cache write $0.18) |
| **Session card: savings** | `Saved $18.72` | Tooltip: "Your prompt cache saved 90% on 124,800 cached input tokens" |
| **Session card: cache status** | `Cache: Warm` or `Cache: Cold` | Tooltip (warm): "Tokens from prior turns are cached. 90% discount on cached input." / Tooltip (cold): "Cache expired (>5 min inactive). Next turn sends all tokens at full price." |
| **Context gauge: 75% warning** | Warning icon | Tooltip: "Context is 75% full. Claude may auto-compact soon, which drops cached context and restarts at full price." |
| **Resume pre-flight: cost estimate** | `~$0.15 (cached)` or `~$1.50 (uncached)` | Shows both scenarios if cache status is uncertain. "Cache expires after 5 min of inactivity." |
| **Aggregate summary bar** | `15 sessions · $12.47 total · Saved $89.30` | Tooltip: per-model breakdown of aggregate costs |

### "Saved you $X" calculation

```
saved = cache_read_tokens × (base_input_rate - cache_read_rate) / 1,000,000

Example for Opus 4.6:
  cache_read_tokens: 124,800
  base_input_rate: $15.00 / 1M
  cache_read_rate: $1.50 / 1M
  saved = 124,800 × ($15.00 - $1.50) / 1,000,000 = $1.68
```

This number is psychologically powerful: it reframes caching from a technical detail into "Claude just saved you $18." Users naturally learn to keep sessions active (warm cache) rather than starting fresh.

---

## 10. Real-Time Update Strategy

Mission Control uses two transport protocols, chosen based on data characteristics:

### SSE (Server-Sent Events) — structured data

| Aspect | Detail |
|--------|--------|
| **What** | Session status, cost, activity, sub-agent updates |
| **Endpoint** | `GET /api/live/stream` |
| **Format** | JSON event objects |
| **Frequency** | 1-5 second intervals (adaptive: faster when sessions are active, slower when all idle) |
| **Reconnection** | Built-in browser auto-reconnect with `retry:` header |
| **Why SSE** | Simpler than WebSocket for one-directional structured data. Browser handles reconnection. Works through HTTP proxies. |

**Adaptive frequency:**

| Condition | Update interval |
|-----------|----------------|
| Any session is `STREAMING` | 1s |
| Any session is `TOOL_USE` | 2s |
| All sessions are `WAITING` or `IDLE` | 5s |
| No active sessions | 10s (heartbeat only) |

### WebSocket — structured message stream (Monitor mode)

| Aspect | Detail |
|--------|--------|
| **What** | Structured JSONL messages parsed into user/assistant/tool/thinking types for RichPane rendering |
| **Endpoint** | `GET /api/live/sessions/:id/terminal` (upgrade to WS) |
| **Format** | JSON text frames (structured message objects) |
| **Frequency** | Real-time (~100ms batching for rendering efficiency) |
| **Reconnection** | Manual reconnect with exponential backoff (1s, 2s, 4s, 8s, max 30s) |
| **Why WebSocket** | Bidirectional (for future interactive features, verbose toggle), low-latency streaming, no proxy buffering issues |

### Vite Dev Proxy workaround

Per project rules, SSE is broken through Vite's proxy. In dev mode, both SSE and WebSocket connect directly to the Rust server:

```tsx
function liveUrl(path: string): string {
  if (window.location.port === '5173') {
    return `http://localhost:47892${path}`;  // bypass Vite proxy
  }
  return path;  // production: same origin
}
```

### Staleness handling

| Condition | UI behavior |
|-----------|-------------|
| SSE connected, data fresh | Normal rendering |
| SSE disconnected < 5s | No visual change (reconnecting) |
| SSE disconnected 5-30s | Session cards dim to 60% opacity, "Reconnecting..." toast |
| SSE disconnected > 30s | Banner: "Connection lost. Data may be stale." + manual retry button |
| SSE reconnected | Toast: "Reconnected" (auto-dismiss 3s), full state refresh |
| WebSocket disconnected (Monitor) | Pane shows "Disconnected" overlay with reconnect spinner |

---

## 11. API Endpoints

### REST Endpoints (Rust / Axum)

All endpoints are prefixed with `/api/live/` to distinguish from the existing historical session API at `/api/sessions/`.

#### `GET /api/live/sessions`

List all discovered live sessions.

**Query params:**

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `status` | string | (all) | Filter by status: `active`, `waiting`, `idle`, `complete` |
| `project` | string | (all) | Filter by project name |
| `sort` | string | `last_activity_at` | Sort field: `project`, `status`, `cost`, `context`, `last_activity_at` |
| `order` | string | `desc` | Sort order: `asc`, `desc` |

**Response:**

```json
{
  "sessions": [
    {
      "session_id": "0191a2b3-c4d5-6e7f-8a9b-0c1d2e3f4a5b",
      "project_name": "myapp",
      "project_path": "/Users/alice/dev/myapp",
      "branch": "feature/auth-flow",
      "status": "streaming",
      "pid": 12345,
      "last_user_message": "Add JWT authentication to login...",
      "last_assistant_message": "I'll update auth.ts with the JWT middleware...",
      "current_activity": "Editing auth.ts",
      "last_activity_at": 1707350400,
      "turn_count": 12,
      "token_usage": {
        "input_tokens": 45200,
        "output_tokens": 12800,
        "cache_read_tokens": 38400,
        "cache_write_tokens": 6800
      },
      "estimated_cost_usd": 2.34,
      "context_usage_pct": 0.72,
      "sub_agents": [
        {
          "agent_type": "test_writer",
          "status": "running",
          "description": "Writing unit tests for auth middleware",
          "started_at": 1707350380,
          "completed_at": null,
          "cost_usd": 0.08
        }
      ],
      "cache_status": "warm",
      "model": "claude-opus-4-6"
    }
  ],
  "total": 8
}
```

#### `GET /api/live/sessions/:id`

Get full detail for a single live session, including recent messages.

**Response:** Same as above with additional fields:

```json
{
  "session": { /* ...LiveSession fields... */ },
  "recent_messages": [
    {"role": "user", "content": "Add JWT auth...", "timestamp": 1707350300},
    {"role": "assistant", "content": "I'll update...", "timestamp": 1707350305}
  ],
  "cost_breakdown": {
    "input_cost": 0.42,
    "output_cost": 1.68,
    "cache_read_cost": 0.06,
    "cache_write_cost": 0.18,
    "total": 2.34,
    "saved": 18.72
  }
}
```

#### `GET /api/live/sessions/:id/messages`

Paginated message history for a live session. Uses tail-first loading (newest messages first) for fast initial render.

**Query params:**

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `limit` | u32 | 20 | Messages per page |
| `before` | i64 | (latest) | Cursor: return messages before this timestamp |

#### `GET /api/live/summary`

Aggregate summary across all live sessions.

**Response:**

```json
{
  "total_sessions": 15,
  "active_sessions": 4,
  "waiting_sessions": 3,
  "idle_sessions": 6,
  "complete_sessions": 2,
  "total_cost_usd": 12.47,
  "total_saved_usd": 89.30,
  "total_tokens": {
    "input_tokens": 892000,
    "output_tokens": 245000,
    "cache_read_tokens": 756000,
    "cache_write_tokens": 136000
  },
  "sessions_by_project": [
    {"project_name": "myapp", "count": 4},
    {"project_name": "backend", "count": 3}
  ]
}
```

#### `GET /api/pricing`

Current model pricing used for cost calculations. Allows the frontend to display rates and validate calculations.

**Response:**

```json
{
  "models": {
    "claude-opus-4-6": {
      "input_per_million": 15.00,
      "output_per_million": 75.00,
      "cache_read_per_million": 1.50,
      "cache_write_per_million": 18.75
    },
    "claude-sonnet-4-5-20241022": {
      "input_per_million": 3.00,
      "output_per_million": 15.00,
      "cache_read_per_million": 0.30,
      "cache_write_per_million": 3.75
    },
    "claude-haiku-4-5-20241022": {
      "input_per_million": 1.00,
      "output_per_million": 5.00,
      "cache_read_per_million": 0.10,
      "cache_write_per_million": 1.25
    }
  },
  "last_updated": "2026-02-10"
}
```

### SSE Endpoint (Rust / Axum)

#### `GET /api/live/stream`

Server-Sent Events stream for real-time session updates.

**Event types:**

| Event | Data | When |
|-------|------|------|
| `session_discovered` | `LiveSession` | New session file detected |
| `session_updated` | `LiveSession` | Status, cost, activity, or context changed |
| `session_completed` | `{session_id, final_cost, total_tokens}` | Session transitioned to COMPLETE |
| `session_removed` | `{session_id}` | Session file deleted or aged out |
| `subagent_started` | `{session_id, SubAgentInfo}` | New Task tool invocation detected |
| `subagent_completed` | `{session_id, agent_type, cost_usd, duration_s}` | Sub-agent finished |
| `summary_updated` | `LiveSummary` | Aggregate stats changed |
| `heartbeat` | `{timestamp}` | Keep-alive (every 10s when idle) |

**Example SSE stream:**

```
event: session_discovered
data: {"session_id":"abc123","project_name":"myapp","status":"discovered",...}

event: session_updated
data: {"session_id":"abc123","status":"streaming","current_activity":"Editing auth.ts",...}

event: subagent_started
data: {"session_id":"abc123","agent_type":"test_writer","description":"Writing unit tests",...}

event: summary_updated
data: {"total_sessions":15,"active_sessions":4,"total_cost_usd":12.47,...}

: heartbeat
data: {"timestamp":1707350400}
```

### WebSocket Endpoint (Rust / Axum)

#### `GET /api/live/sessions/:id/terminal` → Upgrade to WebSocket

Streams structured messages for Monitor mode. The server reads new JSONL entries for the session, parses them into typed message objects (user/assistant/tool_use/tool_result/thinking), and streams them as JSON text frames to RichPane.

**Protocol:**

```
Client → Server (text frame): {"type": "subscribe"}
Server → Client (text frame): {"type": "content", "text": "I'll update auth.ts..."}
Server → Client (text frame): {"type": "activity", "activity": "Editing auth.ts"}
Server → Client (text frame): {"type": "status", "status": "tool_use"}
Server → Client (text frame): {"type": "done"}
```

### Control Endpoints (Node.js Sidecar — Phase F)

These endpoints are served by the Node.js sidecar process, proxied through the Rust server.

#### `POST /api/control/resume`

Resume a session via the Agent SDK.

**Request:**

```json
{
  "session_id": "abc123",
  "message": "Also add rate limiting",
  "jsonl_path": "/Users/alice/.claude/projects/-Users-alice-dev-myapp/abc123.jsonl"
}
```

**Response:**

```json
{
  "control_session_id": "ctrl_abc123",
  "status": "started",
  "estimated_cost": 0.15,
  "cache_status": "warm"
}
```

#### `POST /api/control/send`

Send a follow-up message to a resumed session.

**Request:**

```json
{
  "control_session_id": "ctrl_abc123",
  "message": "Looks good, now run the tests"
}
```

#### `GET /api/control/sessions/:id/stream` → WebSocket

Bidirectional WebSocket for dashboard chat with a controlled session.

```
Client → Server: {"type": "message", "content": "Run the tests"}
Server → Client: {"type": "content", "text": "I'll run `cargo test`..."}
Server → Client: {"type": "tool_use", "tool": "Bash", "input": "cargo test"}
Server → Client: {"type": "tool_result", "output": "142 tests passed"}
Server → Client: {"type": "done", "usage": {...}}
```

---

## 12. Implementation Phases

### Phase A: Read-Only Monitoring (2-3 weeks)

**Goal:** Users can open `/mission-control` and see all active Claude Code sessions with real-time status, cost, and context usage.

| Task | Scope | Estimate |
|------|-------|----------|
| JSONL file watcher (notify) | Rust: watch `~/.claude/projects/`, debounce, track offsets | 2d |
| Incremental JSONL parser | Core: parse new bytes only, SIMD pre-filter, extract state fields | 3d |
| Session state machine | Server: status derivation, transition rules, activity labels | 2d |
| Process detector (sysinfo) | Server: PID correlation, 10s refresh cycle | 1d |
| Cost calculator | Core: pricing table, per-session + aggregate calculation | 1d |
| In-memory state (DashMap) | Server: `LiveState` struct, concurrent read/write | 0.5d |
| SSE endpoint | Server: `/api/live/stream` with adaptive frequency | 1d |
| REST endpoints | Server: `/api/live/sessions`, `/sessions/:id`, `/summary`, `/pricing` | 1d |
| Grid view (React) | Frontend: session cards, status dots, context gauge, cost display | 3d |
| Summary bar | Frontend: aggregate stats bar at top of page | 0.5d |

**Depends on:** Nothing (foundation phase)

**Deliverable:** Working Grid view with real-time session cards showing status, activity, cost, context, and cache indicators.

### Phase B: Views & Layout (1-2 weeks)

**Goal:** Users can switch between Grid, List, and Kanban views. Mobile responsive.

| Task | Scope | Estimate |
|------|-------|----------|
| View mode switcher | Frontend: tab bar with Grid/List/Kanban, URL param persistence | 0.5d |
| List view | Frontend: sortable table, inline expand, column resize | 2d |
| Kanban view | Frontend: status columns, animated card transitions | 2d |
| Keyboard shortcuts | Frontend: `1`=Grid, `2`=List, `3`=Kanban, `4`=Monitor | 0.5d |
| Mobile responsive | Frontend: card stack, swipe, bottom tabs, 44px targets | 2d |
| Filter/sort controls | Frontend: status filter, project filter, sort dropdown | 1d |

**Depends on:** Phase A

### Phase C: Monitor Mode (2-3 weeks)

**Goal:** Users can view live terminal output for multiple sessions in a tiled grid.

| Task | Scope | Estimate |
|------|-------|----------|
| WebSocket endpoint | Server: `/api/live/sessions/:id/terminal`, content streaming | 2d |
| xterm.js integration | Frontend: terminal pane component, WebGL renderer, fit addon | 3d |
| Auto Grid layout | Frontend: responsive pane arrangement based on session count | 2d |
| Pane header | Frontend: project name, status, cost, context mini-bar | 1d |
| Pane controls | Frontend: maximize/restore, pin session, close pane | 1d |
| Session picker | Frontend: dropdown to select which sessions appear in panes | 1d |

**Depends on:** Phase B (needs view switcher infrastructure)

### Phase D: Sub-Agent Visualization (1-2 weeks)

**Goal:** Users can see sub-agent activity (Task tool) in swim lanes and compact pills.

| Task | Scope | Estimate |
|------|-------|----------|
| Sub-agent extraction | Core: detect Task tool_use/result pairs in JSONL, track lifecycle | 2d |
| SSE events | Server: `subagent_started`, `subagent_completed` events | 0.5d |
| Swim lanes | Frontend: horizontal timeline with parallel lanes per agent | 3d |
| Compact pills | Frontend: inline sub-agent badges for card views | 1d |
| Timeline (history) | Frontend: static post-session view of agent orchestration | 1d |

**Depends on:** Phase C (swim lanes appear inside Monitor panes and expanded cards)

### Phase E: Custom Layout (1 week)

**Goal:** Users can drag-and-drop panes to create custom Monitor mode layouts.

| Task | Scope | Estimate |
|------|-------|----------|
| react-mosaic integration | Frontend: mosaic container wrapping xterm panes | 2d |
| Layout save/load | Frontend: localStorage persistence of layout config | 1d |
| Layout presets | Frontend: "2x2", "1+3", "Focus" preset buttons | 1d |
| Drag-and-drop polish | Frontend: drop indicators, resize handles, min size constraints | 1d |

**Depends on:** Phase C (needs Monitor mode panes)

### Phase F: Interactive Control (2-3 weeks)

**Goal:** Users can resume waiting sessions and send messages from the dashboard.

| Task | Scope | Estimate |
|------|-------|----------|
| Node.js sidecar scaffold | Control: process spawning, JSON-RPC IPC, lifecycle management | 2d |
| Agent SDK integration | Control: session resume with JSONL history loading | 3d |
| Bidirectional WebSocket | Control + Server: chat message relay | 2d |
| Resume pre-flight UI | Frontend: cost estimate, cache status, warning panel | 1d |
| Dashboard chat UI | Frontend: message input, streaming response, tool call display | 3d |
| Error handling | All: sidecar crash recovery, API errors, timeout handling | 1d |

**Depends on:** Phase A (needs session list), independent of B-E

### Phase dependency graph

```
A (foundation)
├──► B (views)
│    └──► C (monitor)
│         ├──► D (sub-agents)
│         └──► E (custom layout)
└──► F (interactive control)
```

**Total estimated time:** 10-15 weeks (1 developer, sequential)

**Recommended parallelization:** A developer can work on Phase F independently once Phase A is complete, while another works on B→C→D→E.

---

## 13. Key Constraints

### macOS PTY attachment is not feasible

The ideal solution would be to "attach" to an existing Claude Code terminal session and relay its PTY output to the browser. This is not possible on modern macOS:

| Mechanism | Status | Why |
|-----------|--------|-----|
| `TIOCSTI` (inject keystrokes) | **Blocked** by macOS SIP since Ventura | Security hardening against keystroke injection |
| `reptyr` (re-parent PTY) | **Not supported** on macOS | Linux-only, uses `ptrace` which macOS restricts |
| `/proc/PID/fd/0` | **Not available** on macOS | Linux procfs, no macOS equivalent |
| `screen`/`tmux` session sharing | **Requires pre-setup** | User must start sessions inside tmux. Too much friction. |
| `script` command piping | **One-directional** | Can capture output but cannot send input |

**Our approach:** Read-only monitoring via JSONL file parsing (zero-setup). Interactive control via Agent SDK (spawns new process with conversation history, does not attach to existing).

### Agent SDK limitations

| Limitation | Impact | Mitigation |
|-----------|--------|------------|
| Cannot attach to existing sessions | "Resume" spawns a NEW Claude Code process | Clear UX warning in pre-flight panel |
| npm-only (no Rust SDK) | Requires Node.js sidecar | JSON-RPC IPC keeps it lightweight |
| Each resume creates new API conversation | Full history must be resent | Prompt caching makes this cost-effective |
| No sub-agent control | Cannot pause/cancel individual sub-agents | Display-only visualization for now |

### Anthropic API is stateless

Every API request sends the **full conversation history** as input. There is no server-side session state. This means:

1. **Resuming is expensive without caching.** A 50-turn session with 100K tokens of history costs $1.50 in input alone (Opus 4.6). With warm cache, it costs $0.15 (90% discount).
2. **Prompt caching has a 5-minute TTL.** If the user waits too long to resume, the cache expires and the next request pays full price on all tokens.
3. **Auto-compaction is lossy.** When context exceeds ~75-95% of the model's limit, Claude Code compacts the history (summarizes older turns). This resets the cache.

**Mission Control leverages this by:**
- Showing cache warm/cold status prominently
- Warning users about cache TTL expiry risk
- Computing cost estimates with and without caching
- Educating about "saved $X" on every card

### Existing tools do not solve this

| Tool | Architecture | Why it is not enough |
|------|-------------|---------------------|
| **OpenClaw** (180k+ stars) | Personal AI agent built ON TOP of Claude Code CLI subscription. Uses `claude-max-api-proxy` to route OpenAI-format requests through CLI auth. WebSocket Gateway control plane. Session isolation via agents. | General-purpose life agent (WhatsApp, Telegram, Slack), NOT a session monitoring dashboard. No live JSONL watching. No cost/context visualization. |
| **OpenClaw Dashboard** | Scrapes Claude CLI `/usage` via tmux, reads `~/.openclaw/agents/` session dirs, 5s polling. Glassmorphic dark UI. Rate limit monitoring. | Monitors OpenClaw agent sessions, not raw Claude Code CLI sessions. tmux scraping is fragile. No JSONL incremental parsing. No context gauge. |
| **Claude Code `/insights`** | Built-in command. Uses Haiku (4k output tokens/session) for facet extraction. 6-stage pipeline. Caches at `~/.claude/usage-data/facets/`. Static HTML report. | Snapshot only, no time-series trends. No live monitoring. No cost-quality correlation. No cross-project comparison. Not integrated into any dashboard. |
| **Agent Sessions** | Local-first session aggregator. Reads dirs from 7 CLI tools (Claude Code, Codex, Gemini, etc.). Apple Notes-style search. macOS-only. | Read-only browsing. No cost tracking, no context visualization, no live status monitoring, no interactive control. |
| **NanoClaw** | Uses Agent SDK to spawn sessions in Apple containers via stdin/stdout | Container-based, not local-machine monitoring. No cost tracking. |
| **claude-code-ui** | XState-based web UI wrapping single session | Single-session only. No multi-session dashboard. |
| **claude-code-monitor** | React hooks for session events | Hooks library, not a dashboard. No cost or context tracking. |
| **clog** | Web viewer for conversation logs | Read-only historical viewer. No live sessions. |

Mission Control combines monitoring + cost tracking + context health + multi-view visualization + optional interactive control — none of the existing tools provide this combination.

### Claude Code CLI Integration Approaches (Research)

Three known approaches for programmatic interaction with a user's Claude Code subscription:

| Approach | How it works | Complexity | Reliability | Use case |
|----------|-------------|-----------|-------------|----------|
| **CLI Direct** | `claude -p --model haiku --output-format json "prompt"` | Low | High | Classification, analysis, custom insights |
| **claude-max-api-proxy** (OpenClaw) | Local proxy at `localhost:3456` accepts OpenAI format, translates to CLI commands, routes through subscription | Medium | Medium (3rd party) | OpenAI-compatible tool integration |
| **Agent SDK** (npm) | `@anthropic-ai/agent-sdk` spawns new Claude Code process, IPC via stdin/stdout | High | High (official) | Session resume, interactive control (Phase F) |

**For our Insights/Analysis features:**
- **Default:** Read `/insights` facet cache (`~/.claude/usage-data/facets/*.json`) — zero API calls, free
- **Fallback:** CLI Direct (`claude -p --model haiku`) for custom analysis — uses existing subscription
- **Deferred:** claude-max-api-proxy as opt-in for users who want OpenAI-compatible endpoint access

**For Mission Control Phase F (Interactive Control):**
- Agent SDK via Node.js sidecar (already planned)

**Key constraint:** Claude Pro/Max subscriptions do NOT include API keys. Authentication goes through `claude setup-token` which generates OAuth tokens for the CLI. All approaches above use the CLI as the auth layer.

---

## Appendix A: Navigation Integration

Mission Control adds a new entry to the sidebar navigation:

```
Sidebar
├── Dashboard        (existing)
├── Sessions         (existing)
├── Contributions    (existing, Theme 3)
├── Insights         (pending, Theme 4)
├── Mission Control  ← NEW
├── Projects         (existing)
└── System           (pending, Theme 4)
```

Icon: `Monitor` from Lucide (a screen with a graph/pulse line).

The Mission Control page is independent of project filtering — it always shows all sessions across all projects. Project filtering is available within the page via the status/project filter controls.

## Appendix B: Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `1` | Switch to Grid view |
| `2` | Switch to List view |
| `3` | Switch to Kanban view |
| `4` | Switch to Monitor mode |
| `r` | Refresh sessions (manual) |
| `/` | Focus search/filter input |
| `j` / `k` | Navigate sessions (down/up) |
| `Enter` | Expand selected session |
| `Esc` | Collapse expanded session / close modal |
| `?` | Show keyboard shortcuts help |

## Appendix C: Security Considerations

| Concern | Mitigation |
|---------|-----------|
| JSONL files may contain sensitive code | Mission Control reads the same files the user already has access to. No elevation of privilege. |
| WebSocket/SSE on localhost | Bound to `127.0.0.1` by default. Remote access requires user-configured tunnel. |
| Agent SDK requires API key | Key is read from user's existing `~/.claude/` config. No new credential storage. |
| Process enumeration | `sysinfo` reads process list at user privilege level. No root access needed. |
| Node.js sidecar | Spawned as child process of the Rust server. Inherits user permissions. No network listener (stdio IPC only). |

## Appendix D: Performance Budget

| Metric | Target | Rationale |
|--------|--------|-----------|
| Time to first session card render | < 500ms | Initial scan of JSONL files + first SSE event |
| SSE event latency (file change → browser) | < 2s | 500ms debounce + 500ms parse + 1s SSE interval |
| Memory per monitored session | < 50KB | LiveSession struct + message previews + sub-agent info |
| Memory for 50 concurrent sessions | < 5MB | 50 × 50KB + DashMap overhead + SSE buffers |
| CPU idle (no active sessions) | < 1% | File watcher sleeps, 10s heartbeat only |
| CPU active (10 streaming sessions) | < 5% | Incremental parse, SIMD pre-filter, adaptive SSE |
| WebSocket bandwidth per Monitor pane | < 10KB/s | Text content only, batched at 100ms |

## Appendix E: Future Considerations (Post-MVP)

These are explicitly out of scope for the Phase A-F roadmap but inform architectural decisions:

| Feature | Why deferred | What to preserve |
|---------|-------------|-----------------|
| **Teams / Multi-user** | Enterprise tier, requires auth + data aggregation | Keep session data structure extensible (user field) |
| **Swarm visualization** | Multi-agent orchestration graphs | Sub-agent swim lanes are the foundation |
| **AI auto-triage** | "Which session needs attention?" | Expose priority scoring in LiveSession struct |
| **Cost alerts** | "Notify me when total cost exceeds $X" | Cost calculator already supports thresholds |
| **Historical playback** | Replay a completed session in Monitor mode | WebSocket endpoint can serve from JSONL history |
| **Linux/Windows** | Platform-specific process detection | Abstract `sysinfo` behind a trait |
| **tmux integration** | Actual PTY sharing for read-only terminal view | Phase F adds xterm.js for interactive control; tmux can feed into it |
