use super::tests::seed_git_session;
use super::*;
use tempfile::TempDir;

// Real repository fixture: idle sync must avoid both rescanning and correlation writes.
fn fixture_git(path: &std::path::Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(path)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_COMMON_DIR")
        .env_remove("GIT_INDEX_FILE")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_AUTHOR_NAME", "Fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
        .env("GIT_COMMITTER_NAME", "Fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[tokio::test]
async fn incremental_sync_skips_idle_but_detects_commit_and_session_changes() {
    let db = Database::new_in_memory().await.unwrap();
    let mut cache = GitSyncCache::default();
    let tmp = TempDir::new_in("/tmp").unwrap();
    fixture_git(tmp.path(), &["init", "-b", "main"]);
    fixture_git(tmp.path(), &["commit", "--allow-empty", "-m", "first"]);
    let path = tmp.path().to_str().unwrap();
    let now = chrono::Utc::now().timestamp();
    seed_git_session(
        &db,
        "idle-1",
        path,
        Some(now - 60),
        Some(now + 60),
        "/tmp/idle-1.jsonl",
    )
    .await;
    let first = run_git_sync_incremental(&db, &mut cache, |_| {})
        .await
        .unwrap();
    assert_eq!(first.repos_scanned, 1);
    let idle = run_git_sync_incremental(&db, &mut cache, |_| {})
        .await
        .unwrap();
    assert_eq!(
        idle.repos_scanned, 0,
        "unchanged repo/session inputs must skip git log"
    );
    assert_eq!(idle.commits_found, 0);
    fixture_git(tmp.path(), &["commit", "--allow-empty", "-m", "second"]);
    let changed = run_git_sync_incremental(&db, &mut cache, |_| {})
        .await
        .unwrap();
    assert_eq!(changed.repos_scanned, 1);
    assert_eq!(changed.links_created, 1);
    seed_git_session(
        &db,
        "idle-2",
        path,
        Some(now - 60),
        Some(now + 60),
        "/tmp/idle-2.jsonl",
    )
    .await;
    let session = run_git_sync_incremental(&db, &mut cache, |_| {})
        .await
        .unwrap();
    assert_eq!(
        session.links_created, 2,
        "new session must correlate even with unchanged HEAD"
    );
    assert_eq!(db.count_commits_for_session("idle-2").await.unwrap(), 2);
}

#[tokio::test]
async fn incremental_sync_deduplicates_subdirectories_and_symlinks() {
    let db = Database::new_in_memory().await.unwrap();
    let mut cache = GitSyncCache::default();
    let tmp = TempDir::new_in("/tmp").unwrap();
    let repo = tmp.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    fixture_git(&repo, &["init", "-b", "main"]);
    fixture_git(&repo, &["commit", "--allow-empty", "-m", "first"]);
    let subdir = repo.join("nested");
    std::fs::create_dir(&subdir).unwrap();
    let now = chrono::Utc::now().timestamp();
    let mut paths = vec![repo.clone(), subdir];
    #[cfg(unix)]
    {
        let alias = tmp.path().join("alias");
        std::os::unix::fs::symlink(&repo, &alias).unwrap();
        paths.push(alias);
    }
    let expected_links = paths.len() as u32;
    for (i, path) in paths.iter().enumerate() {
        seed_git_session(
            &db,
            &format!("alias-{i}"),
            path.to_str().unwrap(),
            Some(now - 60),
            Some(now + 60),
            &format!("/tmp/alias-{i}.jsonl"),
        )
        .await;
    }
    let result = run_git_sync_incremental(&db, &mut cache, |_| {})
        .await
        .unwrap();
    assert_eq!(result.repos_scanned, 1, "same gitdir must scan only once");
    assert_eq!(
        result.links_created, expected_links,
        "preserve correlation for each original project path"
    );
}

#[tokio::test]
async fn incremental_sync_tracks_branch_reset_packing_and_worktree_heads() {
    let db = Database::new_in_memory().await.unwrap();
    let mut cache = GitSyncCache::default();
    let tmp = TempDir::new_in("/tmp").unwrap();
    let repo = tmp.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    fixture_git(&repo, &["init", "-b", "main"]);
    fixture_git(&repo, &["commit", "--allow-empty", "-m", "first"]);
    let first = fixture_git(&repo, &["rev-parse", "HEAD"]);
    let now = chrono::Utc::now().timestamp();
    seed_git_session(
        &db,
        "main",
        repo.to_str().unwrap(),
        Some(now - 60),
        Some(now + 60),
        "/tmp/main.jsonl",
    )
    .await;
    run_git_sync_incremental(&db, &mut cache, |_| {})
        .await
        .unwrap();
    // Same OID, different branch still changes commit metadata.
    fixture_git(&repo, &["switch", "-c", "other"]);
    assert_eq!(
        run_git_sync_incremental(&db, &mut cache, |_| {})
            .await
            .unwrap()
            .repos_scanned,
        1
    );
    let branch: (String,) = sqlx::query_as("SELECT branch FROM commits WHERE hash = ?")
        .bind(&first)
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(branch.0, "other");
    fixture_git(&repo, &["commit", "--allow-empty", "-m", "second"]);
    assert_eq!(
        run_git_sync_incremental(&db, &mut cache, |_| {})
            .await
            .unwrap()
            .commits_found,
        2
    );
    fixture_git(&repo, &["reset", "--hard", &first]);
    assert_eq!(
        run_git_sync_incremental(&db, &mut cache, |_| {})
            .await
            .unwrap()
            .commits_found,
        1
    );
    fixture_git(&repo, &["pack-refs", "--all", "--prune"]);
    assert_eq!(
        run_git_sync_incremental(&db, &mut cache, |_| {})
            .await
            .unwrap()
            .repos_scanned,
        1
    );
    assert_eq!(
        run_git_sync_incremental(&db, &mut cache, |_| {})
            .await
            .unwrap()
            .repos_scanned,
        0
    );
    fixture_git(&repo, &["switch", "--detach"]);
    assert_eq!(
        run_git_sync_incremental(&db, &mut cache, |_| {})
            .await
            .unwrap()
            .repos_scanned,
        1
    );
    let worktree = tmp.path().join("worktree");
    fixture_git(
        &repo,
        &[
            "worktree",
            "add",
            "-b",
            "linked",
            worktree.to_str().unwrap(),
        ],
    );
    seed_git_session(
        &db,
        "linked",
        worktree.to_str().unwrap(),
        Some(now - 60),
        Some(now + 60),
        "/tmp/linked.jsonl",
    )
    .await;
    assert_eq!(
        run_git_sync_incremental(&db, &mut cache, |_| {})
            .await
            .unwrap()
            .repos_scanned,
        1,
        "only new worktree scans"
    );
    fixture_git(
        &worktree,
        &["commit", "--allow-empty", "-m", "linked commit"],
    );
    assert_eq!(
        run_git_sync_incremental(&db, &mut cache, |_| {})
            .await
            .unwrap()
            .repos_scanned,
        1,
        "linked HEAD is independent"
    );
    assert_eq!(db.count_commits_for_session("linked").await.unwrap(), 2);
}

#[tokio::test]
async fn incremental_sync_retries_errors_and_evicts_deleted_inputs() {
    let db = Database::new_in_memory().await.unwrap();
    let mut cache = GitSyncCache::default();
    let tmp = TempDir::new_in("/tmp").unwrap();
    let repo = tmp.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    fixture_git(&repo, &["init", "-b", "main"]);
    fixture_git(&repo, &["commit", "--allow-empty", "-m", "first"]);
    let now = chrono::Utc::now().timestamp();
    seed_git_session(
        &db,
        "retry",
        repo.to_str().unwrap(),
        Some(now - 60),
        Some(now + 60),
        "/tmp/retry.jsonl",
    )
    .await;
    sqlx::query("CREATE TRIGGER fail_correlation BEFORE UPDATE OF commit_count ON session_stats BEGIN SELECT RAISE(ABORT, 'fixture failure'); END").execute(db.pool()).await.unwrap();
    assert!(!run_git_sync_incremental(&db, &mut cache, |_| {})
        .await
        .unwrap()
        .errors
        .is_empty());
    assert!(
        cache.entries.is_empty(),
        "failed correlation must not publish a cache success"
    );
    sqlx::query("DROP TRIGGER fail_correlation")
        .execute(db.pool())
        .await
        .unwrap();
    assert_eq!(
        run_git_sync_incremental(&db, &mut cache, |_| {})
            .await
            .unwrap()
            .repos_scanned,
        1
    );
    // A reindex revision and a modified time window invalidate independently of HEAD.
    sqlx::query("UPDATE session_stats SET indexed_at = indexed_at + 1 WHERE session_id = 'retry'")
        .execute(db.pool())
        .await
        .unwrap();
    assert_eq!(
        run_git_sync_incremental(&db, &mut cache, |_| {})
            .await
            .unwrap()
            .repos_scanned,
        1
    );
    sqlx::query(
        "UPDATE session_stats SET last_message_at = last_message_at + 1 WHERE session_id = 'retry'",
    )
    .execute(db.pool())
    .await
    .unwrap();
    assert_eq!(
        run_git_sync_incremental(&db, &mut cache, |_| {})
            .await
            .unwrap()
            .repos_scanned,
        1
    );
    // Prove idle does not even UPDATE derived session fields.
    sqlx::query("CREATE TRIGGER fail_idle_write BEFORE UPDATE OF commit_count ON session_stats BEGIN SELECT RAISE(ABORT, 'idle write'); END").execute(db.pool()).await.unwrap();
    let idle = run_git_sync_incremental(&db, &mut cache, |_| {})
        .await
        .unwrap();
    assert_eq!(idle.repos_scanned, 0);
    assert!(idle.errors.is_empty());
    sqlx::query("DROP TRIGGER fail_idle_write")
        .execute(db.pool())
        .await
        .unwrap();
    let saved = tmp.path().join("saved");
    std::fs::rename(&repo, &saved).unwrap();
    assert_eq!(
        run_git_sync_incremental(&db, &mut cache, |_| {})
            .await
            .unwrap()
            .repos_scanned,
        0
    );
    assert!(cache.entries.is_empty());
    std::fs::rename(&saved, &repo).unwrap();
    assert_eq!(
        run_git_sync_incremental(&db, &mut cache, |_| {})
            .await
            .unwrap()
            .repos_scanned,
        1
    );
    sqlx::query("DELETE FROM session_stats WHERE session_id = 'retry'")
        .execute(db.pool())
        .await
        .unwrap();
    run_git_sync_incremental(&db, &mut cache, |_| {})
        .await
        .unwrap();
    assert!(cache.entries.is_empty());
}

#[tokio::test]
async fn incremental_sync_keeps_manual_refresh_and_safety_refresh() {
    let db = Database::new_in_memory().await.unwrap();
    let mut cache = GitSyncCache::default();
    let tmp = TempDir::new_in("/tmp").unwrap();
    fixture_git(tmp.path(), &["init", "-b", "main"]);
    fixture_git(tmp.path(), &["commit", "--allow-empty", "-m", "first"]);
    let now = chrono::Utc::now().timestamp();
    seed_git_session(
        &db,
        "refresh",
        tmp.path().to_str().unwrap(),
        Some(now - 60),
        Some(now + 60),
        "/tmp/refresh.jsonl",
    )
    .await;
    run_git_sync_incremental(&db, &mut cache, |_| {})
        .await
        .unwrap();
    assert_eq!(run_git_sync(&db, |_| {}).await.unwrap().repos_scanned, 1);
    for entry in cache.entries.values_mut() {
        entry.checked_at -= std::time::Duration::from_secs(601);
    }
    assert_eq!(
        run_git_sync_incremental(&db, &mut cache, |_| {})
            .await
            .unwrap()
            .repos_scanned,
        1
    );
}

/// `--nocapture` boundaries also permit a local posix_spawn interposer to count
/// actual child processes independently of the pipeline's result counters.
#[tokio::test]
async fn git_sync_workload_idle_and_changed_repo() {
    let db = Database::new_in_memory().await.unwrap();
    let mut cache = GitSyncCache::default();
    let tmp = TempDir::new_in("/tmp").unwrap();
    for i in 0..2 {
        let repo = tmp.path().join(format!("repo-{i}"));
        std::fs::create_dir(&repo).unwrap();
        fixture_git(&repo, &["init", "-b", "main"]);
        fixture_git(&repo, &["commit", "--allow-empty", "-m", "first"]);
        // No matching commits: isolates scan cost from diff extraction.
        seed_git_session(
            &db,
            &format!("workload-{i}"),
            repo.to_str().unwrap(),
            Some(1),
            Some(2),
            &format!("/tmp/workload-{i}.jsonl"),
        )
        .await;
    }
    run_git_sync_incremental(&db, &mut cache, |_| {})
        .await
        .unwrap();
    eprintln!("git-workload begin full-idle");
    let full = run_git_sync(&db, |_| {}).await.unwrap();
    eprintln!(
        "git-workload end full-idle repos={} commits={}",
        full.repos_scanned, full.commits_found
    );
    assert_eq!(full.repos_scanned, 2);
    eprintln!("git-workload begin incremental-idle");
    let idle = run_git_sync_incremental(&db, &mut cache, |_| {})
        .await
        .unwrap();
    eprintln!(
        "git-workload end incremental-idle repos={} commits={}",
        idle.repos_scanned, idle.commits_found
    );
    assert_eq!(idle.repos_scanned, 0);
    fixture_git(
        &tmp.path().join("repo-0"),
        &["commit", "--allow-empty", "-m", "second"],
    );
    eprintln!("git-workload begin full-changed");
    let full = run_git_sync(&db, |_| {}).await.unwrap();
    eprintln!(
        "git-workload end full-changed repos={} commits={}",
        full.repos_scanned, full.commits_found
    );
    assert_eq!(full.repos_scanned, 2);
    eprintln!("git-workload begin incremental-changed");
    let changed = run_git_sync_incremental(&db, &mut cache, |_| {})
        .await
        .unwrap();
    eprintln!(
        "git-workload end incremental-changed repos={} commits={}",
        changed.repos_scanned, changed.commits_found
    );
    assert_eq!(changed.repos_scanned, 1);
    assert_eq!(changed.commits_found, 2);
}

#[tokio::test]
async fn incremental_sync_handles_bare_repos_and_unknown_metadata_conservatively() {
    let db = Database::new_in_memory().await.unwrap();
    let mut cache = GitSyncCache::default();
    let tmp = TempDir::new_in("/tmp").unwrap();
    fixture_git(tmp.path(), &["init", "-b", "main"]);
    fixture_git(tmp.path(), &["commit", "--allow-empty", "-m", "first"]);
    let bare = tmp.path().join("bare.git");
    fixture_git(
        tmp.path(),
        &["clone", "--bare", ".", bare.to_str().unwrap()],
    );
    seed_git_session(
        &db,
        "bare",
        bare.to_str().unwrap(),
        Some(1),
        Some(2),
        "/tmp/bare.jsonl",
    )
    .await;
    assert_eq!(
        run_git_sync_incremental(&db, &mut cache, |_| {})
            .await
            .unwrap()
            .repos_scanned,
        1
    );
    assert_eq!(
        run_git_sync_incremental(&db, &mut cache, |_| {})
            .await
            .unwrap()
            .repos_scanned,
        0
    );
    // Unreadable HEAD cannot produce a cache hit or suppress retry.
    let head = std::fs::read(bare.join("HEAD")).unwrap();
    std::fs::write(bare.join("HEAD"), [0xff]).unwrap();
    run_git_sync_incremental(&db, &mut cache, |_| {})
        .await
        .unwrap();
    assert!(cache.entries.is_empty());
    std::fs::write(bare.join("HEAD"), head).unwrap();
    assert_eq!(
        run_git_sync_incremental(&db, &mut cache, |_| {})
            .await
            .unwrap()
            .repos_scanned,
        1
    );
    // A nonstandard backend marker disables caching, while git still owns scanning.
    std::fs::create_dir(bare.join("reftable")).unwrap();
    assert_eq!(
        run_git_sync_incremental(&db, &mut cache, |_| {})
            .await
            .unwrap()
            .repos_scanned,
        1
    );
    assert_eq!(
        run_git_sync_incremental(&db, &mut cache, |_| {})
            .await
            .unwrap()
            .repos_scanned,
        1
    );
}

#[tokio::test]
async fn incremental_sync_isolates_failed_repo_from_healthy_cache() {
    let db = Database::new_in_memory().await.unwrap();
    let mut cache = GitSyncCache::default();
    let tmp = TempDir::new_in("/tmp").unwrap();
    for id in ["healthy", "failing"] {
        let repo = tmp.path().join(id);
        std::fs::create_dir(&repo).unwrap();
        fixture_git(&repo, &["init", "-b", "main"]);
        fixture_git(&repo, &["commit", "--allow-empty", "-m", "first"]);
        seed_git_session(
            &db,
            id,
            repo.to_str().unwrap(),
            Some(1),
            Some(2),
            &format!("/tmp/{id}.jsonl"),
        )
        .await;
    }
    sqlx::query("CREATE TRIGGER fail_one_repo BEFORE UPDATE OF commit_count ON session_stats WHEN OLD.session_id = 'failing' BEGIN SELECT RAISE(ABORT, 'fixture failure'); END").execute(db.pool()).await.unwrap();
    let first = run_git_sync_incremental(&db, &mut cache, |_| {})
        .await
        .unwrap();
    assert_eq!(first.errors.len(), 1);
    let retry = run_git_sync_incremental(&db, &mut cache, |_| {})
        .await
        .unwrap();
    assert_eq!(
        retry.repos_scanned, 1,
        "healthy repo must stay cached while failed repo retries"
    );
    assert_eq!(retry.errors.len(), 1);
    sqlx::query("DROP TRIGGER fail_one_repo")
        .execute(db.pool())
        .await
        .unwrap();
    assert_eq!(
        run_git_sync_incremental(&db, &mut cache, |_| {})
            .await
            .unwrap()
            .repos_scanned,
        1
    );
    assert_eq!(
        run_git_sync_incremental(&db, &mut cache, |_| {})
            .await
            .unwrap()
            .repos_scanned,
        0
    );
}
