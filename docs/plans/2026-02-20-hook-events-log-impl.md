# Hook Events Log — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add chronological hook event timeline to the Log tab, piggybacked on the existing WebSocket, persisted to SQLite for historical viewing.

**Architecture:** Backend captures hook events in `handle_hook()` → stores in-memory on `LiveSession` → streams via existing terminal WebSocket → persists to SQLite on session end. Frontend merges hook events into the action log timeline as a new `hook` category with amber styling.

**Tech Stack:** Rust (Axum, tokio broadcast), SQLite (sqlx), React (TypeScript), react-virtuoso

---

### Task 1: Add `HookEvent` struct and in-memory storage

**Files:**
- Modify: `crates/server/src/live/state.rs:49-111` (LiveSession struct)

**Step 1: Add HookEvent struct to state.rs**

Add after the `SessionEvent` enum (after line ~140):

```rust
/// A single hook lifecycle event, captured for the event log.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookEvent {
    /// Unix timestamp (seconds).
    pub timestamp: i64,
    /// Hook event name: "PreToolUse", "PostToolUse", "Stop", etc.
    pub event_name: String,
    /// Tool name, if applicable.
    pub tool_name: Option<String>,
    /// Human-readable label (from resolve_state_from_hook).
    pub label: String,
    /// Agent state group: "autonomous" or "needs_you".
    pub group: String,
    /// Optional context JSON (tool_input, error, prompt snippet, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
}
```

**Step 2: Add `hook_events` field to `LiveSession`**

Add to the `LiveSession` struct (after `last_cache_hit_at`):

```rust
    /// Hook lifecycle events captured for the event log.
    /// Skipped in SSE serialization (too large); streamed via WS only.
    #[serde(skip_serializing)]
    pub hook_events: Vec<HookEvent>,
```

**Step 3: Initialize `hook_events: Vec::new()` in all LiveSession constructors**

Search for all places that construct `LiveSession` and add `hook_events: Vec::new()`:
- `crates/server/src/routes/hooks.rs` — 2 constructors (lazy creation + SessionStart)
- `crates/server/src/live/manager.rs` — session creation in file watcher
- `crates/server/src/routes/terminal.rs` — test helper `test_state_with_session`

**Step 4: Run tests**

Run: `cargo test -p claude-view-server`
Expected: All existing tests pass (HookEvent is skip_serializing, no existing JSON changes).

**Step 5: Commit**

```bash
git add crates/server/src/live/state.rs crates/server/src/routes/hooks.rs crates/server/src/live/manager.rs crates/server/src/routes/terminal.rs
git commit -m "feat(server): add HookEvent struct and in-memory storage on LiveSession"
```

---

### Task 2: Capture hook events in `handle_hook()`

**Files:**
- Modify: `crates/server/src/routes/hooks.rs:50-369` (handle_hook function)

**Step 1: Write test for hook event capture**

Add to the tests module in `hooks.rs`:

```rust
#[tokio::test]
async fn test_hook_event_captured() {
    // This test verifies that handle_hook appends a HookEvent to the session
    // Setup requires a full AppState — defer to integration test or manual verification
    // For now, test the HookEvent construction helper
    let event = super::build_hook_event(
        1708000000,
        "PreToolUse",
        Some("Read"),
        "Reading file.rs",
        "autonomous",
        None,
    );
    assert_eq!(event.event_name, "PreToolUse");
    assert_eq!(event.tool_name, Some("Read".to_string()));
    assert_eq!(event.group, "autonomous");
}
```

**Step 2: Add `build_hook_event` helper function**

Add before `handle_hook`:

