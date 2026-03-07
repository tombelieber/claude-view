# Claude Code Message Types — Complete Reference

> Definitive catalog of every message type produced by Claude Code, how claude-view parses them, and how they map to the UI.
>
> **Date:** 2026-02-27
> **Status:** Current (Claude Code CLI as of Feb 2026)
> **Audited:** 2026-02-27 against parser.rs, hooks.rs, RichPane.tsx, category.rs, and 4,716 real JSONL files (728,071 entries)
> **Data counts:** All occurrence counts in this document are snapshots from the audit date above — not universal ratios. Your corpus will differ.

---

## 1. Two Message Sources

Claude Code produces messages through **two independent channels**. Claude-view consumes both and merges them into a single conversation timeline.

```text
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

**Two distinct data types — NOT duplicates:**

- **JSONL `hook_progress`** (Channel A) = progress events. These are real-time activity indicators written by Claude CLI to the session file. They belong to the `progress` category.
- **SQLite `hook_events`** (Channel B) = lifecycle events captured by claude-view's hook handler. These are structured state transitions received via `POST /api/live/hook`.

These are **different data from different channels**. Both are shown in the timeline. Never deduplicate them.

---

## 2. JSONL Entry Types (Channel A)

Every line in a session JSONL file is a JSON object with a top-level `"type"` field. The parser handles **9 known types** (two additional types, `pr-link` and `hook_event`, exist in JSONL but are silently ignored via the wildcard).

### 2.1 Core Conversation Types

#### `user`

User-originated messages. The `message.content` shape determines the parsed Role.

```jsonc
// String content → Role::User (human prompt)
{"type":"user","uuid":"u1","parentUuid":"p1","timestamp":"…","sessionId":"…","version":"2.1.56","isSidechain":false,"userType":"external","cwd":"/path","gitBranch":"main","message":{"role":"user","content":"Fix the bug in auth.rs"}}

// Array with tool_result blocks → Role::ToolResult (tool output returning to Claude)
{"type":"user","uuid":"u2","parentUuid":"a1","timestamp":"…","sourceToolAssistantUUID":"a1","toolUseResult":"fn auth() {}","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tu1","content":"fn auth() {}","is_error":false}]}}
```

**Content routing** (parser.rs `parse_user_entry()`):

| `message.content` shape | Parsed Role | Notes |
|---|---|---|
| `String` | `Role::User` | Direct string content |
| `Array` with any `tool_result` block | `Role::ToolResult` | Tool output returning to Claude |
| `Array` without `tool_result` blocks | `Role::User` | Text extracted, command tags cleaned |
| Other / missing | `Role::User` | Legacy deserialization fallback |

**Behaviors:**

- `isMeta: true` entries are **skipped** by the parser. These are system init messages injected by Claude Code at session start — they carry injected context (CLAUDE.md contents, system reminders, MCP tool lists) but are not real user prompts. They have `message.content` as an array of `text` blocks. The parser drops them so they don't appear in the conversation timeline.
- Command tags (`<command-name>`, `<command-args>`, `<command-message>`, `<local-command-stdout>`, `<system-reminder>`) are stripped from content.
- Backslash-newline sequences (`\\\n`) are normalized to `\n`.

**Common top-level fields on every `user` entry** (always present in real data):
`type`, `uuid`, `parentUuid`, `timestamp`, `sessionId`, `version`, `isSidechain`, `userType`, `cwd`, `gitBranch`, `message`

`slug` is present on ~92% of user entries (not all).

**Additional fields on tool_result entries:** `toolUseResult` (denormalized summary), `sourceToolAssistantUUID`

**User content block types** (within `message.content[]` arrays):

| Block `type` | Description |
|---|---|
| `tool_result` | Tool execution result. Fields: `tool_use_id`, `content`, `is_error?: boolean` |
| `text` | Text block (appears in `isMeta` entries) |
| `image` | Screenshot/image block. Fields: `source: {type: "base64", media_type, data}` |

#### `assistant`

Claude's responses. Content blocks determine the parsed Role and extracted data.

```jsonc
{"type":"assistant","uuid":"a1","parentUuid":"u1","timestamp":"…","sessionId":"…","version":"2.1.56","message":{"role":"assistant","model":"claude-sonnet-4-20250514","id":"msg_xxx","type":"message","stop_reason":"end_turn","usage":{"input_tokens":1234,"output_tokens":567,"cache_creation_input_tokens":0,"cache_read_input_tokens":800},"content":[
  {"type":"thinking","thinking":"Let me analyze…"},
  {"type":"text","text":"I'll fix the authentication function."},
  {"type":"tool_use","id":"tu1","name":"Edit","input":{"file_path":"/src/auth.rs"}}
]}}
```

**Role assignment** (parser.rs `parse_assistant_entry()`):

| Content shape | Parsed Role | Description |
|---|---|---|
| Has `text` blocks (with or without tools) | `Role::Assistant` | Normal assistant response |
| Only `tool_use` blocks, no text | `Role::ToolUse` | Pure tool invocation |
| Only `thinking`, no text or tools | *(deferred)* | Stored as `pending_thinking`, attached to next assistant message |
| Empty + no `pending_thinking` | *(skipped)* | Dropped entirely |
| Empty + has `pending_thinking` | `Role::Assistant` | Created with the deferred thinking attached |

**Content block types within `message.content[]`** (types.rs `ContentBlock` enum):

| Block `type` | Fields | Description |
|---|---|---|
| `text` | `text: String` | Plain text output |
| `thinking` | `thinking: String` | Extended thinking / chain-of-thought |
| `tool_use` | `name: String`, `input?: Value` | Tool invocation request |
| `tool_result` | `content?: Value` | Tool execution result (appears in `user` entries, not assistant) |
| *(catch-all)* | — | `ContentBlock::Other` via `#[serde(other)]`. Silently drops unknown block types (e.g. `redacted_thinking`, `server_tool_use`, `server_tool_result`, `citation`) for forward compatibility |

