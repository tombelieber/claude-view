// crates/db/src/git_correlation/sync_tests.rs
//! Tests for the sync pipeline (correlate_session + run_git_sync).

#![allow(deprecated)]

use super::*;
use crate::git_correlation::{CommitSkillInvocation, GitCommit, SessionCorrelationInfo};
use crate::Database;
use tempfile::TempDir;

#[tokio::test]
async fn test_correlate_session_full_pipeline() {
    let db = Database::new_in_memory().await.unwrap();

    db.insert_session_from_index(
        "sess-1",
        "project-1",
        "Project 1",
        "/repo/path",
        "/path/to/sess-1.jsonl",
        "Test session",
        None,
        10,
        1706400800,
        None,
        false,
        5000,
    )
    .await
    .unwrap();

    let commits = vec![
        GitCommit {
            hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            repo_path: "/repo/path".to_string(),
            message: "Tier 1 match".to_string(),
            author: None,
            timestamp: 1706400150,
            branch: None,
            files_changed: None,
            insertions: None,
            deletions: None,
        },
        GitCommit {
            hash: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            repo_path: "/repo/path".to_string(),
            message: "Tier 2 only match".to_string(),
            author: None,
            timestamp: 1706400500,
            branch: None,
            files_changed: None,
            insertions: None,
            deletions: None,
        },
        GitCommit {
            hash: "cccccccccccccccccccccccccccccccccccccccc".to_string(),
            repo_path: "/different/repo".to_string(),
            message: "No match".to_string(),
            author: None,
            timestamp: 1706400200,
            branch: None,
            files_changed: None,
            insertions: None,
            deletions: None,
        },
    ];
    db.batch_upsert_commits(&commits).await.unwrap();

    let session_info = SessionCorrelationInfo {
        session_id: "sess-1".to_string(),
        project_path: "/repo/path".to_string(),
        first_timestamp: Some(1706400100),
        last_timestamp: Some(1706400800),
        commit_skills: vec![CommitSkillInvocation {
            skill_name: "commit".to_string(),
            timestamp_unix: 1706400100,
        }],
    };

    let inserted = correlate_session(&db, &session_info, &commits)
        .await
        .unwrap();

    assert_eq!(inserted, 2, "Should insert 2 matches (Tier 1 + Tier 2)");

    let linked = db.get_commits_for_session("sess-1").await.unwrap();
    assert_eq!(linked.len(), 2);

    let tier1 = linked
        .iter()
        .find(|(c, t, _)| c.hash == "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" && *t == 1);
    assert!(tier1.is_some(), "Should have Tier 1 match");

    let tier2 = linked
        .iter()
        .find(|(c, t, _)| c.hash == "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb" && *t == 2);
    assert!(tier2.is_some(), "Should have Tier 2 match");

    let no_match = linked
        .iter()
        .find(|(c, _, _)| c.hash == "cccccccccccccccccccccccccccccccccccccccc");
    assert!(no_match.is_none(), "Wrong repo should not match");

    let projects = db.list_projects().await.unwrap();
    let session = &projects[0].sessions[0];
    assert_eq!(session.commit_count, 2);
}

#[tokio::test]
async fn test_correlate_session_no_skills() {
    let db = Database::new_in_memory().await.unwrap();

    db.insert_session_from_index(
        "sess-1",
        "project-1",
        "Project 1",
        "/repo/path",
        "/path/to/sess-1.jsonl",
        "Test session",
        None,
        10,
        1706400300,
        None,
        false,
        5000,
    )
    .await
    .unwrap();

    let commits = vec![GitCommit {
        hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        repo_path: "/repo/path".to_string(),
        message: "Within session".to_string(),
        author: None,
        timestamp: 1706400200,
        branch: None,
        files_changed: None,
        insertions: None,
        deletions: None,
    }];
    db.batch_upsert_commits(&commits).await.unwrap();

    let session_info = SessionCorrelationInfo {
        session_id: "sess-1".to_string(),
        project_path: "/repo/path".to_string(),
        first_timestamp: Some(1706400100),
        last_timestamp: Some(1706400300),
        commit_skills: vec![],
    };

    let inserted = correlate_session(&db, &session_info, &commits)
        .await
        .unwrap();

    assert_eq!(inserted, 1, "Should insert Tier 2 match only");

    let linked = db.get_commits_for_session("sess-1").await.unwrap();
    assert_eq!(linked.len(), 1);
    assert_eq!(linked[0].1, 2, "Should be Tier 2 since no skills");
}

