//! Phase 2 indexer_v2 drift comparator — per-field divergence between
//! the legacy `sessions` row and the shadow `session_stats` row for a
//! given session.
//!
//! Used by:
//!   - The 100-session parity test (PR 2.2.3 / handoff §3.3 exit gate):
//!     walk a sample of real sessions, call `compare_session` on each,
//!     assert every report has empty `diffs`.
//!   - The eventual `/metrics` endpoint (Phase 4): each non-empty diff
//!     bumps a `shadow_diff_total{field}` counter so a Grafana board
//!     can show the drift rate over a 24 h window.
//!
//! Only fields that exist on **both** tables are compared. Schema-only
//! columns (e.g. `session_stats.parser_version`, `sessions.archived_at`)
//! are not drift — they're by design.
//!
//! Timestamp normalization: `sessions.first_message_at` is INTEGER unix
//! seconds; `session_stats.first_message_at` is also INTEGER unix
//! seconds (the writer normalizes from RFC3339). Direct integer compare
//! works.

use crate::{Database, DbResult};

/// Single per-field drift between legacy and shadow rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldDiff {
    /// Stable identifier — used as the `field` label on
    /// `shadow_diff_total{field}` when this lands in /metrics.
    pub field: &'static str,
    /// Stringified legacy value (for human-readable diff output).
    pub legacy: String,
    /// Stringified shadow value (for human-readable diff output).
    pub shadow: String,
}

/// Drift comparison between the legacy `sessions` row and the shadow
/// `session_stats` row for a single session.
///
/// `diffs` is empty when every overlapping field matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriftReport {
    pub session_id: String,
    pub diffs: Vec<FieldDiff>,
}

impl DriftReport {
    /// `true` when the shadow row matches the legacy row on every
    /// overlapping field.
    pub fn is_clean(&self) -> bool {
        self.diffs.is_empty()
    }
}

/// Read the legacy `sessions` row and the shadow `session_stats` row
/// for `session_id` and return a per-field drift report.
///
/// Returns `Ok(None)` when either row is missing — drift can only be
/// computed when both writers have produced a row. The 100-session
/// parity test treats `None` as "no comparison possible, skip".
pub async fn compare_session(db: &Database, session_id: &str) -> DbResult<Option<DriftReport>> {
    let legacy: Option<OverlapRow> = sqlx::query_as::<_, OverlapRow>(LEGACY_SELECT_SQL)
        .bind(session_id)
        .fetch_optional(db.pool())
        .await?;
    let Some(legacy) = legacy else {
        return Ok(None);
    };

    let shadow: Option<OverlapRow> = sqlx::query_as::<_, OverlapRow>(SHADOW_SELECT_SQL)
        .bind(session_id)
        .fetch_optional(db.pool())
        .await?;
    let Some(shadow) = shadow else {
        return Ok(None);
    };

    let mut diffs = Vec::new();
    macro_rules! diff_eq {
        ($field:literal, $a:expr, $b:expr) => {
            if $a != $b {
                diffs.push(FieldDiff {
                    field: $field,
                    legacy: format!("{:?}", $a),
                    shadow: format!("{:?}", $b),
                });
            }
        };
    }

    diff_eq!(
        "total_input_tokens",
        legacy.total_input_tokens,
        shadow.total_input_tokens
    );
    diff_eq!(
        "total_output_tokens",
        legacy.total_output_tokens,
        shadow.total_output_tokens
    );
    diff_eq!(
        "cache_read_tokens",
        legacy.cache_read_tokens,
        shadow.cache_read_tokens
    );
    diff_eq!(
        "cache_creation_tokens",
        legacy.cache_creation_tokens,
        shadow.cache_creation_tokens
    );
    diff_eq!("turn_count", legacy.turn_count, shadow.turn_count);
    diff_eq!(
        "user_prompt_count",
        legacy.user_prompt_count,
        shadow.user_prompt_count
    );
    diff_eq!(
        "tool_call_count",
        legacy.tool_call_count,
        shadow.tool_call_count
    );
    diff_eq!(
        "thinking_block_count",
        legacy.thinking_block_count,
        shadow.thinking_block_count
    );
    diff_eq!(
        "api_error_count",
        legacy.api_error_count,
        shadow.api_error_count
    );
    diff_eq!(
        "files_read_count",
        legacy.files_read_count,
        shadow.files_read_count
    );
    diff_eq!(
        "files_edited_count",
        legacy.files_edited_count,
        shadow.files_edited_count
    );
    diff_eq!(
        "agent_spawn_count",
        legacy.agent_spawn_count,
        shadow.agent_spawn_count
    );
    diff_eq!(
        "duration_seconds",
        legacy.duration_seconds,
        shadow.duration_seconds
    );
    diff_eq!("primary_model", legacy.primary_model, shadow.primary_model);
    diff_eq!("git_branch", legacy.git_branch, shadow.git_branch);
    diff_eq!("preview", legacy.preview, shadow.preview);
    diff_eq!("last_message", legacy.last_message, shadow.last_message);
    diff_eq!(
        "first_message_at",
        legacy.first_message_at,
        shadow.first_message_at
    );
    diff_eq!(
        "last_message_at",
        legacy.last_message_at,
        shadow.last_message_at
    );

    Ok(Some(DriftReport {
        session_id: session_id.to_string(),
        diffs,
    }))
}

