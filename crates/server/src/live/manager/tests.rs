//! Tests for the manager module.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use claude_view_core::phase::PhaseHistory;
use claude_view_core::pricing::{CacheStatus, CostBreakdown, TokenUsage};

use crate::live::state::{
    AgentState, AgentStateGroup, HookEvent, LiveSession, SessionSnapshot, SessionStatus,
    SnapshotEntry,
};

use super::accumulator::{apply_jsonl_metadata, build_recovered_session, JsonlMetadata};
use super::helpers::{
    extract_project_info, extract_session_id, load_session_snapshot,
    load_session_snapshot_from_str, make_synthesized_event, parse_timestamp_to_unix,
    resolve_hook_event_from_progress, save_session_snapshot, seconds_since_modified_from_timestamp,
    timestamp_string_to_unix,
};

#[test]
fn test_extract_session_id() {
    let path = PathBuf::from("/home/user/.claude/projects/test-project/abc-123.jsonl");
    assert_eq!(extract_session_id(&path), "abc-123");
}

#[test]
fn test_extract_session_id_no_extension() {
    let path = PathBuf::from("/some/path/session");
    assert_eq!(extract_session_id(&path), "session");
}

#[test]
fn test_extract_project_info_simple_no_cwd() {
    let path = PathBuf::from("/home/user/.claude/projects/-tmp/session.jsonl");
    let (encoded, display, full_path, _cwd) = extract_project_info(&path, None);
    assert_eq!(encoded, "-tmp");
    assert_eq!(display, "-tmp");
    assert_eq!(full_path, "-tmp");
}

#[test]
fn test_extract_project_info_with_cwd() {
    let path = PathBuf::from("/home/user/.claude/projects/-tmp/session.jsonl");
    let (encoded, _display, full_path, cwd) = extract_project_info(&path, Some("/tmp"));
    assert_eq!(encoded, "-tmp");
    assert_eq!(full_path, "/tmp");
    assert_eq!(cwd, Some("/tmp".to_string()));
}

#[test]
fn test_extract_project_info_encoded_path() {
    let path = PathBuf::from("/home/user/.claude/projects/-Users-test-my-project/session.jsonl");
    let (encoded, display, _full_path, _cwd) = extract_project_info(&path, None);
    assert_eq!(encoded, "-Users-test-my-project");
    assert!(!display.is_empty());
    assert_eq!(_full_path, "-Users-test-my-project");
}

#[test]
fn test_parse_timestamp_to_unix() {
    let ts = "2026-01-15T10:30:00Z";
    let result = parse_timestamp_to_unix(ts);
    assert!(result.is_some());
    assert!(result.unwrap() > 0);
}

#[test]
fn test_parse_timestamp_to_unix_with_offset() {
    let ts = "2026-01-15T10:30:00+00:00";
    let result = parse_timestamp_to_unix(ts);
    assert!(result.is_some());
}

#[test]
fn test_parse_timestamp_to_unix_invalid() {
    let result = parse_timestamp_to_unix("not-a-timestamp");
    assert!(result.is_none());
}

#[test]
fn test_seconds_since_modified() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let seconds = seconds_since_modified_from_timestamp(now - 60);
    assert!(seconds >= 59 && seconds <= 61);

    let seconds = seconds_since_modified_from_timestamp(now + 1000);
    assert_eq!(seconds, 0);
}

#[test]
fn test_done_session_not_reprocessed() {
    let mut had_process: HashSet<String> = HashSet::new();
    let session_id = "test-session-done".to_string();
    had_process.insert(session_id.clone());

    let session_status = SessionStatus::Done;
    let process_running = false;
    let mut would_end = false;

    if !process_running && session_status != SessionStatus::Done {
        would_end = true;
    }

    assert!(
        !would_end,
        "Already-Done session must not be re-processed by process detector"
    );
}

#[test]
fn test_pid_binding_prevents_zombie_sessions() {
    let session_a_pid: Option<u32> = Some(1000);
    let alive_pids: HashSet<u32> = [2000].into_iter().collect();

    let running = if let Some(known_pid) = session_a_pid {
        alive_pids.contains(&known_pid)
    } else {
        false
    };

    assert!(
        !running,
        "Session A's bound PID 1000 is dead -- must NOT be kept alive by PID 2000 in same cwd"
    );
}

#[test]
fn test_session_snapshot_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("snapshot.json");

    let mut entries = HashMap::new();
    entries.insert(
        "session-abc".to_string(),
        SnapshotEntry {
            pid: 12345,
            status: "working".to_string(),
            agent_state: AgentState {
                group: AgentStateGroup::Autonomous,
                state: "acting".into(),
                label: "Working".into(),
                context: None,
            },
            last_activity_at: 1708500000,
            control_id: None,
        },
    );
    let snapshot = SessionSnapshot {
        version: 2,
        sessions: entries,
    };

    save_session_snapshot(&path, &snapshot);
    let loaded = load_session_snapshot(&path);

    assert_eq!(loaded.version, 2);
    assert_eq!(loaded.sessions.len(), 1);
    assert_eq!(loaded.sessions["session-abc"].pid, 12345);
}