**Key `message`-level fields** (present on every assistant entry):

- `model` — model ID (e.g. `claude-sonnet-4-20250514`)
- `stop_reason` — `null`, `"tool_use"`, `"stop_sequence"`, or `"end_turn"`
- `usage` — `{input_tokens, output_tokens, cache_creation_input_tokens, cache_read_input_tokens, cache_creation?: {ephemeral_5m_input_tokens, ephemeral_1h_input_tokens}, service_tier?, inference_geo?}`

### 2.2 Metadata Types

#### `system`

System-level metadata events. Has a `subtype` field.

```jsonc
{"type":"system","uuid":"s1","timestamp":"…","subtype":"turn_duration","durationMs":5000,"isMeta":true}
```

| `subtype` | Key Fields | Count in real data | Description |
|---|---|---|---|
| `stop_hook_summary` | `hookCount`, `hookInfos[]`, `hookErrors[]`, `preventedContinuation`, `stopReason`, `hasOutput` | 4,194 | Hook execution summary at session stop |
| `turn_duration` | `durationMs` | 2,509 | How long a turn took |
| `local_command` | `content`, `level` | 238 | Local CLI command (e.g. `/mcp`) |
| `compact_boundary` | `content`, `level`, `compactMetadata: {trigger, preTokens}`, `logicalParentUuid` | 159 | Context compaction boundary marker |
| `api_error` | `error: {status, headers, requestID}`, `retryInMs`, `retryAttempt`, `maxRetries`, `level` | 137 | API error with retry info |
| `microcompact_boundary` | `content`, `level`, `microcompactMetadata: {trigger, preTokens, tokensSaved, compactedToolIds[], clearedAttachmentUUIDs[]}` | 4 | Micro-compaction boundary |
| `informational` | `content`, `level` | 2 | Informational system messages |

Mapped to `Role::System` with category `"system"`.

#### `progress`

Real-time activity indicators. The actual event kind is in `data.type`.

```jsonc
{"type":"progress","uuid":"p1","timestamp":"…","data":{"type":"hook_progress","hookEvent":"PreToolUse","hookName":"lint-check","command":"eslint --fix"}}
```

| `data.type` | Description | Rust category (category.rs) | Frontend override |
|---|---|---|---|
| `hook_progress` | Hook execution progress | `"hook"` | `"hook_progress"` (frontend override, see note below) |
| `agent_progress` | Sub-agent activity | `"agent"` | — |
| `bash_progress` | Bash command running | `"builtin"` | — |
| `mcp_progress` | MCP tool execution | `"mcp"` | — |
| `waiting_for_task` | Sub-agent waiting | `"agent"` | — |
| `query_update` | Search query being executed | *(none)* | — |
| `search_results_received` | Search results returned | *(none)* | — |

