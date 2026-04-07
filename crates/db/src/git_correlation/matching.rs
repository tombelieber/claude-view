// crates/db/src/git_correlation/matching.rs
//! Tier 1 and Tier 2 correlation matching algorithms.

use super::types::{
    CommitSkillInvocation, CorrelationEvidence, CorrelationMatch, GitCommit,
    TIER1_WINDOW_AFTER_SECS, TIER1_WINDOW_BEFORE_SECS,
};

/// Match commit skill invocations to commits (Tier 1 - high confidence).
///
/// A match occurs when:
/// - The session's project path matches the commit's repo path exactly
/// - The commit timestamp is within [-60s, +300s] of the skill invocation time
///
/// Returns matches in order of skill invocation (preserving chronological order).
pub fn tier1_match(
    session_id: &str,
    session_project_path: &str,
    skill_invocations: &[CommitSkillInvocation],
    commits: &[GitCommit],
) -> Vec<CorrelationMatch> {
    let mut matches = Vec::new();

    for skill in skill_invocations {
        let skill_ts = skill.timestamp_unix;
        let window_start = skill_ts - TIER1_WINDOW_BEFORE_SECS;
        let window_end = skill_ts + TIER1_WINDOW_AFTER_SECS;

        for commit in commits {
            // Repo must match exactly
            if commit.repo_path != session_project_path {
                continue;
            }

            // Commit must be within the time window
            if commit.timestamp >= window_start && commit.timestamp <= window_end {
                let evidence = CorrelationEvidence {
                    rule: "commit_skill".to_string(),
                    skill_ts: Some(skill_ts),
                    commit_ts: Some(commit.timestamp),
                    skill_name: Some(skill.skill_name.clone()),
                    session_start: None,
                    session_end: None,
                };

                matches.push(CorrelationMatch {
                    session_id: session_id.to_string(),
                    commit_hash: commit.hash.clone(),
                    tier: 1,
                    evidence,
                });
            }
        }
    }

    matches
}

