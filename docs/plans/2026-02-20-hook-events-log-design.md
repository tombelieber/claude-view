# Hook Events Log — Design

**Date**: 2026-02-20
**Status**: Approved

## Goal

Add a chronological hook event timeline to the existing Log tab in the session detail panel. Shows both lifecycle events (PreToolUse, PostToolUse, PermissionRequest, etc.) and user-defined hook results, interleaved with existing tool actions. Persisted to SQLite for historical viewing going forward.

## Decisions

| Decision | Choice |
|----------|--------|
| Placement | Merged into existing Log tab (no new tab) |
| Scope | Full event timeline + extra detail for user-defined hooks |
| Historical data | Going-forward persistence only — old sessions keep existing JSONL data |
| Live transport | Piggyback on existing per-session WebSocket (not SSE) |

## Data Model

### New struct: `HookEvent`

```rust
// crates/server/src/live/state.rs (or new file crates/server/src/live/hook_event.rs)

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookEvent {
    pub timestamp: i64,           // unix seconds
    pub event_name: String,       // "PreToolUse", "PostToolUse", "Stop", etc.
    pub tool_name: Option<String>,
    pub label: String,            // from resolve_state_from_hook (already computed)
    pub group: String,            // "autonomous" | "needs_you"
    pub context: Option<String>,  // JSON snippet (tool_input, error, prompt text, etc.)
}
```

### In-memory storage

Add to `LiveSession`:
```rust
pub hook_events: Vec<HookEvent>,
```

Skip serializing in SSE payloads (too large):
```rust
#[serde(skip_serializing)]
pub hook_events: Vec<HookEvent>,
```

### SQLite persistence

New table:
```sql
CREATE TABLE hook_events (
    id INTEGER PRIMARY KEY,
    session_id TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    event_name TEXT NOT NULL,
    tool_name TEXT,
    label TEXT NOT NULL,
    group_name TEXT NOT NULL,
    context TEXT,
    FOREIGN KEY (session_id) REFERENCES sessions(id)
);
CREATE INDEX idx_hook_events_session ON hook_events(session_id, timestamp);
```

Written as a **batch transaction** on SessionEnd. Periodic flush every 60s for sessions with >100 unflushed events. Also flush on server shutdown.

Cap in-memory at 5000 events per session (FIFO). SQLite gets them all via periodic flush.

## Live Transport

### Current architecture

- **SSE** (`/api/live/stream`): Session snapshots for grid view. Fires on every hook but only sends `LiveSession` state.
- **WebSocket** (`/api/live/sessions/:id/terminal`): Per-session JSONL streaming. Connects only when detail panel is open.

### Change: Piggyback on existing WebSocket

When the detail panel opens and the WS connects, also stream hook events through the same socket. Type discriminator:

```json
{"type": "jsonl", ...}       // existing JSONL messages
{"type": "hook_event", ...}  // new lifecycle events
```

Backend changes in the terminal WS handler:
1. On WS connect, send buffered `hook_events` from the `LiveSession` in-memory vec
2. Subscribe to a per-session `tokio::sync::broadcast` channel for new hook events
3. Forward new hook events as `{"type": "hook_event", ...}` messages

In `handle_hook()`:
1. Append `HookEvent` to `session.hook_events`
2. Send on the per-session hook broadcast channel (if any WS listeners exist)

### Historical sessions

New endpoint: `GET /api/sessions/:id/hook-events`
- Returns hook events from SQLite for completed sessions
- Frontend fetches on panel open for historical sessions and merges into timeline

## Frontend Integration

### Types

Extend `ActionCategory`:
```typescript
type ActionCategory = 'skill' | 'mcp' | 'builtin' | 'agent' | 'error' | 'hook'
```

### ActionFilterChips

New chip:
```typescript
{ id: 'hook', label: 'Hook', color: 'bg-amber-500/10 text-amber-400 border-amber-500/30' }
```

### ActionRow rendering for hook events

- Amber status dot (informational, not pass/fail)
- Badge: shorthand event type (`Pre`, `Post`, `Stop`, `Perm`, `Start`, etc.)
- Label: derived activity label from `resolve_state_from_hook`
- Expandable detail: context JSON when available
- `needs_you` group events get a subtle highlight (matching existing "needs attention" patterns)

### Data flow

**Live sessions:**
1. `useLiveSessionMessages` already parses WS messages
2. Add parsing for `{"type": "hook_event", ...}` → new `HookEvent` type
3. `use-action-items.ts` merges hook events + JSONL actions by timestamp → single `TimelineItem[]`

**Historical sessions:**
1. Fetch `GET /api/sessions/:id/hook-events` when panel opens
2. Merge with JSONL-derived actions by timestamp
3. If no hook events in DB (old session), Log tab works exactly as today

### Visual example

```
 Skill   Skill: brainstorming                    2.1s
 Bash    git status                              0.3s
 Pre     Reading parser.rs                           ← amber
 Read    ...core/src/parser.rs                   0.1s
 Post    Thinking...                                 ← amber
 Perm    Needs permission: Bash                      ← amber + needs_you highlight
 Bash    cargo test -p claude-view-core          4.2s
```

## Edge Cases

| Scenario | Handling |
|----------|----------|
| Server restart mid-session | Hook events before restart lost. New events resume. Acceptable. |
| Very long session (1000+ hooks) | Cap at 5000 in memory (FIFO). Periodic flush to SQLite. |
| SessionEnd never fires (crash) | Periodic flush every 60s for sessions with >100 unflushed events. Flush on server shutdown. |
| Historical session with no hook data | Log tab works as today — just tool actions from JSONL. No empty state noise. |
| Duplicate events (hook + JSONL) | `hook_progress` from JSONL → `builtin` category. Hook lifecycle → `hook` category. Different types, no overlap. |

## UX Guidelines (from UI/UX Pro Max)

- Virtualized list (already using `react-virtuoso`) for performance
- 150-300ms transitions for expand/collapse
- Respect `prefers-reduced-motion`
- Amber color for hook events consistent with existing `HookSummaryCard`/`HookProgressCard`
- No continuous animations except on pending/loading states
- Keyboard accessible: tab order follows visual order, focus rings on interactive elements