```rust
/// Maximum hook events kept in memory per session.
const MAX_HOOK_EVENTS_PER_SESSION: usize = 5000;

/// Build a HookEvent from hook handler data.
fn build_hook_event(
    timestamp: i64,
    event_name: &str,
    tool_name: Option<&str>,
    label: &str,
    group: &str,
    context: Option<&serde_json::Value>,
) -> HookEvent {
    HookEvent {
        timestamp,
        event_name: event_name.to_string(),
        tool_name: tool_name.map(|s| s.to_string()),
        label: label.to_string(),
        group: group.to_string(),
        context: context.map(|v| v.to_string()),
    }
}
```

**Step 3: Append HookEvent in handle_hook**

In `handle_hook()`, right after `let agent_state = resolve_state_from_hook(&payload);` and the `now` timestamp, add:

```rust
    let group_str = match &agent_state.group {
        AgentStateGroup::NeedsYou => "needs_you",
        AgentStateGroup::Autonomous => "autonomous",
        AgentStateGroup::Delivered => "delivered",
    };

    let hook_event = build_hook_event(
        now,
        &payload.hook_event_name,
        payload.tool_name.as_deref(),
        &agent_state.label,
        group_str,
        payload.tool_input.as_ref().or(payload.error.as_ref().map(|e| &serde_json::json!({"error": e}))).flatten(),
    );
```

Then in each match arm that gets `&mut session`, add after the existing logic:

```rust
    // Append hook event (capped at MAX_HOOK_EVENTS_PER_SESSION)
    if session.hook_events.len() >= MAX_HOOK_EVENTS_PER_SESSION {
        session.hook_events.drain(..100); // drop oldest 100
    }
    session.hook_events.push(hook_event.clone());
```

Note: The `hook_event` needs to be cloned or created per-branch since it's moved. Alternatively, create it once before the match and clone into each branch. The simplest approach: build `hook_event` after the match statement's session mutation, by adding a final block that appends to any session that was just modified.

**Better approach**: After the entire match block, add a unified append:

```rust
    // ── Append hook event to session (unified, after all match arms) ──
    if payload.hook_event_name != "SessionEnd" {
        let mut sessions = state.live_sessions.write().await;
        if let Some(session) = sessions.get_mut(&payload.session_id) {
            if session.hook_events.len() >= MAX_HOOK_EVENTS_PER_SESSION {
                session.hook_events.drain(..100);
            }
            session.hook_events.push(hook_event);
        }
    }
```

Wait — this requires a second write lock acquisition. Since the match arms already hold a write lock, the cleanest approach is to build the event before the match and pass it in. Each arm that has `session` appends `hook_event.clone()` at the end.

Actually simplest: build `hook_event` before the match. Inside each arm's `if let Some(session)` block, add one line: `session.hook_events.push(hook_event.clone());` (except SessionEnd which removes the session).

**Step 4: Run tests**

Run: `cargo test -p claude-view-server`
Expected: PASS — including the new test.

**Step 5: Commit**

```bash
git add crates/server/src/routes/hooks.rs
git commit -m "feat(server): capture hook events in handle_hook"
```

---

### Task 3: Per-session broadcast channel for WS streaming

**Files:**
- Modify: `crates/server/src/state.rs` (add hook_event_channels)
- Modify: `crates/server/src/routes/hooks.rs` (send on channel)

**Step 1: Add per-session hook broadcast map to AppState**

In `state.rs`, add a new field:

```rust
    /// Per-session broadcast channels for hook events (WebSocket streaming).
    /// Key: session_id. Created on demand when a WS connects, cleaned up on SessionEnd.
    pub hook_event_channels: Arc<tokio::sync::RwLock<
        HashMap<String, tokio::sync::broadcast::Sender<crate::live::state::HookEvent>>
    >>,
```

Initialize in all `AppState` constructors:

```rust
    hook_event_channels: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
```

**Step 2: Send on broadcast channel in handle_hook**

In `handle_hook()`, after appending `hook_event` to the session's vec, add:

```rust
    // Broadcast to any connected WS listeners
    let channels = state.hook_event_channels.read().await;
    if let Some(tx) = channels.get(&payload.session_id) {
        let _ = tx.send(hook_event.clone());
    }
```