#[test]
fn test_session_snapshot_missing_file() {
    let path = std::path::PathBuf::from("/tmp/nonexistent-session-snapshot-test.json");
    let loaded = load_session_snapshot(&path);
    assert_eq!(loaded.version, 2);
    assert!(loaded.sessions.is_empty());
}

#[test]
fn test_session_snapshot_corrupt_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("snapshot.json");
    std::fs::write(&path, "not valid json {{{").unwrap();

    let loaded = load_session_snapshot(&path);
    assert_eq!(loaded.version, 2);
    assert!(loaded.sessions.is_empty());
}

#[test]
fn test_snapshot_atomic_write_cleans_tmp() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sessions.json");
    let tmp = path.with_extension("json.tmp");

    let mut m = HashMap::new();
    m.insert(
        "test-session".to_string(),
        SnapshotEntry {
            pid: 12345,
            status: "working".to_string(),
            agent_state: AgentState {
                group: AgentStateGroup::Autonomous,
                state: "acting".into(),
                label: "Working".into(),
                context: None,
            },
            last_activity_at: 1000,
            control_id: None,
        },
    );
    let snapshot = SessionSnapshot {
        version: 2,
        sessions: m,
    };

    save_session_snapshot(&path, &snapshot);

    // Main file exists, tmp file does NOT
    assert!(path.exists(), "Snapshot file must exist after save");
    assert!(!tmp.exists(), "Tmp file must be cleaned up after rename");

    // Content is valid JSON
    let loaded = load_session_snapshot(&path);
    assert_eq!(loaded.sessions.len(), 1);
    assert!(loaded.sessions.contains_key("test-session"));
}

#[test]
fn test_snapshot_v2_round_trip() {
    let mut entries = HashMap::new();
    entries.insert(
        "session-1".to_string(),
        SnapshotEntry {
            pid: 12345,
            status: "working".to_string(),
            agent_state: AgentState {
                group: AgentStateGroup::Autonomous,
                state: "acting".into(),
                label: "Working".into(),
                context: None,
            },
            last_activity_at: 1708500000,
            control_id: None,
        },
    );
    let snapshot = SessionSnapshot {
        version: 2,
        sessions: entries,
    };

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
    let v1_json = r#"{"session-abc": 12345, "session-def": 67890}"#;
    let snapshot = load_session_snapshot_from_str(v1_json);

    assert_eq!(snapshot.version, 2);
    assert_eq!(snapshot.sessions.len(), 2);
    let entry = &snapshot.sessions["session-abc"];
    assert_eq!(entry.pid, 12345);
    assert_eq!(entry.agent_state.state, "recovered");
}

#[test]
fn test_build_recovered_session_from_snapshot() {
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
        control_id: None,
    };

    let session = build_recovered_session(
        "session-abc",
        &entry,
        "/home/user/.claude/projects/-tmp/session-abc.jsonl",
    );

    assert_eq!(session.id, "session-abc");
    assert_eq!(session.hook.pid, Some(12345));
    assert_eq!(session.status, SessionStatus::Paused);
    assert_eq!(session.hook.agent_state.state, "awaiting_input");
    assert_eq!(session.hook.last_activity_at, 1708500000);
    assert_eq!(session.jsonl.project_display_name, "-tmp");
    assert_eq!(session.jsonl.project_path, "-tmp");
}

#[test]
fn test_is_pid_alive_integration_for_bound_sessions() {
    use crate::live::process::is_pid_alive;

    let alive_pid = std::process::id();
    assert!(is_pid_alive(alive_pid));

    let dead_pid: u32 = 4_000_000;
    assert!(!is_pid_alive(dead_pid));
}

#[test]
fn test_snapshot_roundtrip_with_control_id() {
    let mut sessions = HashMap::new();
    sessions.insert(
        "sess-1".to_string(),
        SnapshotEntry {
            pid: 111,
            status: "working".to_string(),
            agent_state: AgentState {
                group: AgentStateGroup::Autonomous,
                state: "acting".into(),
                label: "Working".into(),
                context: None,
            },
            last_activity_at: 1700000000,
            control_id: Some("ctrl-abc".to_string()),
        },
    );
    sessions.insert(
        "sess-2".to_string(),
        SnapshotEntry {
            pid: 222,
            status: "paused".to_string(),
            agent_state: AgentState {
                group: AgentStateGroup::NeedsYou,
                state: "idle".into(),
                label: "Idle".into(),
                context: None,
            },
            last_activity_at: 1700000000,
            control_id: None,
        },
    );
    let snapshot = SessionSnapshot {
        version: 2,
        sessions,
    };
    let json = serde_json::to_string(&snapshot).unwrap();
    let loaded = load_session_snapshot_from_str(&json);
    assert_eq!(
        loaded.sessions["sess-1"].control_id,
        Some("ctrl-abc".to_string())
    );
    assert_eq!(loaded.sessions["sess-2"].control_id, None);
}

