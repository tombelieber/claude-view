// crates/db/src/git_correlation/diff_stats.rs
//! Git diff stats extraction and parsing.

use super::types::{DiffStats, GIT_TIMEOUT_SECS};
use std::path::Path;
use std::time::Duration;
use tokio::process::Command;

impl DiffStats {
    /// Parse numstat output and aggregate stats.
    ///
    /// Expected format (one line per file):
    /// ```text
    /// 10\t5\tfile.rs
    /// 3\t0\tREADME.md
    /// ```
    ///
    /// Binary files show as `-\t-\tfile.bin` and are ignored.
    pub fn from_numstat(output: &str) -> Self {
        let mut stats = Self::default();

        for line in output.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() < 2 {
                continue;
            }

            // Parse additions and deletions
            let added = parts[0].parse::<u32>().ok();
            let removed = parts[1].parse::<u32>().ok();

            if let (Some(a), Some(r)) = (added, removed) {
                stats.files_changed += 1;
                stats.insertions += a;
                stats.deletions += r;
            }
            // Binary files (- - filename) are skipped
        }

        stats
    }

    /// Aggregate multiple DiffStats together.
    pub fn aggregate(stats: &[DiffStats]) -> Self {
        stats.iter().fold(Self::default(), |mut acc, s| {
            acc.files_changed += s.files_changed;
            acc.insertions += s.insertions;
            acc.deletions += s.deletions;
            acc
        })
    }
}