**Step 3: Run tests**

Run: `cargo test -p claude-view-server`
Expected: PASS.

**Step 4: Commit**

```bash
git add crates/server/src/state.rs crates/server/src/routes/hooks.rs
git commit -m "feat(server): add per-session hook event broadcast channels"
```

---

### Task 4: Stream hook events over existing WebSocket

**Files:**
- Modify: `crates/server/src/routes/terminal.rs:553-898` (handle_terminal_ws)

**Step 1: Accept AppState in handle_terminal_ws**

Change the function signature to also receive `state: Arc<AppState>`. Pass it from `ws_terminal_handler`.

**Step 2: Send buffered hook events after scrollback**

After the scrollback buffer is sent and before the `buffer_end` marker, add:

```rust
    // Send buffered hook events from in-memory LiveSession
    {
        let sessions = state.live_sessions.read().await;
        if let Some(session) = sessions.get(&session_id) {
            for event in &session.hook_events {
                let msg = serde_json::json!({
                    "type": "hook_event",
                    "timestamp": event.timestamp,
                    "eventName": event.event_name,
                    "toolName": event.tool_name,
                    "label": event.label,
                    "group": event.group,
                    "context": event.context,
                });
                if socket.send(Message::Text(msg.to_string().into())).await.is_err() {
                    return;
                }
            }
        }
    }
```

**Step 3: Subscribe to hook broadcast in the event loop**

After setting up the file watcher, subscribe to the per-session hook broadcast:

```rust
    // Subscribe to hook event broadcasts for this session
    let mut hook_rx = {
        let mut channels = state.hook_event_channels.write().await;
        let tx = channels
            .entry(session_id.clone())
            .or_insert_with(|| tokio::sync::broadcast::channel(256).0);
        tx.subscribe()
    };
```

Add a new `tokio::select!` arm in the main event loop:

```rust
    // Hook event broadcasts
    hook_event = hook_rx.recv() => {
        match hook_event {
            Ok(event) => {
                let msg = serde_json::json!({
                    "type": "hook_event",
                    "timestamp": event.timestamp,
                    "eventName": event.event_name,
                    "toolName": event.tool_name,
                    "label": event.label,
                    "group": event.group,
                    "context": event.context,
                });
                if socket.send(Message::Text(msg.to_string().into())).await.is_err() {
                    return;
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                // Missed some events — acceptable, they're in the in-memory vec
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                // Channel closed — session ended
            }
        }
    }
```

**Step 4: Clean up channel on SessionEnd**

In `handle_hook()` SessionEnd arm, after removing the session, also remove the channel:

```rust
    state.hook_event_channels.write().await.remove(&session_id);
```

**Step 5: Run tests**

Run: `cargo test -p claude-view-server`
Expected: PASS. Existing WS tests should still pass since hook events are additive.

**Step 6: Commit**

```bash
git add crates/server/src/routes/terminal.rs crates/server/src/routes/hooks.rs
git commit -m "feat(server): stream hook events over existing terminal WebSocket"
```

---

### Task 5: SQLite migration and persistence

**Files:**
- Modify: `crates/db/src/migrations.rs` (add migration 24)
- Modify: `crates/db/src/queries/mod.rs` (add hook_events module)
- Create: `crates/db/src/queries/hook_events.rs`
- Modify: `crates/server/src/routes/hooks.rs` (persist on SessionEnd)

**Step 1: Add migration 24**

In `migrations.rs`, add to the `MIGRATIONS` array:

```rust
    // Migration 24: Hook events log for event timeline
    r#"
CREATE TABLE IF NOT EXISTS hook_events (
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
"#,
    r#"
CREATE INDEX IF NOT EXISTS idx_hook_events_session ON hook_events(session_id, timestamp);
"#,
```

**Step 2: Create hook_events query module**

Create `crates/db/src/queries/hook_events.rs`:

