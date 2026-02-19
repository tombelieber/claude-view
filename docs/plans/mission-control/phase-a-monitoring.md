---
status: done
date: 2026-02-10
phase: A
depends_on: none
---

# Phase A: Read-Only Monitoring

> Foundation layer. Watch all active Claude Code sessions via JSONL file watching, derive session state, calculate costs, and stream updates to the browser via SSE.

## Overview

Phase A delivers the core infrastructure that every subsequent Mission Control phase depends on. It adds a `live/` module to `crates/server/` for real-time JSONL file watching and process detection, extends `crates/core/` with an incremental tail parser and cost calculator, and introduces a new `/mission-control` page in the React frontend with a grid of session cards.

**Key constraints from CLAUDE.md that apply here:**
- `notify` crate is already declared in workspace `Cargo.toml` (marked Phase 2)
- SSE must bypass Vite proxy in dev mode (use `sseUrl()` helper)
- `memmem::Finder` must be created once, reused (never per-line)
- Server must not block on startup work (file watcher spawns in background)
- Timestamps of 0 must never be rendered as dates
- No hooks after early returns in React

## Estimated Effort

| Area | LOC | Time |
|------|-----|------|
| Backend (Rust) | ~1,200 | 3-4 days |
| Frontend (React) | ~800 | 2-3 days |
| Tests | ~600 | 1-2 days |
| **Total** | **~2,600** | **6-9 days** |

---

## Step 1: File Watcher Setup

**Crate:** `crates/server`
**Files to create:**
- `crates/server/src/live/mod.rs`
- `crates/server/src/live/watcher.rs`

**Files to modify:**
- `crates/server/Cargo.toml` (add `notify`, `sysinfo` dependencies)
- `crates/server/src/lib.rs` (add `pub mod live;`)

### 1.1 Add Dependencies

Add to `crates/server/Cargo.toml`:

```toml
[dependencies]
# ... existing deps ...
notify = { workspace = true }
memchr = { workspace = true }      # for memmem::Finder in tail parser
```

Add to `Cargo.toml` (workspace level), if not already present:

```toml
sysinfo = "0.33"
```

Then add to `crates/server/Cargo.toml`:

```toml
sysinfo = { workspace = true }
```

### 1.2 Watcher Implementation

`crates/server/src/live/watcher.rs`:

```rust
//! File system watcher for live JSONL session monitoring.
//!
//! Uses `notify` to watch ~/.claude/projects/**/*.jsonl for modifications.
//! Debounces events (100ms) to avoid processing partial writes.

use notify::{RecommendedWatcher, RecursiveMode, Watcher, Config};
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc;

/// Events emitted by the file watcher.
#[derive(Debug, Clone)]
pub enum FileEvent {
    /// A JSONL file was created or modified.
    Modified(PathBuf),
    /// A JSONL file was removed.
    Removed(PathBuf),
}
```