#[tokio::test]
async fn test_correlate_session_no_timestamps() {
    let db = Database::new_in_memory().await.unwrap();

    db.insert_session_from_index(
        "sess-1",
        "project-1",
        "Project 1",
        "/repo/path",
        "/path/to/sess-1.jsonl",
        "Test session",
        None,
        10,
        1706400300,
        None,
        false,
        5000,
    )
    .await
    .unwrap();

    let commits = vec![GitCommit {
        hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        repo_path: "/repo/path".to_string(),
        message: "Test commit".to_string(),
        author: None,
        timestamp: 1706400150,
        branch: None,
        files_changed: None,
        insertions: None,
        deletions: None,
    }];
    db.batch_upsert_commits(&commits).await.unwrap();

    let session_info = SessionCorrelationInfo {
        session_id: "sess-1".to_string(),
        project_path: "/repo/path".to_string(),
        first_timestamp: None,
        last_timestamp: None,
        commit_skills: vec![CommitSkillInvocation {
            skill_name: "commit".to_string(),
            timestamp_unix: 1706400100,
        }],
    };

    let inserted = correlate_session(&db, &session_info, &commits)
        .await
        .unwrap();

    assert_eq!(inserted, 1);

    let linked = db.get_commits_for_session("sess-1").await.unwrap();
    assert_eq!(linked.len(), 1);
    assert_eq!(linked[0].1, 1, "Should be Tier 1 from commit skill");
}

#[tokio::test]
async fn test_correlate_session_extracts_git_diff_stats() {
    let db = Database::new_in_memory().await.unwrap();

    let tmp = TempDir::new().unwrap();
    let repo_path = tmp.path();

    std::process::Command::new("git")
        .args(["init"])
        .current_dir(repo_path)
        .output()
        .expect("git init");
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(repo_path)
        .output()
        .expect("git config");
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(repo_path)
        .output()
        .expect("git config");

    std::fs::write(repo_path.join("file.txt"), "line1\nline2\nline3\n").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .expect("git add");
    std::process::Command::new("git")
        .args(["commit", "-m", "test commit"])
        .current_dir(repo_path)
        .output()
        .expect("git commit");

    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_path)
        .output()
        .expect("git rev-parse");
    let commit_hash = String::from_utf8_lossy(&output.stdout).trim().to_string();

    let output = std::process::Command::new("git")
        .args(["log", "-1", "--format=%at"])
        .current_dir(repo_path)
        .output()
        .expect("git log timestamp");
    let commit_ts: i64 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap();

    let dir_str = repo_path.to_str().unwrap();
    db.insert_session_from_index(
        "sess-1",
        "project-1",
        "Project 1",
        dir_str,
        "/path/to/sess-1.jsonl",
        "Test session",
        None,
        10,
        commit_ts + 600,
        None,
        false,
        5000,
    )
    .await
    .unwrap();

    let commits = vec![GitCommit {
        hash: commit_hash.clone(),
        repo_path: dir_str.to_string(),
        message: "test commit".to_string(),
        author: Some("Test".to_string()),
        timestamp: commit_ts,
        branch: Some("main".to_string()),
        files_changed: None,
        insertions: None,
        deletions: None,
    }];

    db.batch_upsert_commits(&commits).await.unwrap();

    let session_info = SessionCorrelationInfo {
        session_id: "sess-1".to_string(),
        project_path: dir_str.to_string(),
        first_timestamp: Some(commit_ts - 600),
        last_timestamp: Some(commit_ts + 600),
        commit_skills: Vec::new(),
    };

    let inserted = correlate_session(&db, &session_info, &commits)
        .await
        .unwrap();
    assert_eq!(inserted, 1, "Should insert one match");

    let row: (i64, i64, i64) = sqlx::query_as(
        "SELECT lines_added, lines_removed, loc_source FROM sessions WHERE id = 'sess-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();

    assert_eq!(row.0, 3, "Should have 3 lines added");
    assert_eq!(row.1, 0, "Should have 0 lines removed");
    assert_eq!(row.2, 2, "loc_source should be 2 (git verified)");
}