```rust
use sqlx::SqlitePool;

pub struct HookEventRow {
    pub timestamp: i64,
    pub event_name: String,
    pub tool_name: Option<String>,
    pub label: String,
    pub group_name: String,
    pub context: Option<String>,
}

/// Insert hook events in a batch transaction.
pub async fn insert_hook_events(
    pool: &SqlitePool,
    session_id: &str,
    events: &[HookEventRow],
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    for event in events {
        sqlx::query(
            "INSERT INTO hook_events (session_id, timestamp, event_name, tool_name, label, group_name, context)
             VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(session_id)
        .bind(event.timestamp)
        .bind(&event.event_name)
        .bind(&event.tool_name)
        .bind(&event.label)
        .bind(&event.group_name)
        .bind(&event.context)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Fetch hook events for a session, ordered by timestamp.
pub async fn get_hook_events(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Vec<HookEventRow>, sqlx::Error> {
    let rows = sqlx::query_as!(
        HookEventRow,
        "SELECT timestamp, event_name, tool_name, label, group_name, context
         FROM hook_events
         WHERE session_id = ?
         ORDER BY timestamp ASC",
        session_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
```

Register in `crates/db/src/queries/mod.rs`:
```rust
pub mod hook_events;
```

**Step 3: Persist on SessionEnd**

In `handle_hook()` SessionEnd arm, before removing the session, persist:

```rust
    "SessionEnd" => {
        let session_id = payload.session_id.clone();

        // Persist hook events to SQLite before removing from memory
        {
            let sessions = state.live_sessions.read().await;
            if let Some(session) = sessions.get(&session_id) {
                if !session.hook_events.is_empty() {
                    let rows: Vec<_> = session.hook_events.iter().map(|e| {
                        claude_view_db::queries::hook_events::HookEventRow {
                            timestamp: e.timestamp,
                            event_name: e.event_name.clone(),
                            tool_name: e.tool_name.clone(),
                            label: e.label.clone(),
                            group_name: e.group.clone(),
                            context: e.context.clone(),
                        }
                    }).collect();
                    if let Err(e) = claude_view_db::queries::hook_events::insert_hook_events(
                        state.db.pool(), &session_id, &rows
                    ).await {
                        tracing::warn!(session_id = %session_id, error = %e, "Failed to persist hook events");
                    }
                }
            }
        }

        state.live_sessions.write().await.remove(&session_id);
        // ... existing cleanup
    }
```

**Step 4: Run tests**