**Note on `hook_progress` category split:** The Rust backend (`categorize_progress()`) maps `hook_progress` to category `"hook"`. The frontend (`message-to-rich.ts`) overrides this to `"hook_progress"` for the Action Log filter chips, so hook progress events appear as a distinct filterable group separate from `hook_event` entries (which keep category `"hook"`).

Mapped to `Role::Progress`.

#### `summary`

Context window compression. When the conversation exceeds the context limit, Claude Code compresses earlier messages into a summary.

```jsonc
{"type":"summary","summary":"Fixed authentication bug in auth.rs","leafUuid":"a2"}
```

- `summary` — the compressed text
- `leafUuid` — the last message UUID before compression

Only these 3 keys in real data: `type`, `summary`, `leafUuid`. No `uuid` field.

Mapped to `Role::Summary`.

### 2.3 Operational Types

These are bookkeeping entries. They carry no conversation content but are needed for state reconstruction.

#### `queue-operation`

Message queue management for multi-turn flows.

```jsonc
{"type":"queue-operation","operation":"enqueue","timestamp":"…","sessionId":"…","content":"next task"}
{"type":"queue-operation","operation":"dequeue","timestamp":"…","sessionId":"…"}
{"type":"queue-operation","operation":"remove","timestamp":"…","sessionId":"…"}
{"type":"queue-operation","operation":"popAll","timestamp":"…","sessionId":"…","content":"queued prompt text"}
```

| `operation` | Count in real data | Has `content`? |
|---|---|---|
| `dequeue` | 10,246 | No |
| `enqueue` | 2,837 | Yes (the queued prompt) |
| `remove` | 2,031 | No |
| `popAll` | 36 | Sometimes |

**Note:** Queue-operation entries have `sessionId` and `timestamp` but **no `uuid` field**.

Mapped to `Role::System` with category `"queue"`.

#### `file-history-snapshot`

Point-in-time file state backups for undo/restore.

```jsonc
{"type":"file-history-snapshot","messageId":"a2","snapshot":{"messageId":"a2","trackedFileBackups":{"/src/auth.rs":{"backupFileName":"be8cf68e@v1","version":1,"backupTime":"2026-02-21T06:17:55.974Z"}},"timestamp":"2026-02-21T06:16:22.976Z"},"isSnapshotUpdate":true}
```

- `trackedFileBackups` values are objects `{backupFileName, version, backupTime}`, not simple hash strings.
- The `snapshot` sub-object contains its own `messageId` and `timestamp`.
- Entries have `messageId` but **no `uuid` field**.

Mapped to `Role::System` with category `"snapshot"`.

#### `saved_hook_context`

Hook-injected context persisted into the conversation (e.g. claude-mem memory snapshots).

```jsonc
{"type":"saved_hook_context","uuid":"shc1","timestamp":"…","parentUuid":"…","sessionId":"…","hookName":"SessionStart","hookEvent":"SessionStart","toolUseID":"SessionStart","cwd":"/path","userType":"external","version":"2.1.27","isSidechain":false,"gitBranch":"main","content":["hook context line 1","hook context line 2"]}
```

Has all session metadata fields plus hook-specific: `hookName`, `hookEvent`, `toolUseID`.

Mapped to `Role::System` with category **`"context"`**.

#### `result`

**Dead code — zero occurrences** across 4,716 real JSONL files (728,071 entries). The parser has a 3-line arm that maps it to `Role::System` with category `"result"`, but no real session has ever produced this type. The example below is **fabricated from the code path**, not from real data.

```jsonc
// ⚠️ Fabricated — no real example exists
{"type":"result","subtype":"success","timestamp":"…"}
```

Kept in the parser for forward compatibility. Harmless dead code.

#### `pr-link`

Pull request metadata injected after `gh pr create`. Rare (12 occurrences across all data).

```jsonc
{"type":"pr-link","sessionId":"…","prNumber":9,"prUrl":"https://github.com/user/repo/pull/9","prRepository":"user/repo","timestamp":"…"}
```

**Not handled by the parser** — falls through to the unknown-type wildcard and is silently ignored.

### 2.4 Forward Compatibility

Unknown `type` values are **silently ignored** via a wildcard match arm. This allows newer Claude Code versions to add entry types without breaking older claude-view versions.

```rust
// parser.rs — wildcard match arm in parse_entries()
_ => {
    debug!(
        "Ignoring unknown entry type '{}' at line {}",
        entry_type, line_number
    );
}
```

---

## 3. Parsed Roles (Internal Representation)