const LEGACY_SELECT_SQL: &str = r#"
SELECT total_input_tokens, total_output_tokens, cache_read_tokens, cache_creation_tokens,
       turn_count, user_prompt_count, tool_call_count, thinking_block_count, api_error_count,
       files_read_count, files_edited_count, agent_spawn_count,
       duration_seconds, primary_model, git_branch,
       preview, last_message, first_message_at, last_message_at
FROM sessions
WHERE id = ?
"#;

const SHADOW_SELECT_SQL: &str = r#"
SELECT total_input_tokens, total_output_tokens, cache_read_tokens, cache_creation_tokens,
       turn_count, user_prompt_count, tool_call_count, thinking_block_count, api_error_count,
       files_read_count, files_edited_count, agent_spawn_count,
       duration_seconds, primary_model, git_branch,
       preview, last_message, first_message_at, last_message_at
FROM session_stats
WHERE session_id = ?
"#;

/// Overlap row — every field both `sessions` and `session_stats` carry,
/// in the order returned by [`LEGACY_SELECT_SQL`] / [`SHADOW_SELECT_SQL`].
/// Used for both legacy and shadow rows because the SELECT projections
/// match column-for-column.
#[derive(Debug, PartialEq, Eq)]
struct OverlapRow {
    total_input_tokens: i64,
    total_output_tokens: i64,
    cache_read_tokens: i64,
    cache_creation_tokens: i64,
    turn_count: i64,
    user_prompt_count: i64,
    tool_call_count: i64,
    thinking_block_count: i64,
    api_error_count: i64,
    files_read_count: i64,
    files_edited_count: i64,
    agent_spawn_count: i64,
    duration_seconds: i64,
    primary_model: Option<String>,
    git_branch: Option<String>,
    preview: String,
    last_message: String,
    first_message_at: Option<i64>,
    last_message_at: Option<i64>,
}

