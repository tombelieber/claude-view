// crates/db/src/git_correlation/db_ops_tests.rs
//! Tests for database CRUD operations.

#![allow(deprecated)]

use crate::git_correlation::{CorrelationEvidence, CorrelationMatch, DiffStats, GitCommit};
use crate::Database;

async fn seed_git_session(
    db: &Database,
    id: &str,
    project_id: &str,
    project_path: &str,
    first_message_at: Option<i64>,
    last_message_at: Option<i64>,
    file_path: &str,
) {
    sqlx::query(
        "INSERT INTO session_stats (session_id, source_content_hash, source_size,
             parser_version, stats_version, indexed_at, project_id, project_path,
             first_message_at, last_message_at, file_path)
         VALUES (?1, X'00', 0, 1, 4, 0, ?2, ?3, ?4, ?5, ?6)",
    )
    .bind(id)
    .bind(project_id)
    .bind(project_path)
    .bind(first_message_at)
    .bind(last_message_at)
    .bind(file_path)
    .execute(db.pool())
    .await
    .unwrap();
}

#[tokio::test]
async fn test_batch_upsert_commits() {
    let db = Database::new_in_memory().await.unwrap();

    let commits = vec![
        GitCommit {
            hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            repo_path: "/repo/path".to_string(),
            message: "First commit".to_string(),
            author: Some("Author 1".to_string()),
            timestamp: 1706400100,
            branch: Some("main".to_string()),
            files_changed: None,
            insertions: None,
            deletions: None,
        },
        GitCommit {
            hash: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            repo_path: "/repo/path".to_string(),
            message: "Second commit".to_string(),
            author: Some("Author 2".to_string()),
            timestamp: 1706400200,
            branch: Some("feature".to_string()),
            files_changed: None,
            insertions: None,
            deletions: None,
        },
    ];

    let affected = db.batch_upsert_commits(&commits).await.unwrap();
    assert_eq!(affected, 2);

    let fetched = db
        .get_commits_in_range("/repo/path", 1706400000, 1706400300)
        .await
        .unwrap();
    assert_eq!(fetched.len(), 2);
}

#[tokio::test]
async fn test_batch_upsert_commits_updates_existing() {
    let db = Database::new_in_memory().await.unwrap();

    let commits = vec![GitCommit {
        hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        repo_path: "/repo/path".to_string(),
        message: "Original message".to_string(),
        author: Some("Author".to_string()),
        timestamp: 1706400100,
        branch: Some("main".to_string()),
        files_changed: None,
        insertions: None,
        deletions: None,
    }];

    db.batch_upsert_commits(&commits).await.unwrap();

    let updated = vec![GitCommit {
        hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        repo_path: "/repo/path".to_string(),
        message: "Updated message".to_string(),
        author: Some("Author".to_string()),
        timestamp: 1706400100,
        branch: Some("main".to_string()),
        files_changed: None,
        insertions: None,
        deletions: None,
    }];

    db.batch_upsert_commits(&updated).await.unwrap();

    let fetched = db
        .get_commits_in_range("/repo/path", 1706400000, 1706400200)
        .await
        .unwrap();
    assert_eq!(fetched.len(), 1);
    assert_eq!(fetched[0].message, "Updated message");
}

