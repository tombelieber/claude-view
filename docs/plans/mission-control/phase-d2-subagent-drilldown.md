---
status: done
date: 2026-02-17
phase: D.2
depends_on: D
---

# Phase D.2: Sub-Agent Deep Dive — Real-Time Progress & Drill-Down

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Let users see what each sub-agent is doing in real-time (current tool activity) and drill down into any sub-agent's full conversation — transforming sub-agents from opaque status pills into transparent, inspectable workers.

**Architecture:** Extend the parent JSONL parser to extract `type: "progress"` events carrying `agent_progress` data (early `agentId` + current tool activity). Add a new WebSocket endpoint that streams a sub-agent's own JSONL file using the existing `format_line_for_mode` infrastructure. Frontend renders sub-agent conversations via the existing `RichPane` component, triggered by clicking a swim lane row or sub-agent pill.

**Tech Stack:** Rust (live_parser.rs, terminal.rs), React (RichPane, SwimLanes, useTerminalSocket), WebSocket, notify file watcher

**Depends on:** Phase D (all 8 tasks complete)

---

## Background

### What Phase D Gives Us

Phase D (complete) provides:
- Sub-agent spawn/completion detection from parent JSONL (`tool_use` + `toolUseResult`)
- `SubAgentInfo` type with status, cost, duration, tool_use_count
- SwimLanes (expanded view), SubAgentPills (compact), TimelineView (historical)
- All sub-agent data flows through existing SSE `session_updated` events

### What Phase D.2 Adds

Phase D stopped at spawn → completion. Between those events, the sub-agent is a black box showing only an indeterminate progress bar. Phase D.2 opens the box:

| Capability | Phase D (current) | Phase D.2 (this plan) |
|-----------|-------------------|----------------------|
| Sub-agent status | Running / Complete / Error | Same + **current activity** |
| While running | Indeterminate progress bar | **"Reading crates/server/src/routes/..."** |
| `agentId` available | Only after completion | **From first progress event** (seconds after spawn) |
| Click sub-agent | Nothing | **Drill-down → full conversation in RichPane** |
| Sub-agent JSONL | Ignored | **Streamed via WebSocket** |
| File watching | Parent JSONL only | Parent + **sub-agent JSONL** (per-connection) |

### Key Data Sources

**Progress events in parent JSONL** (already present, currently skipped by `live_parser.rs`):

```json
{
  "type": "progress",
  "parentToolUseID": "toolu_011ewdGwdYzHEU9Twb59itFZ",
  "toolUseID": "agent_msg_016e1sRpgpdjjzhBjZa3qj1v",
  "data": {
    "type": "agent_progress",
    "agentId": "a951849",
    "prompt": "...",
    "message": { "role": "assistant", "content": [
      { "type": "tool_use", "name": "Read", "input": { "file_path": "/path/to/file.rs" } }
    ]}
  }
}
```

- `parentToolUseID` → links to the `tool_use_id` on the spawn line (our existing lookup key)
- `data.agentId` → 7-char short hash, available seconds after spawn (currently only on completion)
- `data.message.content` → contains `tool_use` blocks showing what the sub-agent is doing RIGHT NOW

**Sub-agent JSONL files** at `~/.claude/projects/{project}/{sessionId}/subagents/agent-{agentId}.jsonl`:

```json
{
  "agentId": "a33bda6",
  "sessionId": "c916410d-da69-4a71-800f-35cca5301d8a",
  "isSidechain": true,
  "slug": "snappy-tumbling-scott",
  "type": "user|assistant",
  "timestamp": "2026-02-16T08:34:13.134Z",
  "message": { "role": "user|assistant", "content": "..." }
}
```

- Same JSONL format as parent sessions (same `format_line_for_mode` works)
- `isSidechain: true` on every line
- `slug` is a human-readable name (e.g., "snappy-tumbling-scott")
- File grows as the sub-agent works (can be watched with `notify`)

### Confirmed Constraints

- **Flat hierarchy:** Sub-agents never spawn sub-agents (verified across 20 JSONL files). No recursive drill-down needed.
- **File availability:** Sub-agent JSONL files are created at spawn time and grow during execution. They're available for streaming immediately.
- **Progress event frequency:** ~1 per tool call. A sub-agent making 42 tool calls generates ~42 progress events in the parent JSONL.

### Supersession Note

**Phase D.2 supersedes Phase D3 tasks 1-3** (backend progress tracking). Phase D3 (`phase-d3-realtime-subagent-progress.md`, status: draft) independently designed `current_activity`/`current_tool` fields and progress event parsing. D2 implements these features with a different architecture. After D2 is complete, D3's remaining tasks (if any) should reference D2's types/infrastructure rather than building their own. D3's status should be updated to `superseded` for tasks 1-3.

---

## Task 1: Progress Event SIMD Finder + LiveLine Extension

**Files:**
- Modify: `crates/core/src/live_parser.rs` (TailFinders struct line 87, LiveLine struct line 45, parse_single_line line 185)

**Step 1: Write the failing test**

Add to `crates/core/src/live_parser.rs` (in the `#[cfg(test)]` module):

```rust
#[test]
fn test_progress_event_agent_activity() {
    let finders = TailFinders::new();
    let raw = br#"{"type":"progress","parentToolUseID":"toolu_01ABC","toolUseID":"agent_msg_01","data":{"type":"agent_progress","agentId":"a951849","message":{"role":"assistant","content":[{"type":"tool_use","name":"Read","input":{"file_path":"/path/to/file.rs"}}]}},"timestamp":"2026-02-16T08:34:13.134Z"}"#;
    let line = parse_single_line(raw, &finders);

    assert!(line.sub_agent_progress.is_some());
    let progress = line.sub_agent_progress.unwrap();
    assert_eq!(progress.parent_tool_use_id, "toolu_01ABC");
    assert_eq!(progress.agent_id, "a951849");
    assert_eq!(progress.current_tool, Some("Read".to_string()));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p claude-view-core -- test_progress_event_agent_activity`
Expected: FAIL with "no field `sub_agent_progress` on type `LiveLine`"

**Step 3: Add SIMD finder to TailFinders**

In `TailFinders` struct (after `tool_use_result_key` at line 100):

```rust
pub agent_progress_key: memmem::Finder<'static>,
```

In `TailFinders::new()`:

```rust
agent_progress_key: memmem::Finder::new(b"\"agent_progress\""),
```

**Step 4: Add SubAgentProgress type and LiveLine field**

Above `LiveLine` struct definition:

```rust
/// Extracted from a `type: "progress"` line with `data.type: "agent_progress"`.
#[derive(Debug, Clone)]
pub struct SubAgentProgress {
    /// Links back to the Task spawn's `tool_use_id`.
    pub parent_tool_use_id: String,
    /// 7-char agent ID (available before completion!).
    pub agent_id: String,
    /// Current tool the sub-agent is using (e.g., "Read", "Grep", "Edit").
    /// Extracted from the latest `tool_use` block in `data.message.content`.
    pub current_tool: Option<String>,
}
```

Add to `LiveLine` struct:

```rust
/// If this is a progress line with agent_progress data.
pub sub_agent_progress: Option<SubAgentProgress>,
```

**CRITICAL: Update ALL LiveLine construction sites** — there are exactly 3 (verified via `grep -rn 'LiveLine {' crates/`):

1. `live_parser.rs` JSON parse error fallback (line 206, after `sub_agent_result: None,` at line 223): Add `sub_agent_progress: None,`
2. `live_parser.rs` normal construction (line 400, after `sub_agent_result,` at line 417): Add `sub_agent_progress,` (the variable from Step 5)
3. `crates/server/src/live/state.rs` test helper `make_live_line` body (line 265, after `sub_agent_result: None,` at line 282): Add `sub_agent_progress: None,`

**Step 5: Add progress extraction logic in parse_single_line**

Insert **after** the `sub_agent_result` extraction block (after line 398 `};`), **before** the final `LiveLine { ... }` construction at line 400:

