---
status: done
date: 2026-02-10
phase: D
depends_on: C
audited: 2026-02-16
progress: 8/8 tasks complete (2026-02-16)
completed: 2026-02-16
---

# Phase D: Sub-Agent Visualization

> Parse sub-agent spawn/complete events from JSONL session files and render them as swim lanes, compact pills, and historical timeline views.

**Goal:** Give users real-time visibility into parallel sub-agent execution within a session. Show what each sub-agent is doing, how long it took, and how much it cost -- in both a detailed swim lane view and a compact pill summary for monitor grid panes.

**Depends on:** Phase C (Monitor Mode) -- sub-agent visualizations appear inside Monitor panes and can also be shown in session detail views.

---

## Background

Claude Code's Task tool spawns sub-agents for parallel work (code review, exploration, file search, etc.). These appear in JSONL as `tool_use` blocks with `name: "Task"` and input containing `description`, `prompt`, and optional `subagent_type`. The corresponding `tool_result` block (matched by `tool_use_id`) on a subsequent `type: "user"` line carries the sub-agent's output. A sibling top-level `toolUseResult` object on that same line carries completion metadata (status, duration, token usage).

**Key JSONL patterns (verified against real session files 2026-02-16):**

```jsonl
// Sub-agent spawn (type: "assistant", tool_use block with name: "Task")
{"type":"assistant","message":{"model":"claude-opus-4-5-20251101","content":[{"type":"tool_use","id":"toolu_01CrLm51sUzQGf8Ctwy29qBa","name":"Task","input":{"description":"Find unknown JSONL type handling","prompt":"I need to understand...","subagent_type":"Explore"}}],"usage":{"input_tokens":3,"output_tokens":10,"cache_read_input_tokens":39761}},"timestamp":"2026-02-02T16:34:34.219Z"}

// Sub-agent completion (type: "user", tool_result block + toolUseResult metadata)
{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"toolu_01CrLm51sUzQGf8Ctwy29qBa","content":[{"type":"text","text":"Found the parsing flow..."}]}]},"toolUseResult":{"agentId":"a33bda6","status":"completed","totalDurationMs":32212,"totalTokens":64085,"totalToolUseCount":11,"content":"Found the parsing flow...","prompt":"I need to understand...","usage":{"input_tokens":2,"output_tokens":6,"cache_read_input_tokens":63248}},"timestamp":"2026-02-02T16:35:06.431Z"}
```

**Important format details:**
- Spawn lines are `type: "assistant"` with `tool_use` blocks nested in `message.content[]`
- Completion lines are `type: "user"` (NOT `type: "result"`) with `tool_result` blocks in `message.content[]`
- Completion metadata lives in a **top-level** `toolUseResult` object (NOT inside the tool_result block)
- `toolUseResult.agentId` is a 7-character short hash (e.g., `"a33bda6"`) that matches the sub-agent's JSONL filename
- `toolUseResult.totalToolUseCount` is the number of tool calls the sub-agent made (useful for UI display)
- Duration is in `toolUseResult.totalDurationMs` (NOT `duration_ms`)
- Cost is NOT a field -- must be **computed** from `toolUseResult.usage` token counts using the pricing table
- The `input` field on spawn contains `prompt` (full instructions) in addition to `description` (short label) and optional `subagent_type`

Not all sessions have sub-agents. Many sessions have zero. Some power-user sessions spawn 5+ concurrent agents. The UI must handle all cases gracefully.

### Complete `toolUseResult` Schema (verified 2026-02-16)

The `toolUseResult` object on completion lines has MORE fields than the basic examples above show. Complete verified schema:

```json
{
  "agentId": "a33bda6",
  "status": "completed",
  "totalDurationMs": 462379,
  "totalTokens": 61635,
  "totalToolUseCount": 42,
  "content": "...(sub-agent's final response text)...",
  "prompt": "...(original prompt sent to sub-agent)...",
  "usage": {
    "input_tokens": 1,
    "output_tokens": 760,
    "cache_creation_input_tokens": 88,
    "cache_read_input_tokens": 60786,
    "server_tool_use": { "web_search_requests": 0, "web_fetch_requests": 0 },
    "service_tier": "standard",
    "speed": "standard"
  }
}
```

Key fields for Phase D:
- `agentId`: 7-character short hash (e.g., `"a33bda6"`). Matches the `agent-{id}.jsonl` filename in the subagents directory.
- `totalToolUseCount`: Number of tool calls the sub-agent made. Useful for UI display (e.g., "42 tool calls").
- `content`: The sub-agent's final response text (can be long, truncate for display).
- `prompt`: The original prompt sent to the sub-agent (same as `input.prompt` on spawn line).
- `slug`: Human-readable name (e.g., `"snappy-tumbling-scott"`) — present on sub-agent JSONL lines, NOT on `toolUseResult`.

### Filesystem Architecture (verified 2026-02-16)

Claude Code stores sub-agent data at **two levels**: embedded in the parent JSONL (spawn/completion events) AND as separate JSONL files in a `subagents/` directory. Phase D MVP uses the parent JSONL approach; the subagents directory enables future deep-dive features.

```
~/.claude/projects/{encoded-project-path}/
├── {sessionId}.jsonl                          ← Parent JSONL (tool_use + toolUseResult lines)
├── {sessionId}/
│   ├── subagents/
│   │   ├── agent-a33bda6.jsonl                ← Sub-agent's own full JSONL conversation
│   │   ├── agent-a3d82a3.jsonl                ← agentId = 7-char short hash
│   │   └── agent-acompact-a2c307.jsonl        ← "compact" variant (context-compressed sub-agent)
│   └── tool-results/
│       └── toolu_01KPJc8hLzQj56DJcUdYRb3F.txt ← Large tool stdout saved to disk
└── ...
```

**Sub-agent JSONL line structure** (every line in `agent-{id}.jsonl`):
```json
{
  "agentId": "a33bda6",
  "sessionId": "c916410d-da69-4a71-800f-35cca5301d8a",
  "isSidechain": true,
  "type": "user|assistant",
  "slug": "snappy-tumbling-scott",
  "timestamp": "2026-02-16T08:34:13.134Z",
  "cwd": "/path/to/project",
  "gitBranch": "feature/...",
  "version": "2.1.42",
  "message": { "role": "user|assistant", "content": "..." }
}
```

