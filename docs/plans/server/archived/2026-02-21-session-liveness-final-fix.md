# Session Liveness: Final Fix

**Date**: 2026-02-21
**Status**: Draft
**Supersedes**: `2026-02-20-ppid-session-liveness-design.md` (partially implemented)

## Problem

The original PPID design doc was half-implemented. Hooks deliver `X-Claude-PID` and `is_pid_alive` exists, but the reconciliation loop still falls back to blind CWD-based process matching for both discovery and PID binding. This creates three bugs:

### Bug 1: Ghost session resurrection (CWD discovery)

`spawn_reconciliation_loop` Phase 2 (manager.rs:785-925) finds accumulators without a LiveSession, calls `has_running_process(&processes, proj_path)` which matches ANY Claude process in the same directory, and creates a LiveSession bound to the wrong PID. Concrete scenario: session A ends while server is down, session B starts in the same dir, server starts, session A's accumulator gets bound to session B's process.

**Root cause**: Phase 2 discovery (lines 789-790) uses cwd-path matching instead of requiring a hook-delivered PID.

### Bug 2: Accumulator leak on dead snapshot PIDs

`spawn_file_watcher` crash recovery (manager.rs:396-399) skips dead PIDs with `continue` but doesn't remove their accumulators. The orphaned accumulator survives startup, becomes a Phase 2 discovery candidate, and gets cwd-matched to an unrelated process.

**Root cause**: Line 398 skips the entry but doesn't call `accumulators.remove(session_id)`.

### Bug 3: Snapshot entry leak for alive-but-stale PIDs

Snapshot entries whose PIDs were recycled by the OS to a non-Claude process pass `is_pid_alive()` but never match a JSONL file (24h scan window expired or different session). These entries are never promoted and never pruned. The snapshot save at line 523 only fires when `dead > 0`, so fully-alive-but-useless entries persist forever. 86 stale entries observed on a real system.

**Root cause**: No cleanup path for entries that pass `is_pid_alive` but fail to promote.

## What's Already Implemented

| Component | Status | Location |
|-----------|--------|----------|
| `is_pid_alive(pid)` using `kill(pid, 0)` | Done | process.rs:147-154 |
| `X-Claude-PID` header in hook curl | Done | hook_registrar.rs |
| PID extraction from header in hook handler | Done | hooks.rs |
| v2 snapshot format (`SnapshotEntry` with agent_state) | Done | state.rs |
| Phase 1 liveness check using bound PID | Done | manager.rs:649-715 |
| Zombie prevention: remove accumulator when Phase 1 marks session dead | Done | manager.rs:694-703 |

## What's Still Broken

| Component | Problem | Location |
|-----------|---------|----------|
| Phase 2 discovery | Creates sessions via cwd-match, not hook PID | manager.rs:785-925 |
| Phase 2 PID binding | Binds PID via cwd-match for unbound sessions | manager.rs:946-962 |
| Crash recovery | Doesn't clean accumulators for dead snapshot PIDs | manager.rs:396-399 |
| Crash recovery metadata | Calls `has_running_process` for metadata PID | manager.rs:415-418 |
| Snapshot pruning | No cleanup for alive-but-unmatched entries | manager.rs:522-525 |
| `process_jsonl_update` | Uses cwd-match for metadata PID | manager.rs:1427-1430 |

## Design Principle

**Hook-delivered PID is the sole authority for session identity.** The process table scan (`detect_claude_processes`) is kept only for the `processCount` display metric. It must never be used for PID binding or session creation.

Sessions can only come into existence through two paths:
1. **Hook path** (primary): SessionStart or lazy creation from any hook event, with PID from `X-Claude-PID` header.
2. **Snapshot recovery path** (crash recovery): Snapshot entry with stored PID, validated by `is_pid_alive`.

The Phase 2 discovery path (cwd-match accumulators to processes) is removed entirely.

## Changes

### Change 1: Remove Phase 2 discovery and PID binding

**File**: `crates/server/src/live/manager.rs`
**Lines**: 735-965 (inside `spawn_reconciliation_loop`)

Delete the entire Phase 2.2 discovery candidate gathering (lines 740-782), Phase 2.2 discovery candidate processing (lines 785-925), and Phase 2.3 PID binding (lines 946-964).

Keep:
- Phase 2.1 process table refresh (lines 724-733) — still needed for `process_count` display.
- Phase 2.4 unconditional snapshot save (line 978).

After this change, Phase 2 becomes:

```rust
// Phase 2: Full reconciliation (every 3rd tick = 30s)
if !tick_count.is_multiple_of(3) {
    continue;
}

// 2.1 — Process table refresh (display metric only)
let (new_processes, total_count) =
    tokio::task::spawn_blocking(detect_claude_processes)
        .await
        .unwrap_or_default();
manager.process_count.store(total_count, Ordering::Relaxed);
{
    let mut processes = manager.processes.write().await;
    *processes = new_processes;
}

// 2.2 — Unconditional snapshot save (defense in depth)
manager.save_session_snapshot_from_state().await;
```

