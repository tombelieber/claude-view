// Acceptance tests for Startup UX + Parallel Indexing (AC-1 through AC-12).
//
// These integration tests verify the two-pass indexing pipeline, server startup,
// callback behavior, and schema correctness using temporary directories that
// mimic ~/.claude/projects/<encoded-path>/ layout.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use vibe_recall_db::indexer_parallel::{pass_1_read_indexes, pass_2_deep_index, run_background_index};
use vibe_recall_db::Database;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Realistic JSONL content for a session with 2 user + 2 assistant turns.
const REALISTIC_JSONL: &str = r#"{"parentUuid":null,"isFinal":false,"type":"user","uuid":"u1","message":{"role":"user","content":[{"type":"text","text":"Hello world"}]}}
{"parentUuid":"u1","isFinal":false,"type":"assistant","uuid":"a1","timestamp":1706200000,"message":{"model":"claude-opus-4-5-20251101","role":"assistant","content":[{"type":"text","text":"Hi there!"},{"type":"tool_use","name":"Read","id":"t1","input":{"file_path":"/tmp/test.rs"}}],"usage":{"input_tokens":50,"output_tokens":200,"cache_read_input_tokens":5000,"cache_creation_input_tokens":1000,"service_tier":"standard"}}}
{"parentUuid":"a1","isFinal":true,"type":"user","uuid":"u2","message":{"role":"user","content":[{"type":"text","text":"Thanks for reading that file"}]}}
"#;

/// Create a temp directory that mimics `~/.claude/` structure with N sessions
/// spread across the given number of projects.
///
/// Returns (TempDir, claude_dir_path).
fn setup_claude_dir(
    num_projects: usize,
    sessions_per_project: usize,
) -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::TempDir::new().unwrap();
    let claude_dir = tmp.path().to_path_buf();

    for p in 0..num_projects {
        let project_name = format!("-Users-test-proj{}", (b'A' + p as u8) as char);
        let project_dir = claude_dir.join("projects").join(&project_name);
        std::fs::create_dir_all(&project_dir).unwrap();

        let mut entries = Vec::new();

        for s in 0..sessions_per_project {
            let session_id = format!("sess-{}-{}", p, s);
            let jsonl_path = project_dir.join(format!("{}.jsonl", &session_id));
            std::fs::write(&jsonl_path, REALISTIC_JSONL).unwrap();

            entries.push(serde_json::json!({
                "sessionId": session_id,
                "fullPath": jsonl_path.to_string_lossy(),
                "firstPrompt": format!("Hello from session {}", s),
                "summary": format!("Session {} summary", s),
                "messageCount": 3,
                "modified": "2026-01-25T17:18:30.718Z",
                "gitBranch": "main",
                "isSidechain": false,
                "projectPath": format!("/Users/test/proj{}", (b'A' + p as u8) as char)
            }));
        }

        let index_json = serde_json::to_string_pretty(&entries).unwrap();
        std::fs::write(project_dir.join("sessions-index.json"), index_json).unwrap();
    }

    (tmp, claude_dir)
}

/// Shorthand: create a single project with one session.
fn setup_single_session() -> (tempfile::TempDir, std::path::PathBuf) {
    setup_claude_dir(1, 1)
}

// ---------------------------------------------------------------------------
// AC-1: Server starts before indexing
// ---------------------------------------------------------------------------

#[tokio::test]
async fn ac1_server_responds_while_idle() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use std::sync::Arc;
    use tower::ServiceExt;
    use vibe_recall_server::{create_app_with_indexing, IndexingState, IndexingStatus};

    let db = Database::new_in_memory().await.unwrap();
    let indexing = Arc::new(IndexingState::new());

    // Indexing is Idle — server should still respond
    assert_eq!(indexing.status(), IndexingStatus::Idle);

    let app = create_app_with_indexing(db, indexing);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

// ---------------------------------------------------------------------------
// AC-2: Pass 1 reads sessions-index.json correctly
// ---------------------------------------------------------------------------

