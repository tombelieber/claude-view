# PPID-Based Session Liveness Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace flaky sysinfo-based process detection with direct PID delivery via `$PPID` in hook curl headers, plus a disk snapshot for server restart recovery.

**Architecture:** Every hook curl command includes an `X-Claude-PID` header with `$PPID`. The Axum hook handler extracts it and binds the PID to the session. The process detector simplifies from a full system scan to `kill(pid, 0)` per session. A small JSON file at `~/.claude/live-monitor-pids.json` persists PID bindings across server restarts.

**Tech Stack:** Rust (Axum, libc, serde_json, tokio), existing hook infrastructure

**Design doc:** `docs/plans/2026-02-20-ppid-session-liveness-design.md`

---

### Task 1: Add `is_pid_alive` helper to process.rs

**Files:**
- Modify: `crates/server/src/live/process.rs`

**Step 1: Write the failing test**

Add at the end of the existing `#[cfg(test)] mod tests` block in `process.rs`:

```rust
#[test]
fn test_is_pid_alive_current_process() {
    // Our own PID should be alive
    let pid = std::process::id();
    assert!(is_pid_alive(pid));
}

#[test]
fn test_is_pid_alive_nonexistent() {
    // PID 4_000_000 is above typical PID limits and should not exist
    assert!(!is_pid_alive(4_000_000));
}

#[test]
fn test_is_pid_alive_rejects_zero_and_one() {
    // PID 0 (kernel) and 1 (init/launchd) should be rejected
    assert!(!is_pid_alive(0));
    assert!(!is_pid_alive(1));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p vibe-recall-server is_pid_alive`
Expected: FAIL — `is_pid_alive` not found

**Step 3: Write minimal implementation**

Add above the `#[cfg(test)]` block in `process.rs`:

```rust
/// Check if a process with the given PID is still alive.
///
/// Uses `kill(pid, 0)` which checks process existence without sending a signal.
/// Returns `false` for PIDs <= 1 (kernel/init) to guard against reparented processes.
pub fn is_pid_alive(pid: u32) -> bool {
    if pid <= 1 {
        return false;
    }
    // SAFETY: kill with signal 0 does not send a signal, only checks existence.
    // Returns 0 if process exists and we have permission, -1 with ESRCH if not.
    unsafe { libc::kill(pid as i32, 0) == 0 }
}
```