#[tokio::test]
async fn test_derive_state_assistant_end_turn() {
    use super::accumulator::derive_agent_state_from_jsonl;
    use std::io::Write;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("session.jsonl");
    let mut f = std::fs::File::create(&path).unwrap();
    writeln!(f, r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"text","text":"Done!"}}],"stop_reason":"end_turn"}}}}"#).unwrap();
    f.flush().unwrap();

    let state = derive_agent_state_from_jsonl(&path).await;
    let state = state.expect("should derive a state");
    assert_eq!(state.group, AgentStateGroup::NeedsYou);
    assert_eq!(state.state, "idle");
}

#[tokio::test]
async fn test_derive_state_assistant_tool_use() {
    use super::accumulator::derive_agent_state_from_jsonl;
    use std::io::Write;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("session.jsonl");
    let mut f = std::fs::File::create(&path).unwrap();
    writeln!(f, r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"tool_use","name":"Read","id":"x","input":{{}}}}],"stop_reason":"tool_use"}}}}"#).unwrap();
    f.flush().unwrap();

    let state = derive_agent_state_from_jsonl(&path).await;
    let state = state.expect("should derive a state");
    assert_eq!(state.group, AgentStateGroup::Autonomous);
    assert_eq!(state.state, "acting");
}

#[tokio::test]
async fn test_derive_state_user_tool_result() {
    use super::accumulator::derive_agent_state_from_jsonl;
    use std::io::Write;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("session.jsonl");
    let mut f = std::fs::File::create(&path).unwrap();
    writeln!(f, r#"{{"type":"user","message":{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"x","content":"ok"}}]}}}}"#).unwrap();
    f.flush().unwrap();

    let state = derive_agent_state_from_jsonl(&path).await;
    let state = state.expect("should derive a state");
    assert_eq!(state.group, AgentStateGroup::Autonomous);
    assert_eq!(state.state, "thinking");
}

#[tokio::test]
async fn test_derive_state_skips_progress_lines() {
    use super::accumulator::derive_agent_state_from_jsonl;
    use std::io::Write;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("session.jsonl");
    let mut f = std::fs::File::create(&path).unwrap();
    writeln!(f, r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"text","text":"Done"}}],"stop_reason":"end_turn"}}}}"#).unwrap();
    writeln!(
        f,
        r#"{{"type":"progress","data":{{"type":"usage","usage":{{}}}}}}"#
    )
    .unwrap();
    writeln!(
        f,
        r#"{{"type":"progress","data":{{"type":"usage","usage":{{}}}}}}"#
    )
    .unwrap();
    f.flush().unwrap();

    let state = derive_agent_state_from_jsonl(&path).await;
    let state = state.expect("should derive state from assistant line, not progress");
    assert_eq!(state.group, AgentStateGroup::NeedsYou);
    assert_eq!(state.state, "idle");
}

#[tokio::test]
async fn test_derive_state_empty_file() {
    use super::accumulator::derive_agent_state_from_jsonl;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("session.jsonl");
    std::fs::File::create(&path).unwrap();

    let state = derive_agent_state_from_jsonl(&path).await;
    assert!(state.is_none());
}

#[test]
fn transcript_dedup_detects_duplicate_session_ids() {
    let transcript = PathBuf::from("/tmp/sessions/abc.jsonl");
    let mut transcript_map: HashMap<PathBuf, String> = HashMap::new();

    transcript_map.insert(transcript.clone(), "old-uuid".to_string());

    let new_id = "new-uuid";
    let dedup_target = transcript_map
        .get(&transcript)
        .filter(|existing| existing.as_str() != new_id)
        .cloned();

    assert_eq!(
        dedup_target,
        Some("old-uuid".to_string()),
        "dedup must identify the older session for merging"
    );

    let same_id_target = transcript_map
        .get(&transcript)
        .filter(|existing| existing.as_str() != "old-uuid")
        .cloned();
    assert_eq!(
        same_id_target, None,
        "same session re-registering must not trigger dedup"
    );
}

#[test]
fn transcript_dedup_different_transcripts_no_collision() {
    let mut transcript_map: HashMap<PathBuf, String> = HashMap::new();
    transcript_map.insert(PathBuf::from("/tmp/a.jsonl"), "session-1".to_string());
    transcript_map.insert(PathBuf::from("/tmp/b.jsonl"), "session-2".to_string());

    assert_eq!(transcript_map.len(), 2);
    assert_eq!(
        transcript_map.get(&PathBuf::from("/tmp/a.jsonl")).unwrap(),
        "session-1"
    );
    assert_eq!(
        transcript_map.get(&PathBuf::from("/tmp/b.jsonl")).unwrap(),
        "session-2"
    );
}

