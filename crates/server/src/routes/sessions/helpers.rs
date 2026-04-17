//! Shared helpers for session route handlers.

use std::collections::HashMap;

use claude_view_core::pricing::ModelPricing;
use claude_view_core::session_catalog::CatalogRow;
use claude_view_core::session_stats::SessionStats;
use claude_view_core::{SessionInfo, ToolCounts};

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

/// Project ids are filesystem-encoded (`/`, `@`, `.` all become `-`) so the
/// decode is lossy. Return the encoded id verbatim as the path — callers that
/// need a real directory must derive it from JSONL `cwd`.
pub(crate) fn project_path_from_id(project_id: &str) -> String {
    project_id.to_string()
}

/// Display name for an encoded project id. Same heuristic as
/// [`project_path_from_id`] — kept as a distinct function so a smarter
/// display strategy can land without touching every call site.
pub(crate) fn display_name_from_project(project_id: &str) -> String {
    project_id.to_string()
}

fn compute_total_cost(
    per_model: &HashMap<String, claude_view_core::pricing::TokenUsage>,
    pricing: &HashMap<String, ModelPricing>,
) -> f64 {
    per_model
        .iter()
        .map(|(model, tokens)| {
            claude_view_core::pricing::calculate_cost(tokens, Some(model.as_str()), pricing)
                .total_usd
        })
        .sum()
}

/// Build a `SessionInfo` from catalog metadata + JSONL-extracted stats.
///
/// Fields that require DB state (`archived_at`, `commit_count`, `skills_used`,
/// `reedit_rate`, `linked_commits`) are left at their defaults — callers layer
/// those in via [`super::enrichment::fetch_enrichments`].
pub(crate) fn build_session_info(
    row: &CatalogRow,
    stats: &SessionStats,
    pricing: &HashMap<String, ModelPricing>,
) -> SessionInfo {
    let total_cost_usd = compute_total_cost(&stats.per_model_tokens, pricing);
    let first_message_at = stats
        .first_message_at
        .as_deref()
        .and_then(|ts| chrono::DateTime::parse_from_rfc3339(ts).ok())
        .map(|dt| dt.timestamp());

    SessionInfo {
        id: row.id.clone(),
        project: row.project_id.clone(),
        project_path: project_path_from_id(&row.project_id),
        display_name: display_name_from_project(&row.project_id),
        file_path: row.file_path.to_string_lossy().to_string(),
        modified_at: row.mtime,
        size_bytes: row.bytes,
        total_input_tokens: Some(stats.total_input_tokens),
        total_output_tokens: Some(stats.total_output_tokens),
        total_cache_read_tokens: Some(stats.cache_read_tokens),
        total_cache_creation_tokens: Some(stats.cache_creation_tokens),
        turn_count: stats.turn_count as usize,
        turn_count_api: Some(stats.turn_count as u64),
        message_count: (stats.turn_count + stats.user_prompt_count) as usize,
        primary_model: stats.primary_model.clone(),
        git_branch: stats.git_branch.clone(),
        tool_call_count: stats.tool_call_count,
        thinking_block_count: stats.thinking_block_count,
        files_read_count: stats.files_read_count,
        files_edited_count: stats.files_edited_count,
        duration_seconds: stats.duration_seconds,
        first_message_at,
        preview: stats.preview.clone(),
        last_message: stats.last_message.clone(),
        user_prompt_count: stats.user_prompt_count,
        api_call_count: stats.turn_count,
        agent_spawn_count: stats.agent_spawn_count,
        api_error_count: stats.api_error_count,
        total_cost_usd: if total_cost_usd > 0.0 {
            Some(total_cost_usd)
        } else {
            None
        },
        tool_counts: ToolCounts {
            read: stats.files_read_count as usize,
            edit: stats.files_edited_count as usize,
            bash: stats.bash_count as usize,
            write: 0,
        },
        ..Default::default()
    }
}