/// Get diff stats for a single commit using `git show --stat`.
///
/// # Arguments
/// * `repo_path` - Path to the git repository
/// * `commit_hash` - The commit hash to get stats for
///
/// # Returns
/// `DiffStats` with files_changed, insertions, and deletions, or default if unavailable.
pub async fn get_commit_diff_stats(repo_path: &Path, commit_hash: &str) -> DiffStats {
    // Use git show --stat --format="" to get only the stat line
    // Output format: "N files changed, N insertions(+), N deletions(-)"
    let output = tokio::time::timeout(
        Duration::from_secs(GIT_TIMEOUT_SECS),
        Command::new("git")
            .args(["show", "--stat", "--format=", commit_hash])
            .current_dir(repo_path)
            .output(),
    )
    .await;

    let output = match output {
        Ok(Ok(o)) if o.status.success() => o,
        Ok(Ok(o)) => {
            tracing::debug!(
                "git show returned non-zero for {commit_hash}: {:?}",
                o.status
            );
            return DiffStats::default();
        }
        Ok(Err(e)) => {
            tracing::warn!("git show spawn failed for {commit_hash}: {e}");
            return DiffStats::default();
        }
        Err(_) => {
            tracing::warn!("git show timed out for commit {commit_hash}");
            return DiffStats::default();
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_diff_stats_from_output(&stdout)
}

/// Get diff stats for multiple commits in a batch.
///
/// Spawns one `git show --stat` per commit, parallelized within batches
/// using [`tokio::task::JoinSet`] to limit concurrent subprocesses.
///
/// # Arguments
/// * `repo_path` - Path to the git repository
/// * `commit_hashes` - Slice of commit hashes to get stats for
///
/// # Returns
/// Vec of (hash, DiffStats) tuples. Commits that fail to parse will have default stats.
pub async fn get_batch_diff_stats(
    repo_path: &Path,
    commit_hashes: &[String],
) -> Vec<(String, DiffStats)> {
    let mut results = Vec::with_capacity(commit_hashes.len());

    // Process commits in batches to bound concurrent subprocess count
    const BATCH_SIZE: usize = 50;

    for batch in commit_hashes.chunks(BATCH_SIZE) {
        let mut set = tokio::task::JoinSet::new();

        for (idx, hash) in batch.iter().enumerate() {
            let repo = repo_path.to_path_buf();
            let h = hash.clone();
            set.spawn(async move {
                let stats = get_commit_diff_stats(&repo, &h).await;
                (idx, h, stats)
            });
        }

        // Collect results preserving original order
        let mut batch_results: Vec<(usize, String, DiffStats)> = Vec::with_capacity(batch.len());
        while let Some(res) = set.join_next().await {
            match res {
                Ok(tuple) => batch_results.push(tuple),
                Err(e) => {
                    tracing::warn!("JoinSet task panicked in get_batch_diff_stats: {e}");
                }
            }
        }
        batch_results.sort_by_key(|(idx, _, _)| *idx);

        for (_, hash, stats) in batch_results {
            results.push((hash, stats));
        }
    }

    results
}

/// Parse diff stats from git show --stat output.
///
/// Expected format in the last line:
/// " N files changed, N insertions(+), N deletions(-)"
/// or partial variants like:
/// " N files changed, N insertions(+)"
/// " N files changed, N deletions(-)"
/// " N file changed"
fn parse_diff_stats_from_output(output: &str) -> DiffStats {
    // Find the summary line (last non-empty line containing "changed")
    let summary_line = output.lines().rev().find(|line| line.contains("changed"));

    let line = match summary_line {
        Some(l) => l.trim(),
        None => return DiffStats::default(),
    };

    let mut stats = DiffStats::default();

    // Parse "N file(s) changed"
    if let Some(pos) = line.find("file") {
        let prefix = &line[..pos].trim();
        if let Some(num_str) = prefix.split_whitespace().last() {
            if let Ok(n) = num_str.parse::<u32>() {
                stats.files_changed = n;
            }
        }
    }

    // Parse "N insertion(s)(+)"
    if let Some(pos) = line.find("insertion") {
        // Find the number before "insertion"
        let prefix = &line[..pos];
        if let Some(num_str) = prefix
            .split(',')
            .next_back()
            .and_then(|s| s.split_whitespace().next())
        {
            if let Ok(n) = num_str.parse::<u32>() {
                stats.insertions = n;
            }
        }
    }

    // Parse "N deletion(s)(-)"
    if let Some(pos) = line.find("deletion") {
        // Find the number before "deletion"
        let prefix = &line[..pos];
        if let Some(num_str) = prefix
            .split(',')
            .next_back()
            .and_then(|s| s.split_whitespace().next())
        {
            if let Ok(n) = num_str.parse::<u32>() {
                stats.deletions = n;
            }
        }
    }

    stats
}

/// Extract diff stats for a single commit using `git show --numstat`.
///
/// Uses `git show` instead of `git diff` to handle initial commits
/// (which have no parent).
///
/// Returns None if:
/// - Git command fails
/// - Repo path doesn't exist
/// - Commit hash is invalid
pub async fn extract_commit_diff_stats(repo_path: &Path, commit_hash: &str) -> Option<DiffStats> {
    let output = tokio::time::timeout(
        Duration::from_secs(5),
        Command::new("git")
            .args([
                "show",
                "--numstat",
                "--format=", // Don't show commit message
                commit_hash,
            ])
            .current_dir(repo_path)
            .output(),
    )
    .await
    .ok()?
    .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Some(DiffStats::from_numstat(&stdout))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_diff_stats_from_numstat_empty() {
        let stats = DiffStats::from_numstat("");
        assert_eq!(stats.insertions, 0);
        assert_eq!(stats.deletions, 0);
    }

    #[test]
    fn test_diff_stats_from_numstat_single_file() {
        let output = "10\t5\tfile.rs";
        let stats = DiffStats::from_numstat(output);
        assert_eq!(stats.insertions, 10);
        assert_eq!(stats.deletions, 5);
    }

    #[test]
    fn test_diff_stats_from_numstat_multiple_files() {
        let output = "10\t5\tfile.rs\n3\t0\tREADME.md\n0\t7\ttest.rs";
        let stats = DiffStats::from_numstat(output);
        assert_eq!(stats.insertions, 13); // 10 + 3 + 0
        assert_eq!(stats.deletions, 12); // 5 + 0 + 7
    }

    #[test]
    fn test_diff_stats_from_numstat_binary_files() {
        // Binary files show as "- - filename"
        let output = "10\t5\tfile.rs\n-\t-\timage.png\n3\t0\tREADME.md";
        let stats = DiffStats::from_numstat(output);
        assert_eq!(stats.insertions, 13); // Binary file ignored
        assert_eq!(stats.deletions, 5);
    }

    #[test]
    fn test_diff_stats_from_numstat_malformed_lines() {
        let output = "10\t5\tfile.rs\nmalformed line\n3\t0\tREADME.md";
        let stats = DiffStats::from_numstat(output);
        assert_eq!(stats.insertions, 13); // Malformed line ignored
        assert_eq!(stats.deletions, 5);
    }

    #[test]
    fn test_diff_stats_aggregate() {
        let stats1 = DiffStats {
            files_changed: 1,
            insertions: 10,
            deletions: 5,
        };
        let stats2 = DiffStats {
            files_changed: 1,
            insertions: 3,
            deletions: 7,
        };
        let stats3 = DiffStats {
            files_changed: 1,
            insertions: 0,
            deletions: 2,
        };

        let aggregated = DiffStats::aggregate(&[stats1, stats2, stats3]);
        assert_eq!(aggregated.insertions, 13);
        assert_eq!(aggregated.deletions, 14);
    }

    #[test]
    fn test_diff_stats_aggregate_empty() {
        let aggregated = DiffStats::aggregate(&[]);
        assert_eq!(aggregated.insertions, 0);
        assert_eq!(aggregated.deletions, 0);
    }

    #[tokio::test]
    async fn test_extract_commit_diff_stats_invalid_repo() {
        let result = extract_commit_diff_stats(
            Path::new("/nonexistent/path"),
            "abc123def456789012345678901234567890abcd",
        )
        .await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_extract_commit_diff_stats_real_commit() {
        let tmp = TempDir::new().unwrap();
        let repo_path = tmp.path();

        // Initialize git repo
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

        // Create a file with 10 lines
        std::fs::write(
            repo_path.join("file.txt"),
            "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\n",
        )
        .unwrap();

        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(repo_path)
            .output()
            .expect("git add");

        std::process::Command::new("git")
            .args(["commit", "-m", "initial commit"])
            .current_dir(repo_path)
            .output()
            .expect("git commit");

        // Get the commit hash
        let output = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(repo_path)
            .output()
            .expect("git rev-parse");
        let commit_hash = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Extract stats
        let stats = extract_commit_diff_stats(repo_path, &commit_hash)
            .await
            .expect("should extract stats");

        // Initial commit adds 10 lines, removes 0
        assert_eq!(stats.insertions, 10);
        assert_eq!(stats.deletions, 0);
    }
}
