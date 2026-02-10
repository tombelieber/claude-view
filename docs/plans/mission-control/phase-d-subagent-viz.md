---
status: pending
date: 2026-02-10
phase: D
depends_on: C
---

# Phase D: Sub-Agent Visualization

> Parse sub-agent spawn/complete events from JSONL session files and render them as swim lanes, compact pills, and historical timeline views.

**Goal:** Give users real-time visibility into parallel sub-agent execution within a session. Show what each sub-agent is doing, how long it took, and how much it cost -- in both a detailed swim lane view and a compact pill summary for monitor grid panes.

**Depends on:** Phase C (Monitor Mode) -- sub-agent visualizations appear inside Monitor panes and can also be shown in session detail views.

---

## Background

Claude Code's Task tool spawns sub-agents for parallel work (code review, exploration, file search, etc.). These appear in JSONL as `tool_use` blocks with `name: "Task"` and input containing `description` and optional `subagent_type`. The corresponding `tool_result` block (matched by `tool_use_id`) carries the sub-agent's output and implicitly marks completion.

Key JSONL patterns:

```jsonl
// Sub-agent spawn (assistant message with tool_use)
{"type":"assistant","message":{"content":[{"type":"tool_use","id":"toolu_abc123","name":"Task","input":{"description":"Review the auth module for security issues","subagent_type":"code-reviewer"}}]}}

// Sub-agent completion (tool_result message)
{"type":"result","subtype":"tool_result","tool_use_id":"toolu_abc123","content":"Found 2 potential issues...","duration_ms":4200,"cost_usd":0.032}
```

Not all sessions have sub-agents. Many sessions have zero. Some power-user sessions spawn 5+ concurrent agents. The UI must handle all cases gracefully.

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
#[serde(rename_all = "snake_case")]
pub enum SubAgentStatus {
    Running,
    Complete,
    Error,
}

/// Information about a sub-agent spawned via the Task tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
pub struct SubAgentInfo {
    /// The tool_use_id from the spawning Task call. Used to match
    /// the tool_result that signals completion.
    pub tool_use_id: String,

    /// Agent type label extracted from `subagent_type` field.
    /// Examples: "Explore", "code-reviewer", "search", "edit-files".
    /// Falls back to "Task" if subagent_type is absent.
    pub agent_type: String,

    /// Human-readable description from the Task tool's `description` input.
    pub description: String,

    /// Current execution status.
    pub status: SubAgentStatus,

    /// Unix timestamp (seconds) when the sub-agent was spawned.
    pub started_at: i64,

    /// Unix timestamp (seconds) when the sub-agent completed or errored.
    /// None while status is Running.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<i64>,

    /// Duration in milliseconds (from the tool_result's duration_ms field).
    /// None while status is Running.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,

    /// Cost in USD attributed to this sub-agent's execution.
    /// None if cost data is not available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
}
```

Register this module in `crates/core/src/lib.rs`:
```rust
pub mod subagent;
```

### JSONL Parsing

**File to modify:** `crates/core/src/parser.rs`

Add sub-agent extraction to the existing JSONL line parser. Follow the project's SIMD pre-filter rule: use `memmem::Finder` to check for `"name":"Task"` before attempting JSON parse.

```rust
// Create finder ONCE at top of scan loop (per Rust Performance Rules)
let task_finder = memmem::Finder::new(b"\"name\":\"Task\"");

