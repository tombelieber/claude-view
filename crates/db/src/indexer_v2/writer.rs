//! Phase 2 indexer_v2 writer — the only writer to `session_stats`.
//!
//! `upsert_session_stats` is the single SQL gateway between parsed
//! [`SessionStats`] and the typed mirror table. No other module writes
//! to `session_stats` — this enforces the "writer ownership registry"
//! property from the design doc (§10.2).

use claude_view_session_parser::{PARSER_VERSION, STATS_VERSION};

use super::config::StatsDelta;
use crate::{Database, DbResult};

/// Atomically upsert a `session_stats` row from a [`StatsDelta`].
///
/// The upsert key is `session_id`. On conflict, every column except
/// `session_id` is overwritten — there is no partial update path for the
/// shadow table; either the parser produced a complete delta or the
/// caller skipped the upsert entirely.
///
/// `parser_version` and `stats_version` come from the session-parser
/// crate's compile-time constants. `indexed_at` is `now()` in unix
/// seconds. RFC3339 timestamps from the parser are normalized to unix
/// seconds; unparseable input becomes NULL (consistent with the legacy
/// path that also tolerates malformed timestamps).
///
/// `per_model_tokens_json` serializes the parser's
/// `HashMap<String, TokenUsage>` via `serde_json`. Phase 2 left this as
/// a hardcoded `'{}'` placeholder because Phase 3 readers didn't consume
/// it yet; Phase 3 PR 3.2 (this change) wires cost calculation off the
/// DB row so the real data has to flow through. The serialization is
/// infallible in practice (HashMap + TokenUsage derive Serialize with
/// no trait objects) — the unwrap_or falls back to `'{}'` for
/// belt-and-braces.
pub async fn upsert_session_stats(db: &Database, delta: &StatsDelta) -> DbResult<()> {
    let now = chrono::Utc::now().timestamp();
    let first_ts = delta
        .stats
        .first_message_at
        .as_deref()
        .and_then(rfc3339_to_unix);
    let last_ts = delta
        .stats
        .last_message_at
        .as_deref()
        .and_then(rfc3339_to_unix);
    let per_model_tokens_json =
        serde_json::to_string(&delta.stats.per_model_tokens).unwrap_or_else(|_| "{}".into());

    // Phase 3 PR 3.a: filesystem-mirror columns added in migration 66.
    // The writer persists them on every upsert; the adapter selects
    // them so readers can serve project_id / file_path / mtime without
    // a parallel fs scan.
    let is_compressed_int = i64::from(delta.is_compressed);

    sqlx::query(
        r#"INSERT INTO session_stats (
                session_id, source_content_hash, source_size, source_inode, source_mid_hash,
                parser_version, stats_version, indexed_at,
                total_input_tokens, total_output_tokens, cache_read_tokens, cache_creation_tokens,
                cache_creation_5m_tokens, cache_creation_1hr_tokens,
                turn_count, user_prompt_count, line_count, tool_call_count,
                thinking_block_count, api_error_count,
                files_read_count, files_edited_count, bash_count, agent_spawn_count,
                first_message_at, last_message_at, duration_seconds,
                primary_model, git_branch, preview, last_message,
                per_model_tokens_json,
                project_id, file_path, is_compressed, source_mtime
           ) VALUES (
                ?, ?, ?, ?, ?,
                ?, ?, ?,
                ?, ?, ?, ?,
                ?, ?,
                ?, ?, ?, ?,
                ?, ?,
                ?, ?, ?, ?,
                ?, ?, ?,
                ?, ?, ?, ?,
                ?,
                ?, ?, ?, ?
           )
           ON CONFLICT(session_id) DO UPDATE SET
                source_content_hash = excluded.source_content_hash,
                source_size = excluded.source_size,
                source_inode = excluded.source_inode,
                source_mid_hash = excluded.source_mid_hash,
                parser_version = excluded.parser_version,
                stats_version = excluded.stats_version,
                indexed_at = excluded.indexed_at,
                total_input_tokens = excluded.total_input_tokens,
                total_output_tokens = excluded.total_output_tokens,
                cache_read_tokens = excluded.cache_read_tokens,
                cache_creation_tokens = excluded.cache_creation_tokens,
                cache_creation_5m_tokens = excluded.cache_creation_5m_tokens,
                cache_creation_1hr_tokens = excluded.cache_creation_1hr_tokens,
                turn_count = excluded.turn_count,
                user_prompt_count = excluded.user_prompt_count,
                line_count = excluded.line_count,
                tool_call_count = excluded.tool_call_count,
                thinking_block_count = excluded.thinking_block_count,
                api_error_count = excluded.api_error_count,
                files_read_count = excluded.files_read_count,
                files_edited_count = excluded.files_edited_count,
                bash_count = excluded.bash_count,
                agent_spawn_count = excluded.agent_spawn_count,
                first_message_at = excluded.first_message_at,
                last_message_at = excluded.last_message_at,
                duration_seconds = excluded.duration_seconds,
                primary_model = excluded.primary_model,
                git_branch = excluded.git_branch,
                preview = excluded.preview,
                last_message = excluded.last_message,
                per_model_tokens_json = excluded.per_model_tokens_json,
                project_id = excluded.project_id,
                file_path = excluded.file_path,
                is_compressed = excluded.is_compressed,
                source_mtime = excluded.source_mtime"#,
    )
    .bind(&delta.session_id)
    .bind(&delta.source_content_hash)
    .bind(delta.source_size)
    .bind(delta.source_inode)
    .bind(&delta.source_mid_hash)
    .bind(i64::from(PARSER_VERSION.0))
    .bind(i64::from(STATS_VERSION.0))
    .bind(now)
    .bind(delta.stats.total_input_tokens as i64)
    .bind(delta.stats.total_output_tokens as i64)
    .bind(delta.stats.cache_read_tokens as i64)
    .bind(delta.stats.cache_creation_tokens as i64)
    .bind(delta.stats.cache_creation_5m_tokens as i64)
    .bind(delta.stats.cache_creation_1hr_tokens as i64)
    .bind(delta.stats.turn_count as i64)
    .bind(delta.stats.user_prompt_count as i64)
    .bind(delta.stats.line_count as i64)
    .bind(delta.stats.tool_call_count as i64)
    .bind(delta.stats.thinking_block_count as i64)
    .bind(delta.stats.api_error_count as i64)
    .bind(delta.stats.files_read_count as i64)
    .bind(delta.stats.files_edited_count as i64)
    .bind(delta.stats.bash_count as i64)
    .bind(delta.stats.agent_spawn_count as i64)
    .bind(first_ts)
    .bind(last_ts)
    .bind(delta.stats.duration_seconds as i64)
    .bind(delta.stats.primary_model.as_deref())
    .bind(delta.stats.git_branch.as_deref())
    .bind(delta.stats.preview.as_str())
    .bind(delta.stats.last_message.as_str())
    .bind(per_model_tokens_json)
    .bind(delta.project_id.as_str())
    .bind(delta.source_file_path.as_str())
    .bind(is_compressed_int)
    .bind(delta.source_mtime)
    .execute(db.pool())
    .await?;

    Ok(())
}

/// Best-effort RFC3339 → unix-seconds conversion. Returns None on parse
/// failure; the legacy path stores NULL in the same case.
fn rfc3339_to_unix(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.timestamp())
}