No discovery. No cwd PID binding. Hooks and snapshot recovery are the only session creation paths.

### Change 2: Clean accumulators for dead snapshot PIDs in crash recovery

**File**: `crates/server/src/live/manager.rs`
**Lines**: 396-399 (inside `spawn_file_watcher`, crash recovery loop)

Current code:
```rust
if !is_pid_alive(entry.pid) {
    dead += 1;
    continue;
}
```

Change to:
```rust
if !is_pid_alive(entry.pid) {
    dead += 1;
    dead_ids.push(session_id.clone());
    continue;
}
```

Add `let mut dead_ids: Vec<String> = Vec::new();` before the loop (alongside `promoted` and `dead`).

After the loop, clean the accumulators:
```rust
if !dead_ids.is_empty() {
    let mut accumulators = manager.accumulators.write().await;
    for id in &dead_ids {
        accumulators.remove(id);
    }
    info!(cleaned = dead_ids.len(), "Cleaned accumulators for dead snapshot PIDs");
}
```

This prevents dead-PID accumulators from lingering as discovery candidates. (Phase 2 discovery is also being removed, so this is defense-in-depth.)

### Change 3: Fix snapshot pruning for alive-but-unmatched entries

**File**: `crates/server/src/live/manager.rs`
**Lines**: 522-525 (inside `spawn_file_watcher`, after crash recovery loop)

Current code only saves if dead PIDs were found:
```rust
if dead > 0 {
    manager.save_session_snapshot_from_state().await;
}
```

Change to unconditionally save:
```rust
// Always re-save: prunes dead entries AND alive-but-unmatched entries
// (PIDs recycled by OS, or JSONL rotated past 24h scan window).
// save_session_snapshot_from_state() writes only sessions currently
// in the in-memory map, so anything not promoted is implicitly pruned.
manager.save_session_snapshot_from_state().await;
```

`save_session_snapshot_from_state()` (line 295) serializes only sessions from the in-memory `sessions` map (filtered to non-Done, has-PID). Any snapshot entry that wasn't promoted — whether dead, PID-recycled, or simply old — is implicitly dropped. This stops the 86-entry leak.

### Change 4: Remove `has_running_process` from crash recovery metadata

**File**: `crates/server/src/live/manager.rs`
**Lines**: 415-418 (inside `spawn_file_watcher`, crash recovery enrichment)

Current code:
```rust
let processes = manager.processes.read().await;
let (_, pid) =
    has_running_process(&processes, &project_path);
drop(processes);
```

And line 443:
```rust
pid: pid.or(Some(entry.pid)),
```

Change to: delete lines 415-418 and change line 443 to:
```rust
pid: Some(entry.pid),
```

The snapshot PID is the authority here. `has_running_process` could return a different process's PID from the same directory. While `apply_jsonl_metadata` guards PID overwrite with `is_none()` (so the wrong PID wouldn't actually be applied in this specific path), the metadata struct should not contain a potentially wrong PID at all. Clean it up.

### Change 5: Remove `has_running_process` from `process_jsonl_update`

**File**: `crates/server/src/live/manager.rs`
**Lines**: 1427-1430

Current code:
```rust
let processes = self.processes.read().await;
let (_, pid) = has_running_process(&processes, &project_path);
drop(processes);
```

And the metadata struct uses `pid` at line 1435.

Change to: delete lines 1427-1430 and set `pid: None` in the metadata struct.

`apply_jsonl_metadata` (line 189) only applies the PID if `session.pid.is_none()`. For hook-created sessions, PID is already set. For sessions without a PID, we should NOT guess from the process table — the next hook event will deliver the real PID.

### Change 6: Remove `has_running_process` import and mark unused code

**File**: `crates/server/src/live/manager.rs`
**Line**: 22

After changes 1, 4, and 5, `has_running_process` and `ClaudeProcess` are no longer used in `manager.rs`.

Change:
```rust
use super::process::{detect_claude_processes, has_running_process, is_pid_alive, ClaudeProcess};
```

To:
```rust
use super::process::{detect_claude_processes, is_pid_alive};
```

`has_running_process`, `find_process_for_project`, and `ClaudeProcess` remain exported from `process.rs` for tests and potential future display use, but are no longer on any binding or creation path.

### Change 7: Remove `processes` field from `LiveSessionManager` (optional cleanup)

**File**: `crates/server/src/live/manager.rs`

The `processes: RwLock<HashMap<PathBuf, ClaudeProcess>>` field (line ~228) is only needed now for:
1. `run_eager_process_scan` at startup (line 329-335)
2. Phase 2.1 refresh (line 724-733) which updates `process_count`

