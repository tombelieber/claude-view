---
status: done
date: 2026-02-17
depends_on: derived-agent-state (Tasks 1-12 committed at 3aec59a..dead826)
---

# Task/Todo Progress Tracking on Live Session Cards

## Overview

Show Claude Code's task list (TodoWrite and TaskCreate/TaskUpdate progress) directly on live session cards in the Kanban view. This is critical for monitoring long-running sessions — users need to see at a glance what step the agent is on.

## Design Decisions (Agreed)

| Decision | Choice | Why |
|----------|--------|-----|
| Card detail | Full task list on card | Users need progress visibility without clicking |
| Visual style | CLI-style compact list | Match Claude Code TUI: `✓`/`◼`/`◻` icons, monospace font |
| Card position | Replace SessionSpinner row | When tasks exist; keep spinner as fallback when no tasks |
| Toggle mode | Compact/full per card | Compact = active + pending visible, completed collapsed to "… +N done"; Full = all tasks, no truncation |
| Display model | Unified list | Merge todos and tasks into single `progressItems` array |
| Data source | JSONL parsing | Consistent with existing architecture (not filesystem reads) |
| Scope | Parent session only | Sub-agent tasks visible in drill-down (future), not on card |

## Data Sources

Claude Code has **two** independent task tracking systems:

### 1. TodoWrite (flat checklist, full replacement)

**JSONL location:** Assistant message `content[]` with `name: "TodoWrite"`

```json
{
  "type": "tool_use",
  "id": "toolu_01UrJAXHo8B8E6YzpyYLmGET",
  "name": "TodoWrite",
  "input": {
    "todos": [
      {
        "content": "Review the code files",
        "status": "completed",
        "activeForm": "Reviewing code files"
      },
      {
        "content": "Write proposal.md",
        "status": "in_progress",
        "activeForm": "Writing proposal.md"
      },
      {
        "content": "Run tests",
        "status": "pending",
        "activeForm": "Running tests"
      }
    ]
  }
}
```

**Semantics:** Each TodoWrite call is a FULL REPLACEMENT of the entire list. No stable IDs — items identified by content/position. Status values: `"pending"`, `"in_progress"`, `"completed"`.

**tool_result:** Fixed string `"Todos have been modified successfully..."` (no useful data).

**toolUseResult (outer JSONL field):** `{ "oldTodos": [...], "newTodos": [...] }` — confirmed state after execution. We parse from the assistant line instead (faster, equivalent since TodoWrite always succeeds).

### 2. TaskCreate / TaskUpdate (structured tasks, incremental)

**TaskCreate — JSONL location:** Assistant message `content[]` with `name: "TaskCreate"`

```json
{
  "type": "tool_use",
  "id": "toolu_01SBRVrBvbYvJbLWJwrs581P",
  "name": "TaskCreate",
  "input": {
    "subject": "Implement core crate: cost.rs + live_parser.rs",
    "description": "Agent 1: Create cost calculator and incremental tail parser",
    "activeForm": "Implementing core crate modules"
  }
}
```

Tasks always start as `pending`. The system-assigned ID comes from the `toolUseResult` on the corresponding user line:

```json
"toolUseResult": {
  "task": { "id": "1", "subject": "Implement core crate: cost.rs + live_parser.rs" }
}
```

**TaskUpdate — JSONL location:** Assistant message `content[]` with `name: "TaskUpdate"`

```json
{
  "type": "tool_use",
  "id": "toolu_014Fqw1UUjJ8ySk5iQPS6RPb",
  "name": "TaskUpdate",
  "input": {
    "taskId": "1",
    "status": "in_progress"
  }
}
```

Optional fields: `subject`, `description`, `activeForm`, `addBlocks`, `addBlockedBy`.

### Multi-tool-call behavior (verified)

- Multiple `tool_use` blocks can appear in a single assistant message `content[]` array
- Each `tool_result` gets its own separate user-type JSONL line (never batched)
- `tool_use_id` links tool_use blocks to their corresponding tool_result lines

### Filesystem locations (context only, not used)

- `~/.claude/tasks/<session-uuid>/<N>.json` — one file per TaskCreate task
- `~/.claude/todos/<session-uuid>-agent-<agent-uuid>.json` — full array per session-agent

