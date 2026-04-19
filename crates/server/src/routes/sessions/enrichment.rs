//! DB enrichment layer for JSONL-derived `SessionInfo`.
//!
//! The canonical `/api/sessions` endpoint is JSONL-first — everything it
//! can compute from the JSONL file is computed on demand via
//! [`claude_view_core::session_stats`]. This module provides the thin
//! DB-only layer on top: user state (`archived_at`), commit count,
//! skills classification, and re-edit rate.
//!
//! One bulk query, keyed by session id, replaces the old N+1 pattern.

use std::collections::HashMap;

use claude_view_db::{Database, DbResult, LinkedCommit};

/// Fields that live only in the SQLite mirror. Everything else comes
/// from the JSONL file via [`claude_view_core::session_stats`].
#[derive(Debug, Clone, Default)]
pub struct SessionEnrichment {
    /// RFC-3339 timestamp set when the session was archived, or `None`.
    pub archived_at: Option<String>,
    /// Number of git commits linked to this session.
    pub commit_count: usize,
    /// Skills classification output (array of skill ids, possibly empty).
    pub skills_used: Vec<String>,
    /// `reedited_files_count / files_edited_count`, or `0.0` when no edits.
    pub reedit_rate: f32,
    /// Full linked-commit detail. Empty in the list path; populated by
    /// the detail handler via a secondary query (TODO: wire in detail.rs).
    #[allow(dead_code)]
    pub linked_commits: Vec<LinkedCommit>,
}

/// Bulk-fetch enrichment records for a list of session ids.
///
/// Ids with no row in the `sessions` table are simply absent from the map —
/// callers should treat missing ids as [`SessionEnrichment::default()`].
pub async fn fetch_enrichments(
    db: &Database,
    session_ids: &[String],
) -> DbResult<HashMap<String, SessionEnrichment>> {
    if session_ids.is_empty() {
        return Ok(HashMap::new());
    }

    // CQRS Phase D.3 — `sessions.archived_at` was dropped. The shadow
    // now lives in `session_flags.archived_at` (unix-ms INTEGER);
    // re-emit as RFC3339 so `SessionEnrichment::archived_at`
    // (Option<String>) keeps its public contract.
    let ids_json = serde_json::to_string(session_ids).expect("serialize ids");
    let rows: Vec<(String, Option<String>, i64, String, i64, i64)> = sqlx::query_as(
        r#"
        SELECT
            s.id,
            CASE
                WHEN sf.archived_at IS NULL THEN NULL
                ELSE strftime('%Y-%m-%dT%H:%M:%fZ', sf.archived_at / 1000.0, 'unixepoch')
            END AS archived_at,
            s.commit_count,
            s.skills_used,
            s.reedited_files_count,
            s.files_edited_count
        FROM sessions s
        LEFT JOIN session_flags sf ON sf.session_id = s.id
        WHERE s.id IN (SELECT value FROM json_each(?1))
        "#,
    )
    .bind(&ids_json)
    .fetch_all(db.pool())
    .await?;

    let mut out = HashMap::with_capacity(rows.len());
    for (id, archived_at, commit_count, skills_json, reedited, files_edited) in rows {
        let skills_used: Vec<String> = serde_json::from_str(&skills_json).unwrap_or_default();
        let reedit_rate = if files_edited > 0 {
            reedited as f32 / files_edited as f32
        } else {
            0.0
        };
        out.insert(
            id,
            SessionEnrichment {
                archived_at,
                commit_count: commit_count as usize,
                skills_used,
                reedit_rate,
                linked_commits: Vec::new(),
            },
        );
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use claude_view_db::Database;

    async fn test_db() -> Database {
        Database::new_in_memory().await.expect("in-memory DB")
    }

    #[tokio::test]
    async fn empty_ids_returns_empty_map() {
        let db = test_db().await;
        let out = fetch_enrichments(&db, &[]).await.expect("empty fetch");
        assert!(out.is_empty());
    }

    #[tokio::test]
    async fn fetches_archive_commits_skills_reedit() {
        // One archived session with 3 commits, 2-of-10 re-edits, and one skill.
        // One live session with nothing. One id absent from the DB.
        let db = test_db().await;

        // Phase D.3 — session_flags is the archive source. s1 gets a
        // shadow row mirroring the legacy `'2026-04-01T10:00:00Z'`;
        // s2 stays unarchived by omitting the shadow row entirely.
        sqlx::query(
            "INSERT INTO sessions \
             (id, project_id, file_path, commit_count, skills_used, \
              reedited_files_count, files_edited_count) \
             VALUES ('s1', 'p1', '/tmp/s1.jsonl', 3, '[\"tdd\"]', 2, 10)",
        )
        .execute(db.pool())
        .await
        .unwrap();
        // 2026-04-01T10:00:00.000Z = 1775037600000 ms (UTC).
        sqlx::query(
            "INSERT INTO session_flags (session_id, archived_at, applied_seq) \
             VALUES ('s1', 1775037600000, 1)",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO sessions \
             (id, project_id, file_path, commit_count, skills_used, \
              reedited_files_count, files_edited_count) \
             VALUES ('s2', 'p1', '/tmp/s2.jsonl', 0, '[]', 0, 0)",
        )
        .execute(db.pool())
        .await
        .unwrap();

        let ids = vec!["s1".to_string(), "s2".to_string(), "missing".to_string()];
        let out = fetch_enrichments(&db, &ids).await.unwrap();

        let s1 = out.get("s1").expect("s1");
        // Post Phase D.3 the ms→RFC3339 conversion emits
        // milliseconds ("…:00.000Z"); callers only check `.is_some()`
        // so the extra precision is strictly additive.
        assert_eq!(s1.archived_at.as_deref(), Some("2026-04-01T10:00:00.000Z"));
        assert_eq!(s1.commit_count, 3);
        assert_eq!(s1.skills_used, vec!["tdd".to_string()]);
        assert!((s1.reedit_rate - 0.2).abs() < 0.001);
        assert!(s1.linked_commits.is_empty()); // list path leaves this empty

        let s2 = out.get("s2").expect("s2");
        assert_eq!(s2.archived_at, None);
        assert_eq!(s2.commit_count, 0);
        assert!(s2.skills_used.is_empty());
        assert_eq!(s2.reedit_rate, 0.0);

        assert!(
            !out.contains_key("missing"),
            "ids with no DB row are omitted"
        );
    }
}