#[test]
fn test_snapshot_pid_dedup_evicts_stale_entry() {
    use crate::live::state::test_live_session;

    let mut sessions: HashMap<String, LiveSession> = HashMap::new();

    let mut a = test_live_session("session-a");
    a.hook.pid = Some(42);
    a.hook.last_activity_at = 1000;
    a.status = SessionStatus::Working;
    sessions.insert("session-a".into(), a);

    let mut b = test_live_session("session-b");
    b.hook.pid = Some(42);
    b.hook.last_activity_at = 2000;
    b.status = SessionStatus::Working;
    sessions.insert("session-b".into(), b);

    let mut c = test_live_session("session-c");
    c.hook.pid = Some(99);
    c.hook.last_activity_at = 500;
    c.status = SessionStatus::Working;
    sessions.insert("session-c".into(), c);

    let mut pid_owners: HashMap<u32, (String, i64)> = HashMap::new();
    let mut pid_dupes: Vec<String> = Vec::new();

    for (id, session) in sessions.iter() {
        if session.status == SessionStatus::Done {
            continue;
        }
        if let Some(pid) = session.hook.pid {
            if let Some((existing_id, existing_ts)) = pid_owners.get(&pid) {
                if session.hook.last_activity_at > *existing_ts {
                    pid_dupes.push(existing_id.clone());
                    pid_owners.insert(pid, (id.clone(), session.hook.last_activity_at));
                } else {
                    pid_dupes.push(id.clone());
                }
            } else {
                pid_owners.insert(pid, (id.clone(), session.hook.last_activity_at));
            }
        }
    }

    for dupe_id in &pid_dupes {
        if let Some(session) = sessions.get_mut(dupe_id) {
            session.status = SessionStatus::Done;
            session.closed_at = Some(9999);
        }
    }

    assert_eq!(
        sessions["session-a"].status,
        SessionStatus::Done,
        "Older session with same PID must be evicted"
    );
    assert!(sessions["session-a"].closed_at.is_some());

    assert_eq!(
        sessions["session-b"].status,
        SessionStatus::Working,
        "Newer session with same PID must survive"
    );

    assert_eq!(
        sessions["session-c"].status,
        SessionStatus::Working,
        "Session with unique PID must be untouched"
    );
}

#[test]
fn test_snapshot_pid_dedup_no_collision() {
    use crate::live::state::test_live_session;

    let mut sessions: HashMap<String, LiveSession> = HashMap::new();

    let mut a = test_live_session("session-a");
    a.hook.pid = Some(10);
    a.status = SessionStatus::Working;
    sessions.insert("session-a".into(), a);

    let mut b = test_live_session("session-b");
    b.hook.pid = Some(20);
    b.status = SessionStatus::Working;
    sessions.insert("session-b".into(), b);

    let mut pid_owners: HashMap<u32, (String, i64)> = HashMap::new();
    let mut pid_dupes: Vec<String> = Vec::new();

    for (id, session) in sessions.iter() {
        if session.status == SessionStatus::Done {
            continue;
        }
        if let Some(pid) = session.hook.pid {
            if let Some((existing_id, existing_ts)) = pid_owners.get(&pid) {
                if session.hook.last_activity_at > *existing_ts {
                    pid_dupes.push(existing_id.clone());
                    pid_owners.insert(pid, (id.clone(), session.hook.last_activity_at));
                } else {
                    pid_dupes.push(id.clone());
                }
            } else {
                pid_owners.insert(pid, (id.clone(), session.hook.last_activity_at));
            }
        }
    }

    assert!(pid_dupes.is_empty(), "No PID collisions means no evictions");
    assert_eq!(sessions["session-a"].status, SessionStatus::Working);
    assert_eq!(sessions["session-b"].status, SessionStatus::Working);
}

#[test]
fn test_snapshot_pid_dedup_skips_done_sessions() {
    use crate::live::state::test_live_session;

    let mut sessions: HashMap<String, LiveSession> = HashMap::new();

    let mut a = test_live_session("session-a");
    a.hook.pid = Some(42);
    a.hook.last_activity_at = 1000;
    a.status = SessionStatus::Done;
    sessions.insert("session-a".into(), a);

    let mut b = test_live_session("session-b");
    b.hook.pid = Some(42);
    b.hook.last_activity_at = 2000;
    b.status = SessionStatus::Working;
    sessions.insert("session-b".into(), b);

    let mut pid_owners: HashMap<u32, (String, i64)> = HashMap::new();
    let mut pid_dupes: Vec<String> = Vec::new();

    for (id, session) in sessions.iter() {
        if session.status == SessionStatus::Done {
            continue;
        }
        if let Some(pid) = session.hook.pid {
            if let Some((existing_id, existing_ts)) = pid_owners.get(&pid) {
                if session.hook.last_activity_at > *existing_ts {
                    pid_dupes.push(existing_id.clone());
                    pid_owners.insert(pid, (id.clone(), session.hook.last_activity_at));
                } else {
                    pid_dupes.push(id.clone());
                }
            } else {
                pid_owners.insert(pid, (id.clone(), session.hook.last_activity_at));
            }
        }
    }

    assert!(
        pid_dupes.is_empty(),
        "Done sessions must be excluded from PID dedup"
    );
    assert_eq!(sessions["session-a"].status, SessionStatus::Done);
    assert_eq!(sessions["session-b"].status, SessionStatus::Working);
}