We parse from JSONL instead of filesystem for consistency with the existing live parser architecture.

## Data Pipeline

### Layer 1: SIMD Pre-Filters (`TailFinders`)

Three new finders added to `TailFinders` struct in `crates/core/src/live_parser.rs` (after `agent_progress_key` at line 117):

```rust
    pub todo_write_key: memmem::Finder<'static>,   // b"\"name\":\"TodoWrite\""
    pub task_create_key: memmem::Finder<'static>,   // b"\"name\":\"TaskCreate\""
    pub task_update_key: memmem::Finder<'static>,   // b"\"name\":\"TaskUpdate\""
```

In `TailFinders::new()` (after the `agent_progress_key` initializer at line 140):

```rust
            todo_write_key: memmem::Finder::new(b"\"name\":\"TodoWrite\""),
            task_create_key: memmem::Finder::new(b"\"name\":\"TaskCreate\""),
            task_update_key: memmem::Finder::new(b"\"name\":\"TaskUpdate\""),
```

Fire on ~1-2% of lines. False positives harmless (extra JSON field read returns None). Reuses existing `tool_use_result_key` for TaskIdAssignment on user lines.

### Layer 2: Intermediate Structs

New file: `crates/core/src/progress.rs`

```rust
/// Raw todo item extracted from TodoWrite tool_use input.
#[derive(Debug, Clone)]
pub struct RawTodoItem {
    pub content: String,
    pub status: String,       // "pending" | "in_progress" | "completed"
    pub active_form: String,
}

/// Raw task create extracted from TaskCreate tool_use input.
#[derive(Debug, Clone)]
pub struct RawTaskCreate {
    pub tool_use_id: String,  // links to tool_result for ID assignment
    pub subject: String,
    pub description: String,
    pub active_form: String,
}

/// Raw task update extracted from TaskUpdate tool_use input.
#[derive(Debug, Clone)]
pub struct RawTaskUpdate {
    pub task_id: String,      // system-assigned ID ("1", "2", ...)
    pub status: Option<String>,
    pub subject: Option<String>,
    pub active_form: Option<String>,
}

/// Task ID assignment extracted from toolUseResult on a user line.
#[derive(Debug, Clone)]
pub struct RawTaskIdAssignment {
    pub tool_use_id: String,  // from message.content[].tool_use_id
    pub task_id: String,      // from toolUseResult.task.id
}
```

### Layer 2b: New `LiveLine` Fields

Add 4 fields to `LiveLine` struct (after `sub_agent_progress` at `live_parser.rs:85`):

```rust
pub struct LiveLine {
    // ... existing fields through sub_agent_progress (line 85) ...

    /// Full replacement todo list from TodoWrite tool_use on this assistant line.
    pub todo_write: Option<Vec<RawTodoItem>>,

    /// TaskCreate calls on this assistant line (Vec: one message can create multiple tasks).
    pub task_creates: Vec<RawTaskCreate>,

    /// TaskUpdate calls on this assistant line.
    pub task_updates: Vec<RawTaskUpdate>,

    /// Task ID assignments from toolUseResult on this user line.
    pub task_id_assignments: Vec<RawTaskIdAssignment>,
}
```

**Also update every `LiveLine { ... }` construction site** (3 total):
1. Error-return fallback in `parse_single_line` (line ~250): add `todo_write: None, task_creates: Vec::new(), task_updates: Vec::new(), task_id_assignments: Vec::new(),`
2. Normal return in `parse_single_line` (line ~484): add the actual extracted values (see Layer 2c)
3. `make_test_line` helper in `manager.rs:1165`: add the same 4 default fields (see Compatibility section)

### Layer 2c: Extraction Logic in `parse_single_line`

All extraction code goes in `parse_single_line()` in `live_parser.rs`, inserted AFTER the sub-agent progress detection block (after line 482, before the final `LiveLine { ... }` construction at line 484). This parallels the existing sub-agent extraction pattern.

**TodoWrite** (assistant lines, behind `todo_write_key` SIMD gate):

