use super::*;
use crate::error::SessionIndexError;
use tempfile::TempDir;

// ========================================================================
// read_all_session_indexes Tests
// ========================================================================

#[test]
fn test_read_all_session_indexes() {
    let dir = TempDir::new().unwrap();
    let projects_dir = dir.path().join("projects");
    std::fs::create_dir(&projects_dir).unwrap();

    // Project with a valid sessions-index.json
    let proj_a = projects_dir.join("project-a");
    std::fs::create_dir(&proj_a).unwrap();
    let json_a = r#"{"version":1,"entries":[{"sessionId": "sess-1"}, {"sessionId": "sess-2"}]}"#;
    std::fs::write(proj_a.join("sessions-index.json"), json_a).unwrap();

    // Project without sessions-index.json (should be skipped)
    let proj_b = projects_dir.join("project-b");
    std::fs::create_dir(&proj_b).unwrap();

    // Project with malformed JSON (should be skipped with warning)
    let proj_c = projects_dir.join("project-c");
    std::fs::create_dir(&proj_c).unwrap();
    std::fs::write(proj_c.join("sessions-index.json"), "broken").unwrap();

    let results = read_all_session_indexes(dir.path()).unwrap();
    // Only project-a should succeed
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "project-a");
    assert_eq!(results[0].1.len(), 2);
}

#[test]
fn test_read_all_catches_unlisted_jsonl_files() {
    let dir = TempDir::new().unwrap();
    let projects_dir = dir.path().join("projects");
    std::fs::create_dir(&projects_dir).unwrap();

    let proj = projects_dir.join("my-project");
    std::fs::create_dir(&proj).unwrap();

    // Index lists only sess-1
    let json = r#"{"version":1,"entries":[{"sessionId": "sess-1"}]}"#;
    std::fs::write(proj.join("sessions-index.json"), json).unwrap();

    // But there are also sess-2.jsonl and sess-3.jsonl on disk
    // Real conversation content so classify_jsonl_file returns Conversation
    std::fs::write(
        proj.join("sess-1.jsonl"),
        concat!(
            r#"{"type":"user","uuid":"u1","message":{"content":"hi"},"cwd":"/proj"}"#,
            "\n",
            r#"{"type":"assistant","uuid":"a1","message":{"content":"ok"}}"#,
            "\n",
        ),
    )
    .unwrap();
    std::fs::write(
        proj.join("sess-2.jsonl"),
        concat!(
            r#"{"type":"user","uuid":"u2","message":{"content":"hi"},"cwd":"/proj"}"#,
            "\n",
            r#"{"type":"assistant","uuid":"a2","message":{"content":"ok"}}"#,
            "\n",
        ),
    )
    .unwrap();
    std::fs::write(
        proj.join("sess-3.jsonl"),
        concat!(
            r#"{"type":"user","uuid":"u3","message":{"content":"hi"},"cwd":"/proj"}"#,
            "\n",
            r#"{"type":"assistant","uuid":"a3","message":{"content":"ok"}}"#,
            "\n",
        ),
    )
    .unwrap();

    let results = read_all_session_indexes(dir.path()).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "my-project");
    // Should discover all 3: 1 from index + 2 unlisted
    assert_eq!(results[0].1.len(), 3);

    let ids: Vec<&str> = results[0].1.iter().map(|e| e.session_id.as_str()).collect();
    assert!(ids.contains(&"sess-1"));
    assert!(ids.contains(&"sess-2"));
    assert!(ids.contains(&"sess-3"));

    // Unlisted entries should have full_path set
    let unlisted: Vec<_> = results[0]
        .1
        .iter()
        .filter(|e| e.session_id != "sess-1")
        .collect();
    for entry in unlisted {
        assert!(entry.full_path.is_some());
        assert!(entry.full_path.as_ref().unwrap().ends_with(".jsonl"));
    }
}

#[test]
fn test_read_all_missing_projects_dir() {
    let dir = TempDir::new().unwrap();
    let result = read_all_session_indexes(dir.path());
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        SessionIndexError::ProjectsDirNotFound { .. }
    ));
}

// ========================================================================
// Catch-Up Classification Tests
// ========================================================================