Both of these populate the map, but nothing reads it anymore after changes 1, 4, and 5.

**Option A (minimal):** Keep the field but stop populating it. Change Phase 2.1 to only update `process_count` without storing the map:

```rust
let (_, total_count) =
    tokio::task::spawn_blocking(detect_claude_processes)
        .await
        .unwrap_or_default();
manager.process_count.store(total_count, Ordering::Relaxed);
```

**Option B (thorough):** Remove the `processes` field entirely, remove `run_eager_process_scan`, and inline just the count extraction into the reconciliation tick.

Recommend Option A for this PR — smaller diff, same correctness. Option B can be a follow-up cleanup.

## Reconciliation Loop Startup: Race With File Watcher

The current startup has a subtle ordering dependency:

1. `spawn_file_watcher` starts → runs eager scan + initial JSONL scan + snapshot promotion → starts file watcher
2. `spawn_reconciliation_loop` starts → loads snapshot → tries to bind PIDs to sessions in map

Both are `tokio::spawn` — they can race. If the reconciliation loop's startup (lines 614-642) runs before the file watcher's crash recovery (lines 382-527), the sessions map is empty, `current_count = 0`, and it saves an empty snapshot, wiping all entries before they can be promoted.

**This PR doesn't change the race** but documents it. The reconciliation loop's startup PID binding (lines 614-642) is redundant with the crash recovery in `spawn_file_watcher`. It was an extra layer, not the primary path. With Phase 2 discovery removed, the reconciliation loop's startup becomes even less important — crash recovery in `spawn_file_watcher` is the sole recovery path.

**Recommended follow-up:** Remove the redundant snapshot load at lines 614-642 entirely. Crash recovery in `spawn_file_watcher` already handles it.

## Edge Cases

| Case | Before (broken) | After (fixed) |
|------|-----------------|---------------|
| Dead session + new Claude in same dir | Ghost resurrection via cwd match | No discovery path; dead accumulator cleaned at startup |
| Multiple sessions in same cwd | Arbitrary cwd match picks one | Each session bound to its own hook-delivered PID |
| Server restart, session still alive | Snapshot promotes with correct PID ✅ | Same, plus stale accumulators cleaned |
| Server restart, PID recycled to non-Claude | Snapshot promotes (false positive), session sits as zombie until recycled PID dies | Snapshot pruned unconditionally; only promoted sessions survive |
| Hook never fires (pre-started session, no activity) | Phase 2 discovers via cwd match (wrong PID risk) | Session not tracked until next hook event delivers real PID |
| Session idle for hours, no hooks | Process count still shows it (from scan) | Same; session stays in map if PID alive (Phase 1 checks) |

### Tradeoff: Pre-started sessions without hooks

The removed Phase 2 discovery means sessions that started before the server launched AND never fire another hook event will not appear in the live monitor. This is acceptable because:

1. Hook events fire on every user turn, tool use, and stop — a session that never fires a hook is truly idle with no interaction.
2. The next user interaction triggers a hook, which creates the session immediately.
3. The alternative (cwd matching) has proven to create wrong bindings, which is worse than missing an idle session.

## Files Touched

| File | Change | Lines |
|------|--------|-------|
| `crates/server/src/live/manager.rs` | Remove Phase 2 discovery + PID binding | 735-965 |
| `crates/server/src/live/manager.rs` | Clean accumulators for dead snapshot PIDs | 396-399 |
| `crates/server/src/live/manager.rs` | Unconditional snapshot save after recovery | 522-525 |
| `crates/server/src/live/manager.rs` | Remove `has_running_process` from crash recovery | 415-418, 443 |
| `crates/server/src/live/manager.rs` | Remove `has_running_process` from `process_jsonl_update` | 1427-1430, 1435 |
| `crates/server/src/live/manager.rs` | Clean up imports | 22 |

## Not Doing

- Removing `detect_claude_processes()` — still needed for `process_count` display.
- Removing `processes` field — Option A (stop reading it) is sufficient for this PR.
- Removing reconciliation loop startup snapshot load (lines 614-642) — follow-up PR.
- Changing snapshot format — v2 is fine. The leak is fixed by unconditional save, not format change.
- PPID chain validation (checking parent of parent) — unnecessary. Hook-delivered PID is direct and authoritative.

## Verification

1. `cargo test -p claude-view-server` — all existing tests pass.
2. Delete `~/.claude/live-monitor-pids.json` (clear stale entries).
3. Start server, start a Claude session, verify it appears in live monitor within 1 hook event.
4. Kill the Claude session, verify it disappears within 10s (Phase 1 liveness).
5. Start two Claude sessions in the same directory, verify each has its own PID and they don't cross-bind.
6. Kill server while sessions are running, restart, verify crash recovery promotes the right sessions.
7. Kill one Claude session while server is down, restart, verify dead session is NOT resurrected.