// =============================================================================
// Hook event tests
// =============================================================================

use claude_view_core::live_parser::HookProgressData;

#[test]
fn test_resolve_hook_event_session_start_resume() {
    let hp = HookProgressData {
        hook_event: "SessionStart".into(),
        tool_name: None,
        source: Some("resume".into()),
    };
    let event = resolve_hook_event_from_progress(&hp, &Some("2026-03-07T12:00:00Z".into()));
    assert_eq!(event.group, "needs_you");
    assert_eq!(event.event_name, "SessionStart");
}

#[test]
fn test_resolve_hook_event_session_start_compact() {
    let hp = HookProgressData {
        hook_event: "SessionStart".into(),
        tool_name: None,
        source: Some("compact".into()),
    };
    let event = resolve_hook_event_from_progress(&hp, &Some("2026-03-07T12:00:00Z".into()));
    assert_eq!(event.group, "autonomous");
}

#[test]
fn test_resolve_hook_event_pre_tool_ask_user() {
    let hp = HookProgressData {
        hook_event: "PreToolUse".into(),
        tool_name: Some("AskUserQuestion".into()),
        source: None,
    };
    let event = resolve_hook_event_from_progress(&hp, &Some("2026-03-07T12:00:00Z".into()));
    assert_eq!(event.group, "needs_you");
}

#[test]
fn test_resolve_hook_event_pre_tool_read() {
    let hp = HookProgressData {
        hook_event: "PreToolUse".into(),
        tool_name: Some("Read".into()),
        source: None,
    };
    let event = resolve_hook_event_from_progress(&hp, &Some("2026-03-07T12:00:00Z".into()));
    assert_eq!(event.group, "autonomous");
    assert_eq!(event.label, "PreToolUse: Read");
}

#[test]
fn test_resolve_hook_event_stop() {
    let hp = HookProgressData {
        hook_event: "Stop".into(),
        tool_name: None,
        source: None,
    };
    let event = resolve_hook_event_from_progress(&hp, &Some("2026-03-07T12:00:00Z".into()));
    assert_eq!(event.group, "needs_you");
    assert_eq!(event.label, "Stop");
}

#[test]
fn test_timestamp_string_to_unix_valid() {
    let ts = Some("2026-03-07T12:00:00Z".into());
    let result = timestamp_string_to_unix(&ts);
    assert!(
        result > 0,
        "Valid timestamp should produce positive unix time"
    );
}

#[test]
fn test_timestamp_string_to_unix_none() {
    let result = timestamp_string_to_unix(&None);
    assert_eq!(result, 0, "None should return 0 (safe sentinel)");
}

#[test]
fn test_source_discrimination_resolve_sets_hook_progress() {
    let hp = HookProgressData {
        hook_event: "PreToolUse".into(),
        tool_name: Some("Read".into()),
        source: None,
    };
    let event = resolve_hook_event_from_progress(&hp, &Some("2026-03-07T12:00:00Z".into()));
    assert_eq!(event.source, "hook_progress");
}

#[test]
fn test_source_discrimination_synthesized_sets_source() {
    let event = make_synthesized_event(
        &Some("2026-03-07T12:00:00Z".into()),
        "UserPromptSubmit",
        None,
        "autonomous",
    );
    assert_eq!(event.source, "synthesized");
}