```rust
    // --- TodoWrite detection (assistant lines with TodoWrite tool_use) ---
    let todo_write = if line_type == LineType::Assistant && finders.todo_write_key.find(raw).is_some() {
        msg.and_then(|m| m.get("content")).and_then(|c| c.as_array()).and_then(|blocks| {
            blocks.iter().find_map(|b| {
                if b.get("type").and_then(|t| t.as_str()) == Some("tool_use")
                    && b.get("name").and_then(|n| n.as_str()) == Some("TodoWrite")
                {
                    b.get("input").and_then(|i| i.get("todos")).and_then(|t| t.as_array()).map(|todos| {
                        todos.iter().filter_map(|item| {
                            Some(RawTodoItem {
                                content: item.get("content").and_then(|v| v.as_str())?.to_string(),
                                status: item.get("status").and_then(|v| v.as_str()).unwrap_or("pending").to_string(),
                                active_form: item.get("activeForm").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            })
                        }).collect::<Vec<_>>()
                    })
                } else {
                    None
                }
            })
        })
    } else {
        None
    };
```

**TaskCreate** (assistant lines, behind `task_create_key` SIMD gate):

```rust
    // --- TaskCreate detection (assistant lines with TaskCreate tool_use) ---
    let mut task_creates = Vec::new();
    if line_type == LineType::Assistant && finders.task_create_key.find(raw).is_some() {
        if let Some(content) = msg.and_then(|m| m.get("content")).and_then(|c| c.as_array()) {
            for block in content {
                if block.get("type").and_then(|t| t.as_str()) == Some("tool_use")
                    && block.get("name").and_then(|n| n.as_str()) == Some("TaskCreate")
                {
                    let tool_use_id = block.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let input = block.get("input");
                    if !tool_use_id.is_empty() {
                        task_creates.push(RawTaskCreate {
                            tool_use_id,
                            subject: input.and_then(|i| i.get("subject")).and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            description: input.and_then(|i| i.get("description")).and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            active_form: input.and_then(|i| i.get("activeForm")).and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        });
                    }
                }
            }
        }
    }
```

**TaskUpdate** (assistant lines, behind `task_update_key` SIMD gate):

```rust
    // --- TaskUpdate detection (assistant lines with TaskUpdate tool_use) ---
    let mut task_updates = Vec::new();
    if line_type == LineType::Assistant && finders.task_update_key.find(raw).is_some() {
        if let Some(content) = msg.and_then(|m| m.get("content")).and_then(|c| c.as_array()) {
            for block in content {
                if block.get("type").and_then(|t| t.as_str()) == Some("tool_use")
                    && block.get("name").and_then(|n| n.as_str()) == Some("TaskUpdate")
                {
                    let input = block.get("input");
                    let task_id = input.and_then(|i| i.get("taskId")).and_then(|v| v.as_str()).unwrap_or("").to_string();
                    if !task_id.is_empty() {
                        task_updates.push(RawTaskUpdate {
                            task_id,
                            status: input.and_then(|i| i.get("status")).and_then(|v| v.as_str()).map(String::from),
                            subject: input.and_then(|i| i.get("subject")).and_then(|v| v.as_str()).map(String::from),
                            active_form: input.and_then(|i| i.get("activeForm")).and_then(|v| v.as_str()).map(String::from),
                        });
                    }
                }
            }
        }
    }
```

**TaskIdAssignment** (user lines, behind existing `tool_use_result_key` SIMD gate):

```rust
    // --- TaskIdAssignment detection (user lines with toolUseResult containing task.id) ---
    // Shares the tool_use_result_key SIMD gate with sub_agent_result but extracts different fields.
    // Does NOT conflict: sub_agent_result looks for agentId/status/totalDurationMs,
    // this looks for task.id — mutually exclusive tool types.
    let mut task_id_assignments = Vec::new();
    if line_type == LineType::User && finders.tool_use_result_key.find(raw).is_some() {
        if let Some(task_id) = parsed.get("toolUseResult")
            .and_then(|tur| tur.get("task"))
            .and_then(|t| t.get("id"))
            .and_then(|v| v.as_str())
        {
            // Get tool_use_id from the tool_result block in content
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
                });
            if let Some(tool_use_id) = tool_use_id {
                task_id_assignments.push(RawTaskIdAssignment {
                    tool_use_id,
                    task_id: task_id.to_string(),
                });
            }
        }
    }
```

