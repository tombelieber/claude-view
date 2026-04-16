//! Tests for GET /api/sessions (list, filter, sort, pagination).

#![cfg(test)]

use axum::http::StatusCode;

use super::tests_common::*;

// ========================================================================
// GET /api/sessions tests
// ========================================================================

#[tokio::test]
async fn test_list_sessions_empty() {
    let db = test_db().await;
    let app = build_app(db);
    let (status, body) = do_get(app, "/api/sessions").await;

    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["total"], 0);
    assert!(json["sessions"].as_array().unwrap().is_empty());
    assert_eq!(json["filter"], "all");
    assert_eq!(json["sort"], "recent");
}

#[tokio::test]
async fn test_list_sessions_with_data() {
    let fx = CatalogFixture::new().await;
    let session = make_session("sess-1", "project-a", 1700000000);
    fx.seed(session, "Project A").await;

    let (status, body) = do_get(fx.app(), "/api/sessions").await;

    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["total"], 1);
    assert_eq!(json["sessions"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_list_sessions_invalid_filter() {
    let db = test_db().await;
    let app = build_app(db);
    let (status, body) = do_get(app, "/api/sessions?filter=invalid").await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(json["details"].as_str().unwrap().contains("invalid"));
    assert!(json["details"]
        .as_str()
        .unwrap()
        .contains("all, has_commits"));
}

#[tokio::test]
async fn test_list_sessions_invalid_sort() {
    let db = test_db().await;
    let app = build_app(db);
    let (status, body) = do_get(app, "/api/sessions?sort=invalid").await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(json["details"].as_str().unwrap().contains("invalid"));
    assert!(json["details"].as_str().unwrap().contains("recent, tokens"));
}

#[tokio::test]
async fn test_list_sessions_filter_has_commits() {
    let fx = CatalogFixture::new().await;

    // Session without commits
    let mut session1 = make_session("sess-1", "project-a", 1700000000);
    session1.commit_count = 0;
    fx.seed(session1, "Project A").await;

    // Session with commits
    let mut session2 = make_session("sess-2", "project-a", 1700000100);
    session2.commit_count = 3;
    fx.seed(session2, "Project A").await;

    let (status, body) = do_get(fx.app(), "/api/sessions?filter=has_commits").await;

    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["total"], 1);
    assert_eq!(json["sessions"][0]["id"], "sess-2");
}

#[tokio::test]
async fn test_list_sessions_filter_high_reedit() {
    let fx = CatalogFixture::new().await;

    // Session with low reedit rate (1/10 = 0.1)
    let mut session1 = make_session("sess-1", "project-a", 1700000000);
    session1.files_edited_count = 10;
    session1.reedited_files_count = 1;
    fx.seed(session1, "Project A").await;

    // Session with high reedit rate (5/10 = 0.5)
    let mut session2 = make_session("sess-2", "project-a", 1700000100);
    session2.files_edited_count = 10;
    session2.reedited_files_count = 5;
    fx.seed(session2, "Project A").await;

    let (status, body) = do_get(fx.app(), "/api/sessions?filter=high_reedit").await;

    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["total"], 1);
    assert_eq!(json["sessions"][0]["id"], "sess-2");
}

#[tokio::test]
async fn test_list_sessions_filter_long_session() {
    let fx = CatalogFixture::new().await;

    // Short session (10 minutes)
    let mut session1 = make_session("sess-1", "project-a", 1700000000);
    session1.duration_seconds = 600;
    fx.seed(session1, "Project A").await;

    // Long session (1 hour)
    let mut session2 = make_session("sess-2", "project-a", 1700000100);
    session2.duration_seconds = 3600;
    fx.seed(session2, "Project A").await;

    let (status, body) = do_get(fx.app(), "/api/sessions?filter=long_session").await;

    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["total"], 1);
    assert_eq!(json["sessions"][0]["id"], "sess-2");
}

#[tokio::test]
async fn test_list_sessions_sort_tokens() {
    let fx = CatalogFixture::new().await;

    let mut session1 = make_session("sess-1", "project-a", 1700000000);
    session1.total_input_tokens = Some(1000);
    session1.total_output_tokens = Some(500);
    fx.seed(session1, "Project A").await;

    let mut session2 = make_session("sess-2", "project-a", 1700000100);
    session2.total_input_tokens = Some(10000);
    session2.total_output_tokens = Some(5000);
    fx.seed(session2, "Project A").await;

    let (status, body) = do_get(fx.app(), "/api/sessions?sort=tokens").await;

    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    // sess-2 should be first (more tokens)
    assert_eq!(json["sessions"][0]["id"], "sess-2");
    assert_eq!(json["sessions"][1]["id"], "sess-1");
}