#[tokio::test]
async fn test_correlate_session_aggregates_multiple_commits() {
    let db = Database::new_in_memory().await.unwrap();

    let tmp = TempDir::new().unwrap();
    let repo_path = tmp.path();

    std::process::Command::new("git")
        .args(["init"])
        .current_dir(repo_path)
        .output()
        .expect("git init");
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(repo_path)
        .output()
        .expect("git config");
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(repo_path)
        .output()
        .expect("git config");

    std::fs::write(repo_path.join("file1.txt"), "1\n2\n3\n4\n5\n").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .expect("git add");
    std::process::Command::new("git")
        .args(["commit", "-m", "first"])
        .current_dir(repo_path)
        .output()
        .expect("git commit");

    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_path)
        .output()
        .expect("git rev-parse");
    let hash1 = String::from_utf8_lossy(&output.stdout).trim().to_string();

    std::fs::write(repo_path.join("file2.txt"), "a\nb\nc\n").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .expect("git add");
    std::process::Command::new("git")
        .args(["commit", "-m", "second"])
        .current_dir(repo_path)
        .output()
        .expect("git commit");

    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_path)
        .output()
        .expect("git rev-parse");
    let hash2 = String::from_utf8_lossy(&output.stdout).trim().to_string();

    let now = chrono::Utc::now().timestamp();
    let dir_str = repo_path.to_str().unwrap();

    db.insert_session_from_index(
        "sess-1",
        "project-1",
        "Project 1",
        dir_str,
        "/path/to/sess-1.jsonl",
        "Test session",
        None,
        10,
        now + 600,
        None,
        false,
        5000,
    )
    .await
    .unwrap();

    let commits = vec![
        GitCommit {
            hash: hash1.clone(),
            repo_path: dir_str.to_string(),
            message: "first".to_string(),
            author: Some("Test".to_string()),
            timestamp: now,
            branch: Some("main".to_string()),
            files_changed: None,
            insertions: None,
            deletions: None,
        },
        GitCommit {
            hash: hash2.clone(),
            repo_path: dir_str.to_string(),
            message: "second".to_string(),
            author: Some("Test".to_string()),
            timestamp: now + 100,
            branch: Some("main".to_string()),
            files_changed: None,
            insertions: None,
            deletions: None,
        },
    ];

    db.batch_upsert_commits(&commits).await.unwrap();

    let session_info = SessionCorrelationInfo {
        session_id: "sess-1".to_string(),
        project_path: dir_str.to_string(),
        first_timestamp: Some(now - 600),
        last_timestamp: Some(now + 600),
        commit_skills: Vec::new(),
    };

    let inserted = correlate_session(&db, &session_info, &commits)
        .await
        .unwrap();
    assert_eq!(inserted, 2, "Should insert two matches");

    let row: (i64, i64, i64) = sqlx::query_as(
        "SELECT lines_added, lines_removed, loc_source FROM sessions WHERE id = 'sess-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();

    assert_eq!(row.0, 8, "Should have 8 lines added (5 + 3)");
    assert_eq!(row.1, 0, "Should have 0 lines removed");
    assert_eq!(row.2, 2, "loc_source should be 2 (git verified)");
}