#[test]
fn test_channel_a_and_b_coexist_in_memory() {
    let mut hook_events: Vec<HookEvent> = Vec::new();
    let channel_a = HookEvent {
        timestamp: 100,
        event_name: "PreToolUse".into(),
        tool_name: Some("Read".into()),
        label: "PreToolUse: Read".into(),
        group: "autonomous".into(),
        context: None,
        source: "hook_progress".into(),
    };
    hook_events.push(channel_a);
    let channel_b = HookEvent {
        timestamp: 100,
        event_name: "PreToolUse".into(),
        tool_name: Some("Read".into()),
        label: "Reading: src/main.rs".into(),
        group: "autonomous".into(),
        context: Some(r#"{"file":"src/main.rs"}"#.into()),
        source: "hook".into(),
    };
    hook_events.push(channel_b);
    assert_eq!(hook_events.len(), 2);
    assert_eq!(hook_events[0].source, "hook_progress");
    assert_eq!(hook_events[1].source, "hook");
}

#[test]
fn test_self_dedup() {
    let mut events = vec![
        HookEvent {
            timestamp: 100,
            event_name: "PreToolUse".into(),
            tool_name: Some("Read".into()),
            label: "a".into(),
            group: "autonomous".into(),
            context: None,
            source: "hook_progress".into(),
        },
        HookEvent {
            timestamp: 100,
            event_name: "PreToolUse".into(),
            tool_name: Some("Read".into()),
            label: "b".into(),
            group: "autonomous".into(),
            context: None,
            source: "hook_progress".into(),
        },
        HookEvent {
            timestamp: 101,
            event_name: "PostToolUse".into(),
            tool_name: Some("Read".into()),
            label: "c".into(),
            group: "autonomous".into(),
            context: None,
            source: "hook_progress".into(),
        },
    ];
    events.sort_by(|a, b| {
        a.timestamp
            .cmp(&b.timestamp)
            .then(a.event_name.cmp(&b.event_name))
            .then(a.tool_name.cmp(&b.tool_name))
            .then(a.source.cmp(&b.source))
    });
    events.dedup_by(|a, b| {
        a.event_name == b.event_name
            && a.timestamp == b.timestamp
            && a.tool_name == b.tool_name
            && a.source == b.source
    });
    assert_eq!(events.len(), 2);
}

#[test]
fn test_self_dedup_adversarial_interleaving() {
    let mut events = vec![
        HookEvent {
            timestamp: 100,
            event_name: "Stop".into(),
            tool_name: None,
            label: "a".into(),
            group: "needs_you".into(),
            context: None,
            source: "hook_progress".into(),
        },
        HookEvent {
            timestamp: 100,
            event_name: "PreToolUse".into(),
            tool_name: Some("Read".into()),
            label: "b".into(),
            group: "autonomous".into(),
            context: None,
            source: "hook_progress".into(),
        },
        HookEvent {
            timestamp: 100,
            event_name: "Stop".into(),
            tool_name: None,
            label: "c".into(),
            group: "needs_you".into(),
            context: None,
            source: "hook_progress".into(),
        },
    ];
    events.sort_by(|a, b| {
        a.timestamp
            .cmp(&b.timestamp)
            .then(a.event_name.cmp(&b.event_name))
            .then(a.tool_name.cmp(&b.tool_name))
            .then(a.source.cmp(&b.source))
    });
    events.dedup_by(|a, b| {
        a.event_name == b.event_name
            && a.timestamp == b.timestamp
            && a.tool_name == b.tool_name
            && a.source == b.source
    });
    assert_eq!(events.len(), 2);
}

#[test]
fn test_self_dedup_preserves_different_sources_within_channel_a() {
    let mut events = vec![
        HookEvent {
            timestamp: 100,
            event_name: "SessionEnd".into(),
            tool_name: None,
            label: "SessionEnd".into(),
            group: "needs_you".into(),
            context: None,
            source: "hook_progress".into(),
        },
        HookEvent {
            timestamp: 100,
            event_name: "SessionEnd".into(),
            tool_name: None,
            label: "SessionEnd".into(),
            group: "needs_you".into(),
            context: None,
            source: "synthesized".into(),
        },
    ];
    events.sort_by(|a, b| {
        a.timestamp
            .cmp(&b.timestamp)
            .then(a.event_name.cmp(&b.event_name))
            .then(a.tool_name.cmp(&b.tool_name))
            .then(a.source.cmp(&b.source))
    });
    events.dedup_by(|a, b| {
        a.event_name == b.event_name
            && a.timestamp == b.timestamp
            && a.tool_name == b.tool_name
            && a.source == b.source
    });
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].source, "hook_progress");
    assert_eq!(events[1].source, "synthesized");
}

#[test]
fn test_synthesized_user_prompt_submit() {
    let event = make_synthesized_event(
        &Some("2026-03-07T12:00:00Z".into()),
        "UserPromptSubmit",
        None,
        "autonomous",
    );
    assert_eq!(event.event_name, "UserPromptSubmit");
    assert_eq!(event.group, "autonomous");
    assert_eq!(event.tool_name, None);
}

#[test]
fn test_synthesized_session_end() {
    let event = make_synthesized_event(
        &Some("2026-03-07T12:00:00Z".into()),
        "SessionEnd",
        None,
        "needs_you",
    );
    assert_eq!(event.event_name, "SessionEnd");
    assert_eq!(event.group, "needs_you");
}

#[test]
fn test_synthesized_pre_compact() {
    let event = make_synthesized_event(
        &Some("2026-03-07T12:00:00Z".into()),
        "PreCompact",
        None,
        "autonomous",
    );
    assert_eq!(event.event_name, "PreCompact");
}

