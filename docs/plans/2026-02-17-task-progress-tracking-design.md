---
status: draft
date: 2026-02-17
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

Three new finders in `crates/core/src/live_parser.rs`:

```rust
pub todo_write_key: memmem::Finder<'static>,   // b"\"name\":\"TodoWrite\""
pub task_create_key: memmem::Finder<'static>,   // b"\"name\":\"TaskCreate\""
pub task_update_key: memmem::Finder<'static>,   // b"\"name\":\"TaskUpdate\""
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

```rust
pub struct LiveLine {
    // ... existing fields ...

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

### Layer 2c: Extraction Logic in `parse_single_line`

**TodoWrite** (assistant lines, behind `todo_write_key` SIMD gate):
- Walk `message.content[]`, find `tool_use` with `name == "TodoWrite"`
- Extract `input.todos[]` → Vec of `RawTodoItem { content, status, active_form }`
- Only one TodoWrite per assistant line (find_map)

**TaskCreate** (assistant lines, behind `task_create_key` SIMD gate):
- Walk `message.content[]`, collect ALL `tool_use` blocks with `name == "TaskCreate"`
- Extract `id` (tool_use_id), `input.subject`, `input.description`, `input.activeForm`
- Vec because multiple tasks can be created in one message

**TaskUpdate** (assistant lines, behind `task_update_key` SIMD gate):
- Walk `message.content[]`, collect ALL `tool_use` blocks with `name == "TaskUpdate"`
- Extract `input.taskId`, `input.status`, `input.subject`, `input.activeForm`

**TaskIdAssignment** (user lines, behind existing `tool_use_result_key` SIMD gate):
- Check `parsed.toolUseResult.task.id` exists
- Get `tool_use_id` from `message.content[].tool_use_id` (on the tool_result block)
- Does NOT conflict with existing `sub_agent_result` extraction — different fields checked

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

New fields on `SessionAccumulator`:

```rust
/// Current todo items from the latest TodoWrite call.
/// FULL REPLACEMENT: each TodoWrite overwrites this entirely.
todo_items: Vec<ProgressItem>,

/// Structured tasks from TaskCreate/TaskUpdate.
/// INCREMENTAL: TaskCreate appends, TaskUpdate modifies in-place.
task_items: Vec<ProgressItem>,

/// Pending TaskCreate calls awaiting ID assignment.
/// Key: tool_use_id, Value: index into task_items.
pending_task_ids: HashMap<String, usize>,
```

Processing in the `for line in &new_lines` loop:

```rust
// TodoWrite: full replacement
if let Some(ref todos) = line.todo_write {
    acc.todo_items = todos.iter().map(|t| ProgressItem {
        id: None,
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

// TaskCreate: append with pending ID
for create in &line.task_creates {
    let idx = acc.task_items.len();
    acc.task_items.push(ProgressItem {
        id: None,  // assigned on tool_result
        title: create.subject.clone(),
        status: ProgressStatus::Pending,
        active_form: if create.active_form.is_empty() { None } else { Some(create.active_form.clone()) },
        source: ProgressSource::Task,
    });
    acc.pending_task_ids.insert(create.tool_use_id.clone(), idx);
}

// TaskIdAssignment: assign system ID to pending task
for assignment in &line.task_id_assignments {
    if let Some(idx) = acc.pending_task_ids.remove(&assignment.tool_use_id) {
        if let Some(task) = acc.task_items.get_mut(idx) {
            task.id = Some(assignment.task_id.clone());
        }
    }
}

// TaskUpdate: modify existing task by ID
for update in &line.task_updates {
    if let Some(task) = acc.task_items.iter_mut()
        .find(|t| t.id.as_deref() == Some(&update.task_id))
    {
        if let Some(ref status) = update.status {
            task.status = match status.as_str() {
                "in_progress" => ProgressStatus::InProgress,
                "completed" => ProgressStatus::Completed,
                "deleted" => ProgressStatus::Completed,
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

New field on `LiveSession` in `state.rs`:

```rust
/// Progress items (todos + tasks) for display on session cards.
/// Merged from TodoWrite and TaskCreate/TaskUpdate systems.
#[serde(skip_serializing_if = "Vec::is_empty")]
pub progress_items: Vec<vibe_recall_core::progress::ProgressItem>,
```

Built in `process_jsonl_update`:

```rust
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

New field on `LiveSession` in `use-live-sessions.ts`:

```typescript
progressItems?: ProgressItem[]
```

### Data Flow Diagram

```
JSONL line
  │
  ├─ SIMD: "TodoWrite" hit → parse input.todos → LiveLine.todo_write
  ├─ SIMD: "TaskCreate" hit → parse input.{subject,description,activeForm} → LiveLine.task_creates
  ├─ SIMD: "TaskUpdate" hit → parse input.{taskId,status,...} → LiveLine.task_updates
  └─ SIMD: "toolUseResult" hit + task.id present → LiveLine.task_id_assignments

LiveLine → SessionAccumulator
  ├─ todo_write → OVERWRITE acc.todo_items
  ├─ task_creates → APPEND acc.task_items + pending_task_ids map
  ├─ task_id_assignments → ASSIGN IDs via pending_task_ids lookup
  └─ task_updates → MODIFY acc.task_items by task ID

SessionAccumulator → LiveSession.progress_items = concat(todo_items, task_items)
  └─ SSE session_updated → Frontend LiveSession.progressItems
```

### Edge Cases

| Edge Case | Handling |
|-----------|----------|
| TodoWrite with 0 items | `acc.todo_items` becomes empty → clears the list |
| TaskCreate with no matching tool_result | Task stays with `id: None`, still displayed |
| TaskUpdate for unknown task ID | Silently ignored |
| Session replay from offset 0 | All historical calls replayed → correct final state |
| Multiple TodoWrites | Last one wins (full replacement) |
| `status: "deleted"` | Mapped to `Completed` |
| Sub-agent sessions | Not parsed (file watcher only watches parent dirs) |
| Session ends with running tasks | Tasks show last known status |

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
| `crates/core/src/progress.rs` | Wire types (ProgressItem, ProgressStatus, ProgressSource) + raw extraction structs |
| `src/components/live/TaskProgressList.tsx` | React component for rendering progress items |
| `src/components/live/TaskProgressList.test.tsx` | Tests for the React component |

### Modified Files
| File | Change |
|------|--------|
| `crates/core/src/lib.rs` | Add `pub mod progress;` |
| `crates/core/src/live_parser.rs` | Add 3 SIMD finders to TailFinders, 4 new fields to LiveLine, extraction logic in parse_single_line |
| `crates/server/src/live/manager.rs` | Add 3 fields to SessionAccumulator, processing logic in the new_lines loop |
| `crates/server/src/live/state.rs` | Add `progress_items` field to LiveSession, update `make_live_line` test helper |
| `src/components/live/use-live-sessions.ts` | Add `progressItems` to LiveSession interface |
| `src/components/live/SessionCard.tsx` | Conditional render TaskProgressList vs SessionSpinner |