impl<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> for OverlapRow {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;
        Ok(Self {
            total_input_tokens: row.try_get("total_input_tokens")?,
            total_output_tokens: row.try_get("total_output_tokens")?,
            cache_read_tokens: row.try_get("cache_read_tokens")?,
            cache_creation_tokens: row.try_get("cache_creation_tokens")?,
            turn_count: row.try_get("turn_count")?,
            user_prompt_count: row.try_get("user_prompt_count")?,
            tool_call_count: row.try_get("tool_call_count")?,
            thinking_block_count: row.try_get("thinking_block_count")?,
            api_error_count: row.try_get("api_error_count")?,
            files_read_count: row.try_get("files_read_count")?,
            files_edited_count: row.try_get("files_edited_count")?,
            agent_spawn_count: row.try_get("agent_spawn_count")?,
            duration_seconds: row.try_get("duration_seconds")?,
            primary_model: row.try_get("primary_model")?,
            git_branch: row.try_get("git_branch")?,
            preview: row.try_get("preview")?,
            last_message: row.try_get("last_message")?,
            first_message_at: row.try_get("first_message_at")?,
            last_message_at: row.try_get("last_message_at")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal sessions row insert covering every overlap column.
    /// Returns the session_id used.
    async fn seed_legacy(db: &Database, sid: &str, tokens: i64) -> () {
        sqlx::query(
            r#"INSERT INTO sessions (
                   id, project_id, file_path,
                   total_input_tokens, total_output_tokens,
                   cache_read_tokens, cache_creation_tokens,
                   turn_count, user_prompt_count, tool_call_count,
                   thinking_block_count, api_error_count,
                   files_read_count, files_edited_count, agent_spawn_count,
                   duration_seconds, primary_model, git_branch,
                   preview, last_message, first_message_at, last_message_at
               ) VALUES (?, 'proj', '/tmp/x.jsonl',
                         ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(sid)
        .bind(tokens) // total_input_tokens
        .bind(tokens) // total_output_tokens
        .bind(0_i64) // cache_read_tokens
        .bind(0_i64) // cache_creation_tokens
        .bind(1_i64) // turn_count
        .bind(1_i64) // user_prompt_count
        .bind(0_i64) // tool_call_count
        .bind(0_i64) // thinking_block_count
        .bind(0_i64) // api_error_count
        .bind(0_i64) // files_read_count
        .bind(0_i64) // files_edited_count
        .bind(0_i64) // agent_spawn_count
        .bind(0_i64) // duration_seconds
        .bind(Some("claude-sonnet-4-6")) // primary_model
        .bind(Some("main")) // git_branch
        .bind("preview text") // preview
        .bind("last text") // last_message
        .bind(Some(1_700_000_000_i64)) // first_message_at
        .bind(Some(1_700_000_300_i64)) // last_message_at
        .execute(db.pool())
        .await
        .unwrap();
    }

    async fn seed_shadow(db: &Database, sid: &str, tokens: i64) {
        sqlx::query(
            r#"INSERT INTO session_stats (
                   session_id, source_content_hash, source_size,
                   parser_version, stats_version, indexed_at,
                   total_input_tokens, total_output_tokens,
                   cache_read_tokens, cache_creation_tokens,
                   turn_count, user_prompt_count, tool_call_count,
                   thinking_block_count, api_error_count,
                   files_read_count, files_edited_count, agent_spawn_count,
                   duration_seconds, primary_model, git_branch,
                   preview, last_message, first_message_at, last_message_at
               ) VALUES (?, X'00', 0, 1, 1, 0,
                         ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(sid)
        .bind(tokens)
        .bind(tokens)
        .bind(0_i64)
        .bind(0_i64)
        .bind(1_i64)
        .bind(1_i64)
        .bind(0_i64)
        .bind(0_i64)
        .bind(0_i64)
        .bind(0_i64)
        .bind(0_i64)
        .bind(0_i64)
        .bind(0_i64)
        .bind(Some("claude-sonnet-4-6"))
        .bind(Some("main"))
        .bind("preview text")
        .bind("last text")
        .bind(Some(1_700_000_000_i64))
        .bind(Some(1_700_000_300_i64))
        .execute(db.pool())
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn compare_returns_none_when_neither_row_exists() {
        let db = Database::new_in_memory().await.unwrap();
        let report = compare_session(&db, "ghost").await.unwrap();
        assert!(report.is_none());
    }

    #[tokio::test]
    async fn compare_returns_none_when_only_legacy_row_exists() {
        let db = Database::new_in_memory().await.unwrap();
        seed_legacy(&db, "legacy-only", 100).await;
        let report = compare_session(&db, "legacy-only").await.unwrap();
        assert!(
            report.is_none(),
            "no comparison possible without shadow row"
        );
    }

    #[tokio::test]
    async fn compare_returns_none_when_only_shadow_row_exists() {
        let db = Database::new_in_memory().await.unwrap();
        seed_shadow(&db, "shadow-only", 100).await;
        let report = compare_session(&db, "shadow-only").await.unwrap();
        assert!(report.is_none());
    }

    #[tokio::test]
    async fn compare_returns_clean_report_when_rows_match() {
        let db = Database::new_in_memory().await.unwrap();
        seed_legacy(&db, "match-sess", 500).await;
        seed_shadow(&db, "match-sess", 500).await;

        let report = compare_session(&db, "match-sess")
            .await
            .unwrap()
            .expect("both rows present must yield Some");
        assert_eq!(report.session_id, "match-sess");
        assert!(
            report.is_clean(),
            "matching rows must produce empty diffs, got {:?}",
            report.diffs
        );
    }

    #[tokio::test]
    async fn compare_reports_token_drift_with_field_label() {
        let db = Database::new_in_memory().await.unwrap();
        seed_legacy(&db, "drift-sess", 100).await;
        seed_shadow(&db, "drift-sess", 999).await;

        let report = compare_session(&db, "drift-sess")
            .await
            .unwrap()
            .expect("both rows present");
        assert!(
            !report.is_clean(),
            "differing token counts must produce diffs"
        );

        let fields: Vec<&str> = report.diffs.iter().map(|d| d.field).collect();
        assert!(
            fields.contains(&"total_input_tokens"),
            "expected total_input_tokens diff, got fields: {:?}",
            fields
        );
        assert!(
            fields.contains(&"total_output_tokens"),
            "expected total_output_tokens diff, got fields: {:?}",
            fields
        );
        // Other token columns matched (both 0) — must NOT appear.
        assert!(
            !fields.contains(&"cache_read_tokens"),
            "matching columns must not appear in diffs"
        );

        let token_diff = report
            .diffs
            .iter()
            .find(|d| d.field == "total_input_tokens")
            .unwrap();
        assert_eq!(token_diff.legacy, "100");
        assert_eq!(token_diff.shadow, "999");
    }
}