#[test]
fn test_read_all_catchup_skips_metadata_files() {
    let dir = TempDir::new().unwrap();
    let projects_dir = dir.path().join("projects");
    std::fs::create_dir(&projects_dir).unwrap();

    let proj = projects_dir.join("my-project");
    std::fs::create_dir(&proj).unwrap();

    // Index lists only sess-1
    let json = r#"{"version":1,"entries":[{"sessionId": "sess-1"}]}"#;
    std::fs::write(proj.join("sessions-index.json"), json).unwrap();

    // Real conversation file (not in index -- should be caught up)
    std::fs::write(proj.join("conv-2.jsonl"), concat!(
        r#"{"type":"user","uuid":"u1","message":{"content":"hi"},"cwd":"/Users/dev/@org/proj"}"#, "\n",
        r#"{"type":"assistant","uuid":"a1","message":{"content":"ok"}}"#, "\n",
    )).unwrap();

    // Metadata-only file (file-history-snapshot -- should be SKIPPED)
    std::fs::write(
        proj.join("fhs-3.jsonl"),
        concat!(
            r#"{"type":"file-history-snapshot","messageId":"m1","snapshot":{}}"#,
            "\n",
        ),
    )
    .unwrap();

    // Another metadata file (summary with timestamp -- the dangerous case)
    std::fs::write(
        proj.join("sum-4.jsonl"),
        concat!(
            r#"{"type":"summary","summary":"did stuff","timestamp":"2026-02-25T10:00:00Z"}"#,
            "\n",
        ),
    )
    .unwrap();

    let results = read_all_session_indexes(dir.path()).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "my-project");

    // Should have 2 entries: sess-1 from index + conv-2 from catch-up
    // fhs-3 and sum-4 should be filtered out by classification
    assert_eq!(results[0].1.len(), 2);

    let ids: Vec<&str> = results[0].1.iter().map(|e| e.session_id.as_str()).collect();
    assert!(ids.contains(&"sess-1"));
    assert!(ids.contains(&"conv-2"));
    assert!(!ids.contains(&"fhs-3"));
    assert!(!ids.contains(&"sum-4"));
}

#[test]
fn test_read_all_catchup_captures_cwd_and_parent() {
    let dir = TempDir::new().unwrap();
    let projects_dir = dir.path().join("projects");
    std::fs::create_dir(&projects_dir).unwrap();

    let proj = projects_dir.join("test-proj");
    std::fs::create_dir(&proj).unwrap();

    // Empty index
    std::fs::write(
        proj.join("sessions-index.json"),
        r#"{"version":1,"entries":[]}"#,
    )
    .unwrap();

    // Forked conversation with cwd and parentUuid
    std::fs::write(proj.join("fork-1.jsonl"), concat!(
        r#"{"type":"user","uuid":"u1","parentUuid":"parent-abc","message":{"content":"continue"},"cwd":"/Users/dev/@org/my-project"}"#, "\n",
        r#"{"type":"assistant","uuid":"a1","message":{"content":"ok"}}"#, "\n",
    )).unwrap();

    let results = read_all_session_indexes(dir.path()).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].1.len(), 1);

    let entry = &results[0].1[0];
    assert_eq!(entry.session_id, "fork-1");
    assert_eq!(
        entry.session_cwd.as_deref(),
        Some("/Users/dev/@org/my-project")
    );
    assert_eq!(entry.parent_session_id.as_deref(), Some("parent-abc"));
}

// ========================================================================
// discover_orphan_sessions Tests
// ========================================================================

#[test]
fn test_discover_orphan_sessions_finds_jsonl_without_index() {
    let dir = TempDir::new().unwrap();
    let projects_dir = dir.path().join("projects");
    std::fs::create_dir(&projects_dir).unwrap();

    let orphan_proj = projects_dir.join("orphan-project");
    std::fs::create_dir(&orphan_proj).unwrap();
    std::fs::write(
        orphan_proj.join("abc-123.jsonl"),
        concat!(
            r#"{"type":"user","uuid":"u1","message":{"content":"hi"},"cwd":"/proj"}"#,
            "\n",
            r#"{"type":"assistant","uuid":"a1","message":{"content":"ok"}}"#,
            "\n",
        ),
    )
    .unwrap();
    std::fs::write(
        orphan_proj.join("def-456.jsonl"),
        concat!(
            r#"{"type":"user","uuid":"u2","message":{"content":"hi"},"cwd":"/proj"}"#,
            "\n",
            r#"{"type":"assistant","uuid":"a2","message":{"content":"ok"}}"#,
            "\n",
        ),
    )
    .unwrap();

    let results = discover_orphan_sessions(dir.path()).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "orphan-project");
    assert_eq!(results[0].1.len(), 2);

    let ids: Vec<&str> = results[0].1.iter().map(|e| e.session_id.as_str()).collect();
    assert!(ids.contains(&"abc-123"));
    assert!(ids.contains(&"def-456"));

    // Verify full_path is set
    for entry in &results[0].1 {
        assert!(entry.full_path.is_some());
        assert!(entry.full_path.as_ref().unwrap().ends_with(".jsonl"));
    }
}

#[test]
fn test_discover_orphan_sessions_skips_indexed_dirs() {
    let dir = TempDir::new().unwrap();
    let projects_dir = dir.path().join("projects");
    std::fs::create_dir(&projects_dir).unwrap();

    let indexed_proj = projects_dir.join("indexed-project");
    std::fs::create_dir(&indexed_proj).unwrap();
    std::fs::write(indexed_proj.join("sessions-index.json"), "[]").unwrap();
    std::fs::write(
        indexed_proj.join("abc-123.jsonl"),
        concat!(
            r#"{"type":"user","uuid":"u1","message":{"content":"hi"},"cwd":"/proj"}"#,
            "\n",
            r#"{"type":"assistant","uuid":"a1","message":{"content":"ok"}}"#,
            "\n",
        ),
    )
    .unwrap();

    let results = discover_orphan_sessions(dir.path()).unwrap();
    assert!(results.is_empty());
}