#[tokio::test]
async fn test_batch_insert_session_commits() {
    let db = Database::new_in_memory().await.unwrap();

    crate::test_support::SessionSeedBuilder::new("sess-1")
        .project_id("project-1")
        .project_display_name("Project 1")
        .project_path("/repo/path")
        .file_path("/path/to/sess-1.jsonl")
        .preview("Test session")
        .message_count(10)
        .modified_at(1706400100)
        .size_bytes(5000)
        .seed(&db)
        .await
        .unwrap();

    let commits = vec![GitCommit {
        hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        repo_path: "/repo/path".to_string(),
        message: "Test commit".to_string(),
        author: None,
        timestamp: 1706400100,
        branch: None,
        files_changed: None,
        insertions: None,
        deletions: None,
    }];
    db.batch_upsert_commits(&commits).await.unwrap();

    let matches = vec![CorrelationMatch {
        session_id: "sess-1".to_string(),
        commit_hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        tier: 1,
        evidence: CorrelationEvidence {
            rule: "commit_skill".to_string(),
            skill_ts: Some(1706400100),
            commit_ts: Some(1706400100),
            skill_name: Some("commit".to_string()),
            session_start: None,
            session_end: None,
        },
    }];

    let inserted = db.batch_insert_session_commits(&matches).await.unwrap();
    assert_eq!(inserted, 1);

    let linked = db.get_commits_for_session("sess-1").await.unwrap();
    assert_eq!(linked.len(), 1);
    assert_eq!(linked[0].0.hash, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    assert_eq!(linked[0].1, 1);
    assert!(linked[0].2.contains("commit_skill"));
}

#[tokio::test]
async fn test_tier_priority_tier1_preferred() {
    let db = Database::new_in_memory().await.unwrap();

    crate::test_support::SessionSeedBuilder::new("sess-1")
        .project_id("project-1")
        .project_display_name("Project 1")
        .project_path("/repo/path")
        .file_path("/path/to/sess-1.jsonl")
        .preview("Test session")
        .message_count(10)
        .modified_at(1706400100)
        .size_bytes(5000)
        .seed(&db)
        .await
        .unwrap();

    let commits = vec![GitCommit {
        hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        repo_path: "/repo/path".to_string(),
        message: "Test commit".to_string(),
        author: None,
        timestamp: 1706400100,
        branch: None,
        files_changed: None,
        insertions: None,
        deletions: None,
    }];
    db.batch_upsert_commits(&commits).await.unwrap();

    // Insert Tier 2 first
    let tier2_match = vec![CorrelationMatch {
        session_id: "sess-1".to_string(),
        commit_hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        tier: 2,
        evidence: CorrelationEvidence {
            rule: "during_session".to_string(),
            skill_ts: None,
            commit_ts: Some(1706400100),
            skill_name: None,
            session_start: Some(1706400000),
            session_end: Some(1706400200),
        },
    }];
    db.batch_insert_session_commits(&tier2_match).await.unwrap();

    // Now insert Tier 1
    let tier1_match = vec![CorrelationMatch {
        session_id: "sess-1".to_string(),
        commit_hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        tier: 1,
        evidence: CorrelationEvidence {
            rule: "commit_skill".to_string(),
            skill_ts: Some(1706400100),
            commit_ts: Some(1706400100),
            skill_name: Some("commit".to_string()),
            session_start: None,
            session_end: None,
        },
    }];
    db.batch_insert_session_commits(&tier1_match).await.unwrap();

    let linked = db.get_commits_for_session("sess-1").await.unwrap();
    assert_eq!(linked.len(), 1);
    assert_eq!(linked[0].1, 1, "Tier 1 should be preferred over Tier 2");
    assert!(linked[0].2.contains("commit_skill"));
}

#[tokio::test]
async fn test_count_and_update_commit_count() {
    let db = Database::new_in_memory().await.unwrap();

    crate::test_support::SessionSeedBuilder::new("sess-1")
        .project_id("project-1")
        .project_display_name("Project 1")
        .project_path("/repo/path")
        .file_path("/path/to/sess-1.jsonl")
        .preview("Test session")
        .message_count(10)
        .modified_at(1706400100)
        .size_bytes(5000)
        .seed(&db)
        .await
        .unwrap();

    let commits = vec![
        GitCommit {
            hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            repo_path: "/repo/path".to_string(),
            message: "First".to_string(),
            author: None,
            timestamp: 1706400100,
            branch: None,
            files_changed: None,
            insertions: None,
            deletions: None,
        },
        GitCommit {
            hash: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            repo_path: "/repo/path".to_string(),
            message: "Second".to_string(),
            author: None,
            timestamp: 1706400200,
            branch: None,
            files_changed: None,
            insertions: None,
            deletions: None,
        },
    ];
    db.batch_upsert_commits(&commits).await.unwrap();

    let matches = vec![
        CorrelationMatch {
            session_id: "sess-1".to_string(),
            commit_hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            tier: 1,
            evidence: CorrelationEvidence {
                rule: "commit_skill".to_string(),
                skill_ts: Some(1706400100),
                commit_ts: Some(1706400100),
                skill_name: Some("commit".to_string()),
                session_start: None,
                session_end: None,
            },
        },
        CorrelationMatch {
            session_id: "sess-1".to_string(),
            commit_hash: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            tier: 2,
            evidence: CorrelationEvidence {
                rule: "during_session".to_string(),
                skill_ts: None,
                commit_ts: Some(1706400200),
                skill_name: None,
                session_start: Some(1706400000),
                session_end: Some(1706400300),
            },
        },
    ];
    db.batch_insert_session_commits(&matches).await.unwrap();

    let count = db.count_commits_for_session("sess-1").await.unwrap();
    assert_eq!(count, 2);

    db.update_session_commit_count("sess-1", count as i32)
        .await
        .unwrap();

    let projects = db.list_projects().await.unwrap();
    let session = &projects[0].sessions[0];
    assert_eq!(session.commit_count, 2);
}

#[tokio::test]
async fn test_update_session_loc_from_git() {
    let db = Database::new_in_memory().await.unwrap();

    crate::test_support::SessionSeedBuilder::new("sess-1")
        .project_id("project-1")
        .project_display_name("Project 1")
        .project_path("/repo/path")
        .file_path("/path/to/sess-1.jsonl")
        .preview("Test session")
        .message_count(10)
        .modified_at(1706400100)
        .size_bytes(5000)
        .seed(&db)
        .await
        .unwrap();

    let stats = DiffStats {
        files_changed: 2,
        insertions: 42,
        deletions: 13,
    };
    db.update_session_loc_from_git("sess-1", &stats)
        .await
        .unwrap();

    let row: (i64, i64, i64) = sqlx::query_as(
        "SELECT lines_added, lines_removed, loc_source FROM session_stats WHERE session_id = 'sess-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();

    assert_eq!(row.0, 42, "lines_added should be updated");
    assert_eq!(row.1, 13, "lines_removed should be updated");
    assert_eq!(row.2, 2, "loc_source should be 2 (git verified)");
}

#[tokio::test]
async fn test_update_session_loc_from_git_zero_stats() {
    let db = Database::new_in_memory().await.unwrap();

    crate::test_support::SessionSeedBuilder::new("sess-1")
        .project_id("project-1")
        .project_display_name("Project 1")
        .project_path("/repo/path")
        .file_path("/path/to/sess-1.jsonl")
        .preview("Test session")
        .message_count(10)
        .modified_at(1706400100)
        .size_bytes(5000)
        .seed(&db)
        .await
        .unwrap();

    let stats = DiffStats {
        files_changed: 0,
        insertions: 0,
        deletions: 0,
    };
    db.update_session_loc_from_git("sess-1", &stats)
        .await
        .unwrap();

    let row: (i64, i64, i64) = sqlx::query_as(
        "SELECT lines_added, lines_removed, loc_source FROM session_stats WHERE session_id = 'sess-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();

    assert_eq!(row.0, 0, "lines_added should remain 0");
    assert_eq!(row.1, 0, "lines_removed should remain 0");
    assert_eq!(row.2, 0, "loc_source should remain 0 (not computed)");
}

#[tokio::test]
async fn test_get_sessions_for_git_sync_empty_db() {
    let db = Database::new_in_memory().await.unwrap();
    let sessions = db.get_sessions_for_git_sync().await.unwrap();
    assert!(sessions.is_empty());
}

#[tokio::test]
async fn test_get_sessions_for_git_sync_filters_correctly() {
    let db = Database::new_in_memory().await.unwrap();

    seed_git_session(
        &db,
        "s1",
        "p1",
        "/home/user/project-a",
        Some(1000),
        Some(2000),
        "/tmp/s1.jsonl",
    )
    .await;
    seed_git_session(&db, "s2", "p2", "", None, Some(3000), "/tmp/s2.jsonl").await;
    seed_git_session(
        &db,
        "s3",
        "p3",
        "/home/user/project-b",
        None,
        None,
        "/tmp/s3.jsonl",
    )
    .await;
    seed_git_session(
        &db,
        "s4",
        "p4",
        "/home/user/project-a",
        None,
        Some(4000),
        "/tmp/s4.jsonl",
    )
    .await;

    let sessions = db.get_sessions_for_git_sync().await.unwrap();
    assert_eq!(sessions.len(), 2);

    assert_eq!(sessions[0].session_id, "s4");
    assert_eq!(sessions[0].project_path, "/home/user/project-a");
    assert_eq!(sessions[0].first_message_at, None);
    assert_eq!(sessions[0].last_message_at, Some(4000));

    assert_eq!(sessions[1].session_id, "s1");
    assert_eq!(sessions[1].project_path, "/home/user/project-a");
    assert_eq!(sessions[1].first_message_at, Some(1000));
    assert_eq!(sessions[1].last_message_at, Some(2000));
}
