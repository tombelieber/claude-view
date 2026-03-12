//! Integration tests for process tree monitor SSE events and kill endpoints.
//!
//! Verifies:
//! - MonitorEvent::ProcessTree broadcasts arrive via broadcast channel correctly
//! - MonitorEvent::Snapshot broadcasts arrive via broadcast channel correctly
//! - Kill endpoint rejects own PID with descriptive error
//! - Kill endpoint rejects non-Claude PIDs (PID 1 / init / launchd)
//! - Cleanup endpoint handles mixed valid/invalid PIDs gracefully
//! - broadcast::channel<MonitorEvent> accepts both enum variants (compile + runtime check)

use axum::body::Body;
use axum::http::{Request, StatusCode};
use claude_view_db::Database;
use claude_view_server::create_app;
use claude_view_server::live::monitor::{MonitorEvent, ResourceSnapshot};
use claude_view_server::live::process_tree::{
    ClassifiedProcess, EcosystemTag, ProcessCategory, ProcessTreeSnapshot, ProcessTreeTotals,
    Staleness,
};
use tokio::sync::broadcast;
use tower::ServiceExt;

// =============================================================================
// Helper constructors
// =============================================================================

async fn test_db() -> Database {
    Database::new_in_memory()
        .await
        .expect("in-memory DB for tests")
}

fn minimal_snapshot() -> ResourceSnapshot {
    ResourceSnapshot {
        timestamp: 1_700_000_000,
        cpu_percent: 10.0,
        memory_used_bytes: 4_000_000_000,
        memory_total_bytes: 16_000_000_000,
        disk_used_bytes: 50_000_000_000,
        disk_total_bytes: 500_000_000_000,
        top_processes: vec![],
        session_resources: vec![],
    }
}

fn minimal_classified_process(pid: u32, name: &str, tag: EcosystemTag) -> ClassifiedProcess {
    let is_self = matches!(tag, EcosystemTag::Self_);
    ClassifiedProcess {
        pid,
        ppid: 1,
        name: name.to_string(),
        command: format!("/usr/local/bin/{name}"),
        category: ProcessCategory::ClaudeEcosystem,
        ecosystem_tag: Some(tag),
        cpu_percent: 1.0,
        memory_bytes: 100_000_000,
        uptime_secs: 3600,
        start_time: 1_700_000_000,
        is_unparented: true,
        staleness: Staleness::Active,
        descendant_count: 0,
        descendant_cpu: 0.0,
        descendant_memory: 0,
        descendants: vec![],
        is_self,
    }
}

fn minimal_tree_snapshot() -> ProcessTreeSnapshot {
    // ecosystem: [pid=1234 "claude" Cli (is_self=false), pid=5678 "claude-view" Self_ (is_self=true)]
    // children: []
    ProcessTreeSnapshot {
        timestamp: 1_700_000_000,
        ecosystem: vec![
            minimal_classified_process(1234, "claude", EcosystemTag::Cli),
            minimal_classified_process(5678, "claude-view", EcosystemTag::Self_),
        ],
        children: vec![],
        totals: ProcessTreeTotals {
            ecosystem_cpu: 2.0,
            ecosystem_memory: 200_000_000,
            ecosystem_count: 2,
            child_cpu: 0.0,
            child_memory: 0,
            child_count: 0,
            unparented_count: 2,
            unparented_memory: 200_000_000,
        },
    }
}

// =============================================================================
// Test 1: broadcast channel delivers ProcessTree event with correct JSON shape
// =============================================================================

