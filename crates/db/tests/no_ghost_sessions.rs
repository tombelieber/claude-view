use claude_view_db::indexer_parallel::{build_index_hints, scan_and_index_all};
use claude_view_db::Database;
use tempfile::tempdir;

/// Simulates the exact scenario that caused 966 ghost sessions:
/// 1. First startup: scan + parse + upsert (all sessions complete)
/// 2. Server restart: scan again (unchanged files skipped)
/// 3. Verify: zero ghost rows (message_count=0 AND last_message_at>0)
///
/// NOTE: Mock JSONL content must match the format expected by `parse_bytes()`.
/// Reference the `REALISTIC_JSONL` fixture from `crates/db/tests/acceptance_tests.rs`
/// as the canonical test format, or import it directly.
#[tokio::test]
async fn no_ghost_sessions_after_restart() {
    let db = Database::new_in_memory().await.unwrap();
    let tmp = tempdir().unwrap();
    // Realistic JSONL content matching the format expected by parse_bytes().
    // Same structure as REALISTIC_JSONL in acceptance_tests.rs:
    // user line -> assistant line (with model, usage, tool_use) -> user line.
    let realistic_jsonl = r#"{"parentUuid":null,"isFinal":false,"type":"user","uuid":"u1","message":{"role":"user","content":[{"type":"text","text":"Hello world"}]}}
{"parentUuid":"u1","isFinal":false,"type":"assistant","uuid":"a1","timestamp":1706200000,"message":{"model":"claude-opus-4-5-20251101","role":"assistant","content":[{"type":"text","text":"Hi there!"},{"type":"tool_use","name":"Read","id":"t1","input":{"file_path":"/tmp/test.rs"}}],"usage":{"input_tokens":50,"output_tokens":200,"cache_read_input_tokens":5000,"cache_creation_input_tokens":1000,"service_tier":"standard"}}}
{"parentUuid":"a1","isFinal":true,"type":"user","uuid":"u2","message":{"role":"user","content":[{"type":"text","text":"Thanks for reading that file"}]}}
"#;

    // Create 10 session files across 2 projects
    for p in 0..2 {
        let project_name = format!("-Users-test-proj{}", (b'A' + p as u8) as char);
        let project_dir = tmp.path().join("projects").join(&project_name);
        std::fs::create_dir_all(&project_dir).unwrap();

        for s in 0..5 {
            let session_id = format!("sess-{}-{}", p, s);
            let jsonl_path = project_dir.join(format!("{}.jsonl", &session_id));
            std::fs::write(&jsonl_path, realistic_jsonl).unwrap();
        }
    }

    // First scan (build_index_hints is sync — no .await)
    let hints = build_index_hints(tmp.path());
    let (indexed, _) = scan_and_index_all(tmp.path(), &db, &hints, None, None, |_| {}).await.unwrap();
    assert_eq!(indexed, 10);

    // Verify all rows have real data
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sessions WHERE message_count > 0")
        .fetch_one(db.pool()).await.unwrap();
    assert_eq!(count.0, 10);

    // Simulate restart: scan again without file changes
    let hints2 = build_index_hints(tmp.path());
    let (indexed2, skipped2) = scan_and_index_all(tmp.path(), &db, &hints2, None, None, |_| {}).await.unwrap();
    assert_eq!(indexed2, 0);  // nothing changed
    assert_eq!(skipped2, 10); // all skipped

    // THE CRITICAL ASSERTION: zero ghost rows
    let ghosts: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM sessions WHERE message_count = 0 AND last_message_at > 0"
    ).fetch_one(db.pool()).await.unwrap();
    assert_eq!(ghosts.0, 0, "Ghost sessions detected after restart!");

    // All sessions still visible in valid_sessions
    let valid: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM valid_sessions")
        .fetch_one(db.pool()).await.unwrap();
    assert_eq!(valid.0, 10);
}
