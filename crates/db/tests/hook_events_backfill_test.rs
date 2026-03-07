//! Integration test: hook_progress JSONL lines get parsed and persisted
//! to hook_events table via the deep indexer. Different sources coexist.

use claude_view_db::hook_events_queries::{get_hook_events, insert_hook_events};
use claude_view_db::{Database, HookEventRow};

#[tokio::test]
async fn test_hook_progress_backfill_coexists_with_channel_b() {
    let db = Database::new_in_memory().await.unwrap();

    // Insert Channel B event first (simulating it was captured live)
    let channel_b = vec![HookEventRow {
        timestamp: 1000,
        event_name: "PreToolUse".into(),
        tool_name: Some("Read".into()),
        label: "Reading: src/main.rs".into(),
        group_name: "autonomous".into(),
        context: Some(r#"{"file":"src/main.rs"}"#.into()),
        source: "hook".into(),
    }];
    insert_hook_events(&db, "test-session", &channel_b)
        .await
        .unwrap();

    // Insert Channel A version (from JSONL backfill) — MUST coexist, not be ignored
    let channel_a = vec![HookEventRow {
        timestamp: 1000,
        event_name: "PreToolUse".into(),
        tool_name: Some("Read".into()),
        label: "PreToolUse: Read".into(),
        group_name: "autonomous".into(),
        context: None,
        source: "hook_progress".into(),
    }];
    insert_hook_events(&db, "test-session", &channel_a)
        .await
        .unwrap();

    // Both MUST exist — different sources are different data
    let events = get_hook_events(&db, "test-session").await.unwrap();
    assert_eq!(
        events.len(),
        2,
        "Channel B + Channel A must coexist — NEVER cross-channel dedup"
    );
    let sources: Vec<&str> = events.iter().map(|e| e.source.as_str()).collect();
    assert!(sources.contains(&"hook"), "Channel B present");
    assert!(sources.contains(&"hook_progress"), "Channel A present");
}

#[tokio::test]
async fn test_self_dedup_within_same_source() {
    let db = Database::new_in_memory().await.unwrap();

    let first = vec![HookEventRow {
        timestamp: 1000,
        event_name: "PreToolUse".into(),
        tool_name: Some("Read".into()),
        label: "PreToolUse: Read".into(),
        group_name: "autonomous".into(),
        context: None,
        source: "hook_progress".into(),
    }];
    insert_hook_events(&db, "test-session", &first)
        .await
        .unwrap();

    let dup = vec![HookEventRow {
        timestamp: 1000,
        event_name: "PreToolUse".into(),
        tool_name: Some("Read".into()),
        label: "PreToolUse: Read (from 2nd hook)".into(),
        group_name: "autonomous".into(),
        context: None,
        source: "hook_progress".into(),
    }];
    insert_hook_events(&db, "test-session", &dup).await.unwrap();

    let events = get_hook_events(&db, "test-session").await.unwrap();
    assert_eq!(
        events.len(),
        1,
        "Same source + same key = self-dedup via INSERT OR IGNORE"
    );
}
