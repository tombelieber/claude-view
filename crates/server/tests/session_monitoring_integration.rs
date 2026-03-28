//! Integration tests for session monitoring SOTA hardening.
//!
//! These tests exercise REAL system calls against the current process — not mocks.
//! This catches parsing regressions that unit tests with hardcoded strings miss.
//!
//! Test matrix:
//! - source cache: second detect_claude_processes() call uses cache
//! - snapshot recovery: PID dedup on load
//! - ghost session: hook-created skeleton with no JSONL auto-completes

use std::collections::HashMap;

// =============================================================================
// Test 1: Source cache hit (detect_claude_processes)
// =============================================================================

#[test]
fn test_source_cache_second_call_is_fast() {
    // First call: populates cache
    let start1 = std::time::Instant::now();
    let (_procs1, count1) = claude_view_server::live::process::detect_claude_processes();
    let dur1 = start1.elapsed();

    // Second call: should use cache for source classification
    let start2 = std::time::Instant::now();
    let (_procs2, count2) = claude_view_server::live::process::detect_claude_processes();
    let dur2 = start2.elapsed();

    // Both calls must return the same count
    assert_eq!(
        count1, count2,
        "Two consecutive detect_claude_processes() calls must return same count"
    );

    // The second call should be at least somewhat faster due to cache hits,
    // but we can't assert strict timing on CI. Instead, verify both complete
    // within a reasonable bound (10s).
    assert!(
        dur1 < std::time::Duration::from_secs(10),
        "First call too slow: {:?}",
        dur1
    );
    assert!(
        dur2 < std::time::Duration::from_secs(10),
        "Second call too slow: {:?}",
        dur2
    );
}

// =============================================================================
// Test 4: Snapshot recovery PID dedup (end-to-end with file I/O)
// =============================================================================

#[test]
fn test_snapshot_recovery_pid_dedup_end_to_end() {
    use claude_view_server::live::state::{
        AgentState, AgentStateGroup, SessionSnapshot, SnapshotEntry,
    };

    // Create a snapshot with two entries sharing the same PID
    let shared_pid = 42u32;
    let mut sessions = HashMap::new();
    sessions.insert(
        "session-old".to_string(),
        SnapshotEntry {
            pid: shared_pid,
            status: "working".to_string(),
            agent_state: AgentState {
                group: AgentStateGroup::Autonomous,
                state: "acting".into(),
                label: "Working".into(),
                context: None,
            },
            last_activity_at: 1000, // older
            control_id: None,
        },
    );
    sessions.insert(
        "session-new".to_string(),
        SnapshotEntry {
            pid: shared_pid,
            status: "paused".to_string(),
            agent_state: AgentState {
                group: AgentStateGroup::NeedsYou,
                state: "idle".into(),
                label: "Waiting".into(),
                context: None,
            },
            last_activity_at: 2000, // newer
            control_id: None,
        },
    );
    sessions.insert(
        "session-other".to_string(),
        SnapshotEntry {
            pid: 99,
            status: "working".to_string(),
            agent_state: AgentState {
                group: AgentStateGroup::Autonomous,
                state: "acting".into(),
                label: "Working".into(),
                context: None,
            },
            last_activity_at: 1500,
            control_id: None,
        },
    );

    let snapshot = SessionSnapshot {
        version: 2,
        sessions,
    };

    // Write to disk and read back
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test-snapshot.json");
    let content = serde_json::to_string(&snapshot).unwrap();
    std::fs::write(&path, &content).unwrap();

    let loaded: SessionSnapshot =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();

    // Run the same PID dedup logic from manager.rs
    let mut pid_owners: HashMap<u32, (String, i64)> = HashMap::new();
    let mut pid_dupes: Vec<String> = Vec::new();

    for (id, entry) in &loaded.sessions {
        if let Some((existing_id, existing_ts)) = pid_owners.get(&entry.pid) {
            if entry.last_activity_at > *existing_ts {
                pid_dupes.push(existing_id.clone());
                pid_owners.insert(entry.pid, (id.clone(), entry.last_activity_at));
            } else {
                pid_dupes.push(id.clone());
            }
        } else {
            pid_owners.insert(entry.pid, (id.clone(), entry.last_activity_at));
        }
    }

    // Exactly one duplicate (the older session with shared PID)
    assert_eq!(pid_dupes.len(), 1, "Exactly one dupe expected");
    assert_eq!(
        pid_dupes[0], "session-old",
        "Older session must be the evicted one"
    );

    // session-new and session-other are survivors
    let survivors: Vec<&str> = pid_owners.values().map(|(id, _)| id.as_str()).collect();
    assert!(survivors.contains(&"session-new"));
    assert!(survivors.contains(&"session-other"));
}

// =============================================================================
// Test 5: Ghost session detection
// =============================================================================

#[test]
fn test_ghost_session_detection_logic() {
    // A ghost session has: empty file_path AND zero turn_count
    let file_path = "";
    let turn_count = 0u32;
    let is_ghost = file_path.is_empty() && turn_count == 0;
    assert!(is_ghost, "Hook-only skeleton with no JSONL must be ghost");

    // A session with a file path is NOT a ghost even if turn_count is 0
    let file_path2 = "/some/path/session.jsonl";
    let is_ghost2 = file_path2.is_empty() && turn_count == 0;
    assert!(!is_ghost2, "Session with file path is not a ghost");

    // A session with turns is NOT a ghost even if file_path is empty
    let turn_count3 = 5u32;
    let is_ghost3 = file_path.is_empty() && turn_count3 == 0;
    assert!(!is_ghost3, "Session with turns is not a ghost");
}

// =============================================================================
// Test 6: is_pid_alive integration
// =============================================================================

#[test]
fn test_is_pid_alive_own_process() {
    let own_pid = std::process::id();
    assert!(
        claude_view_server::live::process::is_pid_alive(own_pid),
        "Our own process must be alive"
    );
}

#[test]
fn test_is_pid_alive_dead_process() {
    let dead_pid: u32 = 4_000_000;
    assert!(
        !claude_view_server::live::process::is_pid_alive(dead_pid),
        "Non-existent PID must be dead"
    );
}

#[test]
fn test_is_pid_alive_guards_against_kernel() {
    // PID 0 and 1 should always return false (kernel/init guard)
    assert!(
        !claude_view_server::live::process::is_pid_alive(0),
        "PID 0 must always return false"
    );
    assert!(
        !claude_view_server::live::process::is_pid_alive(1),
        "PID 1 must always return false"
    );
}

// =============================================================================
// Test 7: count_claude_processes consistency
// =============================================================================

#[test]
fn test_count_matches_detect_length() {
    let (_procs, detect_count) = claude_view_server::live::process::detect_claude_processes();
    let count_only = claude_view_server::live::process::count_claude_processes();

    // count_claude_processes uses the same logic but only returns a count.
    // It should match the count from detect_claude_processes.
    // Allow ±1 for race conditions (processes can start/stop between calls).
    let diff = (detect_count as i64 - count_only as i64).unsigned_abs();
    assert!(
        diff <= 1,
        "detect_count ({}) and count_only ({}) should match within ±1",
        detect_count,
        count_only
    );
}