Then update the final `LiveLine { ... }` construction (currently at line 484) to include the new fields:

```rust
    LiveLine {
        // ... existing fields unchanged ...
        sub_agent_spawns,
        sub_agent_result,
        sub_agent_progress,
        // NEW: task progress fields
        todo_write,
        task_creates,
        task_updates,
        task_id_assignments,
    }
```

### Layer 3: Wire Types

```rust
// crates/core/src/progress.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "snake_case")]
pub enum ProgressStatus {
    Pending,
    InProgress,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "snake_case")]
pub enum ProgressSource {
    Todo,
    Task,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ProgressItem {
    /// For tasks: system-assigned ID ("1", "2", "3").
    /// For todos: None (no stable ID in TodoWrite).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    /// Internal dedup key for replay resilience. Matches the `tool_use_id`
    /// from the TaskCreate tool_use block. Not serialized to frontend.
    /// For todos: None (TodoWrite uses full-replacement semantics).
    #[serde(skip)]
    pub tool_use_id: Option<String>,

    /// Display text. From `content` (TodoWrite) or `subject` (TaskCreate).
    pub title: String,

    /// Current status.
    pub status: ProgressStatus,

    /// Present-continuous verb for spinner display (e.g., "Running tests").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_form: Option<String>,

    /// Which system produced this item.
    pub source: ProgressSource,
}
```

Follows the `SubAgentInfo` pattern: `#[derive(TS)]` with `#[ts(export)]` auto-generates TypeScript types.

### Layer 4: Accumulator State (`manager.rs`)

> **Resolved against derived agent state model** (committed at `3aec59a..dead826`).
> The old `handle_status_change` no longer exists. `process_jsonl_update` now calls
> `handle_transitions()` + `derive_agent_state()` (free functions, not methods).

**New fields on `SessionAccumulator`** (after `sub_agents` at `manager.rs:74`):

```rust
    /// Current todo items from the latest TodoWrite call.
    /// FULL REPLACEMENT: each TodoWrite overwrites this entirely.
    todo_items: Vec<ProgressItem>,

    /// Structured tasks from TaskCreate/TaskUpdate.
    /// INCREMENTAL: TaskCreate appends (with dedup guard), TaskUpdate modifies in-place.
    /// Deleted tasks are removed entirely (not mapped to Completed).
    task_items: Vec<ProgressItem>,
```

Add defaults in `SessionAccumulator::new()` (after `sub_agents: Vec::new()` at line 103):

```rust
            todo_items: Vec::new(),
            task_items: Vec::new(),
```

**Offset rollback guard** — insert between `acc.offset = new_offset` (line 724) and the `for line in &new_lines` loop (line 726):

```rust
        // Detect file replacement: offset rollback means file was replaced.
        // Clear task progress to prevent duplicates on replay from offset 0.
        // TodoWrite is naturally idempotent (full replacement); only task_items needs reset.
        if new_offset > 0 && new_offset < current_offset {
            tracing::info!(
                session_id = %session_id,
                old_offset = current_offset,
                new_offset = new_offset,
                "File replaced — clearing task progress for clean re-accumulation"
            );
            acc.task_items.clear();
            // todo_items: no reset needed — next TodoWrite will overwrite entirely
        }
```

**Task progress processing** — insert inside the `for line in &new_lines` loop, AFTER the sub-agent progress tracking block (after line 873, before the closing `}` of the loop at line 874). This follows the existing pattern: sub-agent spawn → sub-agent completion → sub-agent progress → **task progress**:

```rust
            // --- TodoWrite: full replacement (naturally idempotent on replay) ---
            if let Some(ref todos) = line.todo_write {
                acc.todo_items = todos.iter().map(|t| ProgressItem {
                    id: None,
                    tool_use_id: None, // TodoWrite items have no stable tool_use_id
                    title: t.content.clone(),
                    status: match t.status.as_str() {
                        "in_progress" => ProgressStatus::InProgress,
                        "completed" => ProgressStatus::Completed,
                        _ => ProgressStatus::Pending,
                    },
                    active_form: if t.active_form.is_empty() { None } else { Some(t.active_form.clone()) },
                    source: ProgressSource::Todo,
                }).collect();
            }

            // --- TaskCreate: append with dedup guard ---
            // Mirrors sub-agent spawn dedup guard at line 802:
            //   `if acc.sub_agents.iter().any(|a| a.tool_use_id == spawn.tool_use_id) { continue; }`
            for create in &line.task_creates {
                if acc.task_items.iter().any(|t| t.tool_use_id.as_deref() == Some(&create.tool_use_id)) {
                    continue; // Already processed — replay after offset reset or catch-up scan
                }
                acc.task_items.push(ProgressItem {
                    id: None,  // assigned on tool_result
                    tool_use_id: Some(create.tool_use_id.clone()),
                    title: create.subject.clone(),
                    status: ProgressStatus::Pending,
                    active_form: if create.active_form.is_empty() { None } else { Some(create.active_form.clone()) },
                    source: ProgressSource::Task,
                });
            }

            // --- TaskIdAssignment: assign system ID by tool_use_id scan ---
            for assignment in &line.task_id_assignments {
                if let Some(task) = acc.task_items.iter_mut()
                    .find(|t| t.tool_use_id.as_deref() == Some(&assignment.tool_use_id))
                {
                    task.id = Some(assignment.task_id.clone());
                }
                // No matching tool_use_id: ignore gracefully (orphaned assignment)
            }

            // --- TaskUpdate: modify existing task by system ID ---
            for update in &line.task_updates {
                // Deleted tasks are removed, not mapped to Completed.
                // Prevents phantom items inflating the "+N done" count.
                if update.status.as_deref() == Some("deleted") {
                    acc.task_items.retain(|t| t.id.as_deref() != Some(&update.task_id));
                    continue;
                }
                if let Some(task) = acc.task_items.iter_mut()
                    .find(|t| t.id.as_deref() == Some(&update.task_id))
                {
                    if let Some(ref status) = update.status {
                        task.status = match status.as_str() {
                            "in_progress" => ProgressStatus::InProgress,
                            "completed" => ProgressStatus::Completed,
                            _ => task.status,
                        };
                    }
                    if let Some(ref subject) = update.subject {
                        task.title = subject.clone();
                    }
                    if let Some(ref active_form) = update.active_form {
                        task.active_form = Some(active_form.clone());
                    }
                }
                // Unknown task_id: ignore gracefully (orphaned update)
            }
```

### Layer 5: LiveSession + SSE

New field on `LiveSession` in `state.rs` (after `sub_agents` at line 115):

```rust
    /// Progress items (todos + tasks) for display on session cards.
    /// Merged from TodoWrite and TaskCreate/TaskUpdate systems.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub progress_items: Vec<vibe_recall_core::progress::ProgressItem>,
```

Built in `process_jsonl_update` inside the `LiveSession { ... }` construction block (after `sub_agents: acc.sub_agents.clone()` at `manager.rs:981`):

```rust
            sub_agents: acc.sub_agents.clone(),
            // NEW: merge todo_items + task_items into unified progress list
            progress_items: {
                let mut items = acc.todo_items.clone();
                items.extend(acc.task_items.iter().cloned());
                items
            },
```

### Layer 6: Frontend Types

Auto-generated by `ts-rs`:

```typescript
// src/types/generated/ProgressItem.ts (auto-generated)
export type ProgressStatus = "pending" | "in_progress" | "completed";
export type ProgressSource = "todo" | "task";
export interface ProgressItem {
  id?: string;
  title: string;
  status: ProgressStatus;
  activeForm?: string;
  source: ProgressSource;
}
```

New field on `LiveSession` interface in `use-live-sessions.ts` (after `subAgents` at line 44):

```typescript
  progressItems?: ProgressItem[]
```

Import the generated type at the top of `use-live-sessions.ts`:

```typescript
import type { ProgressItem } from '../../types/generated/ProgressItem'
```

### Data Flow Diagram

