// crates/db/src/git_correlation/sync.rs
//! Full correlation pipeline and git sync orchestrator.

use super::diff_stats::extract_commit_diff_stats;
use super::incremental::{discover, fingerprint, CachedInputs, GitSyncCache};
use super::matching::{tier1_match, tier2_match};
use super::scanning::scan_repo_commits;
use super::types::{DiffStats, GitCommit, GitSyncProgress, GitSyncResult, SessionCorrelationInfo};
use crate::{Database, DbResult};

/// Run the full correlation pipeline for a session.
///
/// 1. Scan the repo for commits (if not already scanned)
/// 2. Apply Tier 1 matching (commit skills)
/// 3. Apply Tier 2 matching (session time range)
/// 4. Insert matches, preferring Tier 1 over Tier 2
/// 5. Extract git diff stats for newly linked commits (Phase F)
///
/// Returns the number of matches inserted.
pub async fn correlate_session(
    db: &Database,
    session: &SessionCorrelationInfo,
    commits: &[GitCommit],
) -> DbResult<usize> {
    let mut all_matches = Vec::new();
    let mut tier1_hashes = std::collections::HashSet::new();

    // Tier 1: Commit skill matching
    if !session.commit_skills.is_empty() {
        let tier1_matches = tier1_match(
            &session.session_id,
            &session.project_path,
            &session.commit_skills,
            commits,
        );

        for m in &tier1_matches {
            tier1_hashes.insert(m.commit_hash.clone());
        }
        all_matches.extend(tier1_matches);
    }

    // Tier 2: Session time range matching
    if let (Some(start), Some(end)) = (session.first_timestamp, session.last_timestamp) {
        let tier2_matches = tier2_match(
            &session.session_id,
            &session.project_path,
            start,
            end,
            commits,
        );

        // Exclude commits already matched by Tier 1
        let filtered_tier2: Vec<_> = tier2_matches
            .into_iter()
            .filter(|m| !tier1_hashes.contains(&m.commit_hash))
            .collect();

        all_matches.extend(filtered_tier2);
    }

    // Insert all matches
    let inserted = db.batch_insert_session_commits(&all_matches).await? as usize;

    // Phase F: Extract git diff stats for newly linked commits
    if inserted > 0 {
        let repo_path = std::path::Path::new(&session.project_path);
        let mut all_stats = Vec::new();

        for commit_match in &all_matches {
            if let Some(stats) =
                extract_commit_diff_stats(repo_path, &commit_match.commit_hash).await
            {
                all_stats.push(stats);
            }
        }

        if !all_stats.is_empty() {
            let aggregated = DiffStats::aggregate(&all_stats);
            db.update_session_loc_from_git(&session.session_id, &aggregated)
                .await?;
        }
    }

    // Update session commit count
    let commit_count = db.count_commits_for_session(&session.session_id).await?;
    db.update_session_commit_count(&session.session_id, commit_count as i32)
        .await?;

    Ok(inserted)
}

/// Run the full git sync pipeline: scan repos, correlate sessions, update metadata.
///
/// Auto-sync produces Tier 2 only (no skill data available at this stage).
/// Tier 1 links from pass_2 deep indexing are never overwritten.
/// Per-repo error isolation ensures one bad repo doesn't abort the sync.
/// Idempotent: safe to run multiple times.
pub async fn run_git_sync<F>(db: &Database, on_progress: F) -> DbResult<GitSyncResult>
where
    F: Fn(GitSyncProgress) + Send + 'static,
{
    run_git_sync_incremental(db, &mut GitSyncCache::default(), on_progress).await
}