/// Match commits that occurred during a session (Tier 2 - medium confidence).
///
/// A match occurs when:
/// - The session's project path matches the commit's repo path exactly
/// - The commit timestamp is within [session_start, session_end]
///
/// Note: Commits already matched by Tier 1 should be excluded by the caller.
pub fn tier2_match(
    session_id: &str,
    session_project_path: &str,
    session_start: i64,
    session_end: i64,
    commits: &[GitCommit],
) -> Vec<CorrelationMatch> {
    let mut matches = Vec::new();

    for commit in commits {
        // Repo must match exactly
        if commit.repo_path != session_project_path {
            continue;
        }

        // Commit must be within session time range
        if commit.timestamp >= session_start && commit.timestamp <= session_end {
            let evidence = CorrelationEvidence {
                rule: "during_session".to_string(),
                skill_ts: None,
                commit_ts: Some(commit.timestamp),
                skill_name: None,
                session_start: Some(session_start),
                session_end: Some(session_end),
            };

            matches.push(CorrelationMatch {
                session_id: session_id.to_string(),
                commit_hash: commit.hash.clone(),
                tier: 2,
                evidence,
            });
        }
    }

    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier1_match_exact_time() {
        let skills = vec![CommitSkillInvocation {
            skill_name: "commit".to_string(),
            timestamp_unix: 1706400100,
        }];

        let commits = vec![GitCommit {
            hash: "abc123def456789012345678901234567890abcd".to_string(),
            repo_path: "/repo/path".to_string(),
            message: "Test commit".to_string(),
            author: Some("Author".to_string()),
            timestamp: 1706400100, // Exact match
            branch: Some("main".to_string()),
            files_changed: None,
            insertions: None,
            deletions: None,
        }];

        let matches = tier1_match("sess-1", "/repo/path", &skills, &commits);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].tier, 1);
        assert_eq!(matches[0].session_id, "sess-1");
        assert_eq!(
            matches[0].commit_hash,
            "abc123def456789012345678901234567890abcd"
        );
        assert_eq!(matches[0].evidence.rule, "commit_skill");
        assert_eq!(matches[0].evidence.skill_name, Some("commit".to_string()));
    }

    #[test]
    fn test_tier1_match_before_window() {
        let skills = vec![CommitSkillInvocation {
            skill_name: "commit".to_string(),
            timestamp_unix: 1706400100,
        }];

        // Commit is 60 seconds before skill (within window)
        let commits = vec![GitCommit {
            hash: "abc123def456789012345678901234567890abcd".to_string(),
            repo_path: "/repo/path".to_string(),
            message: "Test commit".to_string(),
            author: None,
            timestamp: 1706400100 - 60, // 60s before (edge of window)
            branch: None,
            files_changed: None,
            insertions: None,
            deletions: None,
        }];

        let matches = tier1_match("sess-1", "/repo/path", &skills, &commits);
        assert_eq!(
            matches.len(),
            1,
            "Should match at edge of window (60s before)"
        );
    }

    #[test]
    fn test_tier1_match_after_window() {
        let skills = vec![CommitSkillInvocation {
            skill_name: "commit".to_string(),
            timestamp_unix: 1706400100,
        }];

        // Commit is 300 seconds after skill (within window)
        let commits = vec![GitCommit {
            hash: "abc123def456789012345678901234567890abcd".to_string(),
            repo_path: "/repo/path".to_string(),
            message: "Test commit".to_string(),
            author: None,
            timestamp: 1706400100 + 300, // 300s after (edge of window)
            branch: None,
            files_changed: None,
            insertions: None,
            deletions: None,
        }];

        let matches = tier1_match("sess-1", "/repo/path", &skills, &commits);
        assert_eq!(
            matches.len(),
            1,
            "Should match at edge of window (300s after)"
        );
    }

    #[test]
    fn test_tier1_match_outside_window() {
        let skills = vec![CommitSkillInvocation {
            skill_name: "commit".to_string(),
            timestamp_unix: 1706400100,
        }];

        // Commit is 301 seconds after skill (outside window)
        let commits = vec![GitCommit {
            hash: "abc123def456789012345678901234567890abcd".to_string(),
            repo_path: "/repo/path".to_string(),
            message: "Test commit".to_string(),
            author: None,
            timestamp: 1706400100 + 301, // 301s after (outside window)
            branch: None,
            files_changed: None,
            insertions: None,
            deletions: None,
        }];

        let matches = tier1_match("sess-1", "/repo/path", &skills, &commits);
        assert!(matches.is_empty(), "Should not match outside window");
    }

    #[test]
    fn test_tier1_match_repo_mismatch() {
        let skills = vec![CommitSkillInvocation {
            skill_name: "commit".to_string(),
            timestamp_unix: 1706400100,
        }];

        let commits = vec![GitCommit {
            hash: "abc123def456789012345678901234567890abcd".to_string(),
            repo_path: "/different/repo".to_string(), // Different repo
            message: "Test commit".to_string(),
            author: None,
            timestamp: 1706400100,
            branch: None,
            files_changed: None,
            insertions: None,
            deletions: None,
        }];

        let matches = tier1_match("sess-1", "/repo/path", &skills, &commits);
        assert!(matches.is_empty(), "Should not match different repo");
    }

    #[test]
    fn test_tier1_match_multiple_skills_multiple_commits() {
        let skills = vec![
            CommitSkillInvocation {
                skill_name: "commit".to_string(),
                timestamp_unix: 1706400100,
            },
            CommitSkillInvocation {
                skill_name: "commit-commands:commit".to_string(),
                timestamp_unix: 1706400500,
            },
        ];

        let commits = vec![
            GitCommit {
                hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                repo_path: "/repo/path".to_string(),
                message: "First commit".to_string(),
                author: None,
                timestamp: 1706400120, // Matches first skill
                branch: None,
                files_changed: None,
                insertions: None,
                deletions: None,
            },
            GitCommit {
                hash: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
                repo_path: "/repo/path".to_string(),
                message: "Second commit".to_string(),
                author: None,
                timestamp: 1706400520, // Matches second skill
                branch: None,
                files_changed: None,
                insertions: None,
                deletions: None,
            },
        ];

        let matches = tier1_match("sess-1", "/repo/path", &skills, &commits);
        assert_eq!(
            matches.len(),
            2,
            "Should match both skills to their commits"
        );
    }

    #[test]
    fn test_tier2_match_within_session() {
        let commits = vec![GitCommit {
            hash: "abc123def456789012345678901234567890abcd".to_string(),
            repo_path: "/repo/path".to_string(),
            message: "Test commit".to_string(),
            author: None,
            timestamp: 1706400200, // Within session range
            branch: None,
            files_changed: None,
            insertions: None,
            deletions: None,
        }];

        let matches = tier2_match("sess-1", "/repo/path", 1706400100, 1706400300, &commits);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].tier, 2);
        assert_eq!(matches[0].evidence.rule, "during_session");
        assert_eq!(matches[0].evidence.session_start, Some(1706400100));
        assert_eq!(matches[0].evidence.session_end, Some(1706400300));
    }

    #[test]
    fn test_tier2_match_at_boundaries() {
        let commits = vec![
            GitCommit {
                hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                repo_path: "/repo/path".to_string(),
                message: "Start commit".to_string(),
                author: None,
                timestamp: 1706400100, // At session start
                branch: None,
                files_changed: None,
                insertions: None,
                deletions: None,
            },
            GitCommit {
                hash: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
                repo_path: "/repo/path".to_string(),
                message: "End commit".to_string(),
                author: None,
                timestamp: 1706400300, // At session end
                branch: None,
                files_changed: None,
                insertions: None,
                deletions: None,
            },
        ];

        let matches = tier2_match("sess-1", "/repo/path", 1706400100, 1706400300, &commits);
        assert_eq!(
            matches.len(),
            2,
            "Should match commits at session boundaries"
        );
    }

    #[test]
    fn test_tier2_match_outside_session() {
        let commits = vec![
            GitCommit {
                hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                repo_path: "/repo/path".to_string(),
                message: "Before session".to_string(),
                author: None,
                timestamp: 1706400099, // 1 second before session
                branch: None,
                files_changed: None,
                insertions: None,
                deletions: None,
            },
            GitCommit {
                hash: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
                repo_path: "/repo/path".to_string(),
                message: "After session".to_string(),
                author: None,
                timestamp: 1706400301, // 1 second after session
                branch: None,
                files_changed: None,
                insertions: None,
                deletions: None,
            },
        ];

        let matches = tier2_match("sess-1", "/repo/path", 1706400100, 1706400300, &commits);
        assert!(
            matches.is_empty(),
            "Should not match commits outside session"
        );
    }

    #[test]
    fn test_tier2_match_repo_mismatch() {
        let commits = vec![GitCommit {
            hash: "abc123def456789012345678901234567890abcd".to_string(),
            repo_path: "/different/repo".to_string(),
            message: "Test commit".to_string(),
            author: None,
            timestamp: 1706400200,
            branch: None,
            files_changed: None,
            insertions: None,
            deletions: None,
        }];

        let matches = tier2_match("sess-1", "/repo/path", 1706400100, 1706400300, &commits);
        assert!(matches.is_empty(), "Should not match different repo");
    }

    #[test]
    fn test_correlation_evidence_serialization() {
        let evidence = CorrelationEvidence {
            rule: "commit_skill".to_string(),
            skill_ts: Some(1706400100),
            commit_ts: Some(1706400120),
            skill_name: Some("commit".to_string()),
            session_start: None,
            session_end: None,
        };

        let json = serde_json::to_string(&evidence).unwrap();
        assert!(json.contains("\"rule\":\"commit_skill\""));
        assert!(json.contains("\"skill_ts\":1706400100"));
        assert!(json.contains("\"commit_ts\":1706400120"));
        assert!(json.contains("\"skill_name\":\"commit\""));
        // None fields should be skipped
        assert!(!json.contains("session_start"));
        assert!(!json.contains("session_end"));

        // Deserialize back
        let parsed: CorrelationEvidence = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.rule, "commit_skill");
        assert_eq!(parsed.skill_ts, Some(1706400100));
    }

    #[test]
    fn test_correlation_evidence_tier2() {
        let evidence = CorrelationEvidence {
            rule: "during_session".to_string(),
            skill_ts: None,
            commit_ts: Some(1706400200),
            skill_name: None,
            session_start: Some(1706400100),
            session_end: Some(1706400300),
        };

        let json = serde_json::to_string(&evidence).unwrap();
        assert!(json.contains("\"rule\":\"during_session\""));
        assert!(json.contains("\"session_start\":1706400100"));
        assert!(json.contains("\"session_end\":1706400300"));
        // None fields should be skipped
        assert!(!json.contains("skill_ts"));
        assert!(!json.contains("skill_name"));
    }
}