// Inside the per-line loop:
if task_finder.find(line).is_some() {
    // Only parse lines that contain Task tool_use
    if let Ok(v) = serde_json::from_slice::<Value>(line) {
        // Extract: tool_use_id, description, subagent_type, timestamp
        // Push SubAgentInfo { status: Running, ... } to session state
    }
}
```

For tool_result matching, also pre-filter with a finder for `"tool_result"` and match `tool_use_id` back to a pending sub-agent to mark it Complete or Error.

### LiveSession Extension

**File to modify:** The live session struct (created in Phase A/C).

Add a `sub_agents: Vec<SubAgentInfo>` field to the `LiveSession` struct. This vec is populated incrementally as the file watcher detects new JSONL lines.

Helper methods on LiveSession:
- `active_sub_agents() -> Vec<&SubAgentInfo>` -- filters to `status == Running`
- `sub_agent_total_cost() -> f64` -- sums `cost_usd` across all sub-agents
- `update_sub_agent(tool_use_id, status, completed_at, duration_ms, cost_usd)` -- called when a tool_result line is parsed

### SSE Events

**File to modify:** The live session SSE endpoint (created in Phase A/C).

Add two new SSE event types to the existing session progress stream:

| Event name | Payload | When emitted |
|------------|---------|--------------|
| `subagent_started` | `SubAgentInfo` (status=Running) | Task tool_use line detected |
| `subagent_completed` | `SubAgentInfo` (status=Complete/Error) | Matching tool_result line detected |

These events are emitted on the existing per-session SSE stream (`/api/live/sessions/{id}/events`). The frontend's EventSource hook (from Phase C) receives them alongside existing events.

### API Endpoint

**File to modify:** `crates/server/src/routes/` (live session routes from Phase A/C)

Ensure `GET /api/live/sessions/{id}` includes the `sub_agents` vec in its response. No new endpoint needed -- sub-agent data is part of the session object.

---

## Frontend

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
- Completed agents collapse to a single line: `[dot] [type] [description snippet] [cost] [duration]`
- Error agents show red dot + error indicator
- Rows sorted: Running agents first (by started_at), then completed (by completed_at desc)
- When `sessionActive` is false, all agents should be Complete/Error
- Empty state: no sub-agents renders nothing (component returns null)

**Styling:**
- Dark theme consistent with Phase C monitor panes
- Monospace font for duration/cost values
- Subtle horizontal separator between lanes
- Max height with scroll if more than 5 agents

#### 2. `src/components/live/SubAgentPills.tsx` -- Compact Pill Summary

Mini cards for display inside monitor grid panes (Phase C) where space is limited.

**Layout:**
```
[E 82%] [C 35%] [S done]    3 agents (2 active)
```

**Props:**
```tsx
interface SubAgentPillsProps {
  subAgents: SubAgentInfo[]
  /** Click handler to expand into swim lane view */
  onExpand?: () => void
}
```

**Behavior:**
- Each agent rendered as a small pill: `[type initial] [status]`
- Status inside pill: percentage if running (indeterminate shows spinner icon), "done" if complete, "err" if error
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
interface TimelineViewProps {
  subAgents: SubAgentInfo[]
  /** Session start time (unix seconds) for calculating offsets */
  sessionStartedAt: i64
  /** Total session duration for scaling the time axis */
  sessionDurationMs: number
}
```

**Behavior:**
- Horizontal time axis at top, scaled to session duration
- Each agent as a horizontal bar positioned by `started_at` offset, width by `duration_ms`
- Overlapping bars clearly show parallel execution
- Hover tooltip: agent type, description, duration, cost
- Color: green for complete, red for error
- If session is still active, running agents show animated right edge (growing bar)
- Time axis labels: "0s", "5s", "10s", etc. (adaptive intervals based on total duration)
- Useful for answering "where was time spent?" after a session completes

**Implementation notes:**
- Use SVG or CSS positioning (not a heavy chart library)
- Calculate bar positions as percentages of total duration for responsive scaling
- Min bar width of 2px so very short agents are still visible

#### 4. Sub-Agent Cost Attribution

**File to modify:** Session cost tooltip component (from Phase A/C)

Add a breakdown section to the session cost tooltip:

```
Session Cost: $0.42
├── Main agent:    $0.34
├── Explore:       $0.03
├── code-reviewer: $0.04
└── search:        $0.01
```

Only show the breakdown if the session has sub-agents with cost data.

### New Hook

**File to create:** `src/hooks/use-sub-agents.ts`

