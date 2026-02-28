# PPID-Based Session Liveness Detection

**Date**: 2026-02-20
**Status**: Approved
**Branch**: feature/mission-control-cde

## Problem

The live monitor's session eviction logic is flaky. The current approach uses:

1. `sysinfo` process table scans (unreliable on macOS — misses processes)
2. Bidirectional CWD matching for PID binding (false positives with nested dirs)
3. A 180s stale timeout as fallback when no process is ever observed

This causes:
- False evictions of live sessions (user idle at prompt > 3 min)
- Wrong PID binding when sessions share parent/child directories
- `System::new()` created fresh every 10s poll (expensive, no history)
- Test/prod threshold mismatch (tests use 300s, prod uses 180s)

## Solution

Pass Claude Code's PID directly via the hook mechanism using `$PPID` shell variable. Every hook command runs in a shell spawned by Claude Code, so `$PPID` is Claude's PID.

### Data Flow

```
Claude Code spawns hook shell
  -> shell inherits PPID = Claude's PID
  -> curl sends X-Claude-PID header
  -> handle_hook extracts PID, binds to session
  -> PID snapshot written to disk (atomic JSON)
  -> process detector: kill(pid, 0) every 10s
  -> PID gone? -> immediate eviction

Server restart:
  -> load PID snapshot from disk
  -> validate each PID with kill(pid, 0)
  -> alive PIDs -> sessions restored with bound PID
  -> dead PIDs -> discard from snapshot
```

## Changes

### 1. Hook command — pass `$PPID` as header

**File**: `crates/server/src/live/hook_registrar.rs`

Add `-H 'X-Claude-PID: '$PPID` to the curl command template. Every hook event (not just SessionStart) carries Claude's PID. The `$PPID` is unquoted so the shell expands it.

### 2. Hook handler — extract and bind PID

**File**: `crates/server/src/routes/hooks.rs`

- Read `X-Claude-PID` from request headers (not JSON body)
- Parse to `u32`, validate > 1 (guard against init/launchd reparenting)
- Set `session.pid` on session creation and on every hook update
- Trigger PID snapshot write on binding change

### 3. PID snapshot file — survive server restarts

**File**: `crates/server/src/live/manager.rs` (new helper functions)

- Path: `~/.claude/live-monitor-pids.json`
- Format: `{ "session_id": pid, ... }` (simple JSON object)
- Written atomically: `tmp + rename` (same pattern as hook_registrar.rs)
- Written on every PID binding change (infrequent — once per session)
- Loaded on server startup to pre-populate session PIDs
- Cleared on graceful shutdown

### 4. Process detector — simplify to `kill(pid, 0)`

**File**: `crates/server/src/live/manager.rs`

Replace `spawn_process_detector` internals:
- For sessions with a bound PID: `kill(pid, 0)` (single syscall, definitive)
- Remove `System::new()` + `detect_claude_processes()` from polling loop
- Remove `had_process` HashSet (no longer needed — PID is known from first hook)
- Keep a generous stale fallback (600s) for the edge case where PID is never received

### 5. Simplify process.rs

**File**: `crates/server/src/live/process.rs`

- Add `is_pid_alive(pid: u32) -> bool` using `kill(pid, 0)` via `libc::kill`
- `detect_claude_processes()` and CWD matching kept for potential display/debug
- No longer used in the hot polling path

## Edge Cases

| Case | Handling |
|------|----------|
| `$PPID` = 1 (reparented to init) | Reject; session works without PID |
| `$PPID` empty or non-numeric | Ignore header; session works without PID |
| Server restart, session idle | PID snapshot restores binding; `kill(pid,0)` validates |
| Server restart, no snapshot file | Hooks re-deliver PID on next activity |
| Multiple sessions in same cwd | Each has unique PID — no confusion |
| PID recycled by OS | Extremely unlikely in 10s window; `kill(pid,0)` on a non-Claude PID returns true but SessionEnd hook would have fired |
| Old Claude Code (no header support) | Header absent; falls back to stale path (600s) |

## Files Touched

| File | Change |
|------|--------|
| `crates/server/src/live/hook_registrar.rs` | Add `X-Claude-PID` header to curl |
| `crates/server/src/routes/hooks.rs` | Extract PID from header, bind to session |
| `crates/server/src/live/manager.rs` | Simplify process detector, add snapshot load/save |
| `crates/server/src/live/process.rs` | Add `is_pid_alive()`, keep existing fns for compat |

## Not Doing

- mmap for snapshot (data is ~1KB, atomic file write is simpler and crash-safe)
- Removing `detect_claude_processes()` entirely (useful for debug/display)
- Changing hook registration format (just adding a header to existing curl)