#[tokio::test]
async fn test_correlate_session_idempotent_loc_stats() {
    let db = Database::new_in_memory().await.unwrap();

    let tmp = TempDir::new().unwrap();
    let repo_path = tmp.path();

    std::process::Command::new("git")
        .args(["init"])
        .current_dir(repo_path)
        .output()
        .expect("git init");
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(repo_path)
        .output()
        .expect("git config");
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(repo_path)
        .output()
        .expect("git config");

    std::fs::write(repo_path.join("file.txt"), "line1\nline2\nline3\n").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .expect("git add");
    std::process::Command::new("git")
        .args(["commit", "-m", "test commit"])
        .current_dir(repo_path)
        .output()
        .expect("git commit");

    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_path)
        .output()
        .expect("git rev-parse");
    let commit_hash = String::from_utf8_lossy(&output.stdout).trim().to_string();

    let now = chrono::Utc::now().timestamp();
    let dir_str = repo_path.to_str().unwrap();

    db.insert_session_from_index(
        "sess-1",
        "project-1",
        "Project 1",
        dir_str,
        "/path/to/sess-1.jsonl",
        "Test session",
        None,
        10,
        now + 600,
        None,
        false,
        5000,
    )
    .await
    .unwrap();

    let commits = vec![GitCommit {
        hash: commit_hash.clone(),
        repo_path: dir_str.to_string(),
        message: "test commit".to_string(),
        author: Some("Test".to_string()),
        timestamp: now,
        branch: Some("main".to_string()),
        files_changed: None,
        insertions: None,
        deletions: None,
    }];

    db.batch_upsert_commits(&commits).await.unwrap();

    let session_info = SessionCorrelationInfo {
        session_id: "sess-1".to_string(),
        project_path: dir_str.to_string(),
        first_timestamp: Some(now - 600),
        last_timestamp: Some(now + 600),
        commit_skills: Vec::new(),
    };

    let inserted1 = correlate_session(&db, &session_info, &commits)
        .await
        .unwrap();
    assert_eq!(inserted1, 1, "Should insert one match on first run");

    let row1: (i64, i64, i64) = sqlx::query_as(
        "SELECT lines_added, lines_removed, loc_source FROM sessions WHERE id = 'sess-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(row1.0, 3, "Should have 3 lines added after first run");
    assert_eq!(row1.2, 2, "loc_source should be 2 after first run");

    let inserted2 = correlate_session(&db, &session_info, &commits)
        .await
        .unwrap();
    assert_eq!(
        inserted2, 0,
        "Should insert zero matches on second run (idempotent)"
    );

    let row2: (i64, i64, i64) = sqlx::query_as(
        "SELECT lines_added, lines_removed, loc_source FROM sessions WHERE id = 'sess-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(row2.0, 3, "lines_added should remain unchanged");
    assert_eq!(row2.2, 2, "loc_source should remain unchanged");
}

#[tokio::test]
async fn test_run_git_sync_empty_db() {
    let db = Database::new_in_memory().await.unwrap();
    let result = run_git_sync(&db, |_| {}).await.unwrap();

    assert_eq!(result.repos_scanned, 0);
    assert_eq!(result.commits_found, 0);
    assert_eq!(result.links_created, 0);
    assert!(result.errors.is_empty());

    let meta = db.get_index_metadata().await.unwrap();
    assert!(meta.last_git_sync_at.is_some());
}

#[tokio::test]
async fn test_run_git_sync_non_git_dirs() {
    let db = Database::new_in_memory().await.unwrap();

    let tmp = TempDir::new_in("/tmp").unwrap();
    let dir = tmp.path().to_str().unwrap();

    sqlx::query("INSERT INTO sessions (id, project_id, project_path, first_message_at, last_message_at, file_path) VALUES ('s1', 'p1', ?1, 1000, 2000, '/tmp/s1.jsonl')")
        .bind(dir).execute(db.pool()).await.unwrap();

    let result = run_git_sync(&db, |_| {}).await.unwrap();

    assert_eq!(result.repos_scanned, 0);
    assert_eq!(result.commits_found, 0);
    assert_eq!(result.links_created, 0);
    assert!(result.errors.is_empty());
}

#[tokio::test]
async fn test_run_git_sync_with_real_repo() {
    let db = Database::new_in_memory().await.unwrap();

    let tmp = TempDir::new().unwrap();
    let repo_path = tmp.path();

    std::process::Command::new("git")
        .args(["init"])
        .current_dir(repo_path)
        .output()
        .expect("git init");
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(repo_path)
        .output()
        .expect("git config email");
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(repo_path)
        .output()
        .expect("git config name");
    std::fs::write(repo_path.join("file.txt"), "hello").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .expect("git add");
    std::process::Command::new("git")
        .args(["commit", "-m", "test commit"])
        .current_dir(repo_path)
        .output()
        .expect("git commit");

    let output = std::process::Command::new("git")
        .args(["log", "-1", "--format=%at"])
        .current_dir(repo_path)
        .output()
        .expect("git log timestamp");
    let commit_ts: i64 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap();

    let dir_str = repo_path.to_str().unwrap();
    sqlx::query("INSERT INTO sessions (id, project_id, project_path, first_message_at, last_message_at, file_path) VALUES ('s1', 'p1', ?1, ?2, ?3, '/tmp/s1.jsonl')")
        .bind(dir_str).bind(commit_ts - 600).bind(commit_ts + 600).execute(db.pool()).await.unwrap();

    let result = run_git_sync(&db, |_| {}).await.unwrap();

    assert_eq!(result.repos_scanned, 1);
    assert_eq!(result.commits_found, 1);
    assert_eq!(result.links_created, 1);
    assert!(result.errors.is_empty());

    let commits = db.get_commits_for_session("s1").await.unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(commits[0].1, 2);

    let count = db.count_commits_for_session("s1").await.unwrap();
    assert_eq!(count, 1);

    let meta = db.get_index_metadata().await.unwrap();
    assert!(meta.last_git_sync_at.is_some());
    assert_eq!(meta.commits_found, 1);
    assert_eq!(meta.links_created, 1);
}

#[tokio::test]
async fn test_run_git_sync_deduplicates_repos() {
    let db = Database::new_in_memory().await.unwrap();

    let tmp = TempDir::new().unwrap();
    let repo_path = tmp.path();

    std::process::Command::new("git")
        .args(["init"])
        .current_dir(repo_path)
        .output()
        .expect("git init");
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(repo_path)
        .output()
        .expect("git config email");
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(repo_path)
        .output()
        .expect("git config name");
    std::fs::write(repo_path.join("file.txt"), "hello").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .expect("git add");
    std::process::Command::new("git")
        .args(["commit", "-m", "test commit"])
        .current_dir(repo_path)
        .output()
        .expect("git commit");

    let dir_str = repo_path.to_str().unwrap();
    let now = chrono::Utc::now().timestamp();

    sqlx::query("INSERT INTO sessions (id, project_id, project_path, first_message_at, last_message_at, file_path) VALUES ('s1', 'p1', ?1, ?2, ?3, '/tmp/s1.jsonl')")
        .bind(dir_str).bind(now - 7200).bind(now + 7200).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO sessions (id, project_id, project_path, first_message_at, last_message_at, file_path) VALUES ('s2', 'p1', ?1, ?2, ?3, '/tmp/s2.jsonl')")
        .bind(dir_str).bind(now - 3600).bind(now + 3600).execute(db.pool()).await.unwrap();

    let result = run_git_sync(&db, |_| {}).await.unwrap();

    assert_eq!(result.repos_scanned, 1);
    assert_eq!(result.links_created, 2);
}

#[tokio::test]
async fn test_run_git_sync_nonexistent_dir() {
    let db = Database::new_in_memory().await.unwrap();

    sqlx::query("INSERT INTO sessions (id, project_id, project_path, first_message_at, last_message_at, file_path) VALUES ('s1', 'p1', '/nonexistent/path/abc123', 1000, 2000, '/tmp/s1.jsonl')")
        .execute(db.pool()).await.unwrap();

    let result = run_git_sync(&db, |_| {}).await.unwrap();

    assert_eq!(result.repos_scanned, 0);
    assert_eq!(result.links_created, 0);
    assert!(result.errors.is_empty());
}

#[tokio::test]
async fn test_phase_f_git_sync_extracts_loc_stats() {
    let db = Database::new_in_memory().await.unwrap();

    let tmp = TempDir::new().unwrap();
    let repo_path = tmp.path();

    std::process::Command::new("git")
        .args(["init"])
        .current_dir(repo_path)
        .output()
        .expect("git init");
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(repo_path)
        .output()
        .expect("git config");
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(repo_path)
        .output()
        .expect("git config");

    std::fs::write(
        repo_path.join("file.txt"),
        "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n",
    )
    .unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .expect("git add");
    std::process::Command::new("git")
        .args(["commit", "-m", "add 10 lines"])
        .current_dir(repo_path)
        .output()
        .expect("git commit");

    let now = chrono::Utc::now().timestamp();
    let dir_str = repo_path.to_str().unwrap();

    sqlx::query("INSERT INTO sessions (id, project_id, project_path, first_message_at, last_message_at, file_path) VALUES ('sess-1', 'p1', ?1, ?2, ?3, '/tmp/s1.jsonl')")
        .bind(dir_str).bind(now - 600).bind(now + 600).execute(db.pool()).await.unwrap();

    let result = run_git_sync(&db, |_| {}).await.unwrap();

    assert_eq!(result.repos_scanned, 1);
    assert_eq!(result.commits_found, 1);
    assert_eq!(result.links_created, 1);

    let row: (i64, i64, i64) = sqlx::query_as(
        "SELECT lines_added, lines_removed, loc_source FROM sessions WHERE id = 'sess-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();

    assert_eq!(row.0, 10, "Should extract 10 lines added from git diff");
    assert_eq!(row.1, 0, "Should extract 0 lines removed from git diff");
    assert_eq!(row.2, 2, "loc_source should be 2 (git verified)");
}

#[tokio::test]
async fn test_run_git_sync_idempotent() {
    let db = Database::new_in_memory().await.unwrap();

    let tmp = TempDir::new().unwrap();
    let repo_path = tmp.path();

    std::process::Command::new("git")
        .args(["init"])
        .current_dir(repo_path)
        .output()
        .expect("git init");
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(repo_path)
        .output()
        .expect("git config email");
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(repo_path)
        .output()
        .expect("git config name");
    std::fs::write(repo_path.join("file.txt"), "hello").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .expect("git add");
    std::process::Command::new("git")
        .args(["commit", "-m", "test commit"])
        .current_dir(repo_path)
        .output()
        .expect("git commit");

    let dir_str = repo_path.to_str().unwrap();
    let now = chrono::Utc::now().timestamp();

    sqlx::query("INSERT INTO sessions (id, project_id, project_path, first_message_at, last_message_at, file_path) VALUES ('s1', 'p1', ?1, ?2, ?3, '/tmp/s1.jsonl')")
        .bind(dir_str).bind(now - 7200).bind(now + 7200).execute(db.pool()).await.unwrap();

    let result1 = run_git_sync(&db, |_| {}).await.unwrap();
    let result2 = run_git_sync(&db, |_| {}).await.unwrap();

    assert_eq!(result1.links_created, 1);
    assert_eq!(result2.links_created, 0);

    let count = db.count_commits_for_session("s1").await.unwrap();
    assert_eq!(count, 1);
}