#[tokio::test]
async fn ac2_pass1_reads_index_fields() {
    let (_tmp, claude_dir) = setup_single_session();
    let db = Database::new_in_memory().await.unwrap();

    let (projects, sessions) = pass_1_read_indexes(&claude_dir, &db).await.unwrap();
    assert_eq!(projects, 1, "Should discover 1 project");
    assert_eq!(sessions, 1, "Should discover 1 session");

    let db_projects = db.list_projects().await.unwrap();
    assert_eq!(db_projects.len(), 1);

    let session = &db_projects[0].sessions[0];
    assert_eq!(session.id, "sess-0-0");
    assert_eq!(session.preview, "Hello from session 0");
    assert_eq!(session.summary.as_deref(), Some("Session 0 summary"));
    assert_eq!(session.message_count, 3);
    assert_eq!(session.git_branch.as_deref(), Some("main"));
    assert!(!session.is_sidechain);
    // Pass 2 hasn't run yet
    assert!(!session.deep_indexed, "deep_indexed should be false after Pass 1 only");
}

// ---------------------------------------------------------------------------
// AC-3: Pass 1 handles missing / malformed index files
// ---------------------------------------------------------------------------

#[tokio::test]
async fn ac3_missing_index_returns_zero() {
    let tmp = tempfile::TempDir::new().unwrap();
    let claude_dir = tmp.path().to_path_buf();
    // Create projects/ dir with a project subdirectory but NO sessions-index.json
    let project_dir = claude_dir.join("projects").join("empty-project");
    std::fs::create_dir_all(&project_dir).unwrap();

    let db = Database::new_in_memory().await.unwrap();
    let (projects, sessions) = pass_1_read_indexes(&claude_dir, &db).await.unwrap();

    assert_eq!(projects, 0, "No valid index files means 0 projects");
    assert_eq!(sessions, 0);
}

#[tokio::test]
async fn ac3_malformed_json_does_not_panic() {
    let tmp = tempfile::TempDir::new().unwrap();
    let claude_dir = tmp.path().to_path_buf();
    let project_dir = claude_dir.join("projects").join("bad-json-project");
    std::fs::create_dir_all(&project_dir).unwrap();
    std::fs::write(
        project_dir.join("sessions-index.json"),
        "this is not valid JSON {{{",
    )
    .unwrap();

    let db = Database::new_in_memory().await.unwrap();
    // Should not panic; read_all_session_indexes logs a warning and skips malformed files
    let result = pass_1_read_indexes(&claude_dir, &db).await;
    // The function should succeed (skipping the bad file) — 0 projects found
    assert!(result.is_ok());
    let (projects, sessions) = result.unwrap();
    assert_eq!(projects, 0);
    assert_eq!(sessions, 0);
}

// ---------------------------------------------------------------------------
// AC-4: Pass 2 fills extended metadata
// ---------------------------------------------------------------------------

#[tokio::test]
async fn ac4_pass2_fills_deep_fields() {
    let (_tmp, claude_dir) = setup_single_session();
    let db = Database::new_in_memory().await.unwrap();

    pass_1_read_indexes(&claude_dir, &db).await.unwrap();

    // Before Pass 2
    let before = db.list_projects().await.unwrap();
    assert!(!before[0].sessions[0].deep_indexed);

    // Run Pass 2
    let (indexed, _) = pass_2_deep_index(&db, None, None, |_| {}, |_, _, _| {}).await.unwrap();
    assert_eq!(indexed, 1);

    // After Pass 2
    let after = db.list_projects().await.unwrap();
    let session = &after[0].sessions[0];
    assert!(session.deep_indexed, "Should be deep indexed");
    assert!(session.turn_count > 0, "turn_count should be > 0");
    assert!(!session.last_message.is_empty(), "last_message should be populated");
    // The JSONL has a Read tool use, verify tool_counts
    assert_eq!(session.tool_counts.read, 1, "Should detect 1 Read tool use");
}

// ---------------------------------------------------------------------------
// AC-7: Batch DB writes (20+ sessions)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn ac7_batch_writes_all_sessions() {
    // 4 projects x 6 sessions = 24 total
    let (_tmp, claude_dir) = setup_claude_dir(4, 6);
    let db = Database::new_in_memory().await.unwrap();

    let (projects, sessions) = pass_1_read_indexes(&claude_dir, &db).await.unwrap();
    assert_eq!(projects, 4, "Should find 4 projects");
    assert_eq!(sessions, 24, "Should find 24 sessions total");

    // Run Pass 2
    let (indexed, _) = pass_2_deep_index(&db, None, None, |_| {}, |_, _, _| {}).await.unwrap();
    assert_eq!(indexed, 24, "All 24 sessions should be deep indexed");

    // Verify all in DB
    let db_projects = db.list_projects().await.unwrap();
    let total_sessions: usize = db_projects.iter().map(|p| p.sessions.len()).sum();
    assert_eq!(total_sessions, 24, "All 24 sessions should be in DB");

    // All should be deep indexed
    for project in &db_projects {
        for session in &project.sessions {
            assert!(session.deep_indexed, "Session {} should be deep indexed", session.id);
        }
    }
}

