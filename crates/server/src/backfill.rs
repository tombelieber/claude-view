// crates/server/src/backfill.rs
//! One-shot backfill tasks that run at server startup.
//!
//! These populate fields that were added after sessions were first indexed.

use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::info;
use claude_view_core::{resolve_git_root, infer_git_root_from_worktree_path};
use claude_view_db::Database;

/// Backfill git_root for sessions that have session_cwd but no git_root.
/// Runs once at server startup. Bounded by available_parallelism.
///
/// Resolution strategy (in order):
/// 1. `git rev-parse --git-common-dir` if the directory still exists
/// 2. Path-based extraction for `.worktrees/<name>` or `.claude/worktrees/<name>` patterns
/// 3. Mark with empty string sentinel (unresolvable)
pub async fn backfill_git_roots(db: Arc<Database>) {
    info!("git_root backfill: starting");
    let parallelism = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let sem = Arc::new(Semaphore::new(parallelism));
    let batch_size = 500_i64;
    let mut resolved = 0usize;
    let mut unresolved = 0usize;

    loop {
        let rows = match db.fetch_sessions_needing_git_root(batch_size).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("backfill_git_roots: query error: {e}");
                break;
            }
        };
        if rows.is_empty() {
            break;
        }

        let mut handles = Vec::with_capacity(rows.len());
        for (id, cwd) in rows {
            let db = Arc::clone(&db);
            let sem = Arc::clone(&sem);
            handles.push(tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                // Strategy 1: live git resolution (directory must exist)
                if let Some(root) = resolve_git_root(&cwd).await {
                    let _ = db.set_git_root(&id, &root).await;
                    return true;
                }
                // Strategy 2: extract parent from worktree path patterns
                if let Some(root) = infer_git_root_from_worktree_path(&cwd) {
                    let _ = db.set_git_root(&id, &root).await;
                    return true;
                }
                // Unresolvable — mark with sentinel to prevent re-processing
                let _ = db.set_git_root(&id, "").await;
                false
            }));
        }

        for h in handles {
            match h.await {
                Ok(true) => resolved += 1,
                Ok(false) => unresolved += 1,
                Err(_) => unresolved += 1,
            }
        }
    }

    if resolved > 0 || unresolved > 0 {
        info!(
            "git_root backfill complete: {resolved} resolved, {unresolved} unresolvable"
        );
    } else {
        info!("git_root backfill: nothing to do");
    }
}

#[cfg(test)]
mod tests {
    use claude_view_core::infer_git_root_from_worktree_path;

    #[test]
    fn test_infer_worktrees() {
        assert_eq!(
            infer_git_root_from_worktree_path("/Users/u/dev/repo/.worktrees/feature-x"),
            Some("/Users/u/dev/repo".to_string())
        );
    }

    #[test]
    fn test_infer_claude_worktrees() {
        assert_eq!(
            infer_git_root_from_worktree_path("/Users/u/dev/repo/.claude/worktrees/mobile-remote"),
            Some("/Users/u/dev/repo".to_string())
        );
    }

    #[test]
    fn test_infer_subdir_in_worktree() {
        assert_eq!(
            infer_git_root_from_worktree_path("/Users/u/dev/repo/.worktrees/feat/crates/server"),
            Some("/Users/u/dev/repo".to_string())
        );
    }

    #[test]
    fn test_no_match() {
        assert_eq!(infer_git_root_from_worktree_path("/Users/u/dev/repo"), None);
        assert_eq!(infer_git_root_from_worktree_path("/Users/u/dev/repo-cold-start"), None);
    }
}