Run: `cargo test -p claude-view-db && cargo test -p claude-view-server`
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/db/src/migrations.rs crates/db/src/queries/hook_events.rs crates/db/src/queries/mod.rs crates/server/src/routes/hooks.rs
git commit -m "feat(db): add hook_events table and persist on SessionEnd"
```

---

### Task 6: Historical hook events REST endpoint

**Files:**
- Modify: `crates/server/src/routes/sessions.rs` (add hook-events endpoint)
- OR Create: `crates/server/src/routes/hook_events.rs` and register in mod.rs

**Step 1: Add endpoint**

```rust
/// GET /api/sessions/:id/hook-events — Fetch stored hook events for a historical session.
async fn get_session_hook_events(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Json<serde_json::Value> {
    match claude_view_db::queries::hook_events::get_hook_events(state.db.pool(), &session_id).await {
        Ok(events) => {
            let json_events: Vec<_> = events.iter().map(|e| serde_json::json!({
                "timestamp": e.timestamp,
                "eventName": e.event_name,
                "toolName": e.tool_name,
                "label": e.label,
                "group": e.group_name,
                "context": e.context,
            })).collect();
            Json(serde_json::json!({ "hookEvents": json_events }))
        }
        Err(e) => {
            Json(serde_json::json!({ "hookEvents": [], "error": e.to_string() }))
        }
    }
}
```

Register the route: `.route("/sessions/{id}/hook-events", get(get_session_hook_events))`

**Step 2: Run tests**

Run: `cargo test -p claude-view-server`
Expected: PASS.

**Step 3: Commit**

```bash
git add crates/server/src/routes/sessions.rs crates/server/src/routes/mod.rs
git commit -m "feat(server): add GET /api/sessions/:id/hook-events endpoint"
```

---

### Task 7: Frontend — Parse hook events from WebSocket

**Files:**
- Modify: `src/hooks/use-live-session-messages.ts`
- Modify: `src/components/live/action-log/types.ts`

**Step 1: Add HookEvent TypeScript type**

In `types.ts`, add:

```typescript
export interface HookEventItem {
  id: string
  timestamp: number
  type: 'hook_event'
  eventName: string
  toolName?: string
  label: string
  group: 'autonomous' | 'needs_you' | 'delivered'
  context?: string
}

export type TimelineItem = ActionItem | TurnSeparator | HookEventItem

export function isHookEvent(item: TimelineItem): item is HookEventItem {
  return 'type' in item && (item as HookEventItem).type === 'hook_event'
}
```

**Step 2: Parse hook_event messages in useLiveSessionMessages**

In `use-live-session-messages.ts`, update `handleMessage` to also emit hook events:

```typescript
const [hookEvents, setHookEvents] = useState<HookEventItem[]>([])

const handleMessage = useCallback((data: string) => {
  try {
    const parsed = JSON.parse(data)
    if (parsed.type === 'hook_event') {
      setHookEvents((prev) => [...prev, {
        id: `hook-${prev.length}`,
        type: 'hook_event',
        timestamp: parsed.timestamp,
        eventName: parsed.eventName,
        toolName: parsed.toolName,
        label: parsed.label,
        group: parsed.group,
        context: parsed.context,
      }])
      return
    }
  } catch { /* not JSON — fall through to rich message parsing */ }

  const richParsed = parseRichMessage(data)
  if (richParsed) {
    setMessages((prev) => [...prev, richParsed])
  }
}, [])
```

Update the return type to include `hookEvents`.

**Step 3: Commit**

```bash
git add src/hooks/use-live-session-messages.ts src/components/live/action-log/types.ts
git commit -m "feat(frontend): parse hook_event messages from WebSocket"
```

---

### Task 8: Frontend — Merge hook events into action log timeline

**Files:**
- Modify: `src/components/live/action-log/use-action-items.ts`
- Modify: `src/components/live/action-log/ActionLogTab.tsx`

**Step 1: Update useActionItems to accept and merge hook events**

Change signature: `useActionItems(messages: RichMessage[], hookEvents?: HookEventItem[])`

After building the `items` array from messages, merge in hook events by timestamp:

```typescript
// Merge hook events into timeline
if (hookEvents && hookEvents.length > 0) {
  for (const event of hookEvents) {
    items.push(event) // HookEventItem is already a TimelineItem
  }
  // Re-sort all items by timestamp
  items.sort((a, b) => {
    const tsA = 'timestamp' in a ? (a as any).timestamp : 0
    const tsB = 'timestamp' in b ? (b as any).timestamp : 0
    return tsA - tsB
  })
}
```

**Step 2: Update ActionLogTab to pass hookEvents**

```typescript
const allItems = useActionItems(messages, hookEvents)
```

Add `hookEvents` to the props and pass through from `SessionDetailPanel`.

**Step 3: Update category counts to include hook events**

```typescript
const counts = useMemo(() => {
  const c: Record<ActionCategory, number> = { skill: 0, mcp: 0, builtin: 0, agent: 0, error: 0, hook: 0 }
  for (const item of allItems) {
    if (isHookEvent(item)) {
      c.hook++
    } else if (!isTurnSeparator(item)) {
      c[item.category]++
    }
  }
  return c
}, [allItems])
```

**Step 4: Commit**

```bash
git add src/components/live/action-log/use-action-items.ts src/components/live/action-log/ActionLogTab.tsx
git commit -m "feat(frontend): merge hook events into action log timeline"
```

---

### Task 9: Frontend — Render hook events in ActionRow + filter chip

**Files:**
- Modify: `src/components/live/action-log/ActionFilterChips.tsx`
- Modify: `src/components/live/action-log/ActionRow.tsx` (or create HookEventRow)
- Modify: `src/components/live/action-log/ActionLogTab.tsx` (render dispatch)

**Step 1: Add hook chip to ActionFilterChips**

Add to `CATEGORIES` array:

```typescript
{ id: 'hook', label: 'Hook', color: 'bg-amber-500/10 text-amber-400 border-amber-500/30' },
```

**Step 2: Create HookEventRow component**

Create `src/components/live/action-log/HookEventRow.tsx`:

```tsx
import { useState } from 'react'
import { ChevronRight, ChevronDown } from 'lucide-react'
import { cn } from '../../../lib/utils'
import type { HookEventItem } from './types'

const EVENT_BADGE: Record<string, string> = {
  PreToolUse: 'Pre',
  PostToolUse: 'Post',
  PostToolUseFailure: 'Fail',
  PermissionRequest: 'Perm',
  Stop: 'Stop',
  SessionStart: 'Start',
  SessionEnd: 'End',
  UserPromptSubmit: 'Prompt',
  Notification: 'Notif',
  SubagentStart: 'Sub+',
  SubagentStop: 'Sub-',
  TeammateIdle: 'Team',
  TaskCompleted: 'Task',
  PreCompact: 'Compact',
}

function shortBadge(eventName: string): string {
  return EVENT_BADGE[eventName] ?? eventName.slice(0, 6)
}

export function HookEventRow({ event }: { event: HookEventItem }) {
  const [expanded, setExpanded] = useState(false)
  const hasContext = !!event.context

  return (
    <div className={cn(
      'border-b border-gray-800/50',
      event.group === 'needs_you' && 'bg-amber-500/5',
    )}>
      <button
        onClick={() => hasContext && setExpanded((v) => !v)}
        className={cn(
          'w-full flex items-center gap-2 px-3 py-2 text-left transition-colors',
          hasContext && 'hover:bg-gray-800/30 cursor-pointer',
        )}
      >
        <span className="w-1.5 h-1.5 rounded-full flex-shrink-0 bg-amber-400" />

        <span className="text-[10px] font-mono px-1.5 py-0.5 rounded flex-shrink-0 min-w-[40px] text-center bg-amber-500/10 text-amber-400">
          {shortBadge(event.eventName)}
        </span>

        <span className="text-xs text-gray-300 truncate flex-1 font-mono" title={event.label}>
          {event.label}
        </span>

        {event.timestamp > 0 && (
          <span className="text-[10px] font-mono tabular-nums text-gray-600 flex-shrink-0">
            {new Date(event.timestamp * 1000).toLocaleTimeString()}
          </span>
        )}

        {hasContext && (
          expanded
            ? <ChevronDown className="w-3 h-3 text-gray-500 flex-shrink-0" />
            : <ChevronRight className="w-3 h-3 text-gray-500 flex-shrink-0" />
        )}
      </button>

      {expanded && event.context && (
        <div className="px-3 pb-3">
          <pre className="text-[10px] font-mono text-amber-300/80 bg-gray-900 rounded p-2 overflow-x-auto max-h-[200px] overflow-y-auto whitespace-pre-wrap break-all">
            {formatContext(event.context)}
          </pre>
        </div>
      )}
    </div>
  )
}

function formatContext(ctx: string): string {
  try {
    return JSON.stringify(JSON.parse(ctx), null, 2)
  } catch {
    return ctx
  }
}
```

**Step 3: Update ActionLogTab itemContent to dispatch**

In the `Virtuoso` `itemContent`, add:

```tsx
itemContent={(_, item) =>
  isTurnSeparator(item) ? (
    <TurnSeparatorRow role={item.role} content={item.content} />
  ) : isHookEvent(item) ? (
    <HookEventRow event={item} />
  ) : (
    <ActionRow action={item} />
  )
}
```

**Step 4: Run dev server and verify visually**

Run: `bun run dev`
Open Live Monitor → select a session → Log tab. Hook events should appear with amber styling.

**Step 5: Commit**

```bash
git add src/components/live/action-log/HookEventRow.tsx src/components/live/action-log/ActionFilterChips.tsx src/components/live/action-log/ActionLogTab.tsx
git commit -m "feat(frontend): render hook events in Log tab with amber styling"
```

---

### Task 10: Frontend — Fetch historical hook events

**Files:**
- Modify: `src/components/live/SessionDetailPanel.tsx`
- Create: `src/hooks/use-hook-events.ts`

**Step 1: Create useHookEvents hook**

Create `src/hooks/use-hook-events.ts`:

```typescript
import { useState, useEffect } from 'react'
import type { HookEventItem } from '../components/live/action-log/types'

/**
 * Fetch stored hook events for a historical session.
 * Returns empty array for sessions with no stored events (old sessions).
 */
export function useHookEvents(sessionId: string, enabled: boolean): HookEventItem[] {
  const [events, setEvents] = useState<HookEventItem[]>([])

  useEffect(() => {
    if (!enabled) {
      setEvents([])
      return
    }
    let cancelled = false

    fetch(`/api/sessions/${sessionId}/hook-events`)
      .then((r) => r.json())
      .then((data) => {
        if (cancelled) return
        const items: HookEventItem[] = (data.hookEvents ?? []).map((e: any, i: number) => ({
          id: `hook-${i}`,
          type: 'hook_event' as const,
          timestamp: e.timestamp,
          eventName: e.eventName,
          toolName: e.toolName,
          label: e.label,
          group: e.group,
          context: e.context,
        }))
        setEvents(items)
      })
      .catch(() => {
        if (!cancelled) setEvents([])
      })

    return () => { cancelled = true }
  }, [sessionId, enabled])

  return events
}
```

**Step 2: Wire into SessionDetailPanel**

In `SessionDetailPanel`, for historical sessions (when `!isLive`), fetch hook events and pass to ActionLogTab:

```typescript
const historicalHookEvents = useHookEvents(data.id, !isLive)
```

Pass to the Log tab:
```tsx
{activeTab === 'log' && (
  <ActionLogTab
    messages={richMessages}
    bufferDone={bufferDone}
    hookEvents={isLive ? liveHookEvents : historicalHookEvents}
  />
)}
```

**Step 3: Commit**

```bash
git add src/hooks/use-hook-events.ts src/components/live/SessionDetailPanel.tsx
git commit -m "feat(frontend): fetch and display historical hook events"
```

---

### Task 11: Wire-up verification and end-to-end test

**Files:** None — this is a manual verification step.

**Step 1: Build backend**

Run: `cargo build -p claude-view-server`
Expected: Compiles without errors.

**Step 2: Run all backend tests**

Run: `cargo test -p claude-view-server -p claude-view-db`
Expected: All tests pass.

**Step 3: Run frontend type check**

Run: `bun run typecheck` (or `npx tsc --noEmit`)
Expected: No type errors.

**Step 4: End-to-end manual test**

1. Start server: `cargo run -p claude-view-server`
2. Start frontend: `bun run dev`
3. Open Live Monitor in browser
4. Start a Claude Code session in another terminal
5. Verify: Hook events appear in the Log tab with amber styling
6. Verify: Filter chip shows "Hook" with count
7. Verify: Expanding a hook event shows context JSON
8. Close the Claude session
9. Open the session from history
10. Verify: Hook events appear in the historical Log tab

**Step 5: Final commit (if any fixups needed)**

```bash
git add -A
git commit -m "fix: wire-up fixes for hook events log"
```
