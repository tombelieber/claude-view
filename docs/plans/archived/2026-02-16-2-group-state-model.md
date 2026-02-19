---
status: done
date: 2026-02-16
---

# Mission Control: 2-Group State Model + Cache TTL Ring + Kill Endpoint

## Problem

The kanban board's 3-column model (Needs You / Running / Done) doesn't work in practice:

1. **Process detection fails for Node.js installs** — Claude runs as `node`, not `claude`.
   When detection fails, `derive_status()` sees `!has_running_process && stale > 60s` → `Done`.
   Sessions skip the `Paused` state entirely (Working → Done), so the classifier never fires.

2. **"Done" is unreliable** — automated Done detection depends on process detection,
   which is inherently fragile. When it fails, sessions skip "Needs You" and jump straight
   to "Done", leaving the user with no indication that Claude is waiting for input.

3. **"Done" creates housekeeping** — sessions that are truly done pile up in the Done
   column. Users either ignore them (cluttered board) or manually archive them (friction).

## Solution

### 2-Group Model

Replace the 3-group model with 2 groups:

| Group | Meaning | When |
|-------|---------|------|
| **Autonomous** (Running) | Claude is actively working | Streaming, tool use, between tool calls |
| **NeedsYou** (Needs You) | Your turn | Claude finished, waiting, error, session ended |

The `Delivered` group is eliminated. Everything that was `Delivered` becomes `NeedsYou`
with a descriptive state string (`"session_ended"`, `"task_complete"`, `"work_delivered"`).

### Cache TTL Countdown Ring