```
JSONL line
  │
  ├─ SIMD: "TodoWrite" hit → parse input.todos → LiveLine.todo_write
  ├─ SIMD: "TaskCreate" hit → parse input.{subject,description,activeForm} → LiveLine.task_creates
  ├─ SIMD: "TaskUpdate" hit → parse input.{taskId,status,...} → LiveLine.task_updates
  └─ SIMD: "toolUseResult" hit + task.id present → LiveLine.task_id_assignments

Pre-processing (before line loop):
  └─ offset rollback detected → CLEAR acc.task_items (file replacement guard)

LiveLine → SessionAccumulator
  ├─ todo_write → OVERWRITE acc.todo_items (naturally idempotent)
  ├─ task_creates → APPEND acc.task_items (with tool_use_id dedup guard)
  ├─ task_id_assignments → ASSIGN IDs via tool_use_id scan (no index map)
  ├─ task_updates (deleted) → REMOVE from acc.task_items
  └─ task_updates (other) → MODIFY acc.task_items by system ID

SessionAccumulator → LiveSession.progress_items = concat(todo_items, task_items)
  └─ SSE session_updated → Frontend LiveSession.progressItems
```

### Edge Cases

| Edge Case | Handling |
|-----------|----------|
| TodoWrite with 0 items | `acc.todo_items` becomes empty → clears the list |
| TaskCreate with no matching tool_result | Task stays with `id: None`, still displayed |
| TaskUpdate for unknown task ID | Silently ignored (orphaned update) |
| Session replay from offset 0 | Dedup guard skips already-seen TaskCreates; TodoWrite overwrites |
| Multiple TodoWrites | Last one wins (full replacement — naturally idempotent) |
| `status: "deleted"` | Task **removed** from `task_items` entirely (not mapped to Completed) |
| Sub-agent sessions | Not parsed (file watcher only watches parent dirs) |
| Session ends with running tasks | Tasks show last known status (frozen) |
| **File replacement (TOCTOU)** | `task_items` cleared on offset rollback; clean re-accumulation from offset 0 |
| **Duplicate TaskCreate on replay** | Dedup'd by `tool_use_id` — same pattern as sub-agent spawn guard (`manager.rs:802`) |
| **Watcher channel overflow** | Catch-up scan (committed at `dead826`) re-reads file; dedup prevents duplicates |
| **TaskIdAssignment before TaskCreate** | Impossible in JSONL (assistant writes before user response); no guard needed |
| **TaskCreate in crashed session** | Task shows as Pending with no system ID — acceptable degraded state |

### Robustness Properties

This design matches the production-grade robustness of the Derived Agent State model (committed at `3aec59a`). Both systems share the same live JSONL pipeline and must survive the same failure modes.

| Property | Agent State (derived model) | Task Progress (this design) | How parity is achieved |
|----------|----------------------------|----------------------------|----------------------|
| **Replay resilience** | Pure function — re-derive on every call | TodoWrite: full replacement (idempotent). TaskCreate: `tool_use_id` dedup guard (mirrors sub-agent spawn dedup at `manager.rs:802`) | Both survive offset-0 replay without corruption |
| **File replacement (TOCTOU)** | Stateless — unaffected | `task_items.clear()` on offset rollback detection (before line loop) | Both recover from file rotation/truncation |
| **No index-based linkage** | N/A | TaskIdAssignment uses `tool_use_id` scan, not positional index into Vec | Eliminates `pending_task_ids` HashMap — indices can't go stale |
| **Watcher overflow recovery** | Catch-up scan re-derives state (Task 6 committed at `dead826`) | Catch-up scan re-reads file; dedup guard prevents duplicate TaskCreates | Both survive dropped file events |
| **Deleted items** | N/A | Removed from Vec entirely (not mapped to Completed) | No phantom items inflate "+N done" count |
| **No unbounded accumulation** | N/A | Deleted tasks removed; stale tasks cleared on file replacement | Memory bounded by actual active task count |

**Design choice: accumulator vs. pure derivation.** Agent state uses a pure function (`derive_agent_state()` at `manager.rs:135`) because classification depends only on current evidence (last line, status, recent messages). Task progress requires history — you need all past TaskCreate/TodoWrite calls to reconstruct the full list. An accumulator (stateful fold over the JSONL event stream) is the correct model. The robustness gap is closed by applying the **same dedup/reset patterns** already proven in sub-agent tracking (`manager.rs:800-803`).