```rust
// --- Sub-agent progress detection (progress lines with agent_progress) ---
let sub_agent_progress = if line_type == LineType::Progress
    && finders.agent_progress_key.find(raw).is_some()
{
    parsed.get("data").and_then(|data| {
        if data.get("type").and_then(|t| t.as_str()) != Some("agent_progress") {
            return None;
        }
        let parent_tool_use_id = parsed.get("parentToolUseID")
            .and_then(|v| v.as_str())
            .map(String::from)?;
        let agent_id = data.get("agentId")
            .and_then(|v| v.as_str())
            .map(String::from)?;
        // Extract current tool from the latest tool_use block in message.content
        let current_tool = data.get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_array())
            .and_then(|blocks| {
                blocks.iter().rev().find_map(|b| {
                    if b.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                        b.get("name").and_then(|n| n.as_str()).map(String::from)
                    } else {
                        None
                    }
                })
            });
        Some(SubAgentProgress {
            parent_tool_use_id,
            agent_id,
            current_tool,
        })
    })
} else {
    None
};
```

**Verified:** `LineType::Progress` exists (line 80 of `live_parser.rs`). The `parse_single_line` function classifies progress lines at line 194-195 via `finders.type_progress`. `parse_tail` does NOT filter out progress lines — all lines are passed through to `parse_single_line`. No extra work needed here.

**Step 6: Run test to verify it passes**

Run: `cargo test -p claude-view-core -- test_progress_event_agent_activity`
Expected: PASS

**Step 7: Add edge case tests**

```rust
#[test]
fn test_progress_event_no_tool_use() {
    let finders = TailFinders::new();
    // Progress event where the assistant is just thinking (no tool_use block)
    let raw = br#"{"type":"progress","parentToolUseID":"toolu_01ABC","data":{"type":"agent_progress","agentId":"a951849","message":{"role":"assistant","content":[{"type":"text","text":"Let me think..."}]}}}"#;
    let line = parse_single_line(raw, &finders);
    let progress = line.sub_agent_progress.unwrap();
    assert_eq!(progress.current_tool, None);
}

#[test]
fn test_progress_event_non_agent_type() {
    let finders = TailFinders::new();
    // Progress event that isn't agent_progress (should be ignored)
    let raw = br#"{"type":"progress","data":{"type":"tool_progress","tool":"Bash"}}"#;
    let line = parse_single_line(raw, &finders);
    assert!(line.sub_agent_progress.is_none());
}

#[test]
fn test_progress_event_missing_agent_id() {
    let finders = TailFinders::new();
    let raw = br#"{"type":"progress","parentToolUseID":"toolu_01ABC","data":{"type":"agent_progress","message":{"role":"assistant","content":[]}}}"#;
    let line = parse_single_line(raw, &finders);
    assert!(line.sub_agent_progress.is_none()); // agentId required
}
```

**Step 8: Run all parser tests**

Run: `cargo test -p claude-view-core -- live_parser`
Expected: ALL PASS

**Step 9: Commit**

```bash
git add crates/core/src/live_parser.rs crates/server/src/live/state.rs
git commit -m "feat(live): parse agent_progress events for sub-agent activity tracking"
```

---

## Task 2: SubAgentInfo Type Extension

**Files:**
- Modify: `crates/core/src/subagent.rs`
- Modify: `src/types/generated/SubAgentInfo.ts` (auto-generated by ts-rs)

**Step 1: Write the failing test**

```rust
// In crates/core/src/subagent.rs, add #[cfg(test)] module:
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subagent_info_serialization_with_activity() {
        let info = SubAgentInfo {
            tool_use_id: "toolu_01ABC".to_string(),
            agent_id: Some("a951849".to_string()),
            agent_type: "Explore".to_string(),
            description: "Search codebase".to_string(),
            status: SubAgentStatus::Running,
            started_at: 1739700000,
            completed_at: None,
            duration_ms: None,
            tool_use_count: None,
            cost_usd: None,
            current_activity: Some("Read".to_string()),
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"currentActivity\":\"Read\""));
    }

    #[test]
    fn test_subagent_info_skips_none_activity() {
        let info = SubAgentInfo {
            tool_use_id: "toolu_01ABC".to_string(),
            agent_id: None,
            agent_type: "Explore".to_string(),
            description: "Search".to_string(),
            status: SubAgentStatus::Running,
            started_at: 1739700000,
            completed_at: None,
            duration_ms: None,
            tool_use_count: None,
            cost_usd: None,
            current_activity: None,
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(!json.contains("currentActivity"));
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p claude-view-core -- subagent`
Expected: FAIL with "no field named `current_activity`"

**Step 3: Add `current_activity` field to SubAgentInfo**

In `crates/core/src/subagent.rs`, add after `cost_usd`:

```rust
    /// Current tool the sub-agent is using (e.g., "Read", "Grep", "Edit").
    /// Populated from progress events while status is Running.
    /// Cleared to None when status transitions to Complete/Error.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_activity: Option<String>,
```

**CRITICAL: Update ALL SubAgentInfo construction sites** (verified via `grep -rn 'SubAgentInfo {' crates/` — exactly 1 site):

1. `crates/server/src/live/manager.rs` spawn tracking (line 660, after `cost_usd: None,` at line 670): Add `current_activity: None,`

**Step 4: Run test to verify it passes**

Run: `cargo test -p claude-view-core -- subagent`
Expected: PASS

**Step 5: Regenerate TypeScript types**

Run: `cargo test -p claude-view-core` (triggers ts-rs generation)
Verify: `src/types/generated/SubAgentInfo.ts` now contains `currentActivity?: string | null`

**Step 6: Compile check**

Run: `cargo check -p claude-view-server`
Expected: Success (no missing field errors)

**Step 7: Commit**

```bash
git add crates/core/src/subagent.rs crates/server/src/live/manager.rs src/types/generated/SubAgentInfo.ts
git commit -m "feat(core): add current_activity field to SubAgentInfo for real-time tool tracking"
```

---

## Task 3: Manager Progress Event Processing

**Files:**
- Modify: `crates/server/src/live/manager.rs` (first `for line in &new_lines` loop, lines 640-706)

**Step 1: Write the failing test**

This is an integration-level test. If the manager has existing tests, add to that suite. Otherwise, verify manually:

```rust
// Expected behavior: after processing a progress event, the matching
// sub-agent's agent_id and current_activity should be updated.
//
// Verify by:
// 1. Process a spawn line (creates SubAgentInfo with agent_id: None, current_activity: None)
// 2. Process a progress line (should populate agent_id and current_activity)
// 3. Check that the LiveSession snapshot has the updated sub-agent
```

**Step 2: Add progress event processing to manager**

In the first `for line in &new_lines` loop (lines 640-706), insert the following block **INSIDE** the loop body, after the sub-agent completion tracking `if let` block (after line 705 `}`), and **BEFORE** the loop's closing `}` at line 706:

```rust
// --- Sub-agent progress tracking (early agentId + current activity) ---
if let Some(ref progress) = line.sub_agent_progress {
    if let Some(agent) = acc.sub_agents.iter_mut()
        .find(|a| a.tool_use_id == progress.parent_tool_use_id)
    {
        // Populate agent_id from progress event (available before completion!)
        if agent.agent_id.is_none() {
            agent.agent_id = Some(progress.agent_id.clone());
        }
        // Update current activity (only while still running)
        if agent.status == SubAgentStatus::Running {
            agent.current_activity = progress.current_tool.clone();
        }
    }
}
```

**Step 3: Clear current_activity on completion**

In the sub-agent completion tracking block (lines 674-705), after `agent.status` is set (line 679-682) and before `agent.agent_id` (line 684), add:

```rust
agent.current_activity = None; // No longer running, clear activity
```

Also in the orphaned sub-agent cleanup (lines 936-942), after `agent.status = SubAgentStatus::Error;` (line 939) and before `agent.completed_at` (line 940), add:

```rust
agent.current_activity = None;
```

**Step 4: Verify progress lines reach the parser**

**Verified:** `parse_tail` does NOT filter by line type — all lines pass through to `parse_single_line`. `LineType::Progress` exists at line 80 and is classified at line 194-195 via `finders.type_progress`. No fix needed.

**Step 5: Run compilation**

Run: `cargo check -p claude-view-server`
Expected: Success

**Step 6: Manual verification**

Start the dev server, trigger a session with sub-agents, and check the SSE stream:
```bash
curl -N http://localhost:47892/api/live/stream
```
Expected: `session_updated` events should contain sub-agents with `currentActivity` populated while running.