The parser normalizes 9 JSONL types into **7 Roles** used throughout the Rust backend and TypeScript frontend.

```text
JSONL type              →  Role              Category
────────────────────────────────────────────────────────
user (string content)   →  User              —
user (array, no tools)  →  User              —
user (tool_result[])    →  ToolResult        —
assistant (has text)    →  Assistant         (from first tool name)
assistant (tools only)  →  ToolUse           (from first tool name)
system                  →  System            "system"
progress                →  Progress          (from data.type, see §2.2)
summary                 →  Summary           —
queue-operation         →  System            "queue"
file-history-snapshot   →  System            "snapshot"
saved_hook_context      →  System            "context"
result                  →  System            "result"
hook_event              →  (ignored)         —
pr-link                 →  (ignored)         —
```

**Rust enum** (types.rs):

```rust
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,       // Real user prompt
    Assistant,  // Claude response with text
    ToolUse,    // Assistant message with only tool_use blocks
    ToolResult, // User message with tool_result array content
    System,     // System events + queue-ops + file-snapshots
    Progress,   // Progress events (agent, bash, hook, mcp, waiting)
    Summary,    // Auto-generated session summaries
}
```

**TypeScript type** (`apps/web/src/types/generated/Role.ts`):

```typescript
export type Role = "user" | "assistant" | "tool_use" | "tool_result" | "system" | "progress" | "summary";
```

---

## 4. Hook Events (Channel B) — SQLite + WebSocket

Hook events are the **second message source**. They arrive via HTTP POST from Claude Code's hook system, are held in memory during a live session, **pushed in real-time via WebSocket** to connected clients, and batch-written to SQLite on `SessionEnd`.

### 4.1 Schema

```sql
-- Migration 24
CREATE TABLE IF NOT EXISTS hook_events (
    id          INTEGER PRIMARY KEY,
    session_id  TEXT NOT NULL,
    timestamp   INTEGER NOT NULL,        -- Unix epoch seconds
    event_name  TEXT NOT NULL,            -- See §4.2
    tool_name   TEXT,                     -- Tool involved (if applicable)
    label       TEXT NOT NULL,            -- Human-readable description
    group_name  TEXT NOT NULL,            -- "autonomous" | "needs_you" | "delivered"
    context     TEXT                      -- JSON blob with event-specific data
);
CREATE INDEX IF NOT EXISTS idx_hook_events_session ON hook_events(session_id, timestamp);
```

**Groups** classify agent state for the Mission Control dashboard:

- `"autonomous"` — agent is working independently
- `"needs_you"` — agent is waiting for user input
- `"delivered"` — reserved for future use (`#[allow(dead_code)]` in AgentStateGroup enum)

### 4.2 All Event Names (15)

| # | `event_name` | Default Group | Overrides | Description |
|---|---|---|---|---|
| 1 | `SessionStart` | **needs_you** | `source=="compact"` → autonomous | Session begins. Default: "Waiting for first prompt" |
| 2 | `UserPromptSubmit` | autonomous | — | User submits a prompt |
| 3 | `PreToolUse` | autonomous | `AskUserQuestion` → needs_you; `ExitPlanMode` → needs_you; `EnterPlanMode` → autonomous | About to invoke a tool |
| 4 | `PostToolUse` | autonomous | — | Tool completed successfully |
| 5 | `PostToolUseFailure` | autonomous | `is_interrupt==true` → needs_you | Tool failed. Interrupt = user stopped it |
| 6 | `PermissionRequest` | needs_you | — | Awaiting user permission |
| 7 | `Notification` | **needs_you** | See subtypes below. All resolve to needs_you | See subtypes below |
| 8 | `Stop` | needs_you | — | Agent stopped or user interrupted |
| 9 | `SessionEnd` | — | — | Triggers SQLite flush; **not stored** as an event |
| 10 | `SubagentStart` | autonomous | — | Sub-agent spawned |
| 11 | `SubagentStop` | *(metadata only)* | — | Sub-agent completed. Resolves to autonomous but does NOT change parent session state |
| 12 | `TeammateIdle` | *(metadata only)* | — | Teammate went idle. Same: resolves to autonomous, does not change parent state |
| 13 | `TaskCompleted` | *(metadata only)* | — | Task marked complete. Same: resolves to autonomous, does not change parent state |
| 14 | `PreCompact` | autonomous | — | Context compaction starting |
| 15 | *(wildcard)* | autonomous | — | Any unknown event name → generic fallback for forward compat |

