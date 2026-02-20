//! Sync endpoints for triggering git commit scanning and deep index rebuilds.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Instant;

use axum::{
    extract::State,
    http::StatusCode,
    response::sse::{Event, Sse},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Serialize;
use tokio::sync::Mutex;
use ts_rs::TS;

use crate::error::{ApiError, ApiResult};
use crate::git_sync_state::GitSyncPhase;
use crate::metrics::record_sync;
use crate::state::AppState;
use claude_view_db::git_correlation::GitSyncProgress;

/// Global mutex to prevent concurrent git syncs.
/// Uses a lazy static pattern via std::sync::OnceLock.
static GIT_SYNC_MUTEX: std::sync::OnceLock<Mutex<()>> = std::sync::OnceLock::new();

/// Global mutex to prevent concurrent deep index rebuilds.
static DEEP_INDEX_MUTEX: std::sync::OnceLock<Mutex<()>> = std::sync::OnceLock::new();

fn get_sync_mutex() -> &'static Mutex<()> {
    GIT_SYNC_MUTEX.get_or_init(|| Mutex::new(()))
}

fn get_deep_index_mutex() -> &'static Mutex<()> {
    DEEP_INDEX_MUTEX.get_or_init(|| Mutex::new(()))
}

/// Status value for accepted sync responses.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "lowercase")]
pub enum SyncStatus {
    Accepted,
}

/// Response for successful sync initiation.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct SyncAcceptedResponse {
    pub message: String,
    pub status: SyncStatus,
}