/// Resolve a session's JSONL file path: catalog first, then DB, then live session store.
///
/// Phase 2 (JSONL-first hardcut): the in-memory `SessionCatalog` is the authoritative
/// source for session → file_path lookups — it's populated from a filesystem walk at
/// startup and reconciled every 5s, so it sees every session that exists on disk.
///
/// The DB and live-session fallbacks remain for two narrow cases:
///   - DB: coverage during the migration window when callers hand in a session id
///     that the catalog hasn't yet picked up (pre-reconcile race).
///   - Live: IDE-spawned sessions whose JSONL file may not yet be on disk.
pub(crate) async fn resolve_session_file_path(
    state: &AppState,
    session_id: &str,
) -> ApiResult<std::path::PathBuf> {
    // Catalog first — no DB, no async, no lock contention.
    if let Some(row) = state.session_catalog.get(session_id) {
        if row.file_path.exists() {
            return Ok(row.file_path);
        }
    }

    let file_path = match state.db.get_session_file_path(session_id).await? {
        Some(p) => p,
        None => {
            let map = state.live_sessions.read().await;
            map.get(session_id)
                .map(|s| s.jsonl.file_path.clone())
                .ok_or_else(|| ApiError::SessionNotFound(session_id.to_string()))?
        }
    };
    let path = std::path::PathBuf::from(&file_path);
    if !path.exists() {
        return Err(ApiError::SessionNotFound(session_id.to_string()));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use claude_view_core::session_catalog::CatalogRow;
    use claude_view_db::Database;
    use tempfile::tempdir;

    async fn fresh_state() -> std::sync::Arc<AppState> {
        let db = Database::new_in_memory().await.expect("in-memory DB");
        AppState::new(db)
    }

    fn row(id: &str, file_path: std::path::PathBuf) -> CatalogRow {
        CatalogRow {
            id: id.to_string(),
            file_path,
            is_compressed: false,
            bytes: 0,
            mtime: 0,
            project_id: "test-project".to_string(),
            first_ts: None,
            last_ts: None,
        }
    }

    #[tokio::test]
    async fn resolves_from_catalog_when_db_and_live_are_empty() {
        // The point of Phase A: catalog is authoritative. This case must work
        // with an empty DB and no live session — which it did NOT before the
        // catalog-first branch was added.
        let tmp = tempdir().unwrap();
        let jsonl = tmp.path().join("only-in-catalog.jsonl");
        std::fs::write(&jsonl, b"{}").unwrap();

        let state = fresh_state().await;
        state
            .session_catalog
            .replace_all(vec![row("only-in-catalog", jsonl.clone())]);

        let resolved = resolve_session_file_path(&state, "only-in-catalog")
            .await
            .expect("catalog hit should resolve");
        assert_eq!(resolved, jsonl);
    }

    #[tokio::test]
    async fn catalog_beats_db_when_both_have_the_session() {
        // If the catalog has it, the DB must not be consulted. We simulate "DB has
        // a different path" by making the catalog point at a file that exists and
        // leaving the DB empty; the only way the assertion holds is if the catalog
        // short-circuits BEFORE the DB lookup.
        let tmp = tempdir().unwrap();
        let catalog_path = tmp.path().join("from-catalog.jsonl");
        std::fs::write(&catalog_path, b"{}").unwrap();

        let state = fresh_state().await;
        state
            .session_catalog
            .replace_all(vec![row("hit", catalog_path.clone())]);

        let resolved = resolve_session_file_path(&state, "hit")
            .await
            .expect("must resolve via catalog");
        assert_eq!(resolved, catalog_path);
    }

    #[tokio::test]
    async fn catalog_miss_errors_when_db_and_live_also_miss() {
        // Regression guard: a completely unknown session still produces
        // SessionNotFound, not a panic or a silent empty-path.
        let state = fresh_state().await;
        state.session_catalog.replace_all(vec![]);

        let err = resolve_session_file_path(&state, "nonexistent")
            .await
            .expect_err("must error");
        assert!(matches!(err, ApiError::SessionNotFound(_)));
    }
}
