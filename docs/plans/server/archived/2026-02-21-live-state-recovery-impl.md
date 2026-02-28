# Live State Recovery Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Persist agent_state alongside PID bindings so the live monitor instantly restores sessions after a server crash/restart.

**Architecture:** Extend the existing atomic PID snapshot (`~/.claude/live-monitor-pids.json`) to include `agent_state`, `status`, and `last_activity_at` per session. On startup, after warming accumulators from JSONL, promote entries with alive PIDs into full `LiveSession` map entries. No new persistence layer — piggybacks on existing infrastructure.

**Tech Stack:** Rust (serde, tokio), existing `manager.rs` + `state.rs` + `hooks.rs`

**Design doc:** `docs/plans/2026-02-21-live-state-recovery-design.md`

---

### Task 1: Add `SnapshotEntry` struct and v2 snapshot format

**Files:**
- Modify: `crates/server/src/live/state.rs` (append after `HookEvent` struct, ~line 148)
- Modify: `crates/server/src/live/manager.rs:1060-1094` (snapshot save/load functions)
- Test: `crates/server/src/live/manager.rs` (existing `#[cfg(test)]` module)

**Step 1: Write the failing test**

Add to `crates/server/src/live/manager.rs` test module:

```rust
#[test]
fn test_snapshot_v2_round_trip() {
    use crate::live::state::{AgentState, AgentStateGroup, SessionSnapshot, SnapshotEntry};
    use std::collections::HashMap;

    let mut entries = HashMap::new();
    entries.insert("session-1".to_string(), SnapshotEntry {
        pid: 12345,
        status: "working".to_string(),
        agent_state: AgentState {
            group: AgentStateGroup::Autonomous,
            state: "acting".into(),
            label: "Working".into(),
            context: None,
        },
        last_activity_at: 1708500000,
    });
    let snapshot = SessionSnapshot { version: 2, sessions: entries };

    let json = serde_json::to_string(&snapshot).unwrap();
    let parsed: SessionSnapshot = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.version, 2);
    assert_eq!(parsed.sessions.len(), 1);
    let entry = &parsed.sessions["session-1"];
    assert_eq!(entry.pid, 12345);
    assert_eq!(entry.status, "working");
    assert_eq!(entry.agent_state.group, AgentStateGroup::Autonomous);
    assert_eq!(entry.last_activity_at, 1708500000);
}

#[test]
fn test_snapshot_v1_migration() {
    // Legacy format: { "session-id": 12345 }
    let v1_json = r#"{"session-abc": 12345, "session-def": 67890}"#;
    let snapshot = load_session_snapshot_from_str(v1_json);

    assert_eq!(snapshot.version, 2);
    assert_eq!(snapshot.sessions.len(), 2);
    let entry = &snapshot.sessions["session-abc"];
    assert_eq!(entry.pid, 12345);
    // v1 migration: default agent_state is Autonomous/recovered
    assert_eq!(entry.agent_state.state, "recovered");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p claude-view-server test_snapshot_v2_round_trip test_snapshot_v1_migration`
Expected: FAIL — `SessionSnapshot`, `SnapshotEntry`, `load_session_snapshot_from_str` don't exist

**Step 3: Add `SnapshotEntry` and `SessionSnapshot` to `state.rs`**

Append after the `HookEvent` struct (after line 148 in `state.rs`):

```rust
/// A per-session snapshot entry persisted to disk for crash recovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotEntry {
    /// Bound PID of the Claude process.
    pub pid: u32,
    /// Session status as string: "working", "paused", "done".
    pub status: String,
    /// Last known agent state (from hooks).
    pub agent_state: AgentState,
    /// Unix timestamp of last activity.
    pub last_activity_at: i64,
}

/// The on-disk snapshot format (v2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub version: u8,
    pub sessions: std::collections::HashMap<String, SnapshotEntry>,
}
```

Also add `PartialEq` to `AgentStateGroup` if not already present (it is — line 24 has `PartialEq, Eq`). Good.

Add `Deserialize` to `AgentState` (line 10 already has `Deserialize`). Good.

**Step 4: Add v2 save/load + v1 migration to `manager.rs`**

Replace `save_pid_snapshot` and `load_pid_snapshot` (lines 1068-1094) with:

```rust
/// Save the extended session snapshot to disk atomically.
fn save_session_snapshot(path: &Path, snapshot: &SessionSnapshot) {
    let content = match serde_json::to_string(snapshot) {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to serialize session snapshot: {}", e);
            return;
        }
    };
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, &content).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

/// Load the session snapshot from disk, handling v1→v2 migration.
fn load_session_snapshot(path: &Path) -> SessionSnapshot {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return SessionSnapshot { version: 2, sessions: HashMap::new() },
    };
    load_session_snapshot_from_str(&content)
}

/// Parse a snapshot string, auto-detecting v1 (bare pid map) vs v2 (structured).
fn load_session_snapshot_from_str(content: &str) -> SessionSnapshot {
    // Try v2 first
    if let Ok(snapshot) = serde_json::from_str::<SessionSnapshot>(content) {
        return snapshot;
    }
    // Fall back to v1: { "session_id": pid, ... }
    if let Ok(v1) = serde_json::from_str::<HashMap<String, u32>>(content) {
        let sessions = v1.into_iter().map(|(id, pid)| {
            (id, SnapshotEntry {
                pid,
                status: "working".to_string(),
                agent_state: AgentState {
                    group: AgentStateGroup::Autonomous,
                    state: "recovered".into(),
                    label: "Recovered from restart".into(),
                    context: None,
                },
                last_activity_at: 0,
            })
        }).collect();
        return SessionSnapshot { version: 2, sessions };
    }
    SessionSnapshot { version: 2, sessions: HashMap::new() }
}
```

Add import at top of `manager.rs`: `use super::state::{SnapshotEntry, SessionSnapshot};`

**Step 5: Run tests to verify they pass**

Run: `cargo test -p claude-view-server test_snapshot_v2_round_trip test_snapshot_v1_migration`
Expected: PASS

**Step 6: Commit**

```bash
git add crates/server/src/live/state.rs crates/server/src/live/manager.rs
git commit -m "feat: add v2 session snapshot format with agent_state persistence"
```

---

### Task 2: Migrate all snapshot callers to v2 format

**Files:**
- Modify: `crates/server/src/live/manager.rs:239-246` (`save_pid_bindings`)
- Modify: `crates/server/src/live/manager.rs:384-409` (`spawn_process_detector` PID restore)
- Modify: `crates/server/src/live/manager.rs:460-466` (process detector dirty save)

**Step 1: Update `save_pid_bindings` to build `SessionSnapshot`**

Replace `save_pid_bindings` (lines 239-246):

```rust
pub async fn save_session_snapshot_from_state(&self) {
    let sessions = self.sessions.read().await;
    let entries: HashMap<String, SnapshotEntry> = sessions
        .iter()
        .filter(|(_, s)| s.status != SessionStatus::Done)
        .filter_map(|(id, s)| {
            s.pid.map(|pid| (id.clone(), SnapshotEntry {
                pid,
                status: match s.status {
                    SessionStatus::Working => "working".to_string(),
                    SessionStatus::Paused => "paused".to_string(),
                    SessionStatus::Done => "done".to_string(),
                },
                agent_state: s.agent_state.clone(),
                last_activity_at: s.last_activity_at,
            }))
        })
        .collect();
    save_session_snapshot(&pid_snapshot_path(), &SessionSnapshot {
        version: 2,
        sessions: entries,
    });
}
```

**Step 2: Update process detector snapshot restore (lines 384-409)**

Replace the PID snapshot loading block in `spawn_process_detector`:

```rust
// On startup, load session snapshot from disk to recover bindings
let snapshot = load_session_snapshot(&pid_snapshot_path());
if !snapshot.sessions.is_empty() {
    let mut sessions = manager.sessions.write().await;
    for (session_id, entry) in &snapshot.sessions {
        if let Some(session) = sessions.get_mut(session_id) {
            if session.pid.is_none() {
                session.pid = Some(entry.pid);
            }
        }
    }
    // Prune stale entries and save back
    let current_count = sessions
        .iter()
        .filter(|(_, s)| s.pid.is_some())
        .count();
    if current_count < snapshot.sessions.len() {
        manager.save_session_snapshot_from_state().await;
    }

    info!(
        count = snapshot.sessions.len(),
        restored = current_count,
        "Restored PID bindings from snapshot"
    );
}
```