Add `use libc;` to the imports at the top of `process.rs` (after the existing `use` statements, but only needed inside the function — no top-level import needed since it's a direct path call, but for clarity add nothing — `libc::kill` works because `libc` is in Cargo.toml).

**Step 4: Run test to verify it passes**

Run: `cargo test -p vibe-recall-server is_pid_alive`
Expected: 3 tests PASS

**Step 5: Commit**

```bash
git add crates/server/src/live/process.rs
git commit -m "feat(live): add is_pid_alive helper using kill(pid, 0)"
```

---

### Task 2: Add PID snapshot persistence

**Files:**
- Modify: `crates/server/src/live/manager.rs`

**Step 1: Write the failing tests**

Add these tests to the existing `#[cfg(test)] mod tests` block at the end of `manager.rs`:

```rust
#[test]
fn test_pid_snapshot_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pids.json");

    let mut pids = HashMap::new();
    pids.insert("session-abc".to_string(), 12345u32);
    pids.insert("session-def".to_string(), 67890u32);

    save_pid_snapshot(&path, &pids);
    let loaded = load_pid_snapshot(&path);

    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded.get("session-abc"), Some(&12345u32));
    assert_eq!(loaded.get("session-def"), Some(&67890u32));
}

#[test]
fn test_pid_snapshot_missing_file() {
    let path = std::path::PathBuf::from("/tmp/nonexistent-pid-snapshot-test.json");
    let loaded = load_pid_snapshot(&path);
    assert!(loaded.is_empty());
}

#[test]
fn test_pid_snapshot_corrupt_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pids.json");
    std::fs::write(&path, "not valid json {{{").unwrap();

    let loaded = load_pid_snapshot(&path);
    assert!(loaded.is_empty());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p vibe-recall-server pid_snapshot`
Expected: FAIL — `save_pid_snapshot` and `load_pid_snapshot` not found

**Step 3: Write minimal implementation**

Add these functions above the `#[cfg(test)]` block in `manager.rs` (after the `seconds_since_modified_from_timestamp` function around line 998):

```rust
/// Path to the PID snapshot file for server restart recovery.
fn pid_snapshot_path() -> PathBuf {
    dirs::home_dir()
        .expect("home dir exists")
        .join(".claude")
        .join("live-monitor-pids.json")
}

/// Save the session-to-PID mapping to disk atomically.
///
/// Written as `{ "session_id": pid, ... }`. Uses tmp+rename for crash safety.
fn save_pid_snapshot(path: &Path, pids: &HashMap<String, u32>) {
    let content = match serde_json::to_string(pids) {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to serialize PID snapshot: {}", e);
            return;
        }
    };
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, &content).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

/// Load the session-to-PID mapping from disk.
///
/// Returns an empty map if the file is missing or corrupt.
fn load_pid_snapshot(path: &Path) -> HashMap<String, u32> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return HashMap::new(),
    };
    serde_json::from_str(&content).unwrap_or_default()
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p vibe-recall-server pid_snapshot`
Expected: 3 tests PASS

**Step 5: Commit**

```bash
git add crates/server/src/live/manager.rs
git commit -m "feat(live): add PID snapshot save/load for restart recovery"
```

---

### Task 3: Add `X-Claude-PID` header to hook curl command

**Files:**
- Modify: `crates/server/src/live/hook_registrar.rs`

**Step 1: Write the failing test**

Add to the existing `#[cfg(test)] mod tests` block in `hook_registrar.rs`:

```rust
#[test]
fn test_hook_command_includes_ppid_header() {
    let group = make_matcher_group(47892, "SessionStart");
    let handler = &group["hooks"][0];
    let command = handler["command"].as_str().unwrap();
    assert!(
        command.contains("X-Claude-PID"),
        "Hook command must include X-Claude-PID header, got: {}",
        command
    );
    // $PPID must NOT be inside quotes (shell must expand it)
    assert!(
        command.contains("'$PPID"),
        "PPID must be outside quotes for shell expansion, got: {}",
        command
    );
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p vibe-recall-server hook_command_includes_ppid`
Expected: FAIL — assertion fails (no X-Claude-PID in current command)

**Step 3: Write minimal implementation**

In `hook_registrar.rs`, modify the `make_hook_handler` function. Change the `command` format string from:

```rust
let command = format!(
    "curl -s -X POST http://localhost:{}/api/live/hook \
     -H 'Content-Type: application/json' \
     --data-binary @- 2>/dev/null || true {}",
    port, SENTINEL
);
```

to:

```rust
let command = format!(
    "curl -s -X POST http://localhost:{}/api/live/hook \
     -H 'Content-Type: application/json' \
     -H 'X-Claude-PID: '$PPID \
     --data-binary @- 2>/dev/null || true {}",
    port, SENTINEL
);
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p vibe-recall-server hook_command_includes_ppid`
Expected: PASS

Also run all existing hook_registrar tests to ensure no regressions:
Run: `cargo test -p vibe-recall-server hook_registrar`
Expected: All existing tests PASS

**Step 5: Commit**

```bash
git add crates/server/src/live/hook_registrar.rs
git commit -m "feat(live): add X-Claude-PID header to hook curl command"
```

---

### Task 4: Extract PID from header in hook handler

**Files:**
- Modify: `crates/server/src/routes/hooks.rs`

**Step 1: Write the failing test**

Add a helper function and test to the existing `#[cfg(test)] mod tests` block in `hooks.rs`:

```rust
#[test]
fn test_extract_pid_from_header_valid() {
    let pid = extract_pid_from_header(Some("12345"));
    assert_eq!(pid, Some(12345));
}

#[test]
fn test_extract_pid_from_header_invalid() {
    assert_eq!(extract_pid_from_header(None), None);
    assert_eq!(extract_pid_from_header(Some("")), None);
    assert_eq!(extract_pid_from_header(Some("abc")), None);
    assert_eq!(extract_pid_from_header(Some("0")), None);
    assert_eq!(extract_pid_from_header(Some("1")), None);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p vibe-recall-server extract_pid_from_header`
Expected: FAIL — `extract_pid_from_header` not found

**Step 3: Write minimal implementation**

Add this function near the bottom of `hooks.rs` (after `short_path`, before `#[cfg(test)]`):

```rust
/// Extract and validate a PID from the X-Claude-PID header value.
///
/// Returns None if the header is missing, empty, non-numeric, or <= 1
/// (PID 0 = kernel, PID 1 = init/launchd — indicates reparenting).
fn extract_pid_from_header(header_value: Option<&str>) -> Option<u32> {
    let value = header_value?.trim();
    let pid: u32 = value.parse().ok()?;
    if pid <= 1 {
        return None;
    }
    Some(pid)
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p vibe-recall-server extract_pid_from_header`
Expected: 2 tests PASS

**Step 5: Now wire the header extraction into `handle_hook`**

Modify the `handle_hook` function signature to accept headers. Change:

```rust
async fn handle_hook(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<HookPayload>,
) -> Json<serde_json::Value> {
```

to:

```rust
async fn handle_hook(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<HookPayload>,
) -> Json<serde_json::Value> {
```

Then, right after the early return for `auth_success` (after line 59), extract the PID:

```rust
let claude_pid = extract_pid_from_header(
    headers.get("x-claude-pid").and_then(|v| v.to_str().ok()),
);
```

Then, in every place where a `LiveSession` is created or updated and `pid: None` is set, change to `pid: claude_pid`. There are two session creation sites:

1. Lazy creation (around line 90-120): change `pid: None,` to `pid: claude_pid,`
2. SessionStart creation (around line 160-186): change `pid: None,` to `pid: claude_pid,`

And for session updates (when session already exists), add PID binding if not yet set. After each `if let Some(session) = sessions.get_mut(...)` block that updates the session, add:

```rust
if session.pid.is_none() {
    if let Some(pid) = claude_pid {
        session.pid = Some(pid);
    }
}
```

Add this to the SessionStart existing-session branch, UserPromptSubmit, Stop, and the catch-all `_` arm.

**Step 6: Run full hook tests**

Run: `cargo test -p vibe-recall-server routes::hooks`
Expected: All tests PASS

**Step 7: Commit**

```bash
git add crates/server/src/routes/hooks.rs
git commit -m "feat(live): extract PID from X-Claude-PID header and bind to sessions"
```

---

### Task 5: Simplify process detector to use `kill(pid, 0)`

**Files:**
- Modify: `crates/server/src/live/manager.rs`

**Step 1: Write the failing test**

Add to the `#[cfg(test)]` block in `manager.rs`:

```rust
#[test]
fn test_pid_based_eviction_alive_process() {
    // Our own PID should be considered alive
    let our_pid = std::process::id();
    assert!(super::super::process::is_pid_alive(our_pid));
}

#[test]
fn test_pid_based_eviction_dead_process() {
    // Very high PID should not exist
    assert!(!super::super::process::is_pid_alive(4_000_000));
}
```

**Step 2: Run test to verify it passes**

Run: `cargo test -p vibe-recall-server pid_based_eviction`
Expected: PASS (these tests use `is_pid_alive` from Task 1)

**Step 3: Rewrite `spawn_process_detector`**

Replace the entire `spawn_process_detector` method body (manager.rs, approximately lines 359-496) with:

```rust
fn spawn_process_detector(self: &Arc<Self>) {
    let manager = self.clone();
    const STALE_THRESHOLD_SECS: u64 = 600; // 10 minutes — generous fallback

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(10));

        // On startup, load PID snapshot from disk to recover bindings
        let snapshot = load_pid_snapshot(&pid_snapshot_path());
        if !snapshot.is_empty() {
            let mut sessions = manager.sessions.write().await;
            for (session_id, pid) in &snapshot {
                if let Some(session) = sessions.get_mut(session_id) {
                    if session.pid.is_none() {
                        session.pid = Some(*pid);
                    }
                }
            }
            info!(
                count = snapshot.len(),
                "Restored PID bindings from snapshot"
            );
        }

        loop {
            interval.tick().await;

            let mut dead_sessions: Vec<String> = Vec::new();
            let mut snapshot_dirty = false;
            {
                let mut sessions = manager.sessions.write().await;

                for (session_id, session) in sessions.iter_mut() {
                    if session.status == SessionStatus::Done {
                        continue;
                    }

                    if let Some(pid) = session.pid {
                        // PID bound — definitive check
                        if !super::process::is_pid_alive(pid) {
                            info!(
                                session_id = %session_id,
                                pid = pid,
                                "Bound PID is dead — marking session ended"
                            );
                            session.agent_state = AgentState {
                                group: AgentStateGroup::NeedsYou,
                                state: "session_ended".into(),
                                label: "Session ended (process exited)".into(),
                                context: None,
                            };
                            session.status = SessionStatus::Done;
                            let _ = manager.tx.send(SessionEvent::SessionUpdated {
                                session: session.clone(),
                            });
                            dead_sessions.push(session_id.clone());
                            snapshot_dirty = true;
                        }
                    } else {
                        // No PID bound — stale fallback (should rarely happen with PPID)
                        let seconds_since =
                            seconds_since_modified_from_timestamp(session.last_activity_at);
                        if seconds_since > STALE_THRESHOLD_SECS {
                            info!(
                                session_id = %session_id,
                                stale_seconds = seconds_since,
                                "No PID + stale — marking session ended (fallback)"
                            );
                            session.agent_state = AgentState {
                                group: AgentStateGroup::NeedsYou,
                                state: "session_ended".into(),
                                label: "Session ended (no process)".into(),
                                context: None,
                            };
                            session.status = SessionStatus::Done;
                            let _ = manager.tx.send(SessionEvent::SessionUpdated {
                                session: session.clone(),
                            });
                            dead_sessions.push(session_id.clone());
                        }
                    }
                }

                // Remove dead sessions from map
                for session_id in &dead_sessions {
                    sessions.remove(session_id);
                }

                // Save PID snapshot if any bindings changed
                if snapshot_dirty {
                    let pids: HashMap<String, u32> = sessions
                        .iter()
                        .filter_map(|(id, s)| s.pid.map(|pid| (id.clone(), pid)))
                        .collect();
                    save_pid_snapshot(&pid_snapshot_path(), &pids);
                }
            }

            // Broadcast completions (outside lock)
            for session_id in dead_sessions {
                let _ = manager
                    .tx
                    .send(SessionEvent::SessionCompleted { session_id });
            }
        }
    });
}
```

Also add a PID snapshot save call to the hook handler flow. In `manager.rs`, add a new public method:

```rust
/// Save current PID bindings to disk for restart recovery.
pub async fn save_pid_bindings(&self) {
    let sessions = self.sessions.read().await;
    let pids: HashMap<String, u32> = sessions
        .iter()
        .filter_map(|(id, s)| s.pid.map(|pid| (id.clone(), pid)))
        .collect();
    save_pid_snapshot(&pid_snapshot_path(), &pids);
}
```

**Step 4: Remove the old `had_process` / `detect_claude_processes` import from the detector loop**

The import at line 22:
```rust
use super::process::{detect_claude_processes, has_running_process, ClaudeProcess};
```

Keep `has_running_process` and `ClaudeProcess` (still used by `process_jsonl_file` for display), but `detect_claude_processes` is no longer used in the hot loop. It may still be referenced elsewhere — leave the import but add `#[allow(unused_imports)]` if the compiler warns.

Also remove `processes` and `process_count` fields from `LiveSessionManager` struct if they are only used by the old detector. Check all references first — `process_jsonl_file` at line 896 reads `self.processes` so it's still needed for initial PID discovery during JSONL parsing. Leave the fields and the eager scan but they become secondary to hook-delivered PIDs.

**Step 5: Update old tests**

Replace the `test_process_disappearance_immediate_detection` and `test_never_seen_process_uses_stale_fallback` tests (they test the old `had_process` logic) with:

```rust
#[test]
fn test_pid_bound_session_eviction_logic() {
    use super::process::is_pid_alive;

    // Session with a bound PID that is dead
    let dead_pid: u32 = 4_000_000;
    assert!(!is_pid_alive(dead_pid));

    // Session with a bound PID that is alive (our own process)
    let alive_pid = std::process::id();
    assert!(is_pid_alive(alive_pid));
}

#[test]
fn test_stale_fallback_for_sessions_without_pid() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    const STALE_THRESHOLD_SECS: u64 = 600;

    // Not stale enough (10s ago)
    let seconds_since = seconds_since_modified_from_timestamp(now - 10);
    assert!(seconds_since < STALE_THRESHOLD_SECS);

    // Stale enough (700s ago)
    let seconds_since = seconds_since_modified_from_timestamp(now - 700);
    assert!(seconds_since > STALE_THRESHOLD_SECS);
}
```

Keep the `test_done_session_not_reprocessed` and `test_pid_binding_prevents_zombie_sessions` tests as they are — they still validate relevant behavior.

**Step 6: Run all manager tests**

Run: `cargo test -p vibe-recall-server live::manager`
Expected: All tests PASS

**Step 7: Commit**

```bash
git add crates/server/src/live/manager.rs
git commit -m "feat(live): simplify process detector to kill(pid,0) with PID snapshot"
```

---

### Task 6: Wire PID snapshot save into hook handler

**Files:**
- Modify: `crates/server/src/routes/hooks.rs`

**Step 1: Add snapshot save call after PID binding**

At the end of `handle_hook`, before the final `Json(...)` return, add:

```rust
// Persist PID bindings to disk when a new PID was bound
if claude_pid.is_some() {
    if let Some(mgr) = &state.live_manager {
        mgr.save_pid_bindings().await;
    }
}
```

**Step 2: Run all tests to verify no regressions**

Run: `cargo test -p vibe-recall-server`
Expected: All tests PASS

**Step 3: Commit**

```bash
git add crates/server/src/routes/hooks.rs
git commit -m "feat(live): trigger PID snapshot save on hook PID binding"
```

---

### Task 7: Integration verification

**Step 1: Build the project**

Run: `cargo build -p vibe-recall-server`
Expected: Compiles without errors or warnings

**Step 2: Run full test suite**

Run: `cargo test -p vibe-recall-server`
Expected: All tests PASS

**Step 3: Manual verification**

1. Start the server: `cargo run -p vibe-recall-server`
2. Check that hooks are re-registered with the new header:
   `cat ~/.claude/settings.json | grep X-Claude-PID`
   Expected: Every hook command contains `-H 'X-Claude-PID: '$PPID`
3. Start a Claude Code session
4. Check the PID snapshot file:
   `cat ~/.claude/live-monitor-pids.json`
   Expected: Contains session ID mapped to a PID
5. Verify the PID matches:
   `ps -p <pid_from_snapshot> -o pid,command`
   Expected: Shows a Claude process
6. Kill the server, restart it, verify the session re-appears with correct PID

**Step 4: Commit any fixups**

```bash
git add -A
git commit -m "fix(live): integration fixups for PPID session liveness"
```
