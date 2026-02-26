# Claude Code Message Types — Complete Reference

> Definitive catalog of every message type produced by Claude Code, how claude-view parses them, and how they map to the UI.
>
> **Date:** 2026-02-22
> **Status:** Current (Claude Code CLI as of Feb 2026)

---

## 1. Two Message Sources

Claude Code produces messages through **two independent channels**. Claude-view consumes both and merges them into a single conversation timeline.

```
┌──────────────────────────────────────────────────────────────┐
│                     Claude Code CLI                          │
│                                                              │
│  Channel A: JSONL file                Channel B: Hooks       │
│  ~/.claude/projects/…/session.jsonl   POST /api/hooks        │
│                                                              │
│  • Conversation content               • Lifecycle events     │
│  • Tool calls + results               • Agent state (FSM)    │
│  • Thinking blocks                    • Permission requests  │
│  • System metadata                    • Sub-agent lifecycle   │
│  • Progress indicators                • Task completion       │
│  • Summaries (context compaction)     • Context compaction    │
└──────────┬───────────────────────────────────┬───────────────┘
           │                                   │
           ▼                                   ▼
   ┌───────────────┐                  ┌────────────────┐
   │  JSONL Parser  │                  │  Hook Handler  │
   │  (Rust: core)  │                  │  (Rust: server)│
   └───────┬───────┘                  └───────┬────────┘
           │                                   │
           │        ┌──────────────┐           │
           └───────►│  Merged UI   │◄──────────┘
                    │  Timeline    │
                    └──────────────┘
```

**Deduplication:** `hook_progress` entries in JSONL overlap with hook events in SQLite. The frontend deduplicates — SQLite versions are richer (have `context` JSON) and win.

---

## 2. JSONL Entry Types (Channel A)

Every line in a session JSONL file is a JSON object with a top-level `"type"` field. There are **9 known types**.

### 2.1 Core Conversation Types

#### `user`

User-originated messages. The `message.content` shape determines the parsed Role.

```jsonc
// String content → Role::User (human prompt)
{"type":"user","uuid":"u1","timestamp":"…","message":{"role":"user","content":"Fix the bug in auth.rs"}}

// Array with tool_result blocks → Role::ToolResult (tool output returning to Claude)
{"type":"user","uuid":"u2","parentUuid":"a1","timestamp":"…","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tu1","content":"fn auth() {}"}]}}
```

- `isMeta: true` entries are **skipped** (system init messages, not real user prompts).
- Command tags (`<command-name>`, `<command-args>`) are stripped from content.
- Backslash-newline sequences (`\\\n`) are normalized to `\n`.

#### `assistant`

Claude's responses. Content blocks determine the parsed Role and extracted data.

```jsonc
{"type":"assistant","uuid":"a1","parentUuid":"u1","timestamp":"…","message":{"role":"assistant","model":"claude-sonnet-4-20250514","content":[
  {"type":"thinking","thinking":"Let me analyze…"},
  {"type":"text","text":"I'll fix the authentication function."},
  {"type":"tool_use","id":"tu1","name":"Edit","input":{"file_path":"/src/auth.rs"}}
]}}
```

| Content shape | Parsed Role | Description |
|---|---|---|
| Has `text` blocks (with or without tools) | `Role::Assistant` | Normal assistant response |
| Only `tool_use` blocks, no text | `Role::ToolUse` | Pure tool invocation |
| Only `thinking`, no text or tools | *(deferred)* | Stored as pending, attached to next assistant message |
| Empty (no text, no tools, no thinking) | *(skipped)* | Dropped entirely |

**Content block types within `message.content[]`:**

| Block `type` | Fields | Description |
|---|---|---|
| `text` | `text` | Plain text output |
| `thinking` | `thinking` | Extended thinking / chain-of-thought |
| `tool_use` | `id`, `name`, `input` | Tool invocation request |
| `tool_result` | `tool_use_id`, `content` | Tool execution result (appears in `user` entries) |

### 2.2 Metadata Types

#### `system`

System-level metadata events. Has a `subtype` field.

```jsonc
{"type":"system","uuid":"s1","timestamp":"…","subtype":"turn_duration","durationMs":5000,"isMeta":true}
```

| `subtype` | Fields | Description |
|---|---|---|
| `turn_duration` | `durationMs` | How long a turn took |

Mapped to `Role::System` with category `"system"`.

#### `progress`

Real-time activity indicators. The actual event kind is in `data.type`.

```jsonc
{"type":"progress","uuid":"p1","timestamp":"…","data":{"type":"hook_progress","hookEvent":"PreToolUse","hookName":"lint-check","command":"eslint --fix"}}
```