**Step 7: Commit**

```bash
git add crates/server/src/live/manager.rs
git commit -m "feat(live): process agent_progress events for early agentId and current tool activity"
```

---

## Task 4: Sub-Agent Activity Display in SwimLanes

**Files:**
- Modify: `src/components/live/SwimLanes.tsx`
- Modify: `src/components/live/SubAgentPills.tsx`
- Test: `src/components/live/SwimLanes.test.tsx`

**Step 1: Write the failing test**

Add to `SwimLanes.test.tsx`:

```tsx
it('shows current activity for running agents', () => {
  const agents: SubAgentInfo[] = [{
    toolUseId: 'toolu_01',
    agentId: 'a951849',
    agentType: 'Explore',
    description: 'Search codebase',
    status: 'running',
    startedAt: Date.now() / 1000,
    currentActivity: 'Read',
  }]
  render(<SwimLanes subAgents={agents} sessionActive={true} />)
  expect(screen.getByText(/Read/)).toBeInTheDocument()
})

it('shows progress bar when no current activity', () => {
  const agents: SubAgentInfo[] = [{
    toolUseId: 'toolu_01',
    agentType: 'Explore',
    description: 'Search codebase',
    status: 'running',
    startedAt: Date.now() / 1000,
  }]
  render(<SwimLanes subAgents={agents} sessionActive={true} />)
  // Should show indeterminate progress bar (no activity text)
  expect(screen.queryByText(/Read/)).not.toBeInTheDocument()
})
```

**Step 2: Run test to verify it fails**

Run: `bun run vitest run src/components/live/SwimLanes.test.tsx`
Expected: FAIL (currentActivity not rendered)

**Step 3: Update SwimLanes running state**

In `SwimLanes.tsx`, replace the running agent's progress bar section (~line 112-116):

```tsx
{/* Running: activity text or progress bar */}
{agent.status === 'running' && (
  <div className="pl-4 flex items-center gap-2">
    {agent.currentActivity ? (
      <span className="text-xs font-mono text-blue-400 flex items-center gap-1.5">
        <span className="inline-block w-1.5 h-1.5 rounded-full bg-blue-400 animate-pulse" />
        {agent.currentActivity}
      </span>
    ) : (
      <ProgressBar />
    )}
  </div>
)}
```

**Step 4: Run test to verify it passes**

Run: `bun run vitest run src/components/live/SwimLanes.test.tsx`
Expected: ALL PASS

**Step 5: Update SubAgentPills tooltip**

In `SubAgentPills.tsx`, add activity text to the running pill's tooltip/title. When `agent.currentActivity` exists, show it as the pill's title attribute:

```tsx
title={agent.currentActivity ? `${agent.agentType}: ${agent.currentActivity}` : `${agent.agentType}: ${agent.description}`}
```

**Step 6: Run all frontend tests**

Run: `bun run vitest run src/components/live/SwimLanes.test.tsx src/components/live/SubAgentPills.test.tsx`
Expected: ALL PASS

**Step 7: Commit**

```bash
git add src/components/live/SwimLanes.tsx src/components/live/SubAgentPills.tsx src/components/live/SwimLanes.test.tsx
git commit -m "feat(ui): show sub-agent current tool activity in SwimLanes and pill tooltips"
```

---

## Task 5: Sub-Agent File Resolution Utility

**Files:**
- Create: `crates/server/src/live/subagent_file.rs`
- Modify: `crates/server/src/live/mod.rs`

**Step 1: Write the failing test**