**Additional data locations** (informational, not used by Phase D MVP):
- `~/.claude/tasks/{sessionId}/{N}.json` — TaskCreate structured tasks with dependency graphs
- `~/.claude/todos/{sessionId}-agent-{agentId}.json` — Legacy per-agent todos (mostly empty `{"todos":[]}`)
- `~/.claude/projects/{project}/{sessionId}/tool-results/` — Large tool output files (referenced by `tool_use_id`)

**Progress events (verified 2026-02-16):**

Between spawn and completion, the parent JSONL contains `type: "progress"` lines with `data.type: "agent_progress"`. These carry the `agentId` (short hash) and `parentToolUseID` (linking back to the spawn's `tool_use_id`):

```json
{
  "type": "progress",
  "parentToolUseID": "toolu_011ewdGwdYzHEU9Twb59itFZ",
  "toolUseID": "agent_msg_016e1sRpgpdjjzhBjZa3qj1v",
  "data": {
    "type": "agent_progress",
    "agentId": "a951849",
    "prompt": "...",
    "message": { "role": "assistant", "content": [...] }
  }
}
```

This means `agentId` is available BEFORE completion (from the first progress event). The MVP plan populates `agent_id` on completion only (simpler), but a future enhancement could extract it from progress events for earlier cross-referencing.

**Confirmed constraint — flat hierarchy (no sub-sub-agents):** Sub-agents do NOT spawn their own sub-agents. Verified across 20 JSONL files: zero instances of `name: "Task"` inside progress events. The hierarchy is always exactly one level deep (parent → sub-agent). No recursive tracking needed.

**Phase D approach:**
- **MVP (this plan):** Parse parent JSONL only (tool_use spawns + toolUseResult completions). This gives all the data needed for swim lanes, pills, timeline, and cost attribution.
- **Future enhancement (Phase D.2):** Watch `subagents/` directory with `notify` for real-time sub-agent progress (line-by-line streaming of sub-agent work). This would enable "drill down into sub-agent" with its own message stream. Progress events in the parent JSONL could also provide real-time activity display (showing which tool the sub-agent is currently using).

---

## Backend

### New Types

**File to create:** `crates/core/src/subagent.rs`

```rust
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Status of a sub-agent within a live session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub enum SubAgentStatus {
    Running,
    Complete,
    Error,
}

/// Information about a sub-agent spawned via the Task tool.
///
/// Note: ts-rs exports i64 as TypeScript `number` (safe for Unix timestamps).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct SubAgentInfo {
    /// The tool_use_id from the spawning Task call. Used to match
    /// the tool_result that signals completion.
    pub tool_use_id: String,

    /// 7-character short hash agent identifier from `toolUseResult.agentId`.
    /// Matches the `agent-{id}.jsonl` filename in the subagents directory.
    /// None while status is Running (only available on completion).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,

    /// Agent type label extracted from `subagent_type` field.
    /// Examples: "Explore", "code-reviewer", "search", "edit-files".
    /// Falls back to "Task" if subagent_type is absent.
    pub agent_type: String,

    /// Human-readable description from the Task tool's `description` input.
    pub description: String,

    /// Current execution status.
    pub status: SubAgentStatus,

    /// Unix timestamp (seconds) when the sub-agent was spawned.
    /// Parsed from the ISO 8601 `timestamp` field on the JSONL line
    /// via `chrono::DateTime::parse_from_rfc3339`.
    pub started_at: i64,

    /// Unix timestamp (seconds) when the sub-agent completed or errored.
    /// None while status is Running.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<i64>,

    /// Duration in milliseconds from `toolUseResult.totalDurationMs`.
    /// None while status is Running.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,

    /// Number of tool calls the sub-agent made, from `toolUseResult.totalToolUseCount`.
    /// None while status is Running.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_use_count: Option<u32>,

    /// Cost in USD attributed to this sub-agent's execution.
    /// Computed from `toolUseResult.usage` token counts via the pricing table.
    /// None while status is Running or if pricing data unavailable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
}
```

Register this module in `crates/core/src/lib.rs` (add after the `pub mod tail;` line at line 22):
```rust
pub mod subagent;
```

Note: Do NOT add `pub use subagent::*;`. The newer modules (`cost`, `live_parser`, `tail`) use qualified imports, not glob re-exports. Consumers import as `claude_view_core::subagent::{SubAgentInfo, SubAgentStatus}`.

### JSONL Parsing

**File to modify:** `crates/core/src/live_parser.rs`

Two changes needed:

#### 1. Add SIMD finders to `TailFinders` struct (line ~58)

Add two new finders to the existing `TailFinders` struct. These are created once at startup and shared across all session polls (per Rust Performance Rules):

```rust
// Add to TailFinders struct fields (after existing fields at line 70):
pub task_name_key: memmem::Finder<'static>,
pub tool_use_result_key: memmem::Finder<'static>,

// Add to TailFinders::new() (after existing initializations at line 87):
task_name_key: memmem::Finder::new(b"\"name\":\"Task\""),
tool_use_result_key: memmem::Finder::new(b"\"toolUseResult\""),
```

#### 2. Extend `LiveLine` struct (line ~20)

Add sub-agent parsing fields to `LiveLine`. These are populated by `parse_single_line` when the line contains Task tool_use or toolUseResult data:

```rust
// Add to LiveLine struct (after existing fields):

/// If this assistant line contains a Task tool_use, the spawn info.
/// Vec because a single assistant message can spawn multiple sub-agents.
pub sub_agent_spawns: Vec<SubAgentSpawn>,

/// If this user line has a `toolUseResult` (Task completion), the result info.
pub sub_agent_result: Option<SubAgentResult>,
```

**CRITICAL: Update ALL `LiveLine` construction sites** after adding these fields. There are 3 sites that construct `LiveLine` with explicit fields:

1. **JSON parse error fallback** (`live_parser.rs` ~line 173): Add `sub_agent_spawns: Vec::new(), sub_agent_result: None,` to the early-return `LiveLine`.
2. **Normal construction** (`live_parser.rs` ~line 281): Add the computed `sub_agent_spawns` and `sub_agent_result` variables (from step 3 below).
3. **`make_live_line` test helper** (`crates/server/src/live/state.rs` ~line 237, `LiveLine` struct body at ~line 242): Add `sub_agent_spawns: Vec::new(), sub_agent_result: None,` to the struct literal.

Without updating all 3 sites, the code will not compile ("missing field" errors).

Add supporting types (above or below LiveLine):

```rust
/// Extracted from a Task tool_use block on an assistant line.
#[derive(Debug, Clone)]
pub struct SubAgentSpawn {
    pub tool_use_id: String,
    pub agent_type: String,
    pub description: String,
}

/// Extracted from a toolUseResult on a user line (Task completion).
#[derive(Debug, Clone)]
pub struct SubAgentResult {
    pub tool_use_id: String,
    /// 7-char short hash from `toolUseResult.agentId` (e.g., "a33bda6").
    pub agent_id: Option<String>,
    pub status: String,  // "completed", "error", etc.
    pub total_duration_ms: Option<u64>,
    /// Number of tool calls from `toolUseResult.totalToolUseCount`.
    pub total_tool_use_count: Option<u32>,
    pub usage_input_tokens: Option<u64>,
    pub usage_output_tokens: Option<u64>,
    pub usage_cache_read_tokens: Option<u64>,
    pub usage_cache_creation_tokens: Option<u64>,
}
```

#### 3. Extract sub-agent data in `parse_single_line` (line ~152)

Insert this code **immediately before the final `LiveLine { ... }` construction** (before line 281). The variables `sub_agent_spawns` and `sub_agent_result` must be in scope when the `LiveLine` is built:

```rust
// --- Sub-agent spawn detection (assistant lines with Task tool_use) ---
let mut sub_agent_spawns = Vec::new();
if line_type == LineType::Assistant && finders.task_name_key.find(raw).is_some() {
    // Already have `parsed` from JSON parse above
    if let Some(content) = msg.and_then(|m| m.get("content")).and_then(|c| c.as_array()) {
        for block in content {
            if block.get("type").and_then(|t| t.as_str()) == Some("tool_use")
                && block.get("name").and_then(|n| n.as_str()) == Some("Task")
            {
                let tool_use_id = block.get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let input = block.get("input");
                let description = input
                    .and_then(|i| i.get("description"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let agent_type = input
                    .and_then(|i| i.get("subagent_type"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("Task")
                    .to_string();
                if !tool_use_id.is_empty() {
                    sub_agent_spawns.push(SubAgentSpawn {
                        tool_use_id,
                        agent_type,
                        description,
                    });
                }
            }
        }
    }
}

// --- Sub-agent completion detection (user lines with toolUseResult) ---
let sub_agent_result = if line_type == LineType::User
    && finders.tool_use_result_key.find(raw).is_some()
{
    // Extract toolUseResult from top-level (NOT inside message.content)
    parsed.get("toolUseResult").and_then(|tur| {
        // Find the matching tool_use_id from the tool_result block in content
        let tool_use_id = msg
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_array())
            .and_then(|blocks| {
                blocks.iter().find_map(|b| {
                    if b.get("type").and_then(|t| t.as_str()) == Some("tool_result") {
                        b.get("tool_use_id").and_then(|v| v.as_str()).map(String::from)
                    } else {
                        None
                    }
                })
            })?;

        let agent_id = tur.get("agentId")
            .and_then(|v| v.as_str())
            .map(String::from);
        let status = tur.get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("completed")
            .to_string();
        let total_duration_ms = tur.get("totalDurationMs").and_then(|v| v.as_u64());
        let total_tool_use_count = tur.get("totalToolUseCount")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);
        let usage = tur.get("usage");
        Some(SubAgentResult {
            tool_use_id,
            agent_id,
            status,
            total_duration_ms,
            total_tool_use_count,
            usage_input_tokens: usage.and_then(|u| u.get("input_tokens")).and_then(|v| v.as_u64()),
            usage_output_tokens: usage.and_then(|u| u.get("output_tokens")).and_then(|v| v.as_u64()),
            usage_cache_read_tokens: usage.and_then(|u| u.get("cache_read_input_tokens")).and_then(|v| v.as_u64()),
            usage_cache_creation_tokens: usage.and_then(|u| u.get("cache_creation_input_tokens")).and_then(|v| v.as_u64()),
        })
    })
} else {
    None
};
```

### LiveSession Extension

**File to modify:** `crates/server/src/live/state.rs` (lines 62-106)

Add a `sub_agents` field to the `LiveSession` struct. Since `LiveSession` uses `#[serde(rename_all = "camelCase")]`, this serializes as `subAgents` for the frontend:

```rust
// Add to LiveSession struct (after cache_status field, line 105):
/// Sub-agents spawned via the Task tool in this session.
/// Empty vec if no sub-agents have been detected.
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub sub_agents: Vec<claude_view_core::subagent::SubAgentInfo>,
```

**CRITICAL: Update ALL `LiveSession` construction sites** after adding this field. There are 2 sites:

1. **Manager snapshot** (`crates/server/src/live/manager.rs` ~line 712): Add `sub_agents: acc.sub_agents.clone(),` (covered below in manager.rs section).
2. **Test helper** (`crates/server/src/routes/terminal.rs` ~line 865): Add `sub_agents: Vec::new(),` after the `cache_status` field in the test `LiveSession` construction.

Without updating both sites, the code will not compile ("missing field" errors).

**File to modify:** `crates/server/src/live/manager.rs`

First, add the required import (after existing `use claude_view_core::...` imports at ~line 16-18):
```rust
use claude_view_core::subagent::{SubAgentInfo, SubAgentStatus};
```

Add sub-agent tracking to the `SessionAccumulator` private struct:

```rust
// Add to SessionAccumulator struct (after recent_messages field, line 65):
/// Sub-agents spawned in this session (accumulated across tail polls).
sub_agents: Vec<claude_view_core::subagent::SubAgentInfo>,
```

Initialize in `SessionAccumulator::new()`:
```rust
sub_agents: Vec::new(),
```

Add sub-agent processing to the `process_jsonl_update` method (inside the `for line in &new_lines` loop, after existing token/model/user tracking):

```rust
// --- Sub-agent spawn tracking ---
for spawn in &line.sub_agent_spawns {
    // Parse timestamp from the JSONL line to get started_at
    let started_at = line.timestamp.as_deref()
        .and_then(parse_timestamp_to_unix)
        .unwrap_or(0);
    acc.sub_agents.push(SubAgentInfo {
        tool_use_id: spawn.tool_use_id.clone(),
        agent_id: None, // populated on completion from toolUseResult.agentId
        agent_type: spawn.agent_type.clone(),
        description: spawn.description.clone(),
        status: SubAgentStatus::Running,
        started_at,
        completed_at: None,
        duration_ms: None,
        tool_use_count: None,
        cost_usd: None,
    });
}

// --- Sub-agent completion tracking ---
if let Some(ref result) = line.sub_agent_result {
    if let Some(agent) = acc.sub_agents.iter_mut()
        .find(|a| a.tool_use_id == result.tool_use_id)
    {
        agent.status = if result.status == "completed" {
            SubAgentStatus::Complete
        } else {
            SubAgentStatus::Error
        };
        agent.agent_id = result.agent_id.clone();
        agent.completed_at = line.timestamp.as_deref()
            .and_then(parse_timestamp_to_unix);
        agent.duration_ms = result.total_duration_ms;
        agent.tool_use_count = result.total_tool_use_count;
        // Compute cost from token usage via pricing table
        if let Some(model) = acc.model.as_deref() {
            let sub_tokens = TokenUsage {
                input_tokens: result.usage_input_tokens.unwrap_or(0),
                output_tokens: result.usage_output_tokens.unwrap_or(0),
                cache_read_tokens: result.usage_cache_read_tokens.unwrap_or(0),
                cache_creation_tokens: result.usage_cache_creation_tokens.unwrap_or(0),
                total_tokens: 0, // not used by calculate_live_cost
            };
            let sub_cost = calculate_live_cost(&sub_tokens, Some(model), &self.pricing);
            if sub_cost.total_usd > 0.0 {
                agent.cost_usd = Some(sub_cost.total_usd);
            }
        }
    }
    // If no matching spawn found, ignore gracefully (orphaned tool_result)
}
```

Then include sub_agents when building the LiveSession snapshot (inside the `LiveSession { ... }` construction, after the `cache_status` field at ~line 712):
```rust
// Add to the LiveSession builder (after cache_status):
sub_agents: acc.sub_agents.clone(),
```

**Orphaned sub-agent cleanup:** In the `handle_status_change` method, when a session transitions to `Done` (~line 811), mark any still-Running sub-agents as Error:

```rust
// Add inside the `if new_status == SessionStatus::Done` block (after setting completed_at):
for agent in &mut acc.sub_agents {
    if agent.status == SubAgentStatus::Running {
        agent.status = SubAgentStatus::Error;
        agent.completed_at = acc.completed_at.map(|t| t as i64);
    }
}
```

Without this, completed sessions would show perpetually-spinning sub-agents in the UI.

**No helper methods needed on LiveSession.** The frontend filters/sums sub-agents directly. The accumulation and update logic lives entirely in the manager.

### SSE Events

**No new SSE event types needed.**

The existing architecture broadcasts full `LiveSession` snapshots as `session_updated` events on the global SSE stream at `/api/live/stream` (see `crates/server/src/routes/live.rs:40`). When sub-agent data changes (spawn detected or completion parsed), the entire `LiveSession` -- including the updated `subAgents` array -- is broadcast automatically via the existing `SessionEvent::SessionUpdated` variant.

The frontend's `useLiveSessions` hook (`src/components/live/use-live-sessions.ts`) already replaces the full session object on `session_updated` events, so sub-agent data will flow to components without any SSE changes.

### API Endpoint

**No changes needed.**

`GET /api/live/sessions/:id` already returns the full `LiveSession` struct (see `crates/server/src/routes/live.rs:166`). Once `sub_agents` is added to the struct, it's automatically included in the response. The list endpoint (`GET /api/live/sessions`) also returns full sessions.

---

## Frontend

### TypeScript Type Update

**File to modify:** `src/components/live/use-live-sessions.ts`

Add the `subAgents` field to the `LiveSession` TypeScript interface (matching the Rust struct's camelCase serialization):

```tsx
// Add to LiveSession interface (after cacheStatus):
subAgents?: SubAgentInfo[]
```

The `SubAgentInfo` and `SubAgentStatus` types will be auto-generated by ts-rs into `src/types/generated/`. Import them:

```tsx
import type { SubAgentInfo } from '../../types/generated/SubAgentInfo'
```

### New Components

#### 1. `src/components/live/SwimLanes.tsx` -- Expanded Sub-Agent View

The primary visualization for sub-agent execution within a session detail or expanded monitor pane.

**Layout:**
```
+----------------------------------------------------------+
| Main Agent: "Waiting for 3 agents..."                    |
+----------------------------------------------------------+
| [green dot] Explore     | Reviewing auth module... | $0.03 |  ████████████░░  82%
| [green dot] code-review | Checking test coverage   | $0.01 |  █████░░░░░░░░░  35%
| [gray dot]  search      | Found 12 matches         | $0.02 |  done (2.1s)
+----------------------------------------------------------+
```

**Props:**
```tsx
import type { SubAgentInfo } from '../../types/generated/SubAgentInfo'

interface SwimLanesProps {
  subAgents: SubAgentInfo[]
  /** Whether the parent session is still active */
  sessionActive: boolean
}
```

**Behavior:**
- Each sub-agent renders as a horizontal row (swim lane)
- Status dot: green=Running, gray=Complete, red=Error
- Running agents show an animated progress bar (indeterminate, since we don't have percentage data)
- Completed agents collapse to a single line: `[dot] [type] [description snippet] [cost] [duration] [N tool calls]`
- Error agents show red dot + error indicator
- Rows sorted: Running agents first (by startedAt), then completed (by completedAt desc)
- When `sessionActive` is false, all agents should be Complete/Error
- Empty state: no sub-agents renders nothing (component returns null)

**Styling:**
- Consistent with existing monitor panes (Tailwind classes, dark palette used in Monitor Mode)
- Monospace font for duration/cost values
- Subtle horizontal separator between lanes
- Max height with scroll if more than 5 agents

#### 2. `src/components/live/SubAgentPills.tsx` -- Compact Pill Summary

Mini cards for display inside monitor grid panes (Phase C) where space is limited.

**Layout:**
```
[E ⟳] [C ⟳] [S done]    3 agents (2 active)
```

**Props:**
```tsx
import type { SubAgentInfo } from '../../types/generated/SubAgentInfo'

interface SubAgentPillsProps {
  subAgents: SubAgentInfo[]
  /** Click handler to expand into swim lane view */
  onExpand?: () => void
}
```

**Behavior:**
- Each agent rendered as a small pill: `[type initial] [status]`
- Status inside pill: spinner icon if running, "done" if complete, "err" if error
- Color coding: green border=running, neutral border=complete, red border=error
- Summary text: "N agents (M active)" or "N agents (all done)"
- Entire row is clickable (calls `onExpand`) to switch to swim lane view
- If 0 sub-agents, component returns null
- If more than 4 agents, show first 3 pills + "+N more" pill

#### 3. `src/components/live/TimelineView.tsx` -- Historical Gantt Chart

A Gantt-like visualization for reviewing completed sessions. Shows when each sub-agent ran relative to the session timeline.

**Layout:**
```
Time:  0s     5s     10s    15s    20s    25s
       |------|------|------|------|------|
Main   ███████████████████████████████████████
Explore       ████████████
code-rev            ██████████████
search  ████████
```

**Props:**
```tsx
import type { SubAgentInfo } from '../../types/generated/SubAgentInfo'

interface TimelineViewProps {
  subAgents: SubAgentInfo[]
  /** Session start time (unix seconds) for calculating offsets */
  sessionStartedAt: number
  /** Total session duration for scaling the time axis */
  sessionDurationMs: number
}
```

**Behavior:**
- Horizontal time axis at top, scaled to session duration
- Each agent as a horizontal bar positioned by `startedAt` offset, width by `durationMs`
- Overlapping bars clearly show parallel execution
- Hover tooltip: agent type, description, duration, cost
- Color: green for complete, red for error
- If session is still active, running agents show animated right edge (growing bar)
- Time axis labels: "0s", "5s", "10s", etc. (adaptive intervals based on total duration)
- Useful for answering "where was time spent?" after a session completes

**Implementation notes:**
- Use CSS positioning (`position: relative` container, `position: absolute` bars with `left`/`width` percentages). No chart library or SVG needed.
- Calculate bar positions as percentages of total duration for responsive scaling
- Min bar width of 2px so very short agents are still visible

#### 4. Sub-Agent Cost Attribution

**File to modify:** `src/components/live/CostTooltip.tsx`

Add a `subAgents` prop to `CostTooltipProps`:

```tsx
import type { SubAgentInfo } from '../../types/generated/SubAgentInfo'

interface CostTooltipProps {
  cost: { ... }  // existing
  cacheStatus: 'warm' | 'cold' | 'unknown'  // existing
  subAgents?: SubAgentInfo[]  // NEW
  children: ReactNode  // existing
}
```

When `subAgents` is non-empty and any have `costUsd`, render a breakdown section:

```
Session Cost: $0.42
├── Main agent:    $0.34
├── Explore:       $0.03
├── code-reviewer: $0.04
└── search:        $0.01
```

Update the existing `SessionCard` `CostTooltip` call to pass `session.subAgents`. `MonitorPane` does not currently use `CostTooltip`, so no prop wiring is needed there unless you introduce a new tooltip usage.

### New Hook

**File to create:** `src/components/live/use-sub-agents.ts`

(Located alongside other live hooks, matching existing convention.)

```tsx
import { useMemo } from 'react'
import type { SubAgentInfo } from '../../types/generated/SubAgentInfo'

interface UseSubAgentsResult {
  all: SubAgentInfo[]
  active: SubAgentInfo[]
  completed: SubAgentInfo[]
  errored: SubAgentInfo[]
  totalCost: number
  activeCount: number
  isAnyRunning: boolean
}

export function useSubAgents(subAgents: SubAgentInfo[]): UseSubAgentsResult {
  return useMemo(() => {
    const active = subAgents.filter(a => a.status === 'running')
    const completed = subAgents.filter(a => a.status === 'complete')
    const errored = subAgents.filter(a => a.status === 'error')
    const totalCost = subAgents.reduce((sum, a) => sum + (a.costUsd ?? 0), 0)
    return {
      all: subAgents,
      active,
      completed,
      errored,
      totalCost,
      activeCount: active.length,
      isAnyRunning: active.length > 0,
    }
  }, [subAgents])
}
```

### Integration Points

1. **Monitor Grid Pane (`src/components/live/MonitorPane.tsx`):** Replace the static sub-agent placeholder in the footer (line 359-363, marked `{/* Sub-agent placeholder (Phase D) */}`) with `<SubAgentPills>`. Pass `onExpand` through to `Footer`, then wire `onExpand` into `SubAgentPills` so clicking pills opens the expanded pane.

2. **Expanded Overlay Composition (`src/components/live/MonitorView.tsx`):** `ExpandedPaneOverlay` is a chrome-only wrapper (header + close + children). Keep `ExpandedPaneOverlay` mostly unchanged, and wrap the overlay children in `MonitorView.tsx` with a flex column that renders `SwimLanes` above the terminal stream:
   ```tsx
   <ExpandedPaneOverlay ...>
     <div className="flex flex-col h-full">
       {expandedSession.subAgents?.length > 0 && (
         <SwimLanes
           subAgents={expandedSession.subAgents}
           sessionActive={expandedSession.status === 'working'}
         />
       )}
       <RichTerminalPane ... />
     </div>
   </ExpandedPaneOverlay>
   ```
   Show `<TimelineView>` as a collapsible section for completed sessions.

3. **Session Card (`src/components/live/SessionCard.tsx`):** Show a sub-agent count indicator when `subAgents` is non-empty, and pass `subAgents` into `<CostTooltip>`.

4. **SSE Data Flow:** Sub-agent data flows through the existing `useLiveSessions` hook automatically. The `session_updated` SSE event carries the full `LiveSession` including `subAgents`. No new event listeners needed. No React Query cache manipulation needed.

---

## Files Summary

### New Files

| File | Purpose |
|------|---------|
| `crates/core/src/subagent.rs` | `SubAgentInfo` (with `agent_id`, `tool_use_count`), `SubAgentStatus` types (camelCase serde) |
| `src/components/live/SwimLanes.tsx` | Expanded swim lane visualization |
| `src/components/live/SubAgentPills.tsx` | Compact pill summary for grid panes |
| `src/components/live/TimelineView.tsx` | Historical Gantt-like timeline |
| `src/components/live/use-sub-agents.ts` | Derived state helper hook |
| `src/components/live/SwimLanes.test.tsx` | Tests for swim lanes |
| `src/components/live/SubAgentPills.test.tsx` | Tests for compact pills |
| `src/components/live/TimelineView.test.tsx` | Tests for timeline view |
| `src/components/live/use-sub-agents.test.ts` | Tests for sub-agents hook |

### Modified Files

| File | Change |
|------|--------|
| `crates/core/src/lib.rs` | Add `pub mod subagent;` (no glob re-export, use qualified imports) |
| `crates/core/src/live_parser.rs` | Add `task_name_key` + `tool_use_result_key` finders to `TailFinders`; add `sub_agent_spawns` + `sub_agent_result` fields to `LiveLine`; add extraction logic to `parse_single_line` |
| `crates/server/src/live/state.rs` | Add `sub_agents: Vec<SubAgentInfo>` field to `LiveSession` |
| `crates/server/src/live/manager.rs` | Add `sub_agents: Vec<SubAgentInfo>` to `SessionAccumulator`; add spawn/completion tracking in `process_jsonl_update` loop; include in `LiveSession` snapshot |
| `src/components/live/use-live-sessions.ts` | Add `subAgents?: SubAgentInfo[]` to `LiveSession` TypeScript interface |
| `src/components/live/MonitorPane.tsx` | Integrate `<SubAgentPills>` |
| `src/components/live/MonitorView.tsx` | Compose expanded overlay content with `<SwimLanes>`, `<TimelineView>`, and `RichTerminalPane` |
| `crates/server/src/routes/terminal.rs` | Add `sub_agents: Vec::new()` to test `LiveSession` construction (~line 892) |
| `src/components/live/SessionCard.tsx` | Pass `subAgents` to `CostTooltip` and render sub-agent count indicator |
| `src/components/live/CostTooltip.tsx` | Add sub-agent cost breakdown UI |

### Dependencies

No new Rust crate dependencies. No new npm dependencies (timeline uses CSS positioning, not a chart library).

---

## Testing Strategy

### Backend Tests

**Scope:**
- `cargo test -p claude-view-core -- subagent`
- `cargo test -p claude-view-core -- live_parser`
- `cargo check -p claude-view-server` (verifies all `LiveSession` construction sites compile with the new field)
- `bun run typecheck`
- `bun run test:client -- src/components/live/SwimLanes.test.tsx src/components/live/SubAgentPills.test.tsx src/components/live/TimelineView.test.tsx src/components/live/use-sub-agents.test.ts`

1. **JSONL parsing -- Task spawn detection:**
   - Feed an assistant line with `name: "Task"` through `parse_single_line`, verify `sub_agent_spawns` is populated
   - Verify `tool_use_id`, `agent_type`, `description` are extracted correctly
   - Feed a line with multiple Task tool_use blocks, verify all are extracted
   - Feed a non-Task tool_use line (e.g., `name: "Bash"`), verify `sub_agent_spawns` is empty

2. **JSONL parsing -- Task completion detection:**
   - Feed a user line with `toolUseResult`, verify `sub_agent_result` is populated
   - Verify `tool_use_id` is extracted from the `tool_result` block in content
   - Verify `agent_id` is extracted from `toolUseResult.agentId` (7-char short hash)
   - Verify `total_duration_ms` from `toolUseResult.totalDurationMs`
   - Verify `total_tool_use_count` from `toolUseResult.totalToolUseCount`
   - Verify `usage_*` token fields from `toolUseResult.usage`
   - Feed a user line without `toolUseResult`, verify `sub_agent_result` is None

3. **SIMD pre-filter:** Verify lines without `"name":"Task"` skip the spawn extraction path entirely. Verify lines without `"toolUseResult"` skip the completion extraction path.

4. **Edge cases:**
   - Task tool_use without `subagent_type` field (should default to "Task")
   - Task tool_use without `description` field (should default to "")
   - `toolUseResult` with status "error" vs "completed"
   - `toolUseResult` with missing `usage` (should produce None for token fields)
   - `toolUseResult` with missing `agentId` (should produce None — older Claude Code versions may not have it)
   - `toolUseResult` with missing `totalToolUseCount` (should produce None)
   - `toolUseResult` with no matching `tool_result` block in content (should produce None)
   - Multiple sub-agents spawned in a single assistant message

### Frontend Tests

1. **SwimLanes:**
   - Renders nothing when `subAgents` is empty
   - Running agents sorted before completed
   - Status dot colors match agent status
   - Completed agents show duration and cost

2. **SubAgentPills:**
   - Returns null for empty array
   - Shows "+N more" when > 4 agents
   - Summary text shows correct active/total counts
   - Calls `onExpand` on click

3. **TimelineView:**
   - Bar widths proportional to duration
   - Overlapping agents visually overlap
   - Hover tooltip shows correct data
   - Handles single-agent and multi-agent cases

4. **useSubAgents hook:**
   - Correctly partitions agents by status
   - Sums cost correctly, treats null cost as 0
   - Returns stable reference when called with the same array instance within a single render

5. **Integration checks:**
   - `MonitorPane` footer renders `SubAgentPills` when `subAgents` exists and wires click-through to `onExpand`
   - Expanded overlay (in `MonitorView.tsx`) renders `SwimLanes` above terminal output and conditionally shows `TimelineView` for completed sessions
   - `SessionCard` passes `subAgents` to `CostTooltip` and displays sub-agent count indicator

---

## Acceptance Criteria

- [ ] Sub-agent spawn/complete detected from JSONL within 2s of the line being written
- [ ] Swim lanes show correct parallel execution (overlapping Running agents)
- [ ] Compact pills show in monitor grid panes with accurate status
- [ ] Timeline view shows historical agent execution with correct time positioning
- [ ] Cost computed correctly from token usage via pricing table (NOT from a `cost_usd` field)
- [ ] Handles sessions with 0 sub-agents gracefully (no empty containers, no errors)
- [ ] Handles sessions with 1 sub-agent (no "agents" plural when count is 1)
- [ ] Handles sessions with 5+ sub-agents (scroll in swim lanes, "+N more" in pills)
- [ ] Sub-agent data flows through existing SSE `session_updated` events (no new event types)
- [ ] SIMD pre-filter in TailFinders skips non-Task lines (no unnecessary JSON parsing)
- [ ] All new components have passing tests
- [ ] TypeScript types generated from Rust via ts-rs (`SubAgentInfo`, `SubAgentStatus`)
- [ ] After creating `subagent.rs`, run `cargo test -p claude-view-core` to trigger ts-rs generation; verify `src/types/generated/SubAgentInfo.ts` and `src/types/generated/SubAgentStatus.ts` exist

---

## Changelog of Fixes Applied (Audit 2026-02-16)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | JSONL format completely wrong (showed `type:"result"` with `duration_ms`/`cost_usd`) | Blocker | Rewrote Background section with actual JSONL format verified against real session files. Completion is `type:"user"` with `toolUseResult` metadata. |
| 2 | Wrong parser file (`parser.rs` instead of `live_parser.rs`) | Blocker | Changed all references to `crates/core/src/live_parser.rs`. Updated parsing section with actual struct names (`TailFinders`, `LiveLine`, `parse_single_line`). |
| 3 | `cost_usd` doesn't exist on tool_result | Blocker | Documented that cost must be COMPUTED from `toolUseResult.usage` token counts via `calculate_live_cost()`. Updated SubAgentInfo doc comments. |
| 4 | Serde `snake_case` instead of `camelCase` | Blocker | Changed `#[serde(rename_all = "snake_case")]` to `#[serde(rename_all = "camelCase")]` on both `SubAgentStatus` and `SubAgentInfo`. |
| 5 | Per-session SSE endpoint doesn't exist | Blocker | Removed entire "SSE Events" section proposing `subagent_started`/`subagent_completed` events. Documented that existing `session_updated` broadcasts carry full LiveSession including sub-agents. |
| 6 | Proposed 2 new SSE event types unnecessarily | Blocker | Same as #5 -- removed. Explained existing data flow through `useLiveSessions` hook. |
| 7 | Accumulator state vs LiveSession snapshot confusion | Warning | Separated: tracking logic goes in `SessionAccumulator` (manager.rs), final vec goes in `LiveSession` (state.rs). Removed helper methods from LiveSession. |
| 8 | No `useEventSource` hook exists | Warning | Replaced references to nonexistent hook with actual `useLiveSessions` hook. Clarified sub-agent data flows automatically through existing SSE infrastructure. |
| 9 | `duration_ms` field naming wrong (`totalDurationMs`) | Warning | Updated `SubAgentResult` to use `total_duration_ms` from `toolUseResult.totalDurationMs`. |
| 10 | tool_result matching approach wrong (wrong pre-filter) | Warning | Changed pre-filter from `"tool_result"` to `"toolUseResult"` (top-level field). Showed extraction from `parsed.get("toolUseResult")`. |
| 11 | `LiveLine` struct needs sub-agent fields | Warning | Added `sub_agent_spawns: Vec<SubAgentSpawn>` and `sub_agent_result: Option<SubAgentResult>` fields with supporting types. |
| 12 | `input` has `prompt` not just `description` | Warning | Documented in Background section. Only extract `description` and `subagent_type` (prompt is too long for display). |
| 13 | Hook file location inconsistent | Minor | Changed from `src/hooks/use-sub-agents.ts` to `src/components/live/use-sub-agents.ts` matching existing convention. |
| 14 | `started_at` timestamp source not explained | Minor | Added doc comment explaining ISO 8601 → Unix seconds parsing via `chrono::DateTime::parse_from_rfc3339`. |
| 15 | Missing `pub use subagent::*;` in lib.rs | Blocker | Added both `pub mod subagent;` and `pub use subagent::*;` to match existing pattern. |

**Adversarial review fixes (round 2):**

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 16 | `LiveLine` JSON-parse error fallback missing new fields | Blocker | Added explicit note listing all 3 `LiveLine` construction sites that must be updated (line 173 fallback, line 281 normal, state.rs test helper). |
| 17 | `make_live_line` test helper in `state.rs` missing new fields | Blocker | Included in issue #16 — test helper at state.rs ~line 242 must add `sub_agent_spawns: Vec::new(), sub_agent_result: None`. |
| 18 | Missing `use claude_view_core::subagent::{SubAgentInfo, SubAgentStatus};` import in manager.rs | Important | Added explicit import instruction in the manager.rs modification section. |
| 19 | Ambiguous insertion point for sub-agent extraction code | Minor | Changed to "Insert immediately before the final `LiveLine { ... }` construction (before line 281)". |
| 20 | Orphaned Running sub-agents never cleaned up on session Done | Important | Added cleanup logic in `handle_status_change` to mark Running sub-agents as Error when session transitions to Done. |
| 21 | `CostTooltip` prop interface change not specified | Important | Added `subAgents?: SubAgentInfo[]` prop to `CostTooltipProps` with parent integration note. |

**Adversarial review fixes (round 3, score 88→94):**

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 22 | Missing `LiveSession` construction site in `terminal.rs:865` | Blocker | Added "CRITICAL: Update ALL LiveSession construction sites" section listing both manager.rs:712 and terminal.rs:865. Added terminal.rs to Modified Files table. |
| 23 | `pub use subagent::*;` doesn't match newer module pattern | Important | Removed glob re-export. Added note explaining newer modules (cost, live_parser, tail) use qualified imports. |
| 24 | Test scope doesn't cover `claude-view-server` compilation | Important | Added `cargo check -p claude-view-server` to testing scope. |

**Polish fixes (round 4, score 94→100):**

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 25 | "Dark theme" phrasing was imprecise | Minor | Changed to "Consistent with existing monitor panes (dark palette used in Monitor Mode)". |
| 26 | TimelineView says "SVG or CSS" — should recommend CSS | Minor | Changed to explicit CSS positioning recommendation. |
| 27 | MonitorPane integration says "add" but should "replace" static placeholder | Minor | Changed to "Replace the static sub-agent placeholder in the footer (line 359-363)". |
| 28 | ExpandedPaneOverlay is chrome-only wrapper — no layout code shown | Minor | Added flex column wrapper code showing SwimLanes above RichPane. |
| 29 | `useSubAgents` memo test claims cross-render stability | Minor | Changed assertion to "stable reference within a single render" (SSE updates create new arrays). |
| 30 | `make_live_line` line reference slightly off (fn at 237, struct at 242) | Minor | Added both line numbers for clarity. |
| 31 | Missing ts-rs verification step | Minor | Added acceptance criterion to run `cargo test` and verify generated `.ts` files exist. |

**Forensic audit fixes (round 5, `~/.claude/` directory discovery):**

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 32 | `toolUseResult` schema incomplete — missing `agentId`, `totalToolUseCount`, `content`, `prompt` fields | Blocker | Added "Complete `toolUseResult` Schema" section with all 8 verified fields. Updated JSONL example to include `agentId` and `totalToolUseCount`. |
| 33 | `SubAgentInfo` missing `agent_id` field for sub-agent JSONL cross-reference | Blocker | Added `agent_id: Option<String>` (7-char short hash from `toolUseResult.agentId`). |
| 34 | `SubAgentInfo` missing `tool_use_count` field | Important | Added `tool_use_count: Option<u32>` from `toolUseResult.totalToolUseCount`. |
| 35 | `SubAgentResult` missing `agent_id` and `total_tool_use_count` fields | Blocker | Added both fields to `SubAgentResult` struct with extraction code in `parse_single_line`. |
| 36 | No documentation of `subagents/` directory (sub-agent JSONL files) | Important | Added "Filesystem Architecture" section showing directory tree, sub-agent JSONL line structure, and MVP vs future enhancement approach. |
| 37 | No documentation of `tool-results/` directory | Minor | Documented in filesystem architecture as additional data location. |
| 38 | Manager completion tracking doesn't populate `agent_id` or `tool_use_count` | Important | Updated manager.rs code to set `agent.agent_id = result.agent_id.clone()` and `agent.tool_use_count = result.total_tool_use_count` on completion. |
| 39 | Manager spawn tracking doesn't initialize new `SubAgentInfo` fields | Warning | Added `agent_id: None, tool_use_count: None` to the `SubAgentInfo` push in spawn tracking. |
| 40 | SwimLanes completed display missing tool use count | Minor | Updated completed line format to include `[N tool calls]`. |
| 41 | Test cases missing `agentId` and `totalToolUseCount` verification | Important | Added test cases for `agent_id`, `total_tool_use_count` extraction and missing-field edge cases. |
| 42 | Missing documentation of progress events as `agentId` source | Important | Added "Progress events" section showing `parentToolUseID` + `agentId` linkage. Noted MVP uses completion-only approach, progress extraction is future enhancement. |
| 43 | No documentation of hierarchy depth constraint | Warning | Added "Confirmed constraint — flat hierarchy" note: sub-agents never spawn sub-agents, verified across 20 JSONL files. No recursive tracking needed. |

**Mechanical audit fixes (round 6, final hardening pass):**

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 44 | `sub_agent_result` parser path not explicitly gated to user lines | Important | Updated code snippet to gate completion extraction with `line_type == LineType::User` plus `"toolUseResult"` finder check. |
| 45 | CostTooltip integration incorrectly required `MonitorPane` prop wiring | Important | Clarified that only `SessionCard` currently uses `CostTooltip`; removed incorrect `MonitorPane` requirement. |
| 46 | Expanded overlay integration targeted wrong file/component (`ExpandedPaneOverlay` + `RichPane`) | Blocker | Moved integration instructions to `MonitorView.tsx` composition point and updated snippet to use `RichTerminalPane` with `expandedSession`. |
| 47 | SessionCard integration was optional/ambiguous | Warning | Changed to required behavior: show sub-agent count and pass `subAgents` into `CostTooltip`. |
| 48 | Modified-files table omitted `MonitorView.tsx` and overstated `ExpandedPaneOverlay` changes | Warning | Updated file matrix to include `MonitorView.tsx` and `SessionCard.tsx` as concrete integration files. |
| 49 | Verification section lacked explicit frontend command sequence | Important | Added `bun run typecheck` and a targeted `vitest` command for all new frontend tests. |
| 50 | Dependency note inconsistent with CSS-only timeline recommendation | Minor | Updated wording to "CSS positioning" only (no SVG/chart library). |
| 51 | `LiveLine` section described new fields as "optional" even though `sub_agent_spawns` is required `Vec` | Minor | Reworded to "Add sub-agent parsing fields" to match the actual struct definitions. |