**Step 3: Update process detector dirty save (lines 460-466)**

Replace the `if snapshot_dirty` block:

```rust
if snapshot_dirty {
    manager.save_session_snapshot_from_state().await;
}
```

Note: this requires changing the closure to move `manager` reference. The `manager` clone is already available (`let manager = self.clone();` at line 379).

**Step 4: Update hook handler caller**

In `crates/server/src/routes/hooks.rs:481-484`, rename the call:

```rust
if pid_newly_bound {
    if let Some(mgr) = &state.live_manager {
        mgr.save_session_snapshot_from_state().await;
    }
}
```

**Step 5: Build to verify no compilation errors**

Run: `cargo build -p claude-view-server`
Expected: PASS (no warnings about unused `save_pid_snapshot` / `load_pid_snapshot` — remove the old functions)

**Step 6: Run full server test suite**

Run: `cargo test -p claude-view-server`
Expected: PASS

**Step 7: Commit**

```bash
git add crates/server/src/live/manager.rs crates/server/src/routes/hooks.rs
git commit -m "refactor: migrate snapshot callers from v1 pid-only to v2 extended format"
```

---

### Task 3: Add snapshot save on agent_state changes

**Files:**
- Modify: `crates/server/src/routes/hooks.rs:87,479-487`

**Step 1: Track state changes alongside PID binding**

At line 87, alongside the existing `pid_newly_bound` flag, add:

```rust
let mut state_changed = false;
```

Then in every hook branch that updates `session.agent_state` (SessionStart, UserPromptSubmit, Stop, PreToolUse/PostToolUse, etc.), set `state_changed = true` after the state update. The relevant locations are every block that does `session.agent_state = agent_state.clone();`.

**Step 2: Trigger snapshot save on state change**

Replace the PID-only save block (lines 479-485):

```rust
// Persist snapshot when PID binding or agent state changed
if pid_newly_bound || state_changed {
    if let Some(mgr) = &state.live_manager {
        mgr.save_session_snapshot_from_state().await;
    }
}
```

**Step 3: Build and test**