```rust
// crates/server/src/live/subagent_file.rs
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_resolve_subagent_path() {
        let parent_jsonl = PathBuf::from(
            "/home/user/.claude/projects/my-project/abc123-def456.jsonl"
        );
        let agent_id = "a951849";
        let resolved = resolve_subagent_path(&parent_jsonl, agent_id);
        assert_eq!(
            resolved,
            PathBuf::from(
                "/home/user/.claude/projects/my-project/abc123-def456/subagents/agent-a951849.jsonl"
            )
        );
    }

    #[test]
    fn test_resolve_subagent_path_strips_extension() {
        let parent_jsonl = PathBuf::from("/path/to/session.jsonl");
        let resolved = resolve_subagent_path(&parent_jsonl, "b789012");
        assert_eq!(
            resolved,
            PathBuf::from("/path/to/session/subagents/agent-b789012.jsonl")
        );
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p claude-view-server -- subagent_file`
Expected: FAIL (module doesn't exist)

**Step 3: Create the file resolution module**

```rust
// crates/server/src/live/subagent_file.rs

//! Utility for resolving sub-agent JSONL file paths.
//!
//! Given a parent session's JSONL path and a sub-agent's 7-char ID,
//! resolves the path to the sub-agent's own JSONL file:
//!
//! ```text
//! Parent: ~/.claude/projects/{project}/{sessionId}.jsonl
//! Agent:  ~/.claude/projects/{project}/{sessionId}/subagents/agent-{agentId}.jsonl
//! ```

use std::path::{Path, PathBuf};

/// Resolve the filesystem path to a sub-agent's JSONL file.
///
/// The path structure is:
/// `{parent_dir}/{session_stem}/subagents/agent-{agent_id}.jsonl`
///
/// where `session_stem` is the parent JSONL filename without `.jsonl` extension.
pub fn resolve_subagent_path(parent_jsonl: &Path, agent_id: &str) -> PathBuf {
    let parent_dir = parent_jsonl.parent().unwrap_or(Path::new("."));
    let session_stem = parent_jsonl
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");
    parent_dir
        .join(session_stem)
        .join("subagents")
        .join(format!("agent-{agent_id}.jsonl"))
}

/// Check if a sub-agent's JSONL file exists on disk.
pub fn subagent_file_exists(parent_jsonl: &Path, agent_id: &str) -> bool {
    resolve_subagent_path(parent_jsonl, agent_id).exists()
}
```

Register in `crates/server/src/live/mod.rs`:

```rust
pub mod subagent_file;
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p claude-view-server -- subagent_file`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/server/src/live/subagent_file.rs crates/server/src/live/mod.rs
git commit -m "feat(live): add sub-agent JSONL file path resolution utility"
```

---

## Task 6: Sub-Agent Terminal WebSocket Endpoint

**Files:**
- Modify: `crates/server/src/routes/terminal.rs`
- (No change needed to `crates/server/src/routes/mod.rs` — terminal routes auto-nest via existing `.nest("/api/live", terminal::router())` call)

This task adds `WS /api/live/sessions/:id/subagents/:agentId/terminal` — a WebSocket endpoint that streams a sub-agent's JSONL file. It reuses the existing `handle_terminal_ws` infrastructure (scrollback, file watching, mode switching).

**Step 1: Add the route**

In `terminal.rs`, add the route to the router:

In terminal.rs `router()` function (line 48-49), add the sub-agent route to the existing chain:

```rust
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/sessions/{id}/terminal", get(ws_terminal_handler))
        .route(
            "/sessions/{id}/subagents/{agent_id}/terminal",
            get(ws_subagent_terminal_handler),
        )
}
```

Also update the doc comment (lines 44-47) to include the new route:

```rust
/// Build the terminal WebSocket sub-router.
///
/// Routes:
/// - `WS /sessions/:id/terminal` - WebSocket stream of JSONL lines
/// - `WS /sessions/:id/subagents/:agentId/terminal` - Sub-agent JSONL stream
```

**Step 2: Implement the handler**

```rust
/// HTTP upgrade handler for sub-agent terminal WebSocket.
///
/// Resolves the sub-agent's JSONL path from the parent session's file_path
/// and the agent_id, then delegates to the same streaming logic.
async fn ws_subagent_terminal_handler(
    State(state): State<Arc<AppState>>,
    Path((session_id, agent_id)): Path<(String, String)>,
    ws: WebSocketUpgrade,
) -> Response {
    // SECURITY: Validate agent_id is a 7-char hex hash (e.g., "a951849").
    // Without this, an attacker could pass "../../etc/passwd" to traverse the filesystem.
    if agent_id.is_empty() || !agent_id.chars().all(|c| c.is_ascii_alphanumeric()) || agent_id.len() > 16 {
        return ws.on_upgrade(move |mut socket| async move {
            let err_msg = serde_json::json!({
                "type": "error",
                "message": format!("Invalid agent ID: '{}'", agent_id),
            });
            let _ = socket.send(Message::Text(err_msg.to_string().into())).await;
            let _ = socket.send(Message::Close(Some(CloseFrame {
                code: 4004,
                reason: "Invalid agent ID".into(),
            }))).await;
        });
    }

    // Look up the parent session to get its JSONL file path
    let parent_file_path = {
        let map = state.live_sessions.read().await;
        map.get(&session_id).map(|s| s.file_path.clone())
    };

    let parent_file_path = match parent_file_path {
        Some(fp) if !fp.is_empty() => PathBuf::from(fp),
        _ => {
            return ws.on_upgrade(move |mut socket| async move {
                let err_msg = serde_json::json!({
                    "type": "error",
                    "message": format!("Parent session '{}' not found", session_id),
                });
                let _ = socket.send(Message::Text(err_msg.to_string().into())).await;
                let _ = socket.send(Message::Close(Some(CloseFrame {
                    code: 4004,
                    reason: "Session not found".into(),
                }))).await;
            });
        }
    };

    // Resolve sub-agent JSONL path
    let subagent_path = crate::live::subagent_file::resolve_subagent_path(
        &parent_file_path,
        &agent_id,
    );

    if !subagent_path.exists() {
        return ws.on_upgrade(move |mut socket| async move {
            let err_msg = serde_json::json!({
                "type": "error",
                "message": format!(
                    "Sub-agent '{}' JSONL file not found for session '{}'",
                    agent_id, session_id
                ),
            });
            let _ = socket.send(Message::Text(err_msg.to_string().into())).await;
            let _ = socket.send(Message::Close(Some(CloseFrame {
                code: 4004,
                reason: "Sub-agent file not found".into(),
            }))).await;
        });
    }

    // Use a separate connection namespace for sub-agents
    // Format: "{session_id}::{agent_id}" to avoid collision with parent
    let connection_key = format!("{}::{}", session_id, agent_id);
    let terminal_connections = state.terminal_connections.clone();

    ws.on_upgrade(move |mut socket| async move {
        if let Err(e) = terminal_connections.connect(&connection_key) {
            let err_msg = serde_json::json!({
                "type": "error",
                "message": e.to_string(),
            });
            let _ = socket.send(Message::Text(err_msg.to_string().into())).await;
            let _ = socket.send(Message::Close(Some(CloseFrame {
                code: 4004,
                reason: "Connection limit exceeded".into(),
            }))).await;
            return;
        }

        let _guard = ConnectionGuard {
            session_id: connection_key.clone(),
            manager: terminal_connections.clone(),
        };

        // Reuse the same streaming logic as parent sessions
        handle_terminal_ws(
            socket,
            connection_key,
            subagent_path,
            terminal_connections.clone(),
        ).await;
    })
}
```

**Step 3: Verify compilation**

Run: `cargo check -p claude-view-server`
Expected: Success

**Step 4: Manual test**

1. Start the dev server: `bun dev`
2. Find a session with sub-agents
3. Connect to the sub-agent WebSocket:
   ```bash
   websocat "ws://localhost:47892/api/live/sessions/{sessionId}/subagents/{agentId}/terminal"
   # Send handshake:
   {"mode":"rich","scrollback":100}
   ```
4. Expected: Receive sub-agent JSONL lines formatted in rich mode

**Step 5: Commit**

```bash
git add crates/server/src/routes/terminal.rs
git commit -m "feat(live): add WebSocket endpoint for sub-agent JSONL streaming"
```

---

## Task 7: Sub-Agent Drill-Down Hook (Frontend)

**Files:**
- Create: `src/components/live/use-subagent-stream.ts`
- Test: `src/components/live/use-subagent-stream.test.ts`

This hook wraps `useTerminalSocket` with the sub-agent-specific URL pattern.

**Step 1: Write the test**

```tsx
// src/components/live/use-subagent-stream.test.ts
import { describe, it, expect, vi } from 'vitest'
import { renderHook, act } from '@testing-library/react'
import { useSubAgentStream } from './use-subagent-stream'

// Mock useTerminalSocket since it creates real WebSockets
vi.mock('../../hooks/use-terminal-socket', () => ({
  useTerminalSocket: vi.fn(() => ({
    connectionState: 'disconnected' as const,
    sendMessage: vi.fn(),
    reconnect: vi.fn(),
  })),
}))

import { useTerminalSocket } from '../../hooks/use-terminal-socket'
const mockUseTerminalSocket = vi.mocked(useTerminalSocket)

describe('useSubAgentStream', () => {
  it('passes correct URL to useTerminalSocket', () => {
    const onMessage = vi.fn()
    renderHook(() =>
      useSubAgentStream({
        sessionId: 'abc123',
        agentId: 'a951849',
        enabled: true,
        onMessage,
      })
    )

    expect(mockUseTerminalSocket).toHaveBeenCalledWith(
      expect.objectContaining({
        sessionId: 'abc123/subagents/a951849',
        mode: 'rich',
        enabled: true,
      })
    )
  })

  it('disables when agentId is null', () => {
    renderHook(() =>
      useSubAgentStream({
        sessionId: 'abc123',
        agentId: null,
        enabled: true,
        onMessage: vi.fn(),
      })
    )

    expect(mockUseTerminalSocket).toHaveBeenCalledWith(
      expect.objectContaining({
        enabled: false,
      })
    )
  })
})
```

**Step 2: Run test to verify it fails**

Run: `bun run vitest run src/components/live/use-subagent-stream.test.ts`
Expected: FAIL (module not found)

**Step 3: Implement the hook**

```tsx
// src/components/live/use-subagent-stream.ts
import { useCallback, useState } from 'react'
import { useTerminalSocket, type ConnectionState } from '../../hooks/use-terminal-socket'
import { parseRichMessage, type RichMessage } from './RichPane'

interface UseSubAgentStreamOptions {
  sessionId: string
  agentId: string | null
  enabled: boolean
  onMessage: (data: string) => void
}

interface UseSubAgentStreamResult {
  connectionState: ConnectionState
  messages: RichMessage[]
  bufferDone: boolean
  reconnect: () => void
}

/**
 * Hook for streaming a sub-agent's JSONL conversation over WebSocket.
 *
 * Connects to `/api/live/sessions/:id/subagents/:agentId/terminal`
 * using the existing terminal socket infrastructure. Messages are
 * parsed into RichMessage format for rendering in RichPane.
 *
 * Uses the session ID format "{sessionId}::{agentId}" to namespace
 * the connection (matching the backend's connection key format).
 */
export function useSubAgentStream(options: UseSubAgentStreamOptions): UseSubAgentStreamResult {
  const { sessionId, agentId, enabled, onMessage } = options
  const [messages, setMessages] = useState<RichMessage[]>([])
  const [bufferDone, setBufDone] = useState(false)

  const handleMessage = useCallback((data: string) => {
    // Check for buffer_end signal
    try {
      const parsed = JSON.parse(data)
      if (parsed.type === 'buffer_end') {
        setBufDone(true)
        return
      }
    } catch { /* not JSON, continue */ }

    const rich = parseRichMessage(data)
    if (rich) {
      setMessages((prev) => [...prev, rich])
    }
    onMessage(data)
  }, [onMessage])

  // useTerminalSocket constructs: /api/live/sessions/${sessionId}/terminal
  // By embedding the subagent path IN the sessionId, the URL becomes:
  // /api/live/sessions/abc123/subagents/a951849/terminal
  // This works because wsUrl() does plain string concatenation (no encoding).
  const { connectionState, reconnect } = useTerminalSocket({
    sessionId: agentId ? `${sessionId}/subagents/${agentId}` : sessionId,
    mode: 'rich',
    scrollback: 100_000,
    enabled: enabled && agentId !== null,
    onMessage: handleMessage,
  })

  return { connectionState, messages, bufferDone, reconnect }
}
```

**Verified:** `useTerminalSocket` constructs the URL via `wsUrl(`/api/live/sessions/${sessionId}/terminal`)` (use-terminal-socket.ts line 124). `wsUrl()` in `src/lib/ws-url.ts` does plain string concatenation — no `encodeURIComponent` or URL encoding. Setting `sessionId` to `"abc123/subagents/a951849"` produces `/api/live/sessions/abc123/subagents/a951849/terminal` which exactly matches the backend route. No modifications to `useTerminalSocket` or `wsUrl` are needed.

**Step 4: Run test to verify it passes**

Run: `bun run vitest run src/components/live/use-subagent-stream.test.ts`
Expected: PASS

**Step 5: Commit**

```bash
git add src/components/live/use-subagent-stream.ts src/components/live/use-subagent-stream.test.ts
git commit -m "feat(ui): add useSubAgentStream hook for drill-down WebSocket connection"
```

---

## Task 8: Sub-Agent Drill-Down Panel Component

**Files:**
- Create: `src/components/live/SubAgentDrillDown.tsx`
- Test: `src/components/live/SubAgentDrillDown.test.tsx`

**Step 1: Write the failing test**

```tsx
// src/components/live/SubAgentDrillDown.test.tsx
import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { SubAgentDrillDown } from './SubAgentDrillDown'

// Mock the stream hook
vi.mock('./use-subagent-stream', () => ({
  useSubAgentStream: vi.fn(() => ({
    connectionState: 'connected',
    messages: [
      { type: 'user', content: 'Search for auth code' },
      { type: 'assistant', content: 'Found 3 files related to authentication.' },
    ],
    bufferDone: true,
    reconnect: vi.fn(),
  })),
}))

describe('SubAgentDrillDown', () => {
  it('renders agent type and description in header', () => {
    render(
      <SubAgentDrillDown
        sessionId="abc123"
        agentId="a951849"
        agentType="Explore"
        description="Search codebase for auth"
        onClose={vi.fn()}
      />
    )
    expect(screen.getByText('Explore')).toBeInTheDocument()
    expect(screen.getByText('Search codebase for auth')).toBeInTheDocument()
  })

  it('renders sub-agent messages in RichPane', () => {
    render(
      <SubAgentDrillDown
        sessionId="abc123"
        agentId="a951849"
        agentType="Explore"
        description="Search codebase"
        onClose={vi.fn()}
      />
    )
    expect(screen.getByText('Search for auth code')).toBeInTheDocument()
    expect(screen.getByText(/Found 3 files/)).toBeInTheDocument()
  })

  it('calls onClose when close button clicked', async () => {
    const onClose = vi.fn()
    render(
      <SubAgentDrillDown
        sessionId="abc123"
        agentId="a951849"
        agentType="Explore"
        description="Search"
        onClose={onClose}
      />
    )
    const closeButton = screen.getByRole('button', { name: /close/i })
    await userEvent.click(closeButton)
    expect(onClose).toHaveBeenCalled()
  })
})
```

**Step 2: Run test to verify it fails**

Run: `bun run vitest run src/components/live/SubAgentDrillDown.test.tsx`
Expected: FAIL (module not found)

**Step 3: Implement the component**

```tsx
// src/components/live/SubAgentDrillDown.tsx
import { useCallback, useState } from 'react'
import { X } from 'lucide-react'
import { RichPane, type RichMessage } from './RichPane'
import { useSubAgentStream } from './use-subagent-stream'
import { cn } from '../../lib/utils'

interface SubAgentDrillDownProps {
  sessionId: string
  agentId: string
  agentType: string
  description: string
  onClose: () => void
}

/**
 * Panel showing a sub-agent's full conversation.
 *
 * Connects to the sub-agent's WebSocket stream and renders
 * messages using RichPane. Displayed inline within SwimLanes
 * or as an overlay panel.
 */
export function SubAgentDrillDown({
  sessionId,
  agentId,
  agentType,
  description,
  onClose,
}: SubAgentDrillDownProps) {
  const [verboseMode, setVerboseMode] = useState(false)

  const noop = useCallback(() => {}, [])

  const { connectionState, messages: streamMessages, bufferDone, reconnect } = useSubAgentStream({
    sessionId,
    agentId,
    enabled: true,
    onMessage: noop,
  })

  return (
    <div className="flex flex-col h-full bg-gray-950 border border-gray-800 rounded-lg overflow-hidden">
      {/* Header */}
      <div className="flex items-center gap-2 px-3 py-2 border-b border-gray-800 bg-gray-900">
        <span className="text-xs font-mono text-gray-400 uppercase tracking-wide">
          {agentType}
        </span>
        <span className="text-xs text-gray-500">id:{agentId}</span>
        <span className="text-sm text-gray-300 flex-1 truncate">{description}</span>

        {/* Connection status */}
        <span className={cn(
          'text-[10px] font-mono',
          connectionState === 'connected' && 'text-green-400',
          connectionState === 'connecting' && 'text-yellow-400',
          connectionState === 'disconnected' && 'text-gray-500',
          connectionState === 'error' && 'text-red-400',
        )}>
          {connectionState}
        </span>

        {/* Verbose toggle */}
        <button
          onClick={() => setVerboseMode(!verboseMode)}
          className={cn(
            'text-[10px] px-1.5 py-0.5 rounded border',
            verboseMode
              ? 'border-blue-500 text-blue-400'
              : 'border-gray-700 text-gray-500 hover:text-gray-400',
          )}
        >
          {verboseMode ? 'verbose' : 'compact'}
        </button>

        {/* Close button */}
        <button
          onClick={onClose}
          aria-label="Close sub-agent drill-down"
          className="text-gray-500 hover:text-gray-300 transition-colors"
        >
          <X className="w-4 h-4" />
        </button>
      </div>

      {/* Conversation */}
      <div className="flex-1 min-h-0">
        <RichPane
          messages={streamMessages}
          isVisible={true}
          verboseMode={verboseMode}
          bufferDone={bufferDone}
        />
      </div>
    </div>
  )
}
```

**Step 4: Run test to verify it passes**

Run: `bun run vitest run src/components/live/SubAgentDrillDown.test.tsx`
Expected: ALL PASS

**Step 5: Commit**

```bash
git add src/components/live/SubAgentDrillDown.tsx src/components/live/SubAgentDrillDown.test.tsx
git commit -m "feat(ui): add SubAgentDrillDown panel for viewing sub-agent conversations"
```

---

## Task 9: SwimLanes Click-to-Expand Integration

**Files:**
- Modify: `src/components/live/SwimLanes.tsx`
- Modify: `src/components/live/MonitorView.tsx`
- Test: `src/components/live/SwimLanes.test.tsx`

**Step 1: Write the failing test**

Add to `SwimLanes.test.tsx`:

```tsx
it('calls onDrillDown when a running agent row is clicked', async () => {
  const onDrillDown = vi.fn()
  const agents: SubAgentInfo[] = [{
    toolUseId: 'toolu_01',
    agentId: 'a951849',
    agentType: 'Explore',
    description: 'Search codebase',
    status: 'running',
    startedAt: Date.now() / 1000,
  }]
  render(
    <SwimLanes
      subAgents={agents}
      sessionActive={true}
      onDrillDown={onDrillDown}
    />
  )
  const row = screen.getByText('Search codebase').closest('[role="button"]')
  await userEvent.click(row!)
  expect(onDrillDown).toHaveBeenCalledWith('a951849', 'Explore', 'Search codebase')
})

it('calls onDrillDown for completed agents with agentId', async () => {
  const onDrillDown = vi.fn()
  const agents: SubAgentInfo[] = [{
    toolUseId: 'toolu_01',
    agentId: 'a951849',
    agentType: 'Explore',
    description: 'Search codebase',
    status: 'complete',
    startedAt: Date.now() / 1000 - 30,
    completedAt: Date.now() / 1000,
    durationMs: 30000,
  }]
  render(
    <SwimLanes
      subAgents={agents}
      sessionActive={false}
      onDrillDown={onDrillDown}
    />
  )
  const row = screen.getByText('Search codebase').closest('[role="button"]')
  await userEvent.click(row!)
  expect(onDrillDown).toHaveBeenCalledWith('a951849', 'Explore', 'Search codebase')
})

it('does not make row clickable when agentId is missing', () => {
  const onDrillDown = vi.fn()
  const agents: SubAgentInfo[] = [{
    toolUseId: 'toolu_01',
    agentType: 'Explore',
    description: 'Search codebase',
    status: 'running',
    startedAt: Date.now() / 1000,
    // no agentId — file doesn't exist yet
  }]
  render(
    <SwimLanes
      subAgents={agents}
      sessionActive={true}
      onDrillDown={onDrillDown}
    />
  )
  const row = screen.getByText('Search codebase').closest('div')
  expect(row).not.toHaveAttribute('role', 'button')
})
```

**Step 2: Run test to verify it fails**

Run: `bun run vitest run src/components/live/SwimLanes.test.tsx`
Expected: FAIL

**Step 3: Add onDrillDown prop to SwimLanes**

Update `SwimLanesProps`:

```tsx
interface SwimLanesProps {
  subAgents: SubAgentInfo[]
  sessionActive: boolean
  /** Callback when user clicks a sub-agent to view its conversation.
   *  Only fired when agentId is available (needed to locate the JSONL file). */
  onDrillDown?: (agentId: string, agentType: string, description: string) => void
}
```

Make each swim lane row clickable when `agent.agentId` exists:

```tsx
{sortedAgents.map((agent) => {
  const canDrillDown = !!agent.agentId && !!onDrillDown
  return (
    <div
      key={agent.toolUseId}
      role={canDrillDown ? 'button' : undefined}
      tabIndex={canDrillDown ? 0 : undefined}
      onClick={canDrillDown ? () => onDrillDown(agent.agentId!, agent.agentType, agent.description) : undefined}
      onKeyDown={canDrillDown ? (e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault()
          onDrillDown(agent.agentId!, agent.agentType, agent.description)
        }
      } : undefined}
      className={cn(
        'flex flex-col gap-1.5 border-b border-gray-800 last:border-b-0 pb-2 last:pb-0',
        canDrillDown && 'cursor-pointer hover:bg-gray-900/50 rounded-md transition-colors -mx-1 px-1',
      )}
    >
      {/* Header row: status + type + description (existing, unchanged) */}
      <div className="flex items-center gap-2">
        <StatusDot status={agent.status} />
        <span className="text-xs font-mono text-gray-400 uppercase tracking-wide min-w-[80px]">
          {agent.agentType}
        </span>
        <span className="text-sm text-gray-300 flex-1 truncate">
          {agent.description}
        </span>
        {agent.status === 'error' && (
          <span className="text-xs text-red-400 font-medium">ERROR</span>
        )}
      </div>

      {/* Running: activity text or progress bar (modified by Task 4) */}
      {agent.status === 'running' && (
        <div className="pl-4 flex items-center gap-2">
          {agent.currentActivity ? (
            <span className="text-xs font-mono text-blue-400 flex items-center gap-1.5">
              <span className="inline-block w-1.5 h-1.5 rounded-full bg-blue-400 animate-pulse" />
              {agent.currentActivity}
            </span>
          ) : (
            <ProgressBar />
          )}
        </div>
      )}

      {/* Complete/Error: metrics row (existing, unchanged) */}
      {agent.status !== 'running' && (
        <div className="flex items-center gap-4 pl-4 text-xs font-mono text-gray-400">
          {agent.costUsd != null && (
            <span>{formatCost(agent.costUsd)}</span>
          )}
          {agent.durationMs != null && (
            <span>{formatDuration(agent.durationMs)}</span>
          )}
          {agent.toolUseCount != null && (
            <span>{agent.toolUseCount} tool call{agent.toolUseCount !== 1 ? 's' : ''}</span>
          )}
          {agent.agentId && (
            <span className="text-gray-500">id:{agent.agentId}</span>
          )}
        </div>
      )}
    </div>
  )
})}
```

**Step 4: Wire drill-down in MonitorView expanded overlay**

In `MonitorView.tsx`, first add the import at the top of the file (after existing live component imports):

```tsx
import { SubAgentDrillDown } from './SubAgentDrillDown'
```

Then add drill-down state management:

```tsx
// Add drill-down state near other MonitorView state declarations:
const [drillDownAgent, setDrillDownAgent] = useState<{
  agentId: string
  agentType: string
  description: string
} | null>(null)

// IMPORTANT: Reset drill-down when expanded session changes (prevents stale state):
// Add to the existing useEffect or create one:
useEffect(() => {
  setDrillDownAgent(null)
}, [expandedPaneId]) // expandedPaneId is the existing state that tracks which session is expanded

// In the expanded overlay JSX (ExpandedPaneOverlay's children, ~line 259):
// Replace the existing <div className="flex flex-col h-full gap-3"> content with:
<div className="flex flex-col h-full gap-3">
  {drillDownAgent ? (
    // Show sub-agent drill-down
    <SubAgentDrillDown
      key={drillDownAgent.agentId}
      sessionId={expandedSession.id}
      agentId={drillDownAgent.agentId}
      agentType={drillDownAgent.agentType}
      description={drillDownAgent.description}
      onClose={() => setDrillDownAgent(null)}
    />
  ) : (
    // Show normal expanded view — PRESERVE existing TimelineView + SwimLanes + terminal
    <>
      {/* Sub-agent timeline — same props as existing code at lines 261-270 */}
      {expandedSession.subAgents && expandedSession.subAgents.length > 0 && expandedSession.startedAt && (
        <TimelineView
          subAgents={expandedSession.subAgents}
          sessionStartedAt={expandedSession.startedAt}
          sessionDurationMs={
            expandedSession.status === 'done'
              ? (expandedSession.lastActivityAt - expandedSession.startedAt) * 1000
              : Date.now() - expandedSession.startedAt * 1000
          }
        />
      )}
      {/* Sub-agent detail list — add onDrillDown prop */}
      {expandedSession.subAgents && expandedSession.subAgents.length > 0 && (
        <SwimLanes
          subAgents={expandedSession.subAgents}
          sessionActive={expandedSession.status === 'working'}
          onDrillDown={(agentId, agentType, description) =>
            setDrillDownAgent({ agentId, agentType, description })
          }
        />
      )}
      {/* Terminal stream */}
      <div className="flex-1 min-h-0">
        <RichTerminalPane
          sessionId={expandedSession.id}
          isVisible={true}
          verboseMode={verboseMode}
        />
      </div>
    </>
  )}
</div>
```

**Step 5: Run tests**

Run: `bun run vitest run src/components/live/SwimLanes.test.tsx`
Expected: ALL PASS

**Step 6: Type check**

Run: `bun run typecheck`
Expected: No errors

**Step 7: Commit**

```bash
git add src/components/live/SwimLanes.tsx src/components/live/MonitorView.tsx src/components/live/SwimLanes.test.tsx
git commit -m "feat(ui): add click-to-drill-down on SwimLanes rows with MonitorView integration"
```

---

## Task 10: Backend Tests — Progress Events + Sub-Agent Streaming

**Files:**
- Modify: `crates/core/src/live_parser.rs` (test module)
- Modify: `crates/server/src/live/subagent_file.rs` (test module)

**Step 1: Add comprehensive parser tests**

Add to the `#[cfg(test)]` module in `live_parser.rs`:

```rust
#[test]
fn test_progress_event_multiple_tool_uses() {
    // Progress event with multiple tool_use blocks — should pick the LAST one
    let finders = TailFinders::new();
    let raw = br#"{"type":"progress","parentToolUseID":"toolu_01ABC","data":{"type":"agent_progress","agentId":"a951849","message":{"role":"assistant","content":[{"type":"tool_use","name":"Read","input":{}},{"type":"text","text":"..."},{"type":"tool_use","name":"Grep","input":{}}]}}}"#;
    let line = parse_single_line(raw, &finders);
    let progress = line.sub_agent_progress.unwrap();
    assert_eq!(progress.current_tool, Some("Grep".to_string())); // Last tool_use
}

#[test]
fn test_progress_event_text_only_content() {
    // Progress event where assistant is thinking (no tool_use)
    let finders = TailFinders::new();
    let raw = br#"{"type":"progress","parentToolUseID":"toolu_01ABC","data":{"type":"agent_progress","agentId":"a951849","message":{"role":"assistant","content":[{"type":"text","text":"Analyzing the codebase..."}]}}}"#;
    let line = parse_single_line(raw, &finders);
    let progress = line.sub_agent_progress.unwrap();
    assert_eq!(progress.agent_id, "a951849");
    assert_eq!(progress.current_tool, None); // No tool_use in content
}

#[test]
fn test_simd_prefilter_skips_non_progress() {
    // A regular assistant line should not trigger progress extraction
    let finders = TailFinders::new();
    let raw = br#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Here is the result."}]},"timestamp":"2026-02-16T08:34:13.134Z"}"#;
    let line = parse_single_line(raw, &finders);
    assert!(line.sub_agent_progress.is_none());
}

#[test]
fn test_spawn_and_progress_same_agent() {
    // Spawn detection and progress detection produce different fields
    // that the manager merges. Verify they're independent.
    let finders = TailFinders::new();

    // Spawn line
    let spawn_raw = br#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"toolu_01ABC","name":"Task","input":{"description":"Search auth","subagent_type":"Explore"}}]},"timestamp":"2026-02-16T08:34:00.000Z"}"#;
    let spawn_line = parse_single_line(spawn_raw, &finders);
    assert_eq!(spawn_line.sub_agent_spawns.len(), 1);
    assert!(spawn_line.sub_agent_progress.is_none());

    // Progress line
    let progress_raw = br#"{"type":"progress","parentToolUseID":"toolu_01ABC","data":{"type":"agent_progress","agentId":"a951849","message":{"role":"assistant","content":[{"type":"tool_use","name":"Grep","input":{}}]}}}"#;
    let progress_line = parse_single_line(progress_raw, &finders);
    assert!(progress_line.sub_agent_spawns.is_empty());
    assert!(progress_line.sub_agent_progress.is_some());
}
```

**Step 2: Run all parser tests**

Run: `cargo test -p claude-view-core -- live_parser`
Expected: ALL PASS

**Step 3: Run full crate check**

Run: `cargo check -p claude-view-server && cargo test -p claude-view-core`
Expected: ALL PASS

**Step 4: Commit**

```bash
git add crates/core/src/live_parser.rs
git commit -m "test(core): comprehensive tests for agent_progress parsing and SIMD pre-filtering"
```

---

## Task 11: Frontend Tests — Drill-Down Integration

**Files:**
- Modify: `src/components/live/SubAgentDrillDown.test.tsx`

**Step 1: Add integration-level tests**

```tsx
describe('SubAgentDrillDown integration', () => {
  it('shows connecting state before buffer loads', () => {
    // Override mock to return 'connecting' state
    vi.mocked(useSubAgentStream).mockReturnValue({
      connectionState: 'connecting',
      messages: [],
      bufferDone: false,
      reconnect: vi.fn(),
    })

    render(
      <SubAgentDrillDown
        sessionId="abc123"
        agentId="a951849"
        agentType="Explore"
        description="Search codebase"
        onClose={vi.fn()}
      />
    )
    expect(screen.getByText('connecting')).toBeInTheDocument()
  })

  it('shows error state and allows reconnect', async () => {
    const reconnect = vi.fn()
    vi.mocked(useSubAgentStream).mockReturnValue({
      connectionState: 'error',
      messages: [],
      bufferDone: false,
      reconnect,
    })

    render(
      <SubAgentDrillDown
        sessionId="abc123"
        agentId="a951849"
        agentType="Explore"
        description="Search codebase"
        onClose={vi.fn()}
      />
    )
    expect(screen.getByText('error')).toBeInTheDocument()
  })

  it('toggles verbose mode', async () => {
    render(
      <SubAgentDrillDown
        sessionId="abc123"
        agentId="a951849"
        agentType="Explore"
        description="Search codebase"
        onClose={vi.fn()}
      />
    )
    const toggle = screen.getByText('compact')
    await userEvent.click(toggle)
    expect(screen.getByText('verbose')).toBeInTheDocument()
  })
})
```

**Step 2: Run all frontend tests**

Run: `bun run vitest run src/components/live/`
Expected: ALL PASS

**Step 3: Commit**

```bash
git add src/components/live/SubAgentDrillDown.test.tsx
git commit -m "test(ui): integration tests for SubAgentDrillDown state and verbose toggle"
```

---

## Task 12: Verification & End-to-End

**Files:** None (verification only)

**Step 1: Run full backend compilation and tests**

```bash
cargo check -p claude-view-server
cargo test -p claude-view-core
cargo test -p claude-view-server -- subagent_file
```

Expected: ALL PASS, no warnings related to new code.

**Step 2: Run full frontend tests and type check**

```bash
bun run typecheck
bun run vitest run src/components/live/
```

Expected: ALL PASS, no type errors.

**Step 3: Verify ts-rs type generation**

Check that `src/types/generated/SubAgentInfo.ts` includes `currentActivity`:

```bash
grep 'currentActivity' src/types/generated/SubAgentInfo.ts
```

Expected: `currentActivity?: string | null;`

**Step 4: Manual end-to-end test**

1. Start dev server: `bun dev`
2. Open Mission Control in browser
3. In a terminal, start a Claude Code session that uses Task tool (sub-agents):
   ```
   claude "Search the codebase for all authentication-related code and also review the test coverage"
   ```
4. Verify in the UI:
   - SwimLanes show sub-agents with `currentActivity` updating (e.g., "Read", "Grep")
   - SubAgentPills show activity in tooltip
   - Clicking a swim lane row opens the drill-down panel
   - Drill-down shows the sub-agent's conversation in real-time
   - Closing drill-down returns to the SwimLanes view
5. Wait for sub-agents to complete:
   - Verify `currentActivity` clears on completion
   - Verify completed agents still clickable for drill-down
   - Verify drill-down shows full conversation history

**Step 5: Verify no regressions**

```bash
bun run vitest run
cargo test -p claude-view-core
```

Expected: ALL PASS

**Step 6: Final commit (if any fixes were needed)**

```bash
git add -A
git commit -m "fix(live): address Phase D.2 verification issues"
```

---

## Files Summary

### New Files

| File | Purpose |
|------|---------|
| `crates/server/src/live/subagent_file.rs` | Sub-agent JSONL file path resolution |
| `src/components/live/use-subagent-stream.ts` | Hook for sub-agent WebSocket connection |
| `src/components/live/use-subagent-stream.test.ts` | Tests for stream hook |
| `src/components/live/SubAgentDrillDown.tsx` | Sub-agent conversation viewer panel |
| `src/components/live/SubAgentDrillDown.test.tsx` | Tests for drill-down component |

### Modified Files

| File | Change |
|------|--------|
| `crates/core/src/live_parser.rs` | Add `agent_progress_key` SIMD finder, `SubAgentProgress` type, `sub_agent_progress` field on LiveLine, progress extraction logic |
| `crates/core/src/subagent.rs` | Add `current_activity: Option<String>` to SubAgentInfo |
| `crates/server/src/live/manager.rs` | Process progress events → populate agent_id early + update current_activity; clear activity on completion |
| `crates/server/src/live/mod.rs` | Register `subagent_file` module |
| `crates/server/src/live/state.rs` | Update `make_live_line` test helper with `sub_agent_progress: None` |
| `crates/server/src/routes/terminal.rs` | Add `ws_subagent_terminal_handler` + route registration + agent_id validation |
| `src/components/live/SwimLanes.tsx` | Add `onDrillDown` prop, clickable rows, activity display |
| `src/components/live/SwimLanes.test.tsx` | Tests for drill-down clicks and activity display |
| `src/components/live/SubAgentPills.tsx` | Activity text in pill tooltip |
| `src/components/live/MonitorView.tsx` | Drill-down state management, SubAgentDrillDown integration |
| `src/types/generated/SubAgentInfo.ts` | Auto-updated by ts-rs with `currentActivity` field |

### Dependencies

No new Rust crate dependencies. No new npm dependencies.

---

## Acceptance Criteria

- [ ] Progress events parsed from parent JSONL — `agentId` available within seconds of spawn (not just on completion)
- [ ] `currentActivity` shows the sub-agent's current tool (e.g., "Read", "Grep", "Edit") in SwimLanes
- [ ] `currentActivity` clears when sub-agent completes or errors
- [ ] SubAgentPills tooltip shows current activity for running agents
- [ ] Clicking a swim lane row with `agentId` opens the drill-down panel
- [ ] Drill-down shows the sub-agent's full conversation via RichPane
- [ ] Drill-down supports verbose/compact mode toggle
- [ ] Drill-down shows connection state (connecting/connected/error)
- [ ] Sub-agent WebSocket endpoint (`/api/live/sessions/:id/subagents/:agentId/terminal`) streams JSONL correctly
- [ ] Sub-agent file resolution handles edge cases (missing files, invalid IDs)
- [ ] Rows without `agentId` (freshly spawned, no progress event yet) are not clickable
- [ ] Closing drill-down returns to SwimLanes view
- [ ] No regressions in existing Phase D tests (57+ frontend, backend compilation)
- [ ] SIMD pre-filter skips non-progress lines (no unnecessary JSON parsing)
- [ ] Connection limits respected for sub-agent WebSockets (namespaced keys)
- [ ] Sub-agent WebSocket validates `agent_id` is alphanumeric (path traversal prevention)
- [ ] Drill-down state resets when expanding a different session (no stale agent panel)

---

## Rollback

All changes are additive (new struct fields, new files, new routes). No existing behavior is modified.

**To roll back completely:** Revert all commits from this phase. The new `sub_agent_progress` field on `LiveLine` and `current_activity` on `SubAgentInfo` are `Option` types with `None` defaults — removing them requires removing the field from all construction sites (3 for LiveLine, 1 for SubAgentInfo) and deleting the new files. After reverting `subagent.rs`, run `cargo test -p claude-view-core` to regenerate `src/types/generated/SubAgentInfo.ts` without `currentActivity`.

**To roll back partially (keep backend, revert frontend drill-down):**
1. Keep Tasks 1-3 (progress parsing + activity tracking in SSE stream)
2. Revert Tasks 7-9 (drill-down hook, panel, and SwimLanes click integration)
3. Task 4 (activity display) and Task 6 (WebSocket endpoint) can stand alone

**Task dependency chain:**
```
Task 1 (parser) → Task 3 (manager) → SSE stream gets currentActivity
Task 2 (type)   ↗
Task 5 (file util) → Task 6 (WS endpoint) → Task 7 (hook) → Task 8 (panel) → Task 9 (integration)
Task 4 (UI display) is independent — just reads currentActivity from SSE
```

## Audit Notes

**Task 5 must complete before Task 6.** Task 6 calls `crate::live::subagent_file::resolve_subagent_path` which is created in Task 5.

**WebSocket URL hack is verified safe.** `wsUrl()` in `src/lib/ws-url.ts` does plain string concatenation without URL-encoding. Passing `"abc123/subagents/a951849"` as sessionId produces the correct URL `/api/live/sessions/abc123/subagents/a951849/terminal`. If `wsUrl()` ever adds encoding, this will break and needs a `urlOverride` prop on `useTerminalSocket`.

**Test style note.** Existing parser tests use `parse_tail` with tempfile fixtures, but the plan's tests call `parse_single_line` directly. Both work (test module has `use super::*`). Direct calls are more focused for unit tests; use whichever the implementer prefers.

---

## Changelog of Fixes Applied (Audit → Final Plan)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | Task 7 test asserted `sessionId: 'abc123::a951849'` (double colon) but implementation uses `'abc123/subagents/a951849'` (slashes) — test would fail | Blocker | Fixed assertion to match implementation's slash-based sessionId |
| 2 | SubAgentDrillDown had dead state: `useState<boolean>(bufferDone)` and `useState<RichMessage[]>` that shadowed hook's return values | Warning | Removed redundant state; component now uses only hook's returns |
| 3 | `useSubAgentStream` tracked `bufferDone` internally but never returned it; SubAgentDrillDown used `connectionState === 'connected'` as incorrect proxy | Warning | Added `bufferDone` to `UseSubAgentStreamResult` interface and return value; SubAgentDrillDown now passes `bufferDone` to RichPane |
| 4 | SubAgentPills tooltip dropped `agentType` prefix in else branch: `agent.description` instead of `` `${agent.agentType}: ${agent.description}` `` | Warning | Both branches now include agentType prefix |
| 5 | MonitorView drill-down JSX replaced entire expanded section, dropping TimelineView when showing normal (non-drill-down) view | Warning | Added TimelineView back to the else branch of the drill-down conditional |
| 6 | Unused `ExternalLink` import in SubAgentDrillDown component | Warning | Removed from import statement |
| 7 | Task 6 files list included `crates/server/src/routes/mod.rs` which needs no changes (terminal routes auto-nest via existing `.nest()`) | Minor | Replaced with explanatory note |
| 8 | Line numbers throughout plan are approximate (off by 20-200 lines due to codebase evolution) | Minor | Added audit note about using context descriptions instead of line numbers |
| 9 | TimelineView in MonitorView else branch had wrong props (`sessionActive` boolean instead of `sessionStartedAt`/`sessionDurationMs`) — would not compile | Blocker | Replaced with exact props from existing MonitorView code (lines 262-270), including duration calculation logic |
| 10 | RichTerminalPane in MonitorView else branch missing required `verboseMode` prop — would not compile | Blocker | Added `verboseMode={verboseMode}` matching existing usage at line 284 |
| 11 | `drillDownAgent` state not cleared when expanded session changes — stale drill-down would persist across sessions | Warning | Added `useEffect` that resets `drillDownAgent` to `null` when `expandedPaneId` changes |
| 12 | Task 8/11 test mocks for `useSubAgentStream` missing `bufferDone` field — TypeScript error in mock returns | Warning | Added `bufferDone: true/false` to all mock return values |
| 13 | All line numbers were approximate (off by 20-200 lines) — would slow implementer | Minor → Fixed | Updated all critical line numbers to exact values verified against codebase |
| 14 | No `agent_id` input validation in WebSocket handler — path traversal vulnerability via `../../` in agent_id | Blocker (security) | Added alphanumeric + length validation before file path resolution |
| 15 | Task 1 "Important" note left `LineType::Progress` existence as an unresolved question | Minor | Replaced with "Verified" note — `LineType::Progress` confirmed at line 80 |
| 16 | Task 3 insertion point ambiguous ("after line 705") — could be inside or outside the for loop | Blocker | Reworded to explicitly say "INSIDE the loop body, BEFORE the loop's closing `}` at line 706" |
| 17 | Task 7 "implementation note" left wsUrl encoding as an open question requiring investigation | Minor | Replaced with "Verified" statement — wsUrl does plain concatenation, no encoding |
| 18 | No rollback section — required for 100/100 plan | Minor | Added Rollback section with full/partial rollback instructions and task dependency chain |
| 19 | SubAgentInfo construction sites listed "2. Any test helpers" — misleading, there are no other sites | Minor | Updated to "exactly 1 site (verified via grep)" with precise line reference |
| 20 | Task 9 Step 3 SwimLanes row had `{/* ... existing content ... */}` placeholder — implementer can't execute without guessing | Blocker | Replaced with complete verbatim row content (header, activity/progress, metrics) matching SwimLanes.tsx lines 96-134, merged with Task 4's activity change |
| 21 | Task 9 Step 4 adds `<SubAgentDrillDown>` JSX to MonitorView but never imports the component — TypeScript compilation error | Blocker | Added explicit `import { SubAgentDrillDown } from './SubAgentDrillDown'` instruction at top of Step 4 |
| 22 | Task 3 Step 4 had stale ambiguous language: "likely includes", "may skip", "If so" — left verification as implementer judgment call | Warning | Replaced with "Verified" statement confirming `parse_tail` passes all lines through and `LineType::Progress` exists at line 80 |
| 23 | `SubAgentDrillDown` not keyed by `agentId` — switching drill-down target shows stale messages from previous agent until new stream arrives | Blocker (UX) | Added `key={drillDownAgent.agentId}` to force React unmount/remount on agent switch |
| 24 | Phase D3 plan (`phase-d3-realtime-subagent-progress.md`) independently designs same backend features — coordination risk | Warning | Added "Supersession Note" to Background section stating D2 supersedes D3 tasks 1-3 |
| 25 | Empty `agent_id` string passes `is_ascii_alphanumeric()` validation (empty iterator returns `true` in Rust) — reaches file resolution with `agent-.jsonl` | Minor (security) | Added `agent_id.is_empty()` check before alphanumeric validation |