**Notification subtypes** (via `notification_type` field):

| `notification_type` | Group | Description |
|---|---|---|
| `permission_prompt` | needs_you | Permission-related notification |
| `idle_prompt` | needs_you | Session idle notification |
| `elicitation_dialog` | needs_you | Dialog prompting user for input |
| `auth_success` | *(filtered out)* | Early return in hook handler — not stored |
| *(unknown)* | needs_you | Fallback for any unrecognized notification_type |

### 4.3 Storage Lifecycle

```text
Live session (in-memory)          SessionEnd           SQLite (persistent)
┌─────────────────────┐          ┌──────────┐         ┌──────────────────┐
│ Vec<HookEvent>      │──flush──►│ INSERT    │────────►│ hook_events table│
│ max 5000 per session│          │ (batch tx)│         │ indexed by       │
│ oldest 100 dropped  │          └──────────┘         │ session_id + ts  │
│ if limit exceeded   │                                └──────────────────┘
└──────────┬──────────┘
           │ (real-time)
           ▼
┌──────────────────────┐
│ WebSocket broadcast   │  hook_event_channels per session
│ to connected clients  │  terminal.rs WebSocket handler
└──────────────────────┘
```

### 4.4 Frontend Retrieval

**REST (for historical/SQLite sessions):**

```text
GET /api/sessions/{sessionId}/hook-events
→ { "hookEvents": [ { timestamp, eventName, toolName, label, group, context }, ... ] }
→ Mapped to TypeScript HookEventItem
→ Merged with JSONL messages into unified conversation timeline
→ Both hook_events AND hook_progress are shown — they are different data, never deduplicated
```

**WebSocket (for live sessions):**
Hook events are pushed in real-time via the terminal WebSocket connection as `{"type": "hook_event", ...}` messages. This is the primary delivery mechanism during active sessions — the REST endpoint is a fallback for completed sessions stored in SQLite.

---

## 5. Frontend Rich Message Types

The frontend transforms both sources into a unified `RichMessage` type for rendering (defined in RichPane.tsx).

```typescript
export interface RichMessage {
  type: 'user' | 'assistant' | 'tool_use' | 'tool_result' | 'thinking'
      | 'error' | 'hook' | 'system' | 'progress' | 'summary'
  content: string
  name?: string          // tool name for tool_use
  input?: string         // tool input summary for tool_use
  inputData?: unknown    // raw parsed object for tool_use
  ts?: number            // timestamp (epoch seconds)
  category?: ActionCategory
  metadata?: Record<string, unknown>
}
```

| RichMessage type | Source | Renderer |
|---|---|---|
| `user` | JSONL `user` (string), or WS `message` with `role:"user"` | User prompt bubble |
| `assistant` | JSONL `assistant` (with text), or WS `message`/`line` | Assistant response bubble |
| `tool_use` | JSONL `assistant` (tools), or WS `tool_use` | Tool card (paired with result) |
| `tool_result` | JSONL `user` (tool_result[]), or WS `tool_result` | Tool result card |
| `thinking` | WS `thinking` messages | Collapsible thinking section |
| `error` | WS `error` messages, hook event errors | Error banner |
| `hook` | — | Hook event chip (HookMessage component exists but `hookEventsToRichMessages()` maps to `type: 'progress'`, not `'hook'`) |
| `system` | JSONL `system` / `queue-operation` / `result` / etc., or WS `system`/`result` | System metadata row |
| `progress` | JSONL `progress`, hook events (via `hookEventsToRichMessages()`), or WS `progress` | Progress indicator |
| `summary` | JSONL `summary`, or WS `summary` | Summary card |

**Hook events conversion** (hook-events-to-messages.ts): SQLite hook events are converted to `type: 'progress'` with `category: 'hook'` and `metadata.type: 'hook_event'`. The original `HookEventItem` is carried in `metadata._hookEvent` for the `HookEventRow` component. These are NOT deduplicated against JSONL `hook_progress` — they are different data (see §1).

---

## 6. Action Categories

Tool calls and events are categorized for filtering in the Action Log. There are **13 categories** total.

### Tool Categories (category.rs `categorize_tool()`)

| Category | Assigned When |
|---|---|
| `skill` | Tool name is `"Skill"` |
| `mcp` | Tool name starts with `"mcp__"` or `"mcp_"` |
| `agent` | Tool name is `"Task"` |
| `builtin` | **Fallback default** — any tool not matching above |