/// Periodic variant. The caller owns the cache, scoped to one database/indexer.
/// Explicit user syncs keep using `run_git_sync` for an unconditional refresh.
pub async fn run_git_sync_incremental<F>(
    db: &Database,
    cache: &mut GitSyncCache,
    on_progress: F,
) -> DbResult<GitSyncResult>
where
    F: Fn(GitSyncProgress) + Send + 'static,
{
    let mut result = GitSyncResult::default();

    // Step 1: Fetch all eligible sessions
    let sessions = db.get_sessions_for_git_sync().await?;
    if sessions.is_empty() {
        cache.entries.clear();
        tracing::debug!("Git sync: no eligible sessions found");
        db.update_git_sync_metadata_on_success(0, 0).await?;
        return Ok(result);
    }

    tracing::info!("Git sync: {} eligible sessions", sessions.len());

    // Discover metadata without spawning git for every non-repo/project alias.
    let mut groups = std::collections::HashMap::new();
    let mut locations = std::collections::HashMap::new();
    for session in &sessions {
        let location = locations
            .entry(session.project_path.clone())
            .or_insert_with(|| discover(std::path::Path::new(&session.project_path)));
        if let Some(repo) = location {
            groups
                .entry(repo.git_dir.clone())
                .or_insert_with(Vec::new)
                .push(session.clone());
        }
    }
    cache.entries.retain(|key, _| groups.contains_key(key));
    let total_repos = groups.len();
    on_progress(GitSyncProgress::ScanningStarted { total_repos });
    let mut commits_by_repo = std::collections::HashMap::new();
    let mut successful_inputs = Vec::new();

    for (git_dir, inputs) in groups {
        let repo = locations[&inputs[0].project_path]
            .as_ref()
            .expect("discovered repo");
        let before = fingerprint(repo);
        if let (Some(state), Some(previous)) = (&before, cache.entries.get(&git_dir)) {
            if previous.unchanged(state, &inputs) {
                continue;
            }
        }
        // Remove stale success before any fallible work: errors must retry.
        cache.entries.remove(&git_dir);
        let scan = scan_repo_commits(&repo.root, None, None).await;
        if let Some(err) = &scan.error {
            if !scan.not_a_repo {
                result
                    .errors
                    .push(format!("{}: {}", repo.root.display(), err));
            }
            continue;
        }
        if scan.not_a_repo {
            continue;
        }
        result.repos_scanned += 1;
        result.commits_found += scan.commits.len() as u32;
        on_progress(GitSyncProgress::RepoScanned {
            repos_done: result.repos_scanned as usize,
            total_repos,
            commits_in_repo: scan.commits.len() as u32,
        });
        // Preserve exact-path matching for sessions in subdirectories/aliases.
        for session in &inputs {
            if commits_by_repo.contains_key(&session.project_path) {
                continue;
            }
            let mut commits = scan.commits.clone();
            for commit in &mut commits {
                commit.repo_path.clone_from(&session.project_path);
            }
            db.batch_upsert_commits(&commits).await?;
            commits_by_repo.insert(session.project_path.clone(), commits);
        }
        // Only publish cache state after all DB work succeeds, and only if the
        // repo stayed stable throughout the scan (a concurrent commit retries).
        if let Some(state) = before {
            if fingerprint(repo).as_ref() == Some(&state) {
                successful_inputs.push((git_dir, state, inputs));
            }
        }
    }

    tracing::info!(
        "Git sync: scanned {} repos, found {} commits",
        result.repos_scanned,
        result.commits_found
    );

    // Step 4: Correlate each session with its repo's commits
    // Count sessions that actually have commits to correlate against
    let correlatable_count = sessions
        .iter()
        .filter(|s| commits_by_repo.contains_key(&s.project_path))
        .count();
    on_progress(GitSyncProgress::CorrelatingStarted {
        total_correlatable_sessions: correlatable_count,
    });

    let mut sessions_done: usize = 0;
    let mut failed_projects = std::collections::HashSet::new();

    for session in &sessions {
        let commits = match commits_by_repo.get(&session.project_path) {
            Some(c) => c,
            None => continue,
        };

        let info = SessionCorrelationInfo {
            session_id: session.session_id.clone(),
            project_path: session.project_path.clone(),
            first_timestamp: session.first_message_at,
            last_timestamp: session.last_message_at,
            commit_skills: Vec::new(), // No skill data in auto-sync -> Tier 2 only
        };

        match correlate_session(db, &info, commits).await {
            Ok(links) => {
                let links_in_session = links as u32;
                result.links_created += links_in_session;
                sessions_done += 1;
                on_progress(GitSyncProgress::SessionCorrelated {
                    sessions_done,
                    total_correlatable_sessions: correlatable_count,
                    links_in_session,
                });
            }
            Err(e) => {
                failed_projects.insert(session.project_path.as_str());
                tracing::warn!(
                    "Git sync: correlation failed for session {}: {}",
                    session.session_id,
                    e
                );
                result
                    .errors
                    .push(format!("session {}: {}", session.session_id, e));
                sessions_done += 1;
                on_progress(GitSyncProgress::SessionCorrelated {
                    sessions_done,
                    total_correlatable_sessions: correlatable_count,
                    links_in_session: 0,
                });
            }
        }
    }

    // Step 5: Update metadata to record successful sync
    db.update_git_sync_metadata_on_success(
        result.commits_found as i64,
        result.links_created as i64,
    )
    .await?;

    for (git_dir, state, sessions) in successful_inputs {
        if !sessions
            .iter()
            .any(|session| failed_projects.contains(session.project_path.as_str()))
        {
            cache.entries.insert(
                git_dir,
                CachedInputs {
                    state,
                    sessions,
                    checked_at: std::time::Instant::now(),
                },
            );
        }
    }

    tracing::info!(
        "Git sync complete: {} repos, {} commits, {} links, {} errors",
        result.repos_scanned,
        result.commits_found,
        result.links_created,
        result.errors.len()
    );

    Ok(result)
}

#[cfg(test)]
#[path = "sync_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "incremental_tests.rs"]
mod incremental_tests;