// ---------------------------------------------------------------------------
// AC-8: Parallel processing completes
// ---------------------------------------------------------------------------

#[tokio::test]
async fn ac8_parallel_processing_completes() {
    // 2 projects x 6 sessions = 12 total
    let (_tmp, claude_dir) = setup_claude_dir(2, 6);
    let db = Database::new_in_memory().await.unwrap();

    pass_1_read_indexes(&claude_dir, &db).await.unwrap();

    let start = std::time::Instant::now();
    let (indexed, _) = pass_2_deep_index(&db, None, None, |_| {}, |_, _, _| {}).await.unwrap();
    let elapsed = start.elapsed();

    assert_eq!(indexed, 12, "Should deep-index all 12 sessions");
    // Sanity check: 12 small files should finish in under 10 seconds
    assert!(
        elapsed.as_secs() < 10,
        "Parallel processing should complete quickly, took {:?}",
        elapsed
    );
}

// ---------------------------------------------------------------------------
// AC-9: run_background_index callbacks fire
// ---------------------------------------------------------------------------

#[tokio::test]
async fn ac9_callbacks_fire_correctly() {
    let (_tmp, claude_dir) = setup_single_session();
    let db = Database::new_in_memory().await.unwrap();

    let pass1_called = Arc::new(std::sync::Mutex::new(false));
    let pass1_projects = Arc::new(AtomicUsize::new(0));
    let pass1_sessions = Arc::new(AtomicUsize::new(0));
    let file_done_count = Arc::new(AtomicUsize::new(0));
    let complete_count = Arc::new(AtomicUsize::new(0));

    let p1c = pass1_called.clone();
    let p1p = pass1_projects.clone();
    let p1s = pass1_sessions.clone();
    let fdc = file_done_count.clone();
    let cc = complete_count.clone();

    run_background_index(
        &claude_dir,
        &db,
        None, // no registry holder in tests
        None, // no search index in tests
        move |projects, sessions| {
            *p1c.lock().unwrap() = true;
            p1p.store(projects, Ordering::Relaxed);
            p1s.store(sessions, Ordering::Relaxed);
        },
        |_total_bytes| {},
        move |done, _total, _bytes| {
            fdc.store(done, Ordering::Relaxed);
        },
        move |total| {
            cc.store(total, Ordering::Relaxed);
        },
    )
    .await
    .unwrap();

    assert!(*pass1_called.lock().unwrap(), "on_pass1_done should have been called");
    assert_eq!(pass1_projects.load(Ordering::Relaxed), 1);
    assert_eq!(pass1_sessions.load(Ordering::Relaxed), 1);
    assert!(file_done_count.load(Ordering::Relaxed) > 0, "on_file_done should have been called");
    assert_eq!(complete_count.load(Ordering::Relaxed), 1, "on_complete should report 1 indexed");
}

// ---------------------------------------------------------------------------
// AC-11: Subsequent launches skip Pass 2
// ---------------------------------------------------------------------------

#[tokio::test]
async fn ac11_second_pass2_returns_zero() {
    let (_tmp, claude_dir) = setup_single_session();
    let db = Database::new_in_memory().await.unwrap();

    // First full pipeline
    pass_1_read_indexes(&claude_dir, &db).await.unwrap();
    let (first_run, _) = pass_2_deep_index(&db, None, None, |_| {}, |_, _, _| {}).await.unwrap();
    assert_eq!(first_run, 1, "First run should index 1 session");

    // Second run of Pass 2 — all sessions already deep-indexed
    let (second_run, _) = pass_2_deep_index(&db, None, None, |_| {}, |_, _, _| {}).await.unwrap();
    assert_eq!(second_run, 0, "Second run should skip all (already indexed)");
}

// ---------------------------------------------------------------------------
// AC-12: New schema fields work end-to-end
// ---------------------------------------------------------------------------