### Compatibility with Derived Agent State (Already Committed)

The derived agent state model (Tasks 1-12) is already committed. This plan adds 4 new fields to `LiveLine`. The `make_test_line` helper at `manager.rs:1159` must be updated:

```rust
    fn make_test_line(
        line_type: LineType,
        tool_names: Vec<String>,
        stop_reason: Option<&str>,
        is_tool_result: bool,
    ) -> LiveLine {
        LiveLine {
            // ... existing fields unchanged (lines 1166-1183) ...
            sub_agent_spawns: Vec::new(),
            sub_agent_result: None,
            sub_agent_progress: None,
            // NEW: task progress defaults (zero-cost)
            todo_write: None,
            task_creates: Vec::new(),
            task_updates: Vec::new(),
            task_id_assignments: Vec::new(),
        }
    }
```

These are zero-cost defaults (`None`/empty Vec). No existing tests are affected — the new fields are `Option`/`Vec` which default to empty. All 9 existing `derive_agent_state` tests and 6 `handle_transitions` tests continue to pass unchanged.

## React Component: `TaskProgressList`

### Visual Treatment

CLI-style, matching Claude Code TUI:

```
◼ Implementing core crate modules          ← in_progress (amber, animated)
◻ Write integration tests                  ← pending (gray)
◻ Run full test suite                      ← pending (gray)
… +3 done                                  ← collapsed completed (compact mode)
```

Icons: `◼` (in_progress), `◻` (pending), `✓` (completed). Monospace font. Single-line per item, truncated with ellipsis.

### Compact vs Full Mode

- **Compact** (default): Show `in_progress` + `pending` items. Completed items collapsed to `"… +N done"` row. Click `+N done` to expand.
- **Full**: All items shown, no truncation. Toggle via small button on the task list header.

Toggle state is per-card, stored in component local state.

### Integration into SessionCard

```tsx
{/* Spinner row OR Task progress */}
<div className="mb-2">
  {session.progressItems && session.progressItems.length > 0 ? (
    <TaskProgressList items={session.progressItems} />
  ) : (
    <SessionSpinner
      mode="live"
      durationSeconds={elapsedSeconds}
      // ... existing props
    />
  )}
</div>
```

### Component Props

```typescript
interface TaskProgressListProps {
  items: ProgressItem[]
}
```

## Files to Create/Modify

### New Files
| File | Purpose |
|------|---------|
| `crates/core/src/progress.rs` | Wire types (ProgressItem, ProgressStatus, ProgressSource) + raw extraction structs (RawTodoItem, RawTaskCreate, RawTaskUpdate, RawTaskIdAssignment) |
| `src/components/live/TaskProgressList.tsx` | React component for rendering progress items |
| `src/components/live/TaskProgressList.test.tsx` | Tests for the React component |

### Modified Files
| File | Change | Lines to touch |
|------|--------|---------------|
| `crates/core/src/lib.rs` | Add `pub mod progress;` | After line 23 (`pub mod subagent;`) |
| `crates/core/src/live_parser.rs` | Add 3 SIMD finders to `TailFinders` (after line 117), 4 new fields to `LiveLine` (after line 85), extraction logic in `parse_single_line` (after line 482, before line 484), update 2 `LiveLine` construction sites | ~4 insertion points |
| `crates/server/src/live/manager.rs` | Add 2 fields to `SessionAccumulator` (after line 74 + line 103), offset rollback guard (between lines 724-726), task progress processing in line loop (after line 873), `progress_items` in `LiveSession` construction (after line 981), update `make_test_line` (line 1165) | ~5 insertion points |
| `crates/server/src/live/state.rs` | Add `progress_items` field to `LiveSession` (after line 115) | 1 insertion point |
| `src/components/live/use-live-sessions.ts` | Add `progressItems` to `LiveSession` interface (after line 44), add import | 2 insertion points |
| `src/components/live/SessionCard.tsx` | Conditional render TaskProgressList vs SessionSpinner | ~1 replacement |