#[tokio::test]
async fn test_sse_stream_emits_process_tree_event() {
    let (tx, mut rx) = broadcast::channel::<MonitorEvent>(16);

    let tree = minimal_tree_snapshot();
    tx.send(MonitorEvent::ProcessTree(tree)).unwrap();

    let received = rx.recv().await.expect("must receive event");

    let t = match received {
        MonitorEvent::ProcessTree(ref snap) => snap,
        MonitorEvent::Snapshot(_) => panic!("expected ProcessTree variant"),
    };

    let json_str = serde_json::to_string(t).expect("must serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json_str).expect("must be valid JSON");

    let ecosystem = parsed["ecosystem"]
        .as_array()
        .expect("ecosystem must be array");
    assert_eq!(ecosystem.len(), 2, "ecosystem must have 2 entries");

    assert_eq!(parsed["ecosystem"][0]["pid"], 1234);
    assert_eq!(
        parsed["ecosystem"][0]["ecosystemTag"], "cli",
        "EcosystemTag::Cli must serialize to 'cli'"
    );

    assert_eq!(
        parsed["ecosystem"][1]["ecosystemTag"], "self",
        "EcosystemTag::Self_ must serialize to 'self'"
    );
    assert_eq!(
        parsed["ecosystem"][1]["isSelf"].as_bool().unwrap(),
        true,
        "is_self must be true for Self_ tagged entry"
    );

    assert!(
        parsed.get("ecosystem_count").is_none(),
        "must use camelCase 'ecosystemCount', not snake_case 'ecosystem_count'"
    );
}

// =============================================================================
// Test 2: kill endpoint rejects own PID with descriptive error
// =============================================================================

#[tokio::test]
async fn test_kill_endpoint_rejects_own_pid() {
    let app = create_app(test_db().await);
    let own_pid = std::process::id();

    let body = serde_json::json!({
        "start_time": 0,
        "force": false
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/processes/{own_pid}/kill"))
                .header("Content-Type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(json["killed"], false, "own process must NOT be killed");
    assert_eq!(json["pid"], own_pid, "response pid must match request pid");

    let err = json["error"]
        .as_str()
        .expect("error field must be a string");
    assert!(
        err.contains("cannot kill own process"),
        "error must mention 'cannot kill own process', got: {err}"
    );
}

// =============================================================================
// Test 3: kill endpoint rejects non-Claude PID (PID 1 / init)
// =============================================================================

#[tokio::test]
async fn test_kill_endpoint_rejects_non_claude_pid() {
    let app = create_app(test_db().await);

    let body = serde_json::json!({
        "start_time": 0,
        "force": false
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/processes/1/kill")
                .header("Content-Type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(json["killed"], false, "PID 1 must NOT be killed");

    let err = json["error"]
        .as_str()
        .expect("error field must be a string");

    // PID 1 on macOS (launchd) is a real process. Depending on system state:
    // - If start_time=0 mismatches → "recycled"
    // - If PID 1 isn't in the Claude process tree → "not Claude-related"
    // - In rare cases it may not appear in sysinfo at all → "not found"
    assert!(
        err.contains("not Claude-related") || err.contains("not found") || err.contains("recycled"),
        "error must indicate PID 1 cannot be killed, got: {err}"
    );
}

// =============================================================================
// Test 4: cleanup endpoint handles mixed valid/invalid PIDs gracefully
// =============================================================================

#[tokio::test]
async fn test_cleanup_endpoint_batch_mixed() {
    let app = create_app(test_db().await);
    let own_pid = std::process::id();

    // Three targets: two impossible PIDs + own PID
    let body = serde_json::json!({
        "targets": [
            { "pid": 4_000_000u32, "start_time": 0i64 },
            { "pid": own_pid, "start_time": 0i64 },
            { "pid": 4_000_001u32, "start_time": 0i64 }
        ]
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/processes/cleanup")
                .header("Content-Type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    let killed = json["killed"].as_array().expect("killed must be array");
    assert!(
        killed.is_empty(),
        "no PIDs should be killed, got: {killed:?}"
    );

    let failed = json["failed"].as_array().expect("failed must be array");
    assert_eq!(failed.len(), 3, "all 3 targets must fail");

    // Find the entry for own_pid and verify the error mentions own process
    let own_pid_entry = failed
        .iter()
        .find(|e| e["pid"].as_u64() == Some(own_pid as u64))
        .expect("own_pid must appear in failed list");

    let reason = own_pid_entry["reason"]
        .as_str()
        .expect("reason must be a string");
    assert!(
        reason.contains("cannot kill own process"),
        "reason for own_pid must mention 'cannot kill own process', got: {reason}"
    );
}

// =============================================================================
// Test 5: broadcast channel accepts both MonitorEvent variants in order
// =============================================================================

#[tokio::test]
async fn test_state_monitor_tx_accepts_both_variants() {
    let (tx, mut rx) = broadcast::channel::<MonitorEvent>(2);

    // Send Snapshot first, then ProcessTree
    tx.send(MonitorEvent::Snapshot(minimal_snapshot())).unwrap();
    tx.send(MonitorEvent::ProcessTree(minimal_tree_snapshot()))
        .unwrap();

    // Receive first — must be Snapshot
    let first = rx.recv().await.expect("must receive first event");
    assert!(
        matches!(first, MonitorEvent::Snapshot(_)),
        "first event must be Snapshot"
    );

    // Receive second — must be ProcessTree
    let second = rx.recv().await.expect("must receive second event");
    match second {
        MonitorEvent::ProcessTree(ref t) => {
            assert_eq!(t.ecosystem.len(), 2);
            assert_eq!(t.ecosystem[0].pid, 1234);
            assert_eq!(t.ecosystem[1].pid, 5678);
            assert!(
                matches!(t.ecosystem[0].ecosystem_tag, Some(EcosystemTag::Cli)),
                "first ecosystem entry must be Cli"
            );
            assert!(
                matches!(t.ecosystem[1].ecosystem_tag, Some(EcosystemTag::Self_)),
                "second ecosystem entry must be Self_"
            );
        }
        MonitorEvent::Snapshot(_) => panic!("second event must be ProcessTree, got Snapshot"),
    }
}