| `data.type` | Description | Category |
|---|---|---|
| `hook_progress` | Hook execution progress | `"hook_progress"` |
| `agent_progress` | Sub-agent activity | `"agent"` |
| `bash_progress` | Bash command running | *(none)* |
| `mcp_progress` | MCP tool execution | *(none)* |
| `waiting_for_task` | Sub-agent waiting | *(none)* |

Mapped to `Role::Progress`.

#### `summary`

Context window compression. When the conversation exceeds the context limit, Claude Code compresses earlier messages into a summary.

```jsonc
{"type":"summary","uuid":"sum1","timestamp":"…","summary":"Fixed authentication bug in auth.rs","leafUuid":"a2"}
```

- `summary` — the compressed text
- `leafUuid` — the last message UUID before compression

Mapped to `Role::Summary`.

### 2.3 Operational Types

These are bookkeeping entries. They carry no conversation content but are needed for state reconstruction.

#### `queue-operation`

Message queue management for multi-turn flows.

```jsonc
{"type":"queue-operation","uuid":"q1","timestamp":"…","operation":"enqueue","content":"next task"}
{"type":"queue-operation","uuid":"q2","timestamp":"…","operation":"dequeue"}
```

Mapped to `Role::System` with category `"queue"`.

#### `file-history-snapshot`

Point-in-time file state backups for undo/restore.

```jsonc
{"type":"file-history-snapshot","uuid":"fhs1","timestamp":"…","messageId":"a2","snapshot":{"trackedFileBackups":{"/src/auth.rs":"backup-hash"}},"isSnapshotUpdate":false}
```

Mapped to `Role::System` with category `"snapshot"`.

#### `saved_hook_context`

Hook-injected context persisted into the conversation (e.g. claude-mem memory snapshots).

```jsonc
{"type":"saved_hook_context","uuid":"shc1","timestamp":"…","content":["hook context line 1","hook context line 2"]}
```

Mapped to `Role::System` with category `"hook"`.

#### `result`

Final session result written at the end of a JSONL file. Marks session completion.

```jsonc
{"type":"result","uuid":"r1","timestamp":"…","result":"success"}
```

### 2.4 Forward Compatibility

Unknown `type` values are **silently ignored** (`parser.rs:401-404`). This allows newer Claude Code versions to add entry types without breaking older claude-view versions.

---

## 3. Parsed Roles (Internal Representation)

The parser normalizes 9 JSONL types into **7 Roles** used throughout the Rust backend and TypeScript frontend.

```
JSONL type              →  Role
──────────────────────────────────────
user (string content)   →  User
user (tool_result[])    →  ToolResult
assistant (has text)    →  Assistant
assistant (tools only)  →  ToolUse
system                  →  System
progress                →  Progress
summary                 →  Summary
queue-operation         →  System
file-history-snapshot   →  System
saved_hook_context      →  System
result                  →  (skipped or System)
```

**TypeScript type** (`apps/web/src/types/generated/Role.ts`):
```typescript
export type Role = "user" | "assistant" | "tool_use" | "tool_result" | "system" | "progress" | "summary";
```

---

## 4. Hook Events (Channel B) — SQLite

Hook events are the **second message source**. They arrive via HTTP POST from Claude Code's hook system, are held in memory during a live session, and batch-written to SQLite on `SessionEnd`.

### 4.1 Schema

```sql
CREATE TABLE hook_events (
    id          INTEGER PRIMARY KEY,
    session_id  TEXT NOT NULL,
    timestamp   INTEGER NOT NULL,        -- Unix epoch seconds
    event_name  TEXT NOT NULL,            -- See §4.2
    tool_name   TEXT,                     -- Tool involved (if applicable)
    label       TEXT NOT NULL,            -- Human-readable description
    group_name  TEXT NOT NULL,            -- "autonomous" | "needs_you" | "delivered"
    context     TEXT                      -- JSON blob with event-specific data
);
```

**Groups** classify agent state for the Mission Control dashboard:
- `"autonomous"` — agent is working independently
- `"needs_you"` — agent is waiting for user input
- `"delivered"` — reserved for future use

### 4.2 All Event Names (15)

| # | `event_name` | Default Group | Description |
|---|---|---|---|
| 1 | `SessionStart` | autonomous | Session begins. Context: `{source?, model?}` |
| 2 | `UserPromptSubmit` | autonomous | User submits a prompt. Context: `{prompt}` |
| 3 | `PreToolUse` | autonomous | About to invoke a tool. Context: tool input JSON |
| 4 | `PostToolUse` | autonomous | Tool completed successfully |
| 5 | `PostToolUseFailure` | autonomous | Tool execution failed. Context: `{error, is_interrupt?}` |
| 6 | `PermissionRequest` | needs_you | Awaiting user permission. Context: `{permission_suggestions}` |
| 7 | `Notification` | varies | See subtypes below |
| 8 | `Stop` | needs_you | Agent stopped or user interrupted |
| 9 | `SessionEnd` | — | Session terminates (triggers SQLite flush; not stored as event) |
| 10 | `SubagentStart` | autonomous | Sub-agent spawned. Context: `{agent_type, agent_id}` |
| 11 | `SubagentStop` | *(metadata only)* | Sub-agent completed. Does not change parent state |
| 12 | `TeammateIdle` | *(metadata only)* | Teammate went idle. Context: `{teammate_name, team_name}` |
| 13 | `TaskCompleted` | *(metadata only)* | Task marked complete. Context: `{task_id, task_subject}` |
| 14 | `PreCompact` | autonomous | Context compaction starting. Context: `{trigger: "manual"|"auto"}` |
| 15 | *(wildcard)* | autonomous | Any unknown event name → generic fallback for forward compat |

