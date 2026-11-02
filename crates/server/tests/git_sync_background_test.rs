//! Production background-sync helper + real HTTP surface, isolated from the
//! installed server, user sessions, telemetry, hooks and the default port.
use claude_view_db::git_correlation::GitSyncCache;
use claude_view_db::Database;
use claude_view_server::startup::background::run_git_sync_logged;

fn git(path: &std::path::Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(path)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_COMMON_DIR")
        .env_remove("GIT_INDEX_FILE")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
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
}

#[tokio::test]
async fn periodic_sync_cache_and_manual_refresh_reach_http_status() {
    let tmp = tempfile::tempdir_in("/tmp").unwrap();
    let db = Database::new(&tmp.path().join("fixture.db")).await.unwrap();
    git(tmp.path(), &["init", "-b", "main"]);
    git(tmp.path(), &["commit", "--allow-empty", "-m", "first"]);
    let now = chrono::Utc::now().timestamp();
    sqlx::query("INSERT INTO session_stats (session_id, source_content_hash, source_size, parser_version, stats_version, indexed_at, project_id, project_path, first_message_at, last_message_at, file_path) VALUES ('fixture', X'00', 0, 1, 4, 0, 'fixture', ?, ?, ?, '/tmp/fixture.jsonl')")
        .bind(tmp.path().to_str().unwrap()).bind(now-60).bind(now+60).execute(db.pool()).await.unwrap();
    let app = claude_view_server::create_app_with_telemetry_path(
        db.clone(),
        tmp.path().join("telemetry.json"),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await
            .unwrap();
    });
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap();
    let url = format!("http://{addr}/api/status");
    let mut cache = GitSyncCache::default();
    run_git_sync_logged(&db, &mut cache, "initial").await;
    let initial: serde_json::Value = client
        .get(&url)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(initial["commitsFound"], 1);
    assert_eq!(db.count_commits_for_session("fixture").await.unwrap(), 1);
    run_git_sync_logged(&db, &mut cache, "periodic").await;
    let idle: serde_json::Value = client.get(&url).send().await.unwrap().json().await.unwrap();
    assert_eq!(idle["commitsFound"], 0, "idle metadata reports zero work");
    git(tmp.path(), &["commit", "--allow-empty", "-m", "second"]);
    run_git_sync_logged(&db, &mut cache, "periodic").await;
    let changed: serde_json::Value = client.get(&url).send().await.unwrap().json().await.unwrap();
    assert_eq!(changed["commitsFound"], 2);
    assert_eq!(db.count_commits_for_session("fixture").await.unwrap(), 2);
    // Manual sync still refreshes even when the periodic cache is warm.
    let response = client
        .post(format!("http://{addr}/api/sync/git"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::ACCEPTED);
    let events = client
        .get(format!("http://{addr}/api/sync/git/progress"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(events.contains("event: done"), "{events}");
    assert!(events.contains("\"reposScanned\":1"), "{events}");
    shutdown_tx.send(()).unwrap();
    server.await.unwrap();
    assert!(
        tokio::net::TcpStream::connect(addr).await.is_err(),
        "fixture listener must close"
    );
}
