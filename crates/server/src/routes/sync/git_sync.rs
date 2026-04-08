//! Git sync trigger and SSE progress handlers.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Instant;

use axum::{
    extract::State,
    http::StatusCode,
    response::sse::{Event, Sse},
    response::{IntoResponse, Response},
    Json,
};

use crate::error::{ApiError, ApiResult};
use crate::git_sync_state::GitSyncPhase;
use crate::metrics::record_sync;
use crate::state::AppState;
use claude_view_db::git_correlation::GitSyncProgress;

use super::mutex::get_sync_mutex;
use super::types::{SyncAcceptedResponse, SyncStatus};

/// POST /api/sync/git - Trigger git commit scanning (A8.5).
///
/// Returns:
/// - 202 Accepted: Sync started (no sync was running)
/// - 409 Conflict: Sync already in progress
///
/// The sync runs in the background. Poll /api/status for completion.
#[utoipa::path(post, path = "/api/sync/git", tag = "sync",
    responses(
        (status = 202, description = "Git sync started", body = crate::routes::sync::SyncAcceptedResponse),
        (status = 409, description = "Sync already in progress"),
    )
)]
pub async fn trigger_git_sync(State(state): State<Arc<AppState>>) -> ApiResult<Response> {
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
            let on_progress = move |p: GitSyncProgress| match p {
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
        Err(_) => Err(ApiError::Conflict(
            "Git sync already in progress. Please wait for it to complete.".to_string(),
        )),
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
/// GET /api/sync/git/progress — SSE stream of git sync progress.
#[utoipa::path(get, path = "/api/sync/git/progress", tag = "sync",
    responses(
        (status = 200, description = "SSE stream of git sync progress events", content_type = "text/event-stream"),
    )
)]
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