**Notification subtypes** (via `notification_type` field):

| `notification_type` | Group | Description |
|---|---|---|
| `permission_prompt` | needs_you | Permission-related notification |
| `idle_prompt` | needs_you | Session idle notification |
| `elicitation_dialog` | needs_you | Dialog prompting user for input |
| `auth_success` | *(filtered out)* | Not stored |

### 4.3 Storage Lifecycle

```
Live session (in-memory)          SessionEnd           SQLite (persistent)
┌─────────────────────┐          ┌──────────┐         ┌──────────────────┐
│ Vec<HookEvent>      │──flush──►│ INSERT    │────────►│ hook_events table│
│ max 5000 per session│          │ (batch tx)│         │ indexed by       │
│ oldest 100 dropped  │          └──────────┘         │ session_id + ts  │
│ if limit exceeded   │                                └──────────────────┘
└─────────────────────┘
```

### 4.4 Frontend Retrieval

```
GET /api/sessions/{sessionId}/hook-events
→ JSON array of { timestamp, eventName, toolName, label, group, context }
→ Mapped to TypeScript HookEventItem
→ Merged with JSONL messages into unified conversation timeline
→ Deduplicated against hook_progress entries from JSONL
```

---

## 5. Frontend Rich Message Types

The frontend transforms both sources into a unified `RichMessage` type for rendering.

| RichMessage type | Source | Renderer |
|---|---|---|
| `user` | JSONL `user` (string) | User prompt bubble |
| `assistant` | JSONL `assistant` (with text) | Assistant response bubble |
| `tool_use` | JSONL `assistant` (tools only) | Tool card (paired with result) |
| `tool_result` | JSONL `user` (tool_result[]) | Tool result card |
| `thinking` | JSONL `assistant` (thinking block) | Collapsible thinking section |
| `error` | Hook events / system errors | Error banner |
| `hook` | SQLite hook events | Hook event chip in timeline |
| `system` | JSONL `system` / `queue-operation` / etc. | System metadata row |
| `progress` | JSONL `progress` | Progress indicator |
| `summary` | JSONL `summary` | Summary card |

---

## 6. Action Categories

Tool calls and events are categorized for filtering in the Action Log.

| Category | Assigned When |
|---|---|
| `skill` | Tool name is `Skill` |
| `mcp` | Tool name starts with `mcp__` |
| `builtin` | Known tools: Read, Edit, Write, Bash, Grep, Glob, WebFetch, WebSearch, Task, etc. |
| `agent` | Tool name is `Task` (sub-agent) |
| `hook` | `saved_hook_context` entries |
| `hook_progress` | Progress entries with `data.type == "hook_progress"` |
| `error` | Error events |
| `system` | System metadata entries |
| `snapshot` | `file-history-snapshot` entries |
| `queue` | `queue-operation` entries |

---

## 7. Key Implementation Files

| Layer | File | What it does |
|---|---|---|
| **JSONL Parser** | `crates/core/src/parser.rs` | Parses all 9 JSONL types → `Vec<Message>` |
| **Live Parser** | `crates/core/src/live_parser.rs` | Streaming parser for active sessions |
| **Role enum** | `crates/core/src/types.rs` | `Role` enum definition (7 variants) |
| **Category** | `crates/core/src/category.rs` | Tool → category mapping |
| **Hook Handler** | `crates/server/src/routes/hooks.rs` | Receives hook POSTs, resolves agent state |
| **Hook DB** | `crates/db/src/queries/hook_events.rs` | SQLite read/write for hook_events |
| **Schema** | `crates/db/src/migrations.rs` | Migration 24: hook_events table |
| **TS Types** | `apps/web/src/types/generated/Role.ts` | Generated TypeScript Role type |
| **Rich Pane** | `apps/web/src/components/live/RichPane.tsx` | RichMessage type + renderer dispatch |
| **Hook→Message** | `apps/web/src/lib/hook-events-to-messages.ts` | Converts SQLite hook events to timeline items |
| **Action Types** | `apps/web/src/components/live/action-log/types.ts` | ActionCategory type + HookEventItem |