Run: `cargo build -p claude-view-server && cargo test -p claude-view-server`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/server/src/routes/hooks.rs
git commit -m "feat: save session snapshot on every agent_state change for crash recovery"
```

---

### Task 4: Implement startup session promotion

**Files:**
- Modify: `crates/server/src/live/manager.rs:268-301` (`spawn_file_watcher`)
- Test: `crates/server/src/live/manager.rs` (test module)

**Step 1: Write the failing test**

Add to the test module in `manager.rs`:

```rust
#[test]
fn test_build_recovered_session_from_snapshot() {
    use crate::live::state::{
        AgentState, AgentStateGroup, LiveSession, SessionStatus, SnapshotEntry,
    };

    let entry = SnapshotEntry {
        pid: 12345,
        status: "paused".to_string(),
        agent_state: AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "awaiting_input".into(),
            label: "Asked a question".into(),
            context: None,
        },
        last_activity_at: 1708500000,
    };

    let session = build_recovered_session(
        "session-abc",
        &entry,
        "/home/user/.claude/projects/-tmp/session-abc.jsonl",
    );

    assert_eq!(session.id, "session-abc");
    assert_eq!(session.pid, Some(12345));
    assert_eq!(session.status, SessionStatus::Paused);
    assert_eq!(session.agent_state.state, "awaiting_input");
    assert_eq!(session.last_activity_at, 1708500000);
    assert_eq!(session.project_display_name, "tmp");
    assert_eq!(session.project_path, "/tmp");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p claude-view-server test_build_recovered_session_from_snapshot`
Expected: FAIL — `build_recovered_session` not defined

**Step 3: Implement `build_recovered_session`**

Add near `apply_jsonl_metadata` (~line 118 area):

```rust
/// Build a skeleton LiveSession from a crash-recovery snapshot entry.
/// The session will be enriched by `apply_jsonl_metadata` on the next JSONL poll.
fn build_recovered_session(
    session_id: &str,
    entry: &SnapshotEntry,
    file_path: &str,
) -> LiveSession {
    let path = Path::new(file_path);
    let (project, project_display_name, project_path) = extract_project_info(path);

    let status = match entry.status.as_str() {
        "working" => SessionStatus::Working,
        "paused" => SessionStatus::Paused,
        _ => status_from_agent_state(&entry.agent_state),
    };

    LiveSession {
        id: session_id.to_string(),
        project,
        project_display_name,
        project_path,
        file_path: file_path.to_string(),
        status,
        agent_state: entry.agent_state.clone(),
        git_branch: None,
        pid: Some(entry.pid),
        title: String::new(),
        last_user_message: String::new(),
        current_activity: entry.agent_state.label.clone(),
        turn_count: 0,
        started_at: None,
        last_activity_at: entry.last_activity_at,
        model: None,
        tokens: TokenUsage::default(),
        context_window_tokens: 0,
        cost: CostBreakdown::default(),
        cache_status: CacheStatus::Unknown,
        current_turn_started_at: None,
        last_turn_task_seconds: None,
        sub_agents: Vec::new(),
        progress_items: Vec::new(),
        tools_used: Vec::new(),
        last_cache_hit_at: None,
        hook_events: Vec::new(),
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p claude-view-server test_build_recovered_session_from_snapshot`
Expected: PASS

**Step 5: Add the promotion step to `spawn_file_watcher`**

In `spawn_file_watcher()`, after the accumulator warm-up loop (after line 301 `}`) and before starting the file watcher (line 303), insert:

```rust
// 3. Promote sessions from crash-recovery snapshot
//    Sessions with alive PIDs get full LiveSession entries immediately,
//    populated with metrics from accumulators and agent_state from snapshot.
{
    let snapshot = load_session_snapshot(&pid_snapshot_path());
    if !snapshot.sessions.is_empty() {
        let accumulators = manager.accumulators.read().await;
        let mut sessions = manager.sessions.write().await;
        let processes = manager.processes.read().await;
        let mut promoted = 0u32;
        let mut dead = 0u32;

        for (session_id, entry) in &snapshot.sessions {
            if sessions.contains_key(session_id) {
                continue; // Hook already created it
            }
            if !is_pid_alive(entry.pid) {
                dead += 1;
                continue; // Process died while we were down
            }

            // Find the JSONL file path from the accumulator or reconstruct it
            // The accumulator was populated in step 2 above; its offset > 0 means
            // we found and parsed the file.
            if let Some(acc) = accumulators.get(session_id) {
                // We need the file path — reconstruct from project dir scan
                // The accumulator doesn't store the path, but we can find it
                // via the initial_paths we already scanned.
                // Look up the file path from the paths we scanned.
            }

            // Simpler approach: scan initial_paths for this session_id
            if let Some(path) = initial_paths.iter().find(|p| extract_session_id(p) == *session_id) {
                let file_path_str = path.to_string_lossy().to_string();
                let mut session = build_recovered_session(session_id, entry, &file_path_str);

                // Enrich with accumulator metrics if available
                let (project, project_display_name, project_path) = extract_project_info(path);
                if let Some(acc) = accumulators.get(session_id) {
                    let (_, pid) = has_running_process(&processes, &PathBuf::from(&project_path));
                    let cost = acc.model.as_ref().map_or(CostBreakdown::default(), |model| {
                        let pricing = manager.pricing.read().unwrap();
                        pricing.get(model).map_or(CostBreakdown::default(), |p| {
                            calculate_cost(&acc.tokens, p)
                        })
                    });
                    let cache_status = if acc.tokens.cache_read > 0 {
                        CacheStatus::Warm
                    } else if acc.tokens.cache_creation > 0 {
                        CacheStatus::Cold
                    } else {
                        CacheStatus::Unknown
                    };
                    let metadata = JsonlMetadata {
                        git_branch: acc.git_branch.clone(),
                        pid: pid.or(Some(entry.pid)),
                        title: acc.first_user_message.clone(),
                        last_user_message: acc.last_user_message.clone(),
                        turn_count: acc.user_turn_count,
                        started_at: acc.started_at,
                        last_activity_at: entry.last_activity_at,
                        model: acc.model.clone(),
                        tokens: acc.tokens.clone(),
                        context_window_tokens: acc.context_window_tokens,
                        cost,
                        cache_status,
                        current_turn_started_at: acc.current_turn_started_at,
                        last_turn_task_seconds: acc.last_turn_task_seconds,
                        sub_agents: acc.sub_agents.clone(),
                        progress_items: {
                            let mut items = acc.todo_items.clone();
                            items.extend(acc.task_items.clone());
                            items
                        },
                        tools_used: {
                            let mut mcp: Vec<_> = acc.mcp_servers.iter().cloned().collect();
                            mcp.sort();
                            let mut skill: Vec<_> = acc.skills.iter().cloned().collect();
                            skill.sort();
                            let mut tools = Vec::with_capacity(mcp.len() + skill.len());
                            for name in mcp {
                                tools.push(super::state::ToolUsed {
                                    name,
                                    kind: "mcp".to_string(),
                                });
                            }
                            for name in skill {
                                tools.push(super::state::ToolUsed {
                                    name,
                                    kind: "skill".to_string(),
                                });
                            }
                            tools
                        },
                        last_cache_hit_at: acc.last_cache_hit_at,
                    };
                    apply_jsonl_metadata(
                        &mut session,
                        &metadata,
                        &file_path_str,
                        &project,
                        &project_display_name,
                        &project_path,
                    );
                }

                sessions.insert(session_id.clone(), session.clone());
                let _ = manager.tx.send(SessionEvent::SessionDiscovered { session });
                promoted += 1;
            }
        }

        drop(accumulators);
        drop(sessions);
        drop(processes);

        if promoted > 0 || dead > 0 {
            info!(
                promoted,
                dead,
                total = snapshot.sessions.len(),
                "Startup recovery: promoted sessions from crash snapshot"
            );
        }

        // Save cleaned snapshot (remove dead entries)
        if dead > 0 {
            manager.save_session_snapshot_from_state().await;
        }
    }
}
```

**Step 6: Build and run full test suite**

Run: `cargo build -p claude-view-server && cargo test -p claude-view-server`
Expected: PASS

**Step 7: Commit**

```bash
git add crates/server/src/live/manager.rs
git commit -m "feat: promote crash-snapshot sessions to live map on startup recovery"
```

---

### Task 5: End-to-end manual verification

**Step 1: Start the server**

Run: `cargo run -p claude-view-server`

**Step 2: Open the live monitor in browser**

Navigate to `http://localhost:47892` → Live Monitor tab.

**Step 3: Start a Claude session in a terminal**

Run `claude` in any project directory. Verify the session appears in the live monitor with correct agent_state.

**Step 4: Check the snapshot file**

Run: `cat ~/.claude/live-monitor-pids.json | python3 -m json.tool`
Expected: v2 format with `version: 2`, `sessions` object containing the session with `pid`, `status`, `agent_state`, `last_activity_at`.

**Step 5: Kill the server (simulate crash)**

`Ctrl+C` the server process.

**Step 6: Restart the server**

Run: `cargo run -p claude-view-server`

**Step 7: Verify recovery**

Open live monitor — the session should appear **immediately** (within 1-2 seconds of page load) with the correct agent_state from the snapshot, not a placeholder "recovered" state.

**Step 8: Verify the session enriches on next hook**

Interact with the Claude session (type something). Verify the live monitor updates correctly — agent_state should transition normally.

**Step 9: Commit (if any fixups needed)**

```bash
git add -A
git commit -m "fix: address issues found during e2e verification of crash recovery"
```

---

### Task 6: Clean up old function names and dead code

**Files:**
- Modify: `crates/server/src/live/manager.rs` — remove old `save_pid_snapshot`, `load_pid_snapshot` if still present
- Modify: `crates/server/src/routes/hooks.rs` — verify no references to old function names

**Step 1: Search for any remaining references to old functions**

Run: `cargo build -p claude-view-server 2>&1 | grep warning`
Expected: No warnings about unused functions

**Step 2: Remove dead code if any**

Delete any remaining `save_pid_snapshot` / `load_pid_snapshot` functions that are now unused.

**Step 3: Final test suite**

Run: `cargo test -p claude-view-server`
Expected: PASS, no warnings

**Step 4: Commit**

```bash
git add crates/server/src/live/manager.rs crates/server/src/routes/hooks.rs
git commit -m "chore: remove deprecated v1 pid-only snapshot functions"
```