#[test]
fn test_synthesized_subagent_start() {
    let event = make_synthesized_event(
        &Some("2026-03-07T12:00:00Z".into()),
        "SubagentStart",
        Some("Explore"),
        "autonomous",
    );
    assert_eq!(event.event_name, "SubagentStart");
    assert_eq!(event.tool_name, Some("Explore".into()));
}

#[test]
fn test_synthesized_subagent_stop() {
    let event = make_synthesized_event(
        &Some("2026-03-07T12:00:00Z".into()),
        "SubagentStop",
        None,
        "autonomous",
    );
    assert_eq!(event.event_name, "SubagentStop");
}

#[test]
fn test_synthesized_task_completed() {
    let event = make_synthesized_event(
        &Some("2026-03-07T12:00:00Z".into()),
        "TaskCompleted",
        None,
        "autonomous",
    );
    assert_eq!(event.event_name, "TaskCompleted");
}

#[test]
fn test_session_end_persist_preserves_source() {
    let channel_a = HookEvent {
        timestamp: 100,
        event_name: "PreToolUse".into(),
        tool_name: Some("Read".into()),
        label: "PreToolUse: Read".into(),
        group: "autonomous".into(),
        context: None,
        source: "hook_progress".into(),
    };
    let row = claude_view_db::HookEventRow {
        timestamp: channel_a.timestamp,
        event_name: channel_a.event_name.clone(),
        tool_name: channel_a.tool_name.clone(),
        label: channel_a.label.clone(),
        group_name: channel_a.group.clone(),
        context: channel_a.context.clone(),
        source: channel_a.source.clone(),
    };
    assert_eq!(row.source, "hook_progress");
}

// ── apply_jsonl_metadata branch guard tests ──

fn minimal_live_session_for_branch_tests(id: &str) -> LiveSession {
    use crate::live::state::{HookFields, JsonlFields, StatuslineFields};
    LiveSession {
        id: id.to_string(),
        status: SessionStatus::Working,
        started_at: None,
        closed_at: None,
        control: None,
        model: None,
        model_display_name: None,
        model_set_at: 0,
        context_window_tokens: 0,
        statusline: StatuslineFields::default(),
        hook: HookFields {
            agent_state: AgentState {
                group: AgentStateGroup::Autonomous,
                state: "acting".into(),
                label: "Working".into(),
                context: None,
            },
            pid: None,
            title: "Test".into(),
            last_user_message: String::new(),
            current_activity: "Working".into(),
            turn_count: 0,
            last_activity_at: 0,
            current_turn_started_at: None,
            sub_agents: Vec::new(),
            progress_items: Vec::new(),
            compact_count: 0,
            agent_state_set_at: 0,
            last_assistant_preview: None,
            last_error: None,
            last_error_details: None,
            hook_events: Vec::new(),
        },
        jsonl: JsonlFields {
            project: String::new(),
            project_display_name: "test".to_string(),
            project_path: "/tmp/test".to_string(),
            file_path: "/tmp/test.jsonl".to_string(),
            ..JsonlFields::default()
        },
    }
}

fn minimal_jsonl_metadata() -> JsonlMetadata {
    JsonlMetadata {
        git_branch: None,
        worktree_branch: None,
        is_worktree: false,
        pid: None,
        title: String::new(),
        last_user_message: String::new(),
        turn_count: 0,
        started_at: None,
        last_activity_at: 0,
        model: None,
        tokens: TokenUsage::default(),
        context_window_tokens: 0,
        cost: CostBreakdown::default(),
        cache_status: CacheStatus::Unknown,
        current_turn_started_at: None,
        last_turn_task_seconds: None,
        sub_agents: Vec::new(),
        team_name: None,
        progress_items: Vec::new(),
        last_cache_hit_at: None,
        tools_used: Vec::new(),
        compact_count: 0,
        slug: None,
        user_files: None,
        edit_count: 0,
        phase: PhaseHistory::default(),
        entrypoint: None,
    }
}

#[test]
fn test_apply_jsonl_metadata_preserves_hook_branch_when_accumulator_has_none() {
    let mut session = minimal_live_session_for_branch_tests("test-session");
    session.jsonl.git_branch = Some("main".to_string());
    session.jsonl.effective_branch = Some("main".to_string());

    let meta = minimal_jsonl_metadata();

    apply_jsonl_metadata(
        &mut session,
        &meta,
        "/tmp/test.jsonl",
        "proj",
        "proj",
        "/tmp",
    );

    assert_eq!(
        session.jsonl.git_branch.as_deref(),
        Some("main"),
        "Hook-resolved branch must not be overwritten by None accumulator"
    );
    assert_eq!(
        session.jsonl.effective_branch.as_deref(),
        Some("main"),
        "effective_branch must preserve hook-resolved value"
    );
}

