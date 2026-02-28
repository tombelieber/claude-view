# Live State Recovery on Server Restart

**Date:** 2026-02-21
**Status:** Design approved, pending implementation plan

## Problem

When the claude-view server crashes or restarts, the live monitor goes blank. All `LiveSession` map entries are lost (in-memory only). Sessions only reappear when the next hook fires from a running Claude process, which could be seconds to minutes — or never for idle sessions.

## Current Architecture

| Component | Survives crash? | Contents |
|-----------|----------------|----------|
| JSONL files (`~/.claude/projects/`) | Yes | Full session transcript |
| PID snapshot (`~/.claude/live-monitor-pids.json`) | Yes | `{ session_id: pid }` |
| Accumulators (in-memory) | Rebuilt from JSONL on startup | Tokens, cost, title, branch, turns |
| **LiveSession map** | **No** | Agent state, status, metrics, PID |

The gap: accumulators are warmed on startup, but sessions are **only created by hooks**. Comment at `manager.rs:298`: "No sessions are created here — hooks are the sole authority."

## Design: Extended Snapshot + Startup Promotion

### 1. Extend the PID snapshot format

Current format:
```json
{ "session_id_1": 12345, "session_id_2": 67890 }
```

New format:
```json
{
  "version": 2,
  "sessions": {
    "session_id_1": {
      "pid": 12345,
      "status": "Working",
      "agent_state": {
        "group": "NeedsYou",
        "state": "waiting_for_user",
        "label": "Waiting for your input",
        "context": null
      },
      "last_activity_at": 1708500000
    }
  }
}
```

**Migration:** On load, if the top-level object has no `"version"` key, treat it as v1 (legacy `{ id: pid }` format). Convert in-memory and write back as v2 on next save. No separate migration step needed.

### 2. Snapshot write triggers

Currently written on: PID bind, session death.

**Add writes on:**
- Agent state change (hook handler updates `agent_state` → save snapshot)
- Session completion (already triggers write via death path)

All writes are **atomic** (tmp file + rename), already implemented in `save_pid_snapshot()`. The snapshot is small (one entry per live session, typically < 10) so write cost is negligible.

**Coalesce writes:** If multiple state changes arrive within the same event loop tick (e.g., batch of hooks), only write once. Use a `dirty` flag + debounced write (100ms) or simply write after each hook batch (the hook handler already processes one HTTP request at a time).

### 3. Startup promotion in `spawn_file_watcher()`

After the existing accumulator warm-up loop (`manager.rs:297-301`), add:

```
// 3. Promote accumulators with live PIDs to full sessions
let snapshot = load_extended_snapshot(&pid_snapshot_path());
for (session_id, snapshot_entry) in &snapshot.sessions {
    if !is_pid_alive(snapshot_entry.pid) {
        continue; // Process died while we were down
    }
    if sessions already contains session_id {
        continue; // Hook already created it (race with early hooks)
    }
    if let Some(acc) = accumulators.get(session_id) {
        let session = build_recovered_session(session_id, snapshot_entry, acc, &processes);
        sessions.insert(session_id, session);
        broadcast SessionDiscovered
    }
}
```

`build_recovered_session()` creates a `LiveSession` using:
- `agent_state` and `status` from the snapshot (accurate as of last state change before crash)
- `pid` from the snapshot
- All metrics (tokens, cost, title, branch, turns, sub_agents, tools_used) from the accumulator
- `file_path`, `project`, `project_path` from the accumulator's JSONL path

### 4. Move PID snapshot loading earlier

Currently, the PID snapshot is loaded in `spawn_process_detector()` (`manager.rs:385`). For promotion to work, it must be loaded **before** the promotion step in `spawn_file_watcher()`.

**Change:** Load the snapshot once in `LiveSessionManager::new()` or at the top of `spawn_file_watcher()`, store it as a field or pass it to both the watcher and detector.

### 5. Self-correction

Once a promoted session receives its first hook event, the hook handler overwrites `agent_state` with the real current state. This means:

- **Active sessions** (generating hooks every few seconds): corrected almost instantly
- **Idle sessions** (NeedsYou, waiting for user): snapshot's `agent_state` is already accurate since no state change happened
- **Dead sessions** (PID died during downtime): filtered out by `is_pid_alive()` during promotion

## Edge Cases

| Scenario | Behavior | Acceptable? |
|----------|----------|-------------|
| PID reuse (OS recycles PID to non-Claude) | Ghost session appears; process detector catches it in ≤10s via sysinfo name check | Yes |
| Session created after last snapshot write, before crash | Not recovered; appears when next hook fires (same as today) | Yes — window is tiny |
| Snapshot file missing or corrupted | Falls back to current behavior (blank until hooks) | Yes — graceful degradation |
| Server crashes mid-atomic-write | Previous snapshot used (slightly stale) | Yes — tmp+rename guarantees |
| Two server instances racing | Last writer wins; no corruption due to atomic writes | Yes |
| Accumulator exists but no snapshot entry (pre-crash session never sent a hook) | Not promoted; same as today | Yes |

## Files Changed

| File | Change |
|------|--------|
| `crates/server/src/live/manager.rs` | Extended snapshot types, promotion logic in `spawn_file_watcher()`, earlier snapshot loading, snapshot write on agent_state change |
| `crates/server/src/live/state.rs` | `SnapshotEntry` struct (pid, status, agent_state, last_activity_at) |
| `crates/server/src/routes/hooks.rs` | Trigger snapshot save after agent_state mutations |

No frontend changes. No new crates. No schema migrations. No new files.

## What This Does NOT Solve

- **In-memory hook_events buffer lost on crash** — acceptable, debug-level data. They're flushed to SQLite on clean SessionEnd; crash = lost. Could add periodic flush later if needed.
- **Accumulator byte offsets** — rebuilt from scratch on startup (JSONL re-parse). This is already the behavior today and adds ~100ms for typical session counts.
- **Frontend state** — the frontend already reconnects SSE and rebuilds from the server's state. No frontend persistence needed.