/// POST /api/sync/git - Trigger git commit scanning (A8.5).
///
/// Returns:
/// - 202 Accepted: Sync started (no sync was running)
/// - 409 Conflict: Sync already in progress
///
/// The sync runs in the background. Poll /api/status for completion.
pub async fn trigger_git_sync(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Response> {
    let mutex = get_sync_mutex();

    match mutex.try_lock() {
        Ok(guard) => {
            let db = state.db.clone();
            let git_sync = state.git_sync.clone();

            // Reset all counters + error from previous run so SSE clients
            // never see stale data.
            git_sync.reset();
            // Set phase to Scanning BEFORE spawn so SSE clients that connect
            // immediately see active state, not Idle.
            git_sync.set_phase(GitSyncPhase::Scanning);

            // Build progress closure that updates atomics.
            let git_sync_cb = git_sync.clone();
            let on_progress = move |p: GitSyncProgress| {
                match p {
                    GitSyncProgress::ScanningStarted { total_repos } => {
                        git_sync_cb.set_total_repos(total_repos);
                    }
                    GitSyncProgress::RepoScanned {
                        repos_done,
                        commits_in_repo,
                        ..
                    } => {
                        git_sync_cb.set_repos_scanned(repos_done);
                        git_sync_cb.add_commits_found(commits_in_repo as usize);
                    }
                    GitSyncProgress::CorrelatingStarted {
                        total_correlatable_sessions,
                    } => {
                        git_sync_cb.set_phase(GitSyncPhase::Correlating);
                        git_sync_cb.set_total_correlatable_sessions(total_correlatable_sessions);
                    }
                    GitSyncProgress::SessionCorrelated {
                        sessions_done,
                        links_in_session,
                        ..
                    } => {
                        git_sync_cb.set_sessions_correlated(sessions_done);
                        git_sync_cb.add_links_created(links_in_session as usize);
                    }
                }
            };

            tokio::spawn(async move {
                // Hold the mutex guard for the entire duration of the sync.
                let _guard = guard;
                let start = Instant::now();

                tracing::info!("Git sync triggered via API");
                match claude_view_db::git_correlation::run_git_sync(&db, on_progress).await {
                    Ok(result) => {
                        let duration = start.elapsed();
                        tracing::info!(
                            repos_scanned = result.repos_scanned,
                            commits_found = result.commits_found,
                            links_created = result.links_created,
                            errors = result.errors.len(),
                            duration_secs = duration.as_secs_f64(),
                            "Git sync complete"
                        );
                        git_sync.set_phase(GitSyncPhase::Done);
                        // Record sync metrics
                        record_sync("git", duration, Some(result.commits_found as u64));
                    }
                    Err(e) => {
                        let duration = start.elapsed();
                        tracing::error!(
                            error = %e,
                            duration_secs = duration.as_secs_f64(),
                            "Git sync failed"
                        );
                        git_sync.set_error(format!("{e}"));
                        // Still record duration for failed syncs
                        record_sync("git", duration, None);
                    }
                }
            });

            let response = SyncAcceptedResponse {
                message: "Git sync initiated".to_string(),
                status: SyncStatus::Accepted,
            };

            Ok((StatusCode::ACCEPTED, Json(response)).into_response())
        }
        Err(_) => {
            Err(ApiError::Conflict(
                "Git sync already in progress. Please wait for it to complete.".to_string(),
            ))
        }
    }
}

/// GET /api/sync/git/progress - SSE stream of git sync progress.
///
/// # Events
///
/// | Event name     | When emitted                     |
/// |----------------|----------------------------------|
/// | `scanning`     | Repos being scanned              |
/// | `correlating`  | Linking commits to sessions      |
/// | `done`         | Sync complete                    |
/// | `error`        | Sync failed                      |
///
/// The stream terminates after `done` or `error`.
pub async fn git_sync_progress(
    State(state): State<Arc<AppState>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let git_sync = state.git_sync.clone();
    let mut shutdown = state.shutdown.clone();

    let stream = async_stream::stream! {
        let mut last_phase = GitSyncPhase::Idle;
        let mut last_repos_scanned = 0usize;
        let mut last_sessions_correlated = 0usize;
        let started = std::time::Instant::now();
        let max_duration = std::time::Duration::from_secs(300); // 5 minute timeout

        loop {
            let phase = git_sync.phase();
            let repos_scanned = git_sync.repos_scanned();
            let total_repos = git_sync.total_repos();
            let commits_found = git_sync.commits_found();
            let sessions_correlated = git_sync.sessions_correlated();
            let total_correlatable_sessions = git_sync.total_correlatable_sessions();
            let links_created = git_sync.links_created();

            match phase {
                GitSyncPhase::Idle => {
                    // Not started yet, wait
                }
                GitSyncPhase::Scanning => {
                    if last_phase != GitSyncPhase::Scanning
                        || repos_scanned != last_repos_scanned
                    {
                        let data = serde_json::json!({
                            "phase": "scanning",
                            "reposScanned": repos_scanned,
                            "totalRepos": total_repos,
                            "commitsFound": commits_found,
                        });
                        yield Ok(Event::default().event("scanning").data(data.to_string()));
                        last_phase = phase;
                        last_repos_scanned = repos_scanned;
                    }
                }
                GitSyncPhase::Correlating => {
                    if last_phase != GitSyncPhase::Correlating
                        || sessions_correlated != last_sessions_correlated
                    {
                        let data = serde_json::json!({
                            "phase": "correlating",
                            "sessionsCorrelated": sessions_correlated,
                            "totalCorrelatableSessions": total_correlatable_sessions,
                            "commitsFound": commits_found,
                            "linksCreated": links_created,
                        });
                        yield Ok(Event::default().event("correlating").data(data.to_string()));
                        last_phase = phase;
                        last_sessions_correlated = sessions_correlated;
                    }
                }
                GitSyncPhase::Done => {
                    let data = serde_json::json!({
                        "phase": "done",
                        "reposScanned": repos_scanned,
                        "commitsFound": commits_found,
                        "linksCreated": links_created,
                    });
                    yield Ok(Event::default().event("done").data(data.to_string()));
                    break;
                }
                GitSyncPhase::Error => {
                    let error_msg = git_sync.error().unwrap_or_default();
                    let data = serde_json::json!({
                        "phase": "error",
                        "message": error_msg,
                    });
                    yield Ok(Event::default().event("error").data(data.to_string()));
                    break;
                }
            }

            // Safety: timeout after 5 minutes to prevent infinite loops if background task panics
            if started.elapsed() > max_duration {
                let data = serde_json::json!({
                    "phase": "error",
                    "message": "Sync timed out after 5 minutes",
                });
                yield Ok(Event::default().event("error").data(data.to_string()));
                break;
            }

            tokio::select! {
                _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {}
                _ = shutdown.changed() => {
                    if *shutdown.borrow() { break; }
                }
            }
        }
    };

    Sse::new(stream)
}

/// POST /api/sync/deep - Trigger a full deep index rebuild.
///
/// This endpoint:
/// 1. Marks all sessions for re-indexing (clears deep_indexed_at)
/// 2. Runs Pass 2 deep indexing on all sessions
///
/// Returns:
/// - 202 Accepted: Deep index rebuild started
/// - 409 Conflict: A rebuild is already in progress
///
/// The rebuild runs in the background. Poll /api/status or /api/indexing/progress for completion.
pub async fn trigger_deep_index(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Response> {
    let mutex = get_deep_index_mutex();

    match mutex.try_lock() {
        Ok(guard) => {
            let db = state.db.clone();
            let indexing = state.indexing.clone();
            // Read-lock the holder, clone Option<Arc<SearchIndex>>, drop lock.
            // After clear_cache recreates the index, this grabs the fresh one.
            let search_index: Option<Arc<claude_view_search::SearchIndex>> = state
                .search_index
                .read()
                .ok()
                .and_then(|g| g.clone());

            // Reset indexing state BEFORE spawning so SSE clients that
            // connect after receiving the 202 never see stale `Done` from
            // a previous run.
            indexing.set_indexed(0);
            indexing.set_total(0);
            indexing.set_status(crate::indexing_state::IndexingStatus::ReadingIndexes);

            tokio::spawn(async move {
                // Hold the mutex guard for the entire duration of the rebuild.
                let _guard = guard;
                let start = Instant::now();

                tracing::info!("Deep index rebuild triggered via API");

                // Step 1: Mark all sessions for re-indexing
                match db.mark_all_sessions_for_reindex().await {
                    Ok(count) => {
                        tracing::info!(
                            sessions_marked = count,
                            "Marked sessions for re-indexing"
                        );
                    }
                    Err(e) => {
                        tracing::error!(
                            error = %e,
                            "Failed to mark sessions for re-indexing"
                        );
                        indexing.set_error(format!("Failed to mark sessions: {e}"));
                        return;
                    }
                }

                // Transition to deep indexing phase
                indexing.set_status(crate::indexing_state::IndexingStatus::DeepIndexing);

                // Step 2: Run deep indexing pass with progress wired to IndexingState
                let indexing_start = indexing.clone();
                let indexing_cb = indexing.clone();
                let result = claude_view_db::indexer_parallel::pass_2_deep_index(
                    &db,
                    None, // No registry needed for rebuild
                    search_index.as_deref(), // Pass search index for Tantivy indexing
                    move |total_bytes| {
                        indexing_start.set_bytes_total(total_bytes);
                    },
                    move |indexed, total, file_bytes| {
                        indexing_cb.set_total(total);
                        indexing_cb.set_indexed(indexed);
                        indexing_cb.add_bytes_processed(file_bytes);
                    },
                )
                .await;

                match result {
                    Ok((indexed_count, _)) => {
                        let duration = start.elapsed();
                        tracing::info!(
                            sessions_indexed = indexed_count,
                            duration_secs = duration.as_secs_f64(),
                            "Deep index rebuild complete"
                        );
                        indexing.set_status(crate::indexing_state::IndexingStatus::Done);
                        // Persist index metadata so Settings > Data Status shows real values
                        let duration_ms = duration.as_millis() as i64;
                        let project_count = db.get_project_count().await.unwrap_or(0);
                        if let Err(e) = db.update_index_metadata_on_success(duration_ms, indexed_count as i64, project_count).await {
                            tracing::warn!(error = %e, "Failed to persist index metadata after rebuild");
                        }
                        // Record sync metrics
                        record_sync("deep", duration, Some(indexed_count as u64));
                    }
                    Err(e) => {
                        let duration = start.elapsed();
                        tracing::error!(
                            error = %e,
                            duration_secs = duration.as_secs_f64(),
                            "Deep index rebuild failed"
                        );
                        indexing.set_error(format!("Deep index failed: {e}"));
                        // Still record duration for failed rebuilds
                        record_sync("deep", duration, None);
                    }
                }
            });

            let response = SyncAcceptedResponse {
                message: "Deep index rebuild initiated".to_string(),
                status: SyncStatus::Accepted,
            };

            Ok((StatusCode::ACCEPTED, Json(response)).into_response())
        }
        Err(_) => {
            Err(ApiError::Conflict(
                "Deep index rebuild already in progress. Please wait for it to complete.".to_string(),
            ))
        }
    }
}

/// Create the sync routes router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/sync/git", post(trigger_git_sync))
        .route("/sync/git/progress", get(git_sync_progress))
        .route("/sync/deep", post(trigger_deep_index))
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Method, Request, StatusCode},
    };
    use tower::ServiceExt;
    use claude_view_db::Database;

    async fn test_db() -> Database {
        Database::new_in_memory().await.expect("in-memory DB")
    }

    fn build_app(db: Database) -> axum::Router {
        crate::create_app(db)
    }

    async fn do_post(app: axum::Router, uri: &str) -> (StatusCode, String) {
        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    #[tokio::test]
    async fn test_sync_git_accepted() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_post(app, "/api/sync/git").await;

        assert_eq!(status, StatusCode::ACCEPTED);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["status"], "accepted");
        assert!(json["message"].as_str().unwrap().contains("initiated"));
    }

    #[tokio::test]
    async fn test_sync_deep_accepted() {
        let db = test_db().await;
        let app = build_app(db);
        let (status, body) = do_post(app, "/api/sync/deep").await;

        assert_eq!(status, StatusCode::ACCEPTED);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["status"], "accepted");
        assert!(json["message"].as_str().unwrap().contains("Deep index"));
    }

    // Note: Testing the 409 Conflict case requires holding the mutex during the test,
    // which is tricky with the current design. In a real implementation, we would
    // have a more sophisticated sync state management that allows better testing.

    // ========================================================================
    // SSE Git Sync Progress Tests
    // ========================================================================

    use std::sync::Arc;
    use crate::git_sync_state::{GitSyncPhase, GitSyncState};
    use crate::create_app_with_git_sync;

    #[tokio::test]
    async fn test_sse_done_emits_done_event() {
        let db = test_db().await;
        let state = Arc::new(GitSyncState::new());
        state.set_phase(GitSyncPhase::Done);
        state.set_repos_scanned(3);
        state.set_total_repos(3);
        state.add_commits_found(42);
        state.add_links_created(7);

        let app = create_app_with_git_sync(db, state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/sync/git/progress")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = String::from_utf8(body.to_vec()).unwrap();

        assert!(
            body_str.contains("event: done"),
            "Expected 'event: done' in body: {}",
            body_str
        );
        assert!(
            body_str.contains("\"reposScanned\":3"),
            "Expected reposScanned=3 in body: {}",
            body_str
        );
        assert!(
            body_str.contains("\"commitsFound\":42"),
            "Expected commitsFound=42 in body: {}",
            body_str
        );
        assert!(
            body_str.contains("\"linksCreated\":7"),
            "Expected linksCreated=7 in body: {}",
            body_str
        );
    }

    #[tokio::test]
    async fn test_sse_error_emits_error_event() {
        let db = test_db().await;
        let state = Arc::new(GitSyncState::new());
        state.set_error("disk full".to_string());

        let app = create_app_with_git_sync(db, state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/sync/git/progress")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = String::from_utf8(body.to_vec()).unwrap();

        assert!(
            body_str.contains("event: error"),
            "Expected 'event: error' in body: {}",
            body_str
        );
        assert!(
            body_str.contains("disk full"),
            "Expected 'disk full' in body: {}",
            body_str
        );
    }

    #[tokio::test]
    async fn test_sse_content_type() {
        let db = test_db().await;
        let state = Arc::new(GitSyncState::new());
        // Set to Done so the stream terminates quickly
        state.set_phase(GitSyncPhase::Done);

        let app = create_app_with_git_sync(db, state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/sync/git/progress")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let content_type = response
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(
            content_type.contains("text/event-stream"),
            "Expected text/event-stream, got: {}",
            content_type
        );
    }
}