#[tokio::test]
async fn test_list_sessions_sort_duration() {
    let fx = CatalogFixture::new().await;

    let mut session1 = make_session("sess-1", "project-a", 1700000000);
    session1.duration_seconds = 600;
    fx.seed(session1, "Project A").await;

    let mut session2 = make_session("sess-2", "project-a", 1700000100);
    session2.duration_seconds = 3600;
    fx.seed(session2, "Project A").await;

    let (status, body) = do_get(fx.app(), "/api/sessions?sort=duration").await;

    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    // sess-2 should be first (longer duration)
    assert_eq!(json["sessions"][0]["id"], "sess-2");
}

#[tokio::test]
async fn test_list_sessions_pagination() {
    let fx = CatalogFixture::new().await;

    // Insert 5 sessions
    for i in 0..5 {
        let session = make_session(&format!("sess-{}", i), "project-a", 1700000000 + i);
        fx.seed(session, "Project A").await;
    }

    let (status, body) = do_get(fx.app(), "/api/sessions?limit=2&offset=1").await;

    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["total"], 5); // Total count before pagination
    assert_eq!(json["sessions"].as_array().unwrap().len(), 2); // Only 2 returned
}

// ========================================================================
// New multi-facet filter tests
// ========================================================================

#[tokio::test]
#[ignore = "branches filter not yet wired into JSONL-first list handler (see list.rs:235 TODO re: CatalogRow branch field)"]
async fn test_list_sessions_filter_by_branches() {
    let fx = CatalogFixture::new().await;

    let mut session1 = make_session("sess-1", "project-a", 1700000000);
    session1.git_branch = Some("main".to_string());
    fx.seed(session1, "Project A").await;

    let mut session2 = make_session("sess-2", "project-a", 1700000100);
    session2.git_branch = Some("feature/auth".to_string());
    fx.seed(session2, "Project A").await;

    let mut session3 = make_session("sess-3", "project-a", 1700000200);
    session3.git_branch = Some("fix/bug".to_string());
    fx.seed(session3, "Project A").await;

    let (status, body) = do_get(fx.app(), "/api/sessions?branches=main,feature/auth").await;

    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["total"], 2);
    let ids: Vec<&str> = json["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&"sess-1"));
    assert!(ids.contains(&"sess-2"));
    assert!(!ids.contains(&"sess-3"));
}

#[tokio::test]
async fn test_list_sessions_filter_by_models() {
    // TODO: This test is currently skipped because insert_session() doesn't persist
    // primary_model to the database. This is a pre-existing bug that needs to be fixed
    // in the db crate's insert_session SQL query.
    //
    // Once fixed, uncomment the test below.

    /*
    let db = test_db().await;

    let mut session1 = make_session("sess-1", "project-a", 1700000000);
    session1.primary_model = Some("claude-opus-4".to_string());
    db.insert_session(&session1, "project-a", "Project A")
        .await
        .unwrap();

    let mut session2 = make_session("sess-2", "project-a", 1700000100);
    session2.primary_model = Some("claude-sonnet-4".to_string());
    db.insert_session(&session2, "project-a", "Project A")
        .await
        .unwrap();

    let app = build_app(db);
    let (status, body) = do_get(app, "/api/sessions?models=claude-opus-4").await;

    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["total"], 1);
    assert_eq!(json["sessions"][0]["id"], "sess-1");
    */
}

#[tokio::test]
async fn test_list_sessions_filter_has_skills() {
    let fx = CatalogFixture::new().await;

    let mut session1 = make_session("sess-1", "project-a", 1700000000);
    session1.skills_used = vec!["git".to_string()];
    fx.seed(session1, "Project A").await;

    let mut session2 = make_session("sess-2", "project-a", 1700000100);
    session2.skills_used = vec![];
    fx.seed(session2, "Project A").await;

    let (status, body) = do_get(fx.app(), "/api/sessions?has_skills=true").await;

    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["total"], 1);
    assert_eq!(json["sessions"][0]["id"], "sess-1");
}

#[tokio::test]
async fn test_list_sessions_filter_min_duration() {
    let fx = CatalogFixture::new().await;

    let mut session1 = make_session("sess-1", "project-a", 1700000000);
    session1.duration_seconds = 300; // 5 minutes
    fx.seed(session1, "Project A").await;

    let mut session2 = make_session("sess-2", "project-a", 1700000100);
    session2.duration_seconds = 2400; // 40 minutes
    fx.seed(session2, "Project A").await;

    let (status, body) = do_get(fx.app(), "/api/sessions?min_duration=1800").await; // 30 minutes

    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["total"], 1);
    assert_eq!(json["sessions"][0]["id"], "sess-2");
}