#[test]
fn test_discover_orphan_sessions_ignores_non_jsonl() {
    let dir = TempDir::new().unwrap();
    let projects_dir = dir.path().join("projects");
    std::fs::create_dir(&projects_dir).unwrap();

    let proj = projects_dir.join("some-project");
    std::fs::create_dir(&proj).unwrap();
    std::fs::write(proj.join("notes.txt"), "text").unwrap();
    std::fs::write(proj.join("config.json"), "{}").unwrap();

    let results = discover_orphan_sessions(dir.path()).unwrap();
    assert!(results.is_empty());
}

#[test]
fn test_discover_orphan_sessions_empty_projects_dir() {
    let dir = TempDir::new().unwrap();
    let projects_dir = dir.path().join("projects");
    std::fs::create_dir(&projects_dir).unwrap();

    let results = discover_orphan_sessions(dir.path()).unwrap();
    assert!(results.is_empty());
}

#[test]
fn test_discover_orphan_sessions_no_projects_dir() {
    let dir = TempDir::new().unwrap();
    let results = discover_orphan_sessions(dir.path()).unwrap();
    assert!(results.is_empty());
}

#[test]
fn test_discover_orphan_sessions_skips_metadata_files() {
    let tmp = tempfile::tempdir().unwrap();
    let proj_dir = tmp.path().join("projects").join("test-project");
    std::fs::create_dir_all(&proj_dir).unwrap();

    // Real session file (has user + assistant)
    let session = proj_dir.join("abc-123.jsonl");
    std::fs::write(
        &session,
        concat!(
            r#"{"type":"user","uuid":"u1","message":{"content":"hello"},"cwd":"/proj"}"#,
            "\n",
            r#"{"type":"assistant","uuid":"a1","message":{"content":"hi"}}"#,
            "\n",
        ),
    )
    .unwrap();

    // Metadata-only file (should NOT count as session)
    let snapshot = proj_dir.join("fhs-456.jsonl");
    std::fs::write(
        &snapshot,
        concat!(
            r#"{"type":"file-history-snapshot","messageId":"m1","snapshot":{}}"#,
            "\n",
        ),
    )
    .unwrap();

    let results = discover_orphan_sessions(tmp.path()).unwrap();
    let entries: Vec<_> = results.into_iter().flat_map(|(_, v)| v).collect();

    // Only the real session should be discovered
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].session_id, "abc-123");
    assert_eq!(entries[0].session_cwd.as_deref(), Some("/proj"));
}

#[test]
fn test_discover_orphan_sessions_captures_parent_id() {
    let tmp = tempfile::tempdir().unwrap();
    let proj_dir = tmp.path().join("projects").join("test-project");
    std::fs::create_dir_all(&proj_dir).unwrap();

    let session = proj_dir.join("fork-789.jsonl");
    std::fs::write(&session, concat!(
        r#"{"type":"user","uuid":"u1","parentUuid":"parent-abc","message":{"content":"continue"},"cwd":"/proj"}"#, "\n",
        r#"{"type":"assistant","uuid":"a1","message":{"content":"ok"}}"#, "\n",
    )).unwrap();

    let results = discover_orphan_sessions(tmp.path()).unwrap();
    let entries: Vec<_> = results.into_iter().flat_map(|(_, v)| v).collect();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].parent_session_id.as_deref(), Some("parent-abc"));
}

// ========================================================================
// resolve_cwd_for_project Tests
// ========================================================================

#[test]
fn test_resolve_cwd_for_project_finds_cwd() {
    let tmp = tempfile::tempdir().unwrap();
    let proj_dir = tmp.path();

    // Write a JSONL with cwd
    let session = proj_dir.join("abc-123.jsonl");
    std::fs::write(&session, concat!(
        r#"{"type":"user","uuid":"u1","message":{"content":"hi"},"cwd":"/Users/dev/@org/my-project"}"#, "\n",
        r#"{"type":"assistant","uuid":"a1","message":{"content":"ok"}}"#, "\n",
    )).unwrap();

    let cwd = resolve_cwd_for_project(proj_dir);
    assert_eq!(cwd.as_deref(), Some("/Users/dev/@org/my-project"));
}

#[test]
fn test_resolve_cwd_for_project_returns_none_for_empty_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let cwd = resolve_cwd_for_project(tmp.path());
    assert!(cwd.is_none());
}

#[test]
fn test_resolve_cwd_for_project_skips_metadata_files() {
    let tmp = tempfile::tempdir().unwrap();
    let proj_dir = tmp.path();

    // Only metadata file -- no cwd
    let snapshot = proj_dir.join("fhs-456.jsonl");
    std::fs::write(
        &snapshot,
        concat!(
            r#"{"type":"file-history-snapshot","messageId":"m1","snapshot":{}}"#,
            "\n",
        ),
    )
    .unwrap();

    let cwd = resolve_cwd_for_project(proj_dir);
    assert!(cwd.is_none());
}