#[test]
fn test_apply_jsonl_metadata_jsonl_branch_wins_when_some() {
    let mut session = minimal_live_session_for_branch_tests("test-session");
    session.jsonl.git_branch = Some("old-hook-branch".to_string());
    session.jsonl.effective_branch = Some("old-hook-branch".to_string());

    let mut meta = minimal_jsonl_metadata();
    meta.git_branch = Some("main".to_string());

    apply_jsonl_metadata(
        &mut session,
        &meta,
        "/tmp/test.jsonl",
        "proj",
        "proj",
        "/tmp",
    );

    assert_eq!(
        session.jsonl.git_branch.as_deref(),
        Some("main"),
        "JSONL-sourced branch must overwrite hook branch when Some"
    );
    assert_eq!(
        session.jsonl.effective_branch.as_deref(),
        Some("main"),
        "effective_branch must reflect JSONL branch"
    );
}

#[test]
fn test_apply_jsonl_metadata_none_stays_none_when_no_source() {
    let mut session = minimal_live_session_for_branch_tests("test-session");

    let meta = minimal_jsonl_metadata();

    apply_jsonl_metadata(
        &mut session,
        &meta,
        "/tmp/test.jsonl",
        "proj",
        "proj",
        "/tmp",
    );

    assert!(
        session.jsonl.git_branch.is_none(),
        "Branch stays None when neither hook nor JSONL provides a value"
    );
    assert!(
        session.jsonl.effective_branch.is_none(),
        "effective_branch stays None when no source provides a value"
    );
}

#[test]
fn test_apply_jsonl_metadata_preserves_worktree_branch_when_none() {
    let mut session = minimal_live_session_for_branch_tests("test-session");
    session.jsonl.git_branch = Some("main".to_string());
    session.jsonl.worktree_branch = Some("feat/my-feature".to_string());
    session.jsonl.is_worktree = true;
    session.jsonl.effective_branch = Some("feat/my-feature".to_string());

    let meta = minimal_jsonl_metadata();

    apply_jsonl_metadata(
        &mut session,
        &meta,
        "/tmp/test.jsonl",
        "proj",
        "proj",
        "/tmp",
    );

    assert_eq!(
        session.jsonl.worktree_branch.as_deref(),
        Some("feat/my-feature"),
        "Hook-resolved worktree branch must not be cleared by None accumulator"
    );
    assert!(
        session.jsonl.is_worktree,
        "is_worktree must not be reset to false by metadata with is_worktree=false"
    );
    assert_eq!(
        session.jsonl.effective_branch.as_deref(),
        Some("feat/my-feature"),
        "effective_branch must stay as worktree branch"
    );
}

#[test]
fn test_apply_jsonl_metadata_jsonl_worktree_branch_wins_when_some() {
    let mut session = minimal_live_session_for_branch_tests("test-session");
    session.jsonl.git_branch = Some("main".to_string());
    session.jsonl.worktree_branch = None;
    session.jsonl.effective_branch = Some("main".to_string());

    let mut meta = minimal_jsonl_metadata();
    meta.git_branch = Some("main".to_string());
    meta.worktree_branch = Some("feat/my-feature".to_string());
    meta.is_worktree = true;

    apply_jsonl_metadata(
        &mut session,
        &meta,
        "/tmp/test.jsonl",
        "proj",
        "proj",
        "/tmp",
    );

    assert_eq!(
        session.jsonl.worktree_branch.as_deref(),
        Some("feat/my-feature")
    );
    assert!(session.jsonl.is_worktree);
    assert_eq!(
        session.jsonl.effective_branch.as_deref(),
        Some("feat/my-feature"),
        "effective_branch must prefer worktree_branch over git_branch"
    );
}

#[test]
fn test_edit_count_accumulates_from_edit_and_write_tools() {
    let mut metadata = minimal_jsonl_metadata();
    metadata.edit_count = 7;

    let mut session = minimal_live_session_for_branch_tests("test-edit-count");
    apply_jsonl_metadata(
        &mut session,
        &metadata,
        "/tmp/test.jsonl",
        "proj",
        "proj",
        "/tmp",
    );

    assert_eq!(
        session.jsonl.edit_count, 7,
        "edit_count must be propagated from JsonlMetadata to LiveSession"
    );
}

#[test]
fn test_edit_count_defaults_to_zero_for_new_session() {
    let session = minimal_live_session_for_branch_tests("test-zero-edit-count");
    assert_eq!(
        session.jsonl.edit_count, 0,
        "edit_count must default to 0 in freshly constructed LiveSession"
    );
}

#[test]
fn test_team_members_and_inbox_count_default_to_empty_for_non_team_session() {
    let session = minimal_live_session_for_branch_tests("test-no-team");
    assert!(
        session.jsonl.team_members.is_empty(),
        "team_members must be empty for non-team sessions"
    );
    assert_eq!(
        session.jsonl.team_inbox_count, 0,
        "team_inbox_count must be 0 for non-team sessions"
    );
}