```tsx
import { useMemo } from 'react'
import type { SubAgentInfo } from '@/types/generated/SubAgentInfo'

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
    const totalCost = subAgents.reduce((sum, a) => sum + (a.cost_usd ?? 0), 0)
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

1. **Monitor Grid Pane (Phase C):** Each session pane shows `<SubAgentPills>` below the session status line. Clicking pills expands to `<SwimLanes>` within the pane.

2. **Session Detail View:** When viewing a specific session's conversation, show `<TimelineView>` as a collapsible section above the message list. Show `<SwimLanes>` for active sessions.

3. **SSE Event Handling:** The existing `useEventSource` hook (Phase C) receives `subagent_started` and `subagent_completed` events. Update the live session's `sub_agents` array in the React Query cache via `queryClient.setQueryData`.

---

## Files Summary

### New Files

| File | Purpose |
|------|---------|
| `crates/core/src/subagent.rs` | `SubAgentInfo`, `SubAgentStatus` types |
| `src/components/live/SwimLanes.tsx` | Expanded swim lane visualization |
| `src/components/live/SubAgentPills.tsx` | Compact pill summary for grid panes |
| `src/components/live/TimelineView.tsx` | Historical Gantt-like timeline |
| `src/hooks/use-sub-agents.ts` | Derived state helper hook |
| `src/components/live/SwimLanes.test.tsx` | Tests for swim lanes |
| `src/components/live/SubAgentPills.test.tsx` | Tests for compact pills |
| `src/components/live/TimelineView.test.tsx` | Tests for timeline view |
| `src/hooks/use-sub-agents.test.ts` | Tests for sub-agents hook |

### Modified Files

| File | Change |
|------|--------|
| `crates/core/src/lib.rs` | Add `pub mod subagent;` |
| `crates/core/src/parser.rs` | Add Task tool_use / tool_result extraction with SIMD pre-filter |
| Live session struct (Phase A/C) | Add `sub_agents: Vec<SubAgentInfo>` field |
| Live session SSE endpoint (Phase A/C) | Emit `subagent_started` / `subagent_completed` events |
| Monitor grid pane component (Phase C) | Integrate `<SubAgentPills>` |
| Session detail view | Integrate `<SwimLanes>` and `<TimelineView>` |
| Session cost tooltip (Phase A/C) | Add sub-agent cost breakdown |

### Dependencies

No new Rust crate dependencies. No new npm dependencies (timeline uses SVG/CSS, not a chart library).

---

## Testing Strategy

### Backend Tests

**Scope:** `cargo test -p vibe-recall-core -- subagent`

1. **JSONL parsing:** Feed sample JSONL lines through the parser, verify `SubAgentInfo` structs are extracted correctly
2. **SIMD pre-filter:** Verify lines without `"name":"Task"` are never JSON-parsed (benchmark with many non-matching lines)
3. **tool_result matching:** Verify `tool_use_id` correctly links spawn to completion
4. **Edge cases:**
   - Task tool_use without `subagent_type` field (should default to "Task")
   - tool_result with error status
   - tool_result with no matching spawn (should be ignored gracefully)
   - Multiple sub-agents with overlapping execution

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
   - Memoizes result (same input array = same output reference)

---

## Acceptance Criteria

- [ ] Sub-agent spawn/complete detected from JSONL within 2s of the line being written
- [ ] Swim lanes show correct parallel execution (overlapping Running agents)
- [ ] Compact pills show in monitor grid panes with accurate status
- [ ] Timeline view shows historical agent execution with correct time positioning
- [ ] Cost attributed correctly to sub-agents (sum matches session total)
- [ ] Handles sessions with 0 sub-agents gracefully (no empty containers, no errors)
- [ ] Handles sessions with 1 sub-agent (no "agents" plural when count is 1)
- [ ] Handles sessions with 5+ sub-agents (scroll in swim lanes, "+N more" in pills)
- [ ] SSE events arrive in real time without polling
- [ ] SIMD pre-filter skips non-Task lines (no unnecessary JSON parsing)
- [ ] All new components have passing tests
- [ ] TypeScript types generated from Rust via ts-rs (`SubAgentInfo`, `SubAgentStatus`)