#[tokio::test]
async fn test_list_sessions_filter_min_files() {
    let fx = CatalogFixture::new().await;

    // JSONL-first: `min_files` = files_read_count + files_edited_count. Zero
    // reads here keeps the test's intent (differentiate by edits) while
    // honouring the handler's semantics.
    let mut session1 = make_session("sess-1", "project-a", 1700000000);
    session1.files_read_count = 0;
    session1.files_edited_count = 2;
    fx.seed(session1, "Project A").await;

    let mut session2 = make_session("sess-2", "project-a", 1700000100);
    session2.files_read_count = 0;
    session2.files_edited_count = 10;
    fx.seed(session2, "Project A").await;

    let (status, body) = do_get(fx.app(), "/api/sessions?min_files=5").await;

    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["total"], 1);
    assert_eq!(json["sessions"][0]["id"], "sess-2");
}

#[tokio::test]
async fn test_list_sessions_filter_min_tokens() {
    // TODO: This test is currently skipped because insert_session() doesn't persist
    // token counts to the database (only deep_index_session does via aggregation).
    // This is a pre-existing limitation of the test helper.
    //
    // Once we add proper token persistence or use deep_index_session in tests,
    // uncomment the test below.

    /*
    let db = test_db().await;

    let mut session1 = make_session("sess-1", "project-a", 1700000000);
    session1.total_input_tokens = Some(1000);
    session1.total_output_tokens = Some(500);
    db.insert_session(&session1, "project-a", "Project A")
        .await
        .unwrap();

    let mut session2 = make_session("sess-2", "project-a", 1700000100);
    session2.total_input_tokens = Some(50000);
    session2.total_output_tokens = Some(25000);
    db.insert_session(&session2, "project-a", "Project A")
        .await
        .unwrap();

    let app = build_app(db);
    let (status, body) = do_get(app, "/api/sessions?min_tokens=10000").await;

    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["total"], 1);
    assert_eq!(json["sessions"][0]["id"], "sess-2");
    */
}

#[tokio::test]
async fn test_list_sessions_filter_time_range() {
    let fx = CatalogFixture::new().await;

    // The catalog filter matches against `CatalogRow::sort_ts()`, which in
    // the fixture derives from the last JSONL timestamp (= modified_at +
    // duration_seconds). The default 600s duration shifts each session by
    // only +600s — within the test's time window resolution.
    let session1 = make_session("sess-1", "project-a", 1700000000); // Jan 2024
    fx.seed(session1, "Project A").await;

    let session2 = make_session("sess-2", "project-a", 1720000000); // Jul 2024
    fx.seed(session2, "Project A").await;

    let session3 = make_session("sess-3", "project-a", 1740000000); // Dec 2024
    fx.seed(session3, "Project A").await;

    // Filter for sessions between Feb 2024 and Nov 2024
    let (status, body) = do_get(
        fx.app(),
        "/api/sessions?time_after=1710000000&time_before=1730000000",
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["total"], 1);
    assert_eq!(json["sessions"][0]["id"], "sess-2");
}

#[tokio::test]
#[ignore = "branches filter not yet wired into JSONL-first list handler (see list.rs:235 TODO re: CatalogRow branch field)"]
async fn test_list_sessions_multiple_filters_combined() {
    let fx = CatalogFixture::new().await;

    let mut session1 = make_session("sess-1", "project-a", 1700000000);
    session1.git_branch = Some("main".to_string());
    session1.commit_count = 3;
    session1.duration_seconds = 2400;
    fx.seed(session1, "Project A").await;

    let mut session2 = make_session("sess-2", "project-a", 1700000100);
    session2.git_branch = Some("feature/auth".to_string());
    session2.commit_count = 0;
    session2.duration_seconds = 2400;
    fx.seed(session2, "Project A").await;

    let mut session3 = make_session("sess-3", "project-a", 1700000200);
    session3.git_branch = Some("main".to_string());
    session3.commit_count = 5;
    session3.duration_seconds = 600;
    fx.seed(session3, "Project A").await;

    // Filter: main branch AND has commits AND duration >= 30 mins
    let (status, body) = do_get(
        fx.app(),
        "/api/sessions?branches=main&has_commits=true&min_duration=1800",
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["total"], 1);
    assert_eq!(json["sessions"][0]["id"], "sess-1");
}