### Progress Categories (category.rs `categorize_progress()`)

| Category | Assigned When |
|---|---|
| `hook` | `data.type == "hook_progress"` |
| `agent` | `data.type == "agent_progress"` or `"waiting_for_task"` |
| `builtin` | `data.type == "bash_progress"` |
| `mcp` | `data.type == "mcp_progress"` |
| *(none)* | Unknown `data.type` (e.g. `query_update`, `search_results_received`) |

**Frontend override:** `hook_progress` data type gets Rust category `"hook"` but the frontend (message-to-rich.ts) overrides to `"hook_progress"` for the filter chip UI.

### Entry-Type Categories (parser.rs)

| Category | Assigned When |
|---|---|
| `system` | `system` JSONL entries |
| `queue` | `queue-operation` entries |
| `snapshot` | `file-history-snapshot` entries |
| `context` | `saved_hook_context` entries |
| `result` | `result` entries |
| `hook` | Hook events from SQLite (Channel B, converted by hook-events-to-messages.ts) |
| `summary` | `summary` entries (frontend-assigned) |
| `error` | Error messages (frontend-assigned) |

### Full ActionCategory TypeScript Type

```typescript
// apps/web/src/components/live/action-log/types.ts
export type ActionCategory =
  | 'skill' | 'mcp' | 'builtin' | 'agent'
  | 'hook' | 'hook_progress'
  | 'error' | 'system' | 'snapshot' | 'queue'
  | 'context' | 'result' | 'summary'
```

---

## 7. Key Implementation Files

| Layer | File | What it does |
|---|---|---|
| **JSONL Parser** | `crates/core/src/parser.rs` | Parses 9 JSONL types → `Vec<Message>` (ignores unknown types) |
| **Live Parser** | `crates/core/src/live_parser.rs` | Streaming parser for active sessions |
| **Role enum** | `crates/core/src/types.rs` | `Role` enum definition (7 variants), `ContentBlock` enum (5 variants) |
| **Category** | `crates/core/src/category.rs` | `categorize_tool()` + `categorize_progress()` |
| **Hook Handler** | `crates/server/src/routes/hooks.rs` | Receives hook POSTs, resolves agent state, WebSocket broadcast |
| **Hook DB** | `crates/db/src/queries/hook_events.rs` | SQLite read/write for hook_events |
| **Schema** | `crates/db/src/migrations.rs` | Migration 24: hook_events table + index |
| **TS Types (web)** | `apps/web/src/types/generated/Role.ts` | Generated TypeScript Role type |
| **TS Types (shared)** | `packages/shared/src/types/generated/` | Generated types: HookEvent, LiveSession, AgentState, AgentStateGroup, TokenUsage, SubAgentInfo, etc. |
| **Rich Pane** | `apps/web/src/components/live/RichPane.tsx` | RichMessage type + `parseRichMessage()` renderer dispatch |
| **Hook→Message** | `apps/web/src/lib/hook-events-to-messages.ts` | Converts SQLite hook events to timeline items (both Message and RichMessage formats) |
| **Action Types** | `apps/web/src/components/live/action-log/types.ts` | ActionCategory (13 variants), ActionItem, HookEventItem, TimelineItem |

---

## 8. Common Session Metadata Fields

Every JSONL entry (user, assistant, system, etc.) shares a set of session metadata fields. These are always present in real data but not part of the conversation content:

| Field | Type | Description |
|---|---|---|
| `sessionId` | string | Session identifier |
| `timestamp` | string (ISO 8601) | Entry creation time |
| `version` | string | Claude Code CLI version (e.g. `"2.1.56"`) |
| `cwd` | string | Working directory |
| `gitBranch` | string | Current git branch |
| `isSidechain` | boolean | Whether this is a sidechain message |
| `userType` | string | Always `"external"` |
| `slug` | string | Human-readable session slug (~92% of user/assistant entries, not all) |
| `uuid` | string | Entry identifier (most types, but NOT summary/queue-operation/file-history-snapshot) |
| `parentUuid` | string | Link to parent message (most types) |
| `todos` | array \| null | Todo list state `[{content, status, activeForm}]`. **Only present when todo feature is active** — 0.9% of user entries, 0% of assistant entries. Absent (not `null`, not `[]`) on entries without active todos |
| `permissionMode` | string | Permission mode (e.g. `"bypassPermissions"`, `"default"`). Present on `user` entries only |