Each session card shows a circular progress ring (16px) that depletes over 300 seconds
(Anthropic's prompt cache TTL). This communicates urgency without a separate column:

- **Green** (>180s remaining): Cache is warm, cheap to resume
- **Amber** (60–180s remaining): Cache cooling, respond soon
- **Red** (<60s remaining): Cache about to expire
- **Gray** (expired): Cache cold, resuming costs more (full context re-read)

Within the "Needs You" column/group, sessions sort warm-first (cache warm on top).
Cold sessions render with `opacity-70` for visual hierarchy.

The frontend computes the ring from `lastActivityAt` — no new backend field needed:
`remaining = 300 - (now - lastActivityAt)`

### Process Detection Fix

When `!has_running_process && seconds_since_modified <= 300`:
- Don't trust the negative process detection
- Treat as `Paused` (not `Done`)
- This gives the classifier time to fire and assign the correct NeedsYou state
- 300s matches the cache TTL — if JSONL was written within the cache window,
  the process is almost certainly still running

### Kill Endpoint

`POST /api/live/sessions/{id}/kill`

- Looks up the session's PID
- Sends `SIGTERM` to the process
- Returns `200 { killed: true, pid }` on success
- Returns `404 { canDismiss: true }` if PID unknown (frontend shows soft-dismiss option)
- Frontend shows confirmation dialog before killing ("End this session?")

Sessions disappear from the board when the process exits (detected by process detector on
next poll). No time-based auto-removal.

### Session Lifecycle

1. Session appears on board → **Running** (Autonomous)
2. Claude finishes a turn → **Needs You** (NeedsYou) with cache countdown ring
3. Cache warm (ring green/amber) → session sorted to top of Needs You
4. Cache cold (ring gray) → session sorted below, dimmed
5. User kills session → confirmation → SIGTERM → process exits → card disappears
6. Process exits naturally → process detector removes card on next poll

No "Done" column. No manual housekeeping. No time-based auto-removal.

## Implementation Plan

### Layer 1: Backend State Machine (3 files)

#### 1.1 `crates/server/src/live/manager.rs`

In `handle_status_change()`:

```rust
// BEFORE: Done → Delivered
if new_status == SessionStatus::Done && acc.completed_at.is_none() {
    acc.agent_state = AgentState {
        group: AgentStateGroup::Delivered,  // ← REMOVE
        state: "session_ended".into(),
        ...
    };
}

// AFTER: Done → NeedsYou
if new_status == SessionStatus::Done && acc.completed_at.is_none() {
    acc.agent_state = AgentState {
        group: AgentStateGroup::NeedsYou,   // ← CHANGE
        state: "session_ended".into(),
        label: "Session ended".into(),
        ...
    };
}
```

In `pause_classification_to_agent_state()`:

```rust
// BEFORE
PauseReason::TaskComplete => (AgentStateGroup::Delivered, "task_complete"),
PauseReason::WorkDelivered => (AgentStateGroup::Delivered, "work_delivered"),

// AFTER
PauseReason::TaskComplete => (AgentStateGroup::NeedsYou, "task_complete"),
PauseReason::WorkDelivered => (AgentStateGroup::NeedsYou, "work_delivered"),
```

#### 1.2 `crates/server/src/routes/hooks.rs`

In `resolve_state_from_hook()`:

```rust
// BEFORE
"SessionEnd" => AgentState { group: AgentStateGroup::Delivered, ... }
"TaskCompleted" => AgentState { group: AgentStateGroup::Delivered, ... }

// AFTER
"SessionEnd" => AgentState { group: AgentStateGroup::NeedsYou, ... }
"TaskCompleted" => AgentState { group: AgentStateGroup::NeedsYou, ... }
```

#### 1.3 `crates/server/src/routes/live.rs`

In `build_summary()`:

```rust
// BEFORE: 3-way count
AgentStateGroup::NeedsYou => needs_you_count += 1,
AgentStateGroup::Autonomous => autonomous_count += 1,
AgentStateGroup::Delivered => delivered_count += 1,

// AFTER: 2-way count (delivered maps to needs_you)
AgentStateGroup::NeedsYou | AgentStateGroup::Delivered => needs_you_count += 1,
AgentStateGroup::Autonomous => autonomous_count += 1,
```

Keep `delivered_count: 0` in the SSE summary for backwards compatibility until frontend
is updated. Then remove.

#### 1.4 `crates/server/src/live/state.rs` (enum stays, variant annotated)

Keep `Delivered` variant in `AgentStateGroup` enum to avoid breaking compilation across
the workspace. Add `#[allow(dead_code)]` above it (matches existing convention in
`terminal.rs` and `file_tracker.rs`):

```rust
#[allow(dead_code)]
Delivered,
```

Also in `SessionEvent::Summary`, the `delivered_count` field will always be 0 after
this change. Keep it for backwards compatibility but add `#[allow(dead_code)]`:

```rust
#[allow(dead_code)]
#[serde(rename = "deliveredCount")]
delivered_count: usize,
```

Both can be removed in a follow-up cleanup PR.

### Layer 2: Process Detection Fix (2 files)

#### 2.1 `crates/server/src/live/state.rs` — `derive_status()`

```rust
// BEFORE: No process + stale > 60s → Done
if !has_running_process && seconds_since_modified > 60 {
    return SessionStatus::Done;
}

// AFTER: Only trust "Done" when JSONL is also cold (>300s)
if !has_running_process && seconds_since_modified > 300 {
    return SessionStatus::Done;
}
// Between 60-300s with no process: treat as Paused (let classifier decide)
```

Also update the `derive_status()` docstring (`"stale >60s"` → `"stale >300s"`) and the
`SessionStatus::Done` doc comment (`"no new writes for 60s"` → `"no new writes for 300s"`).

Update the existing test `test_status_done_at_61s_no_process` — it expects `Done` at 61s
but will now get `Paused`. Change to use `301` and add a new test verifying `61` returns `Paused`:

```rust
#[test]
fn test_status_done_at_301s_no_process() {
    let last = make_live_line(LineType::Assistant, vec![], Some("end_turn"));
    let status = derive_status(Some(&last), 301, false);
    assert_eq!(status, SessionStatus::Done);
}

#[test]
fn test_status_paused_at_61s_no_process() {
    let last = make_live_line(LineType::Assistant, vec![], Some("end_turn"));
    let status = derive_status(Some(&last), 61, false);
    assert_eq!(status, SessionStatus::Paused); // was Done before; now in grace window
}
```

#### 2.2 `crates/server/src/live/manager.rs` — update MidWork comment

The comment at `handle_status_change()` (around line 775-777) says sessions without a
process transition to Done at >60s. Update to reflect the new 300s threshold:

```rust
// BEFORE
// Sessions with no process + no JSONL activity for >60s transition to
// Done via derive_status, so they never reach this MidWork check.

// AFTER
// Sessions with no process + no JSONL activity for >300s transition to
// Done via derive_status. Between 60-300s with no process, they may reach
// this MidWork check — the 60s threshold here is intentional (show NeedsYou
// after 60s of silence, but don't immediately classify as Done).
```

This gives the classifier a 300s window (matching cache TTL) to fire before the session
transitions to Done. Process-based detection remains the authority, but we no longer
trust negative detection on short timescales.

#### 2.3 `crates/server/src/live/state_resolver.rs` — add `work_delivered` to Terminal

The `state_category()` function doesn't list `"work_delivered"` — it falls through to
`StateCategory::Transient` and expires after 60s. After our changes, `work_delivered`
maps to `NeedsYou` instead of `Delivered`. A transient NeedsYou state that expires
would revert to JSONL-derived state, potentially flipping back to Autonomous.

```rust
// BEFORE
"task_complete" | "session_ended" => StateCategory::Terminal,

// AFTER
"task_complete" | "session_ended" | "work_delivered" => StateCategory::Terminal,
```

### Layer 3: Kill Endpoint (2 files + 1 dep)

#### 3.1 Add `libc` dependency to `crates/server/Cargo.toml`

```toml
[dependencies]
libc = "0.2"
```

#### 3.2 `crates/server/src/routes/live.rs` — new handler

Add `post` to the `routing` import: `use axum::routing::{get, post};`

```rust
async fn kill_session(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Response {
    let map = state.live_sessions.read().await;
    match map.get(&session_id) {
        Some(session) => {
            if let Some(pid) = session.pid {
                let pid_i32 = pid as i32; // safe: macOS PIDs max ~99999, Linux ~4M
                let result = unsafe { libc::kill(pid_i32, libc::SIGTERM) };
                if result != 0 {
                    tracing::warn!(session_id = %session_id, pid, "Failed to send SIGTERM");
                }
                Json(serde_json::json!({ "killed": true, "pid": pid })).into_response()
            } else {
                (
                    axum::http::StatusCode::NOT_FOUND,
                    Json(serde_json::json!({ "canDismiss": true })),
                ).into_response()
            }
        }
        None => {
            (
                axum::http::StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "Session not found" })),
            ).into_response()
        }
    }
}
```

#### 3.3 Wire route in `routes/live.rs::router()`

Add to the `router()` function (after the existing `/live/sessions/{id}/messages` route):

```rust
.route("/live/sessions/{id}/kill", post(kill_session))
```

Note: No `/api` prefix here — that's added by `api_routes()` in `lib.rs`.

### Layer 4: Frontend — Type Definitions (1 file)

#### 4.1 `src/components/live/types.ts`

```typescript
// BEFORE
export type AgentStateGroup = 'needs_you' | 'autonomous' | 'delivered'

// AFTER
export type AgentStateGroup = 'needs_you' | 'autonomous'
```

Remove `delivered` from:
- `KNOWN_STATES` map
- `GROUP_DEFAULTS` map
- `GROUP_ORDER` map

### Layer 5: Frontend — All Views (15 files)

#### 5.1 `KanbanView.tsx` — 2 columns

Remove the "Done" column from `COLUMNS` array. Remove `delivered` from the `groups` Record.

#### 5.2 `SessionCard.tsx` — remove delivered, add cache ring

Remove `delivered` from `GROUP_CONFIG`. Add `<CacheCountdownRing />` component.
Show ring only when `group === 'needs_you'`.

#### 5.3 `StatusDot.tsx` — remove delivered color

Remove `delivered: 'bg-blue-500'` from `GROUP_COLORS`.

#### 5.4 `ContextGauge.tsx` — simplify inactive check

```typescript
// BEFORE
const isInactive = group === 'needs_you' || group === 'delivered'
// AFTER
const isInactive = group === 'needs_you'
```

#### 5.5 `MonitorPane.tsx` — remove delivered cases

Remove `delivered` cases from `groupDotColor()` and `GroupIcon()` functions.

#### 5.6 `ExpandedPaneOverlay.tsx` — same as MonitorPane

Remove `delivered` cases from duplicated `groupDotColor()` and `GroupIcon()`.

#### 5.7 `MobileStatusTabs.tsx` — 2 tabs

Remove "Done" tab from `TABS` array. Remove `delivered` case from `getCount()`.

#### 5.8 `LiveCommandPalette.tsx` — remove delivered filter

Remove `{ label: 'delivered', ... }` from `statusFilters` array.

#### 5.9 `useAutoFill.ts` — simplify idle check

```typescript
// BEFORE
const isIdle = session.agentState.group === 'needs_you' || session.agentState.group === 'delivered'
// AFTER
const isIdle = session.agentState.group === 'needs_you'
```

#### 5.10 `live-filter.ts` — update comments

Remove `delivered` from the comment listing valid group values.

#### 5.11 `use-live-sessions.ts` — remove deliveredCount

Remove `deliveredCount` from `LiveSummary` interface.

#### 5.12 `src/pages/MissionControlPage.tsx` — remove delivered from summary

Remove `case 'delivered'` from summary switch (line 64). Remove `deliveredCount`
rendering from `SummaryBar` (lines 275-278).

#### 5.13 `ListView.tsx` — auto-fixed

Uses `GROUP_ORDER` from types.ts — will auto-fix when types.ts is updated.

#### 5.14 `KanbanColumn.tsx` — no change needed

Component is generic, just won't receive delivered sessions anymore.

#### 5.15 `src/components/spinner/SessionSpinner.tsx` — remove delivered type and branch

Remove `'delivered'` from the `agentStateGroup` prop type union (line 24):
```typescript
// BEFORE
agentStateGroup?: 'needs_you' | 'autonomous' | 'delivered'
// AFTER
agentStateGroup?: 'needs_you' | 'autonomous'
```

Remove the `if (agentStateGroup === 'delivered')` branch (lines 123-131) that renders
a checkmark "Done" indicator — these sessions now show as `'needs_you'` with a descriptive
state string instead.

### Layer 6: Cache Countdown Ring (1 new component)

#### 6.1 `src/components/live/CacheCountdownRing.tsx`

New component: SVG circular progress ring.

Props:
- `lastActivityAt: number` (Unix timestamp)
- `size?: number` (default 16)

Behavior:
- Computes `elapsed = Math.floor((Date.now() / 1000) - lastActivityAt)`
- `remaining = Math.max(0, 300 - elapsed)`
- `progress = remaining / 300` (1.0 = full, 0.0 = expired)
- Updates every second via `useEffect` + `setInterval(1000)`
- SVG circle with `stroke-dashoffset` based on progress
- Colors: green (progress > 0.6) → amber (0.2–0.6) → red (< 0.2) → gray (0)

Used in:
- `SessionCard.tsx` — next to status dot
- `MonitorPane.tsx` — in pane header
- `ExpandedPaneOverlay.tsx` — in overlay header

### Layer 7: Sort Warm-First in Needs You

In `KanbanView.tsx`, update the needs_you sort:

```typescript
groups.needs_you.sort((a, b) => {
  // Warm sessions first; 'unknown' sorts between warm and cold
  const cacheRank = (s: LiveSession) =>
    s.cacheStatus === 'warm' ? 0 : s.cacheStatus === 'unknown' ? 1 : 2
  const cacheDiff = cacheRank(a) - cacheRank(b)
  if (cacheDiff !== 0) return cacheDiff

  // Within same cache tier: sort by urgency then recency
  const keyDiff = needsYouSortKey(a) - needsYouSortKey(b)
  if (keyDiff !== 0) return keyDiff
  return b.lastActivityAt - a.lastActivityAt
})
```

Cold sessions (not `'warm'`) render with `opacity-70` via inline style or class.
`'unknown'` sessions get the same dimming since cache state is indeterminate.

## File Change Summary

| Layer | Files | Type |
|-------|-------|------|
| Backend state machine | `manager.rs`, `hooks.rs`, `live.rs` | Modify |
| Process detection fix | `state.rs`, `manager.rs` (comment), `state_resolver.rs` | Modify |
| Kill endpoint | `live.rs`, `Cargo.toml` | Modify |
| Frontend types | `types.ts` | Modify |
| Frontend views | 15 component files (incl. `SessionSpinner.tsx`) | Modify |
| Cache ring | `CacheCountdownRing.tsx` | New |
| **Total** | **24 files modified, 1 new** | |

## Implementation Order

1. Backend state machine (Layer 1) — all Delivered → NeedsYou
2. Process detection fix (Layer 2) — 60s → 300s threshold
3. Kill endpoint (Layer 3) — new route
4. Frontend types (Layer 4) — remove delivered from types
5. Frontend views (Layer 5) — remove delivered references, 2 columns
6. Cache ring (Layer 6) — new component
7. Sort + dimming (Layer 7) — warm-first sort, opacity on cold

Each layer can be tested independently. Backend changes are backwards-compatible
(frontend just won't see `delivered` anymore). Frontend changes depend on backend
being deployed first.

## Changelog of Fixes Applied (Audit → Final Plan)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | Kill handler extractor order wrong (`Path` before `State`) | Blocker | Reordered to `State(state): State<Arc<AppState>>` first, matching codebase pattern |
| 2 | Kill handler State type missing `Arc` wrapper | Blocker | Changed `State<AppState>` → `State<Arc<AppState>>` |
| 3 | Kill handler return type won't compile (mixed `Json` and `(StatusCode, Json)`) | Blocker | Changed to `-> Response` with `.into_response()` on all return paths |
| 4 | `state.live_manager.sessions().await` doesn't exist | Blocker | Changed to `state.live_sessions.read().await` (AppState has no `live_manager` field) |
| 5 | Route syntax `:id` is Axum 0.6; project uses Axum 0.8 `{id}` | Blocker | Changed to `"/live/sessions/{id}/kill"` |
| 6 | Route wiring pointed at `create_app_full()` in `lib.rs` | Blocker | Corrected to `routes/live.rs::router()` function |
| 7 | Route path had `/api` prefix (added automatically by `api_routes()`) | Blocker | Removed `/api` prefix from route definition |
| 8 | `libc` dependency missing from `Cargo.toml` | Blocker | Added step 3.1 to add `libc = "0.2"` to `crates/server/Cargo.toml` |
| 9 | `libc::kill()` return value ignored | Warning | Added error handling with `tracing::warn!` on failure |
| 10 | Missing `post` import from `axum::routing` | Warning | Added note to import `post` alongside `get` |
| 11 | `MissionControlPage.tsx` path wrong (`src/components/live/` → `src/pages/`) | Warning | Corrected path to `src/pages/MissionControlPage.tsx` |
| 12 | File Change Summary listed `lib.rs` for kill endpoint | Minor | Changed to `Cargo.toml` (the actual file that needs modification) |
| 13 | Kill handler uses `json!()` without `serde_json::` prefix | Blocker | Changed to `serde_json::json!()` to match existing code convention |
| 14 | Kill handler uses unqualified `StatusCode` | Blocker | Changed to `axum::http::StatusCode::NOT_FOUND` matching file convention |
| 15 | `SessionSpinner.tsx` has `'delivered'` type and branch — not in plan | Blocker | Added Section 5.15 to remove delivered from SessionSpinner |
| 16 | `work_delivered` missing from `state_category()` Terminal list | Warning | Added Section 2.3 to add `work_delivered` to Terminal match arm |
| 17 | MidWork comment in `manager.rs` says 60s but threshold becomes 300s | Warning | Added Section 2.2 to update comment and explain threshold interaction |
| 18 | Test `test_status_done_at_61s_no_process` will fail after 60→300 change | Blocker | Added updated and new test to Section 2.1 |
| 19 | `derive_status()` docstring and `SessionStatus::Done` doc say "60s" | Minor | Added note to Section 2.1 to update docstrings |
| 20 | `Delivered` variant triggers `#[warn(dead_code)]` | Minor | Added `#[allow(dead_code)]` annotation (matches codebase convention) |
| 21 | `SessionEvent::Summary` has dead `delivered_count` field | Minor | Added `#[allow(dead_code)]` annotation + backwards-compat note |
| 22 | `pid as i32` cast undocumented | Minor | Added inline safety comment explaining PID range |
| 23 | Layer 7 sort groups `'unknown'` with `'cold'` without distinction | Minor | Added 3-tier `cacheRank` function: warm=0, unknown=1, cold=2 |