#[tokio::test]
async fn ac12_new_schema_fields_end_to_end() {
    let (_tmp, claude_dir) = setup_single_session();
    let db = Database::new_in_memory().await.unwrap();

    // Run full pipeline
    run_background_index(&claude_dir, &db, None, None, |_, _| {}, |_| {}, |_, _, _| {}, |_| {})
        .await
        .unwrap();

    let projects = db.list_projects().await.unwrap();
    assert!(!projects.is_empty(), "Should have at least 1 project");

    let session = &projects[0].sessions[0];

    // Verify summary field from sessions-index.json
    assert!(
        session.summary.is_some(),
        "summary should be populated from sessions-index.json"
    );
    assert_eq!(session.summary.as_deref(), Some("Session 0 summary"));

    // Verify git_branch field from sessions-index.json
    assert!(
        session.git_branch.is_some(),
        "git_branch should be populated from sessions-index.json"
    );
    assert_eq!(session.git_branch.as_deref(), Some("main"));

    // Verify is_sidechain
    assert!(!session.is_sidechain);

    // Verify deep_indexed is true after full pipeline
    assert!(session.deep_indexed);

    // Verify project-level fields
    assert!(!projects[0].display_name.is_empty(), "display_name should not be empty");
    assert!(!projects[0].path.is_empty(), "project path should not be empty");
}

// ---------------------------------------------------------------------------
// AC-12 extended: JSON serialization uses camelCase for new fields
// ---------------------------------------------------------------------------

#[tokio::test]
async fn ac12_json_serialization_includes_new_fields() {
    let (_tmp, claude_dir) = setup_single_session();
    let db = Database::new_in_memory().await.unwrap();

    run_background_index(&claude_dir, &db, None, None, |_, _| {}, |_| {}, |_, _, _| {}, |_| {})
        .await
        .unwrap();

    let projects = db.list_projects().await.unwrap();
    let json = serde_json::to_string(&projects).unwrap();

    // New fields should serialize with camelCase
    assert!(json.contains("\"gitBranch\""), "gitBranch should appear in JSON: {}", json);
    assert!(json.contains("\"isSidechain\""), "isSidechain should appear in JSON: {}", json);
    assert!(json.contains("\"deepIndexed\""), "deepIndexed should appear in JSON: {}", json);
    // summary is present (not None), so it should be serialized
    assert!(json.contains("\"summary\""), "summary should appear in JSON: {}", json);
}

// ---------------------------------------------------------------------------
// Additional: Pass 1 + Pass 2 with multiple projects
// ---------------------------------------------------------------------------

#[tokio::test]
async fn multiple_projects_indexed_correctly() {
    let (_tmp, claude_dir) = setup_claude_dir(3, 2);
    let db = Database::new_in_memory().await.unwrap();

    let (projects, sessions) = pass_1_read_indexes(&claude_dir, &db).await.unwrap();
    assert_eq!(projects, 3);
    assert_eq!(sessions, 6);

    let (indexed, _) = pass_2_deep_index(&db, None, None, |_| {}, |_, _, _| {}).await.unwrap();
    assert_eq!(indexed, 6);

    let db_projects = db.list_projects().await.unwrap();
    assert_eq!(db_projects.len(), 3);

    // Each project should have 2 sessions
    for project in &db_projects {
        assert_eq!(project.sessions.len(), 2, "Project {} should have 2 sessions", project.name);
    }
}

// ---------------------------------------------------------------------------
// AC-13: Token tracking end-to-end
// ---------------------------------------------------------------------------

#[tokio::test]
async fn ac13_turns_and_models_populated_after_pipeline() {
    let (_tmp, claude_dir) = setup_single_session();
    let db = Database::new_in_memory().await.unwrap();

    // Run full pipeline
    run_background_index(&claude_dir, &db, None, None, |_, _| {}, |_| {}, |_, _, _| {}, |_| {})
        .await
        .unwrap();

    // Verify models table has data
    let models = db.get_all_models().await.unwrap();
    assert!(
        !models.is_empty(),
        "Models table should have at least 1 model after indexing"
    );
    assert_eq!(models[0].id, "claude-opus-4-5-20251101");
    assert_eq!(models[0].provider.as_deref(), Some("anthropic"));
    assert_eq!(models[0].family.as_deref(), Some("opus"));
    assert!(models[0].total_turns > 0, "Model should have turn count > 0");

    // Verify token stats
    let token_stats = db.get_token_stats().await.unwrap();
    assert!(token_stats.turns_count > 0, "Should have at least 1 turn");
    assert!(token_stats.total_input_tokens > 0, "Should have input tokens");
    assert!(token_stats.total_output_tokens > 0, "Should have output tokens");
    assert!(token_stats.total_cache_read_tokens > 0, "Should have cache read tokens");
}