**Implementation requirements:**
- Watch `~/.claude/projects/` recursively using `notify::RecommendedWatcher`
- Filter events to only `.jsonl` files
- Debounce: collect events for 100ms before forwarding (Claude Code writes multiple lines in rapid succession)
- On startup, scan for files modified in the last 24 hours and emit `Modified` events for each
- Send events via `tokio::sync::mpsc::Sender<FileEvent>`
- Handle watch errors gracefully (log + continue, don't crash)

**Startup scan logic:**
```rust
/// Initial scan: find all JSONL files modified in the last 24h.
/// This avoids scanning ancient sessions on first boot.
async fn initial_scan(projects_dir: &Path) -> Vec<PathBuf> {
    let cutoff = SystemTime::now() - Duration::from_secs(24 * 60 * 60);
    // Walk ~/.claude/projects/*/*.jsonl
    // Filter: metadata.modified() > cutoff
    // Return sorted by modification time (newest first)
}
```

### 1.3 Register Module

In `crates/server/src/lib.rs`, add:
```rust
pub mod live;
```

In `crates/server/src/live/mod.rs`:
```rust
pub mod watcher;
pub mod state;
pub mod process;
pub mod manager;
```

---

## Step 2: Incremental JSONL Tail Parser

**Crate:** `crates/core`
**Files to create:**
- `crates/core/src/live_parser.rs`

**Files to modify:**
- `crates/core/Cargo.toml` (add `memchr` dependency)
- `crates/core/src/lib.rs` (add `pub mod live_parser;`)

### 2.1 Add Dependencies

Add to `crates/core/Cargo.toml`:
```toml
memchr = { workspace = true }
```

### 2.2 Tail Parser Implementation

`crates/core/src/live_parser.rs`:

```rust
//! Incremental JSONL tail parser for live session monitoring.
//!
//! Tracks file read position per session, reads only new bytes on each
//! update. Uses memmem::Finder (created ONCE) for SIMD pre-filtering
//! before JSON deserialization.

use memchr::memmem;
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Tracks read position for incremental parsing.
pub struct TailState {
    /// Last known file size (byte offset we've read up to).
    pub offset: u64,
    /// Last known file modification time.
    pub last_modified: std::time::SystemTime,
}

/// Data extracted from a single new JSONL line.
#[derive(Debug, Clone)]
pub struct LiveLine {
    pub line_type: LineType,
    pub role: Option<String>,
    pub content_preview: String,  // Truncated to 200 chars
    pub tool_names: Vec<String>,
    pub model: Option<String>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub cache_creation_tokens: Option<u64>,
    pub timestamp: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineType {
    User,
    Assistant,
    System,
    Progress,
    Summary,
    Other,
}

/// Parse new lines from a JSONL file starting at `offset`.
///
/// Returns the parsed lines and the new offset (for the next call).
/// Handles partial line writes: if the last line doesn't end with `\n`,
/// it is NOT parsed (the offset stays before it, so it will be re-read
/// next time when the write completes).
pub fn parse_tail(
    path: &Path,
    offset: u64,
    finders: &TailFinders,
) -> std::io::Result<(Vec<LiveLine>, u64)> {
    // 1. Open file, seek to offset
    // 2. Read remaining bytes into buffer
    // 3. Split on newlines
    // 4. If last chunk doesn't end with \n, exclude it (partial write)
    // 5. For each complete line, use SIMD pre-filter then JSON parse
    // 6. Return (lines, new_offset)
    todo!()
}

/// Pre-computed SIMD finders. Create ONCE at startup, pass by reference.
///
/// Per CLAUDE.md rules: "memmem::Finder: create once, reuse"
pub struct TailFinders {
    pub type_user: memmem::Finder<'static>,
    pub type_assistant: memmem::Finder<'static>,
    pub type_system: memmem::Finder<'static>,
    pub type_progress: memmem::Finder<'static>,
    pub type_summary: memmem::Finder<'static>,
    pub content_key: memmem::Finder<'static>,
    pub model_key: memmem::Finder<'static>,
    pub usage_key: memmem::Finder<'static>,
    pub tool_use_key: memmem::Finder<'static>,
    pub name_key: memmem::Finder<'static>,
}

impl TailFinders {
    pub fn new() -> Self {
        Self {
            type_user: memmem::Finder::new(b"\"type\":\"user\""),
            type_assistant: memmem::Finder::new(b"\"type\":\"assistant\""),
            type_system: memmem::Finder::new(b"\"type\":\"system\""),
            type_progress: memmem::Finder::new(b"\"type\":\"progress\""),
            type_summary: memmem::Finder::new(b"\"type\":\"summary\""),
            content_key: memmem::Finder::new(b"\"content\":"),
            model_key: memmem::Finder::new(b"\"model\":"),
            usage_key: memmem::Finder::new(b"\"usage\":"),
            tool_use_key: memmem::Finder::new(b"\"tool_use\""),
            name_key: memmem::Finder::new(b"\"name\":"),
        }
    }
}
```

**Implementation requirements:**
- Use `std::fs::File` (not async) since reads are fast (<1ms per tail) and called from a `spawn_blocking` context
- SIMD pre-filter: check `type_user`/`type_assistant`/etc. finders BEFORE `serde_json::from_slice`
- Content preview: truncate to 200 chars (reuse `truncate_preview` from `discovery.rs`)
- Handle the "content is array of blocks" format (same as existing parser)
- Extract model from `message.model` field (Claude Code's JSONL format includes this)
- Extract usage from `usage` top-level field: `{input_tokens, output_tokens, cache_read_input_tokens, cache_creation_input_tokens}`
- Extract tool names from `tool_use` content blocks

**Partial line handling:**
```
Bytes read: ...}\n{"type":"assi
                   ^^^^^^^^^^ incomplete - do NOT parse
                   new_offset = position of this incomplete line start
```

---

## Step 3: Session State Machine

**Crate:** `crates/server`
**Files to create:**
- `crates/server/src/live/state.rs`

### 3.1 Status Enum

```rust
/// Current status of a live Claude Code session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    /// Claude is actively generating a response (streaming tokens).
    Streaming,
    /// Claude is executing a tool (Read, Edit, Bash, etc.).
    ToolUse,
    /// Waiting for the user to provide input.
    WaitingForUser,
    /// Session exists but no activity in >60s.
    Idle,
    /// Session appears finished (JSONL not modified in >5min AND no running process).
    Complete,
}
```

### 3.2 State Transition Rules

Transitions are derived from the combination of:
1. **Last JSONL entry type** (from tail parser)
2. **File modification recency** (mtime delta from now)
3. **Process existence** (is there a `claude` process with matching CWD?)

| Last Entry Type | mtime < 10s | mtime 10s-60s | mtime 60s-5min | mtime > 5min |
|-----------------|-------------|---------------|----------------|--------------|
| `assistant` (streaming) | `Streaming` | `Streaming` | `Idle` | `Complete`* |
| `assistant` (tool_use block) | `ToolUse` | `ToolUse` | `Idle` | `Complete`* |
| `user` | `WaitingForUser` | `WaitingForUser` | `Idle` | `Complete`* |
| `system` (turn_duration) | `WaitingForUser` | `WaitingForUser` | `Idle` | `Complete`* |
| `progress` | `ToolUse` | `ToolUse` | `Idle` | `Complete`* |

*`Complete` is only set if there is also NO running `claude` process for this session. If a process exists, the status stays `Idle` (the session might be doing a long operation that doesn't write to JSONL).

```rust
/// Derive session status from parsed state.
pub fn derive_status(
    last_entry: Option<&LiveLine>,
    seconds_since_modified: u64,
    has_running_process: bool,
) -> SessionStatus {
    // Implementation follows the table above
}
```

### 3.3 LiveSession Struct

```rust
/// A live (currently active or recently active) Claude Code session.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveSession {
    /// Session ID (JSONL filename without extension).
    pub id: String,
    /// Encoded project directory name.
    pub project: String,
    /// Human-readable project name.
    pub project_display_name: String,
    /// Full filesystem path to the project.
    pub project_path: String,
    /// Path to the JSONL file.
    pub file_path: String,
    /// Current session status.
    pub status: SessionStatus,
    /// Git branch (if detectable from project path).
    pub git_branch: Option<String>,
    /// PID of the running claude process (if detected).
    pub pid: Option<u32>,
    /// Truncated preview of the last user message.
    pub last_user_message: String,
    /// Current activity description (e.g., "Running Bash command", "Editing file.rs").
    pub current_activity: String,
    /// Total turns (user-assistant pairs) in this session.
    pub turn_count: u32,
    /// Session start time (first JSONL entry timestamp).
    pub started_at: Option<i64>,
    /// Last modification time of the JSONL file (Unix seconds).
    pub last_activity_at: i64,
    /// Primary model used in this session.
    pub model: Option<String>,
    /// Token usage breakdown.
    pub tokens: TokenUsage,
    /// Calculated cost breakdown.
    pub cost: CostBreakdown,
    /// Cache status.
    pub cache_status: CacheStatus,
}
```

---

## Step 4: Process Detection

**Crate:** `crates/server`
**Files to create:**
- `crates/server/src/live/process.rs`

### 4.1 Implementation

```rust
//! Process detection for correlating running `claude` processes with JSONL sessions.
//!
//! Uses the `sysinfo` crate to enumerate processes. Polls every 5 seconds
//! (process list doesn't change fast enough to justify more frequent checks).

use sysinfo::{System, ProcessesToUpdate, UpdateKind};
use std::collections::HashMap;
use std::path::PathBuf;

/// A detected Claude Code process.
#[derive(Debug, Clone)]
pub struct ClaudeProcess {
    pub pid: u32,
    pub cwd: PathBuf,
    pub start_time: u64,
}

/// Scan for running `claude` processes and return a map of CWD -> process info.
///
/// We match processes whose executable name contains "claude" (covers
/// `claude`, `claude-code`, etc.) and have a valid CWD.
pub fn detect_claude_processes() -> HashMap<PathBuf, ClaudeProcess> {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, /* refresh_kind */ true);

    let mut result = HashMap::new();
    for (pid, process) in sys.processes() {
        let name = process.name().to_string_lossy();
        if !name.contains("claude") {
            continue;
        }
        if let Some(cwd) = process.cwd() {
            result.insert(cwd.to_path_buf(), ClaudeProcess {
                pid: pid.as_u32(),
                cwd: cwd.to_path_buf(),
                start_time: process.start_time(),
            });
        }
    }
    result
}
```

**Implementation requirements:**
- Create a fresh `System` on each poll (simpler than maintaining a long-lived instance)
- Match process name containing "claude" (handles both `claude` and `claude-code` binaries)
- Extract CWD to correlate with project paths
- Return `HashMap<PathBuf, ClaudeProcess>` for O(1) lookup by project path
- The polling loop runs in a `tokio::spawn` task with `tokio::time::interval(Duration::from_secs(5))`
- Share results via `Arc<RwLock<HashMap<PathBuf, ClaudeProcess>>>`

### 4.2 Correlation Logic

To match a JSONL session to a process:
1. Resolve the session's project encoded name to a filesystem path (reuse `resolve_project_path` from `crates/core/src/discovery.rs`)
2. Look up that path in the process CWD map
3. If found, attach the PID to the `LiveSession`

---

## Step 5: Cost Calculator

**Crate:** `crates/core`
**Files to create:**
- `crates/core/src/cost.rs`

**Files to modify:**
- `crates/core/src/lib.rs` (add `pub mod cost;`)

### 5.1 Types

```rust
//! Cost calculation for live session monitoring.
//!
//! Reuses `ModelPricing` from `vibe-recall-db` for per-model rates.
//! This module provides types and functions for real-time cost tracking.

use serde::Serialize;

/// Accumulated token usage for a live session.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub total_tokens: u64,
}

/// Cost breakdown for a live session.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CostBreakdown {
    /// Total cost in USD.
    pub total_usd: f64,
    /// Cost of input tokens.
    pub input_cost_usd: f64,
    /// Cost of output tokens.
    pub output_cost_usd: f64,
    /// Cost of cache read tokens.
    pub cache_read_cost_usd: f64,
    /// Cost of cache creation tokens.
    pub cache_creation_cost_usd: f64,
    /// Estimated savings from prompt caching.
    /// = cache_read_tokens * (base_input_rate - cache_read_rate)
    pub cache_savings_usd: f64,
}

/// Cache warmth status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheStatus {
    /// Last API call was <5 minutes ago. Cache is likely warm.
    Warm,
    /// Last API call was >5 minutes ago. Cache has likely expired.
    Cold,
    /// No API call data available.
    Unknown,
}
```

### 5.2 Cost Calculation Function

```rust
/// Calculate cost breakdown for accumulated token usage.
///
/// Uses the pricing table from `vibe-recall-db::pricing`. If the model
/// is not found, falls back to `FALLBACK_COST_PER_TOKEN_USD`.
pub fn calculate_live_cost(
    tokens: &TokenUsage,
    model: Option<&str>,
    pricing: &HashMap<String, ModelPricing>,
) -> CostBreakdown {
    // 1. Look up model pricing (or fallback)
    // 2. Calculate each cost component
    // 3. Calculate savings: cache_read_tokens * (input_rate - cache_read_rate)
    // 4. Return CostBreakdown
}

/// Derive cache status from time since last API call.
pub fn derive_cache_status(seconds_since_last_api_call: Option<u64>) -> CacheStatus {
    match seconds_since_last_api_call {
        Some(s) if s < 300 => CacheStatus::Warm,   // 5 minutes
        Some(_) => CacheStatus::Cold,
        None => CacheStatus::Unknown,
    }
}
```

**Implementation requirements:**
- Reuse `vibe_recall_db::pricing::lookup_pricing` for model lookup
- Reuse `vibe_recall_db::pricing::FALLBACK_COST_PER_TOKEN_USD` for unknown models
- Cache savings formula: `cache_read_tokens * (input_cost_per_token - cache_read_cost_per_token)`
- All monetary values in USD as `f64`

---

## Step 6: LiveSession State Manager

**Crate:** `crates/server`
**Files to create:**
- `crates/server/src/live/manager.rs`

### 6.1 Manager Struct

```rust
//! Central manager for all live session state.
//!
//! Owns the file watcher, process detector, and session map.
//! Spawns background tasks and provides a broadcast channel for SSE.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

/// Shared live session state, accessible from route handlers.
pub type LiveSessionMap = Arc<RwLock<HashMap<String, LiveSession>>>;

/// Events broadcast to SSE clients.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEvent {
    /// A new session was discovered (JSONL file appeared or became active).
    SessionDiscovered { session: LiveSession },
    /// An existing session was updated (new lines, status change, cost change).
    SessionUpdated { session: LiveSession },
    /// A session is now complete (no process, no recent activity).
    SessionCompleted { session_id: String },
    /// Summary of all sessions (sent on initial connect and periodically).
    Summary {
        active_count: usize,
        waiting_count: usize,
        idle_count: usize,
        total_cost_today_usd: f64,
        total_tokens_today: u64,
    },
}

pub struct LiveSessionManager {
    /// All tracked live sessions.
    sessions: LiveSessionMap,
    /// Broadcast channel for SSE clients.
    tx: broadcast::Sender<SessionEvent>,
    /// Pre-computed SIMD finders for JSONL parsing.
    finders: Arc<TailFinders>,
    /// Per-session file read offsets.
    tail_states: Arc<RwLock<HashMap<String, TailState>>>,
    /// Detected claude processes (updated every 5s).
    processes: Arc<RwLock<HashMap<PathBuf, ClaudeProcess>>>,
    /// Model pricing table.
    pricing: Arc<HashMap<String, ModelPricing>>,
}
```

### 6.2 Lifecycle

```rust
impl LiveSessionManager {
    /// Create a new manager and spawn background tasks.
    ///
    /// Per CLAUDE.md: "Startup: never block on work the server doesn't need"
    /// The manager spawns all background work via tokio::spawn.
    pub fn start(pricing: HashMap<String, ModelPricing>) -> (Self, LiveSessionMap) {
        let (tx, _) = broadcast::channel(256);
        let sessions: LiveSessionMap = Arc::new(RwLock::new(HashMap::new()));
        let finders = Arc::new(TailFinders::new());
        let tail_states = Arc::new(RwLock::new(HashMap::new()));
        let processes = Arc::new(RwLock::new(HashMap::new()));

        let manager = Self {
            sessions: sessions.clone(),
            tx,
            finders,
            tail_states,
            processes,
            pricing: Arc::new(pricing),
        };

        // Spawn file watcher task
        manager.spawn_file_watcher();
        // Spawn process detector task
        manager.spawn_process_detector();
        // Spawn session cleanup task (mark stale sessions as Complete)
        manager.spawn_cleanup_task();

        (manager, sessions)
    }

    /// Subscribe to session events (for SSE clients).
    pub fn subscribe(&self) -> broadcast::Receiver<SessionEvent> {
        self.tx.subscribe()
    }
}
```

### 6.3 Background Task Details

**File Watcher Task:**
1. Receive `FileEvent::Modified(path)` from watcher
2. Extract session ID and project from path
3. Call `parse_tail()` with stored offset
4. Accumulate tokens into session's `TokenUsage`
5. Derive session status via `derive_status()`
6. Calculate cost via `calculate_live_cost()`
7. Update session in `LiveSessionMap`
8. Broadcast `SessionUpdated` or `SessionDiscovered` event

**Process Detector Task:**
1. Every 5 seconds, call `detect_claude_processes()`
2. Store result in `Arc<RwLock<HashMap<PathBuf, ClaudeProcess>>>`
3. For each session, re-derive status (process presence affects Idle vs Complete)
4. If status changed, broadcast `SessionUpdated`

**Cleanup Task:**
1. Every 30 seconds, scan all sessions
2. Sessions with `status == Complete` and `last_activity_at > 10 min ago`: remove from map
3. Broadcast `SessionCompleted` for removed sessions

### 6.4 Integration with AppState

**File to modify:** `crates/server/src/state.rs`

Add to `AppState`:
```rust
pub struct AppState {
    // ... existing fields ...
    /// Live session state for Mission Control.
    pub live_sessions: LiveSessionMap,
    /// Broadcast sender for live session SSE events.
    pub live_tx: broadcast::Sender<SessionEvent>,
}
```

Add a new constructor or modify existing ones to accept the live session manager's shared state.

---

## Step 7: SSE Endpoint

**Crate:** `crates/server`
**Files to create:**
- `crates/server/src/routes/live.rs`

**Files to modify:**
- `crates/server/src/routes/mod.rs` (add `pub mod live;`, register routes)

### 7.1 SSE Stream

```rust
//! SSE endpoint for real-time live session updates.
//!
//! GET /api/live/stream
//!
//! Follows the same pattern as the existing indexing SSE endpoint
//! in `crates/server/src/routes/indexing.rs`.

/// SSE handler for live session updates.
///
/// # Events
///
/// | Event name           | When emitted                                    |
/// |----------------------|-------------------------------------------------|
/// | `session_discovered` | New session detected (JSONL file appeared)       |
/// | `session_updated`    | Session state changed (new lines, status, cost)  |
/// | `session_completed`  | Session is done (removed from active tracking)   |
/// | `summary`            | Aggregate stats (on connect + every 15s)         |
/// | `heartbeat`          | Keep-alive ping (every 15s between summaries)    |
///
/// The stream does NOT terminate (unlike the indexing SSE which ends on "done").
/// Clients should handle reconnection with exponential backoff.
pub async fn live_stream(
    State(state): State<Arc<AppState>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let mut rx = state.live_tx.subscribe();
    let sessions = state.live_sessions.clone();

    let stream = async_stream::stream! {
        // 1. On connect: send current summary + all active sessions
        {
            let map = sessions.read().await;
            let summary = build_summary(&map);
            yield Ok(Event::default().event("summary").data(
                serde_json::to_string(&summary).unwrap()
            ));
            for session in map.values() {
                yield Ok(Event::default().event("session_discovered").data(
                    serde_json::to_string(session).unwrap()
                ));
            }
        }

        // 2. Stream events from broadcast channel
        let mut heartbeat_interval = tokio::time::interval(Duration::from_secs(15));
        loop {
            tokio::select! {
                event = rx.recv() => {
                    match event {
                        Ok(session_event) => {
                            let event_name = match &session_event {
                                SessionEvent::SessionDiscovered { .. } => "session_discovered",
                                SessionEvent::SessionUpdated { .. } => "session_updated",
                                SessionEvent::SessionCompleted { .. } => "session_completed",
                                SessionEvent::Summary { .. } => "summary",
                            };
                            yield Ok(Event::default()
                                .event(event_name)
                                .data(serde_json::to_string(&session_event).unwrap()));
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!("SSE client lagged by {} events", n);
                            // Send fresh summary to catch up
                            let map = sessions.read().await;
                            let summary = build_summary(&map);
                            yield Ok(Event::default().event("summary").data(
                                serde_json::to_string(&summary).unwrap()
                            ));
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
                _ = heartbeat_interval.tick() => {
                    yield Ok(Event::default().event("heartbeat").data("{}"));
                }
            }
        }
    };

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("heartbeat")
    )
}
```

---

## Step 8: REST Endpoints

**Crate:** `crates/server`
**File:** `crates/server/src/routes/live.rs` (same file as SSE, add REST handlers)

### 8.1 Route Definitions

```rust
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/live/stream", get(live_stream))
        .route("/live/sessions", get(list_live_sessions))
        .route("/live/sessions/{id}", get(get_live_session))
        .route("/live/sessions/{id}/messages", get(get_live_session_messages))
        .route("/live/summary", get(get_live_summary))
        .route("/live/pricing", get(get_pricing))
}
```

### 8.2 Handler Specifications

**GET /api/live/sessions**
- Returns all tracked live sessions as a JSON array
- Response: `LiveSession[]`
- Source: reads from `LiveSessionMap`

**GET /api/live/sessions/:id**
- Returns a single session by ID
- Response: `LiveSession`
- 404 if not found

**GET /api/live/sessions/:id/messages?limit=20**
- Returns the last N parsed messages from the session's JSONL file
- Uses the existing `parse_session_paginated` from `crates/core/src/parser.rs`
- Query params: `limit` (default 20), `offset` (default: total - limit, i.e., last N)
- Response: `PaginatedMessages`

**GET /api/live/summary**
- Returns aggregate statistics for the summary bar
- Response:
```json
{
  "activeCount": 3,
  "waitingCount": 1,
  "idleCount": 2,
  "completeCount": 0,
  "totalCostTodayUsd": 1.47,
  "totalTokensToday": 245000,
  "totalCacheSavingsUsd": 0.89
}
```

**GET /api/live/pricing**
- Returns the full pricing table so the frontend can display rates
- Response: `HashMap<String, { inputPerMillion, outputPerMillion, cacheReadPerMillion, cacheWritePerMillion }>`
- Transforms `ModelPricing` per-token rates to per-million for readability

### 8.3 Register Routes

In `crates/server/src/routes/mod.rs`:

```rust
pub mod live;

pub fn api_routes(state: Arc<AppState>) -> Router {
    Router::new()
        // ... existing routes ...
        .nest("/api", live::router())
        .with_state(state)
}
```

---

## Step 9: Mission Control Page

**Frontend**
**Files to create:**
- `src/pages/MissionControlPage.tsx`

**Files to modify:**
- `src/router.tsx` (add route)
- `src/components/Sidebar.tsx` (add nav link)

### 9.1 Route Registration

In `src/router.tsx`, add inside the `children` array:

```tsx
{ path: 'mission-control', element: <MissionControlPage /> },
```

### 9.2 Page Component

`src/pages/MissionControlPage.tsx`:

```tsx
import { useLiveSessions } from '../hooks/use-live-sessions'
import { SessionCard } from '../components/live/SessionCard'

export function MissionControlPage() {
  const { sessions, summary, isConnected, lastUpdate } = useLiveSessions()

  return (
    <div className="flex flex-col gap-4 p-4">
      {/* Connection status indicator */}
      <div className="flex items-center gap-2 text-sm text-muted-foreground">
        <span className={`h-2 w-2 rounded-full ${isConnected ? 'bg-green-500' : 'bg-red-500'}`} />
        {isConnected ? 'Connected' : 'Reconnecting...'}
      </div>

      {/* Summary bar */}
      <SummaryBar summary={summary} />

      {/* Session grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
        {sessions.map(session => (
          <SessionCard key={session.id} session={session} />
        ))}
      </div>

      {/* Empty state */}
      {sessions.length === 0 && isConnected && (
        <div className="text-center text-muted-foreground py-16">
          No active Claude Code sessions detected.
          <br />
          Start a session in your terminal and it will appear here.
        </div>
      )}
    </div>
  )
}

function SummaryBar({ summary }: { summary: LiveSummary | null }) {
  if (!summary) return null
  return (
    <div className="flex gap-4 p-3 rounded-lg bg-muted/50 text-sm">
      <div>
        <span className="text-green-500 font-medium">{summary.activeCount}</span> active
      </div>
      <div>
        <span className="text-amber-500 font-medium">{summary.waitingCount}</span> waiting
      </div>
      <div>
        <span className="text-muted-foreground font-medium">{summary.idleCount}</span> idle
      </div>
      <div className="ml-auto">
        ${summary.totalCostTodayUsd.toFixed(2)} today
      </div>
      <div>
        {(summary.totalTokensToday / 1000).toFixed(0)}k tokens
      </div>
    </div>
  )
}
```

### 9.3 Sidebar Navigation

Add a "Mission Control" link to the sidebar navigation. Use an icon like `Monitor` or `Radar` from lucide-react. Place it above or below the existing navigation items, depending on prominence desired.

---

## Step 10: Session Card Component

**Files to create:**
- `src/components/live/SessionCard.tsx`

### 10.1 Card Layout

```
+-------------------------------------------+
| [STATUS DOT] Project Name     [COST]      |
| branch: main                              |
|-------------------------------------------|
| Last: "Fix the auth bug in..."            |
| Activity: Running Bash command            |
|-------------------------------------------|
| [====CACHED====][==NEW==][...AVAILABLE...]|
|  Context: 45k/200k tokens                |
|-------------------------------------------|
| Turns: 12  |  Duration: 14m  |  $0.47    |
+-------------------------------------------+
```

### 10.2 Status Dot Colors

| Status | Color | Animation |
|--------|-------|-----------|
| `Streaming` | Green (`bg-green-500`) | Pulse animation (`animate-pulse`) |
| `ToolUse` | Green (`bg-green-500`) | Solid (no pulse) |
| `WaitingForUser` | Amber (`bg-amber-500`) | Solid |
| `Idle` | Gray (`bg-zinc-500`) | Solid |
| `Complete` | Dim gray (`bg-zinc-700`) | Solid |

### 10.3 Implementation Requirements

- Card uses standard Tailwind classes (no new design system components needed)
- Dark mode compatible (use `text-foreground`, `bg-card`, etc.)
- Click on card navigates to `/sessions/:sessionId` (links to existing conversation view)
- Cost display shows `$X.XX` format
- Duration shows human-readable format (`14m`, `1h 23m`, `2d 3h`)
- "Last" shows the truncated last user message
- "Activity" derives from current status + last tool name

---

## Step 11: Context Gauge Component

**Files to create:**
- `src/components/live/ContextGauge.tsx`

### 11.1 Segmented Bar

A horizontal bar divided into three segments:

```
[====GREEN====][==WHITE==][..........DARK..........]
  ^cached        ^new         ^available
```

| Segment | Color | Represents |
|---------|-------|------------|
| Cached | `bg-emerald-500` | `cache_read_tokens` (cheap, 10% of base rate) |
| New | `bg-white` (dark: `bg-zinc-200`) | `input_tokens - cache_read_tokens` (full price) |
| Available | `bg-zinc-800` (dark: `bg-zinc-900`) | `context_window - total_input_tokens` |

### 11.2 Context Window Sizes

Map model to context window for calculating the "available" segment:

| Model prefix | Context window |
|-------------|---------------|
| `claude-opus-4` | 200,000 |
| `claude-sonnet-4` | 200,000 |
| `claude-haiku-4` | 200,000 |
| `claude-3-*` | 200,000 |
| Unknown | 200,000 (safe default) |

### 11.3 Tooltip

On hover, show exact numbers:

```
Cached: 42,150 tokens (21%)
New: 8,300 tokens (4%)
Available: 149,550 tokens (75%)

Prompt caching saves ~90% on repeated context.
Cached tokens cost $0.30/M vs $3.00/M (Sonnet).
```

---

## Step 12: Cost Tooltip Component

**Files to create:**
- `src/components/live/CostTooltip.tsx`

### 12.1 Hover Popover Content

```
Cost Breakdown
--------------
Input:        $0.12  (40k tokens)
Output:       $0.30  (20k tokens)
Cache read:   $0.01  (42k tokens)
Cache write:  $0.04  (3k tokens)
--------------
Total:        $0.47

Prompt caching saved you ~$0.13
Cache status: Warm (2m 30s remaining)
```

### 12.2 Implementation Requirements

- Use the existing popover/tooltip pattern from the codebase
- Per CLAUDE.md popover rules: draft state only resets on open transition (`prevIsOpenRef` pattern)
- Cache status line:
  - **Warm**: "Warm (Xm Ys remaining)" where remaining = 300s - seconds_since_last_call
  - **Cold**: "Cold (expired Xm ago)"
  - **Unknown**: "Unknown (no API call data)"
- "Saved you" line only appears when `cache_savings_usd > 0`

---

## Step 13: SSE Hook

**Files to create:**
- `src/hooks/use-live-sessions.ts`

### 13.1 Hook Interface

```tsx
interface LiveSummary {
  activeCount: number
  waitingCount: number
  idleCount: number
  totalCostTodayUsd: number
  totalTokensToday: number
  totalCacheSavingsUsd: number
}

interface UseLiveSessionsResult {
  sessions: LiveSession[]
  summary: LiveSummary | null
  isConnected: boolean
  lastUpdate: Date | null
}

export function useLiveSessions(): UseLiveSessionsResult
```

### 13.2 Implementation

```tsx
export function useLiveSessions(): UseLiveSessionsResult {
  const [sessions, setSessions] = useState<Map<string, LiveSession>>(new Map())
  const [summary, setSummary] = useState<LiveSummary | null>(null)
  const [isConnected, setIsConnected] = useState(false)
  const [lastUpdate, setLastUpdate] = useState<Date | null>(null)

  useEffect(() => {
    let es: EventSource | null = null
    let retryDelay = 1000 // Start at 1s, exponential backoff

    function connect() {
      // Per CLAUDE.md: bypass Vite proxy for SSE
      const url = sseUrl('/api/live/stream')
      es = new EventSource(url)

      es.onopen = () => {
        setIsConnected(true)
        retryDelay = 1000 // Reset on successful connect
      }

      es.addEventListener('session_discovered', (e) => {
        const session = JSON.parse(e.data).session
        setSessions(prev => new Map(prev).set(session.id, session))
        setLastUpdate(new Date())
      })

      es.addEventListener('session_updated', (e) => {
        const session = JSON.parse(e.data).session
        setSessions(prev => new Map(prev).set(session.id, session))
        setLastUpdate(new Date())
      })

      es.addEventListener('session_completed', (e) => {
        const { session_id } = JSON.parse(e.data)
        setSessions(prev => {
          const next = new Map(prev)
          next.delete(session_id)
          return next
        })
        setLastUpdate(new Date())
      })

      es.addEventListener('summary', (e) => {
        setSummary(JSON.parse(e.data))
        setLastUpdate(new Date())
      })

      es.onerror = () => {
        setIsConnected(false)
        es?.close()
        // Exponential backoff: 1s, 2s, 4s, 8s, max 30s
        setTimeout(connect, retryDelay)
        retryDelay = Math.min(retryDelay * 2, 30000)
      }
    }

    connect()

    return () => {
      es?.close()
    }
  }, [])

  const sessionList = useMemo(
    () => Array.from(sessions.values()).sort((a, b) => b.lastActivityAt - a.lastActivityAt),
    [sessions]
  )

  return { sessions: sessionList, summary, isConnected, lastUpdate }
}
```

### 13.3 SSE URL Helper

```tsx
// src/lib/sse-url.ts
// Per CLAUDE.md: Vite's http-proxy buffers SSE responses.
// Bypass the proxy in dev mode by connecting directly to Rust server.

export function sseUrl(path: string): string {
  if (window.location.port === '5173') {
    return `http://localhost:47892${path}`
  }
  return path
}
```

---

## Acceptance Criteria

### Functional

- [ ] File watcher detects new/modified JSONL files within 3 seconds
- [ ] Incremental tail parser reads only new bytes (verified by offset tracking)
- [ ] Session status correctly derived: Streaming vs ToolUse vs WaitingForUser vs Idle vs Complete
- [ ] Process detection correlates running `claude` PIDs to sessions by CWD
- [ ] Cost calculation matches manual computation: `tokens * pricing_rate` for each token type
- [ ] Cache savings calculated correctly: `cache_read_tokens * (base_rate - cache_read_rate)`
- [ ] SSE delivers updates to browser within 2 seconds of JSONL file change
- [ ] SSE reconnects automatically with exponential backoff on disconnect
- [ ] Grid view shows all active sessions with correct status colors and pulse animation
- [ ] Context gauge shows cached (green) vs new (white) vs available (dark) segments
- [ ] Cost tooltip shows breakdown with input/output/cache-read/cache-write lines
- [ ] Cost tooltip shows "Saved you $X" when cache savings > 0
- [ ] Summary bar shows correct aggregate totals (active, waiting, cost, tokens)
- [ ] Clicking a session card navigates to the existing conversation view
- [ ] Mobile responsive: cards stack vertically on small screens (grid cols collapse)

### Performance

- [ ] File watching uses <1% CPU when sessions are idle
- [ ] Tail parser processes a 10MB JSONL append in <10ms
- [ ] UI handles 50+ session cards without visible lag (<16ms frame time)
- [ ] SSE heartbeat keeps connection alive (no timeout disconnects)
- [ ] Process detection poll (every 5s) completes in <50ms

### Edge Cases

- [ ] Handles JSONL files with partial writes (mid-line) without crashing
- [ ] Handles sessions with no usage data (tokens all zero, cost $0.00)
- [ ] Handles unknown model IDs (falls back to blended rate)
- [ ] Handles sessions with `last_activity_at = 0` (guard per CLAUDE.md epoch-zero rule)
- [ ] Handles `~/.claude/projects/` directory not existing (graceful empty state)
- [ ] Handles file watcher errors (permission denied, etc.) without crashing server

---

## Testing Plan

### Unit Tests

**`cargo test -p vibe-recall-core -- live_parser`**

| Test | Description |
|------|-------------|
| `test_parse_tail_empty_file` | Returns empty vec, offset 0 |
| `test_parse_tail_single_line` | Parses one complete user line |
| `test_parse_tail_multiple_lines` | Parses 3 lines, returns correct offset |
| `test_parse_tail_partial_line` | Excludes incomplete last line, offset before it |
| `test_parse_tail_incremental` | Two calls: first reads lines 1-3, second reads lines 4-5 |
| `test_parse_tail_extracts_tokens` | Verifies input/output/cache token extraction |
| `test_parse_tail_extracts_model` | Verifies model field extraction |
| `test_parse_tail_extracts_tools` | Verifies tool_use block name extraction |
| `test_parse_tail_simd_filter` | Only lines matching SIMD filter are JSON-parsed |

**`cargo test -p vibe-recall-core -- cost`**

| Test | Description |
|------|-------------|
| `test_cost_zero_tokens` | All zero tokens = $0.00 |
| `test_cost_input_only` | 100k input tokens at Sonnet rate = $0.30 |
| `test_cost_cache_savings` | Verifies savings = cache_read * (base - cache_read_rate) |
| `test_cost_unknown_model` | Falls back to blended rate |
| `test_cache_status_warm` | <300s = Warm |
| `test_cache_status_cold` | >300s = Cold |
| `test_cache_status_unknown` | None = Unknown |

**`cargo test -p vibe-recall-server -- live::state`**

| Test | Description |
|------|-------------|
| `test_derive_status_streaming` | Last assistant entry + recent mtime = Streaming |
| `test_derive_status_tool_use` | Last assistant with tool_use block = ToolUse |
| `test_derive_status_waiting` | Last user entry + recent mtime = WaitingForUser |
| `test_derive_status_idle` | Any entry + stale mtime (>60s) = Idle |
| `test_derive_status_complete` | Stale mtime (>5min) + no process = Complete |
| `test_derive_status_idle_with_process` | Stale mtime (>5min) + has process = Idle (not Complete) |

### Integration Tests

**`cargo test -p vibe-recall-server -- live::integration`**

| Test | Description |
|------|-------------|
| `test_live_sessions_endpoint_empty` | GET /api/live/sessions returns empty array when no sessions |
| `test_live_summary_endpoint` | GET /api/live/summary returns correct shape |
| `test_live_pricing_endpoint` | GET /api/live/pricing returns pricing table |
| `test_live_sse_returns_event_stream` | GET /api/live/stream has content-type text/event-stream |
| `test_live_session_not_found` | GET /api/live/sessions/nonexistent returns 404 |

### Frontend Tests

| Test | Description |
|------|-------------|
| `SessionCard.test.tsx` | Renders status dot with correct color for each status |
| `SessionCard.test.tsx` | Displays truncated last message |
| `SessionCard.test.tsx` | Shows cost formatted as $X.XX |
| `ContextGauge.test.tsx` | Renders three segments with correct widths |
| `ContextGauge.test.tsx` | Handles zero tokens (empty bar) |
| `CostTooltip.test.tsx` | Shows breakdown lines |
| `CostTooltip.test.tsx` | Shows "Saved you" line when savings > 0 |
| `CostTooltip.test.tsx` | Hides "Saved you" when savings = 0 |

---

## File Summary

### Files to Create

| File | Purpose |
|------|---------|
| `crates/server/src/live/mod.rs` | Module root, re-exports |
| `crates/server/src/live/watcher.rs` | `notify` file watcher + debounce |
| `crates/server/src/live/state.rs` | `SessionStatus` enum, `LiveSession` struct, `derive_status()` |
| `crates/server/src/live/process.rs` | `sysinfo`-based process detection |
| `crates/server/src/live/manager.rs` | `LiveSessionManager`, background tasks, broadcast channel |
| `crates/server/src/routes/live.rs` | SSE + REST endpoints |
| `crates/core/src/live_parser.rs` | Incremental tail parser with SIMD pre-filter |
| `crates/core/src/cost.rs` | `TokenUsage`, `CostBreakdown`, `CacheStatus`, cost calculation |
| `src/pages/MissionControlPage.tsx` | Page component with summary bar + grid |
| `src/components/live/SessionCard.tsx` | Session card with status dot, activity, cost |
| `src/components/live/ContextGauge.tsx` | Segmented context usage bar |
| `src/components/live/CostTooltip.tsx` | Cost breakdown hover popover |
| `src/hooks/use-live-sessions.ts` | SSE hook with reconnection |
| `src/lib/sse-url.ts` | SSE URL helper (bypasses Vite proxy in dev) |

### Files to Modify

| File | Change |
|------|--------|
| `Cargo.toml` | Add `sysinfo` to workspace dependencies |
| `crates/server/Cargo.toml` | Add `notify`, `sysinfo`, `memchr` deps |
| `crates/core/Cargo.toml` | Add `memchr` dep |
| `crates/server/src/lib.rs` | Add `pub mod live;` |
| `crates/core/src/lib.rs` | Add `pub mod live_parser;` and `pub mod cost;` |
| `crates/server/src/state.rs` | Add `live_sessions` and `live_tx` fields to `AppState` |
| `crates/server/src/routes/mod.rs` | Add `pub mod live;`, register in `api_routes()` |
| `src/router.tsx` | Add `/mission-control` route |
| `src/components/Sidebar.tsx` | Add Mission Control nav link |

---

## Implementation Order

Execute steps in this order to maintain a compilable codebase at each stage:

1. **Step 5** (Cost Calculator) - Pure types and functions in `crates/core`, no dependencies on other new code
2. **Step 2** (Tail Parser) - Pure parsing in `crates/core`, testable in isolation
3. **Step 3** (State Machine) - Types and `derive_status()` in `crates/server`, depends on Step 2 types
4. **Step 4** (Process Detection) - Independent module, testable with mocks
5. **Step 1** (File Watcher) - Infrastructure that connects to Steps 2-4
6. **Step 6** (Manager) - Orchestrates Steps 1-5, modifies `AppState`
7. **Step 8** (REST Endpoints) - REST handlers, testable with `axum-test`
8. **Step 7** (SSE Endpoint) - SSE handler, depends on Step 6 broadcast channel
9. **Step 13** (SSE Hook) - Frontend hook, needs backend running
10. **Step 9** (Mission Control Page) - Page shell, depends on Step 13
11. **Step 10** (Session Card) - UI component, depends on types from Step 13
12. **Step 11** (Context Gauge) - UI component, standalone
13. **Step 12** (Cost Tooltip) - UI component, standalone

Each step should have passing tests before moving to the next.
