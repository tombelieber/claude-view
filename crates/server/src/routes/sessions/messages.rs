//! Session message endpoints: parsed, paginated messages, sub-agent messages, and rich data.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::Json;
use claude_view_core::accumulator::SessionAccumulator;
use claude_view_core::hook_to_block::make_hook_progress_block;
use claude_view_core::ParsedSession;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

use super::helpers::resolve_session_file_path;
use super::types::{PaginatedBlocks, SessionMessagesQuery};

/// GET /api/sessions/:id/parsed — Get full parsed session by ID.
///
/// Resolves the JSONL file path from the DB's `file_path` column.
/// No `project_dir` parameter needed — the server owns path resolution.
#[utoipa::path(get, path = "/api/sessions/{id}/parsed", tag = "sessions",
    params(("id" = String, Path, description = "Session ID")),
    responses(
        (status = 200, description = "Full parsed session messages", body = serde_json::Value),
        (status = 404, description = "Session not found"),
    )
)]
pub async fn get_session_parsed(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> ApiResult<Json<ParsedSession>> {
    let path = resolve_session_file_path(&state, &session_id).await?;
    let session = claude_view_core::parse_session(&path).await?;
    Ok(Json(session))
}

/// GET /api/sessions/:id/messages — Get paginated messages by session ID.
///
/// Resolves the JSONL file path from the DB's `file_path` column.
/// No `project_dir` parameter needed — the server owns path resolution.
#[utoipa::path(get, path = "/api/sessions/{id}/messages", tag = "sessions",
    params(
        ("id" = String, Path, description = "Session ID"),
        SessionMessagesQuery,
    ),
    responses(
        (status = 200, description = "Paginated session messages (block or legacy format)", body = serde_json::Value),
        (status = 404, description = "Session not found"),
    )
)]
pub async fn get_session_messages_by_id(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Query(query): Query<SessionMessagesQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let path = resolve_session_file_path(&state, &session_id).await?;

    if query.format.as_deref() == Some("block") {
        // Block format — use BlockAccumulator
        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| ApiError::Internal(format!("Read error: {e}")))?;

        let parsed = claude_view_core::block_accumulator::parse_session(&content);
        let mut blocks = parsed.blocks;

        // Merge DB hook events (Channel B) into the block list.
        // Channel A (JSONL hook_progress) and Channel B (DB hook_events) are
        // separate data sources with different schemas — both always render.
        // See CLAUDE.md: "Separate Channels = Separate Data = No Dedup".
        //
        // NOTE: The frontend must NOT also fire FETCH_HOOK_EVENTS on initial load,
        // or the same events will be duplicated (different ID prefixes: hook-db- vs hook-).
        // FETCH_HOOK_EVENTS is only needed after TURN_COMPLETE for live sessions
        // where hook events are in memory and not yet flushed to DB.
        match claude_view_db::hook_events_queries::get_hook_events(&state.db, &session_id).await {
            Ok(hook_rows) if !hook_rows.is_empty() => {
                let hook_blocks: Vec<_> = hook_rows
                    .iter()
                    .enumerate()
                    .map(|(i, row)| {
                        make_hook_progress_block(
                            format!("hook-db-{}-{}", row.timestamp, i),
                            row.timestamp as f64,
                            &row.event_name,
                            row.tool_name.as_deref(),
                            &row.label,
                        )
                    })
                    .collect();

                // Positional merge: insert hook blocks at correct timestamp
                // positions without disturbing existing block order.
                // hook_blocks are already sorted by ts ASC (DB ORDER BY).
                blocks = merge_hook_blocks_by_timestamp(blocks, hook_blocks);
            }
            Ok(_) => {} // no hook events in DB
            Err(e) => {
                tracing::warn!(
                    session_id = %session_id,
                    error = %e,
                    "Failed to fetch hook events for block merge — serving JSONL blocks only"
                );
            }
        }

        let total = blocks.len();
        let offset = query.offset.unwrap_or(0);
        let limit = query.limit.unwrap_or(50);
        let end = std::cmp::min(offset + limit, total);
        let page: Vec<_> = if offset < total {
            blocks.into_iter().skip(offset).take(limit).collect()
        } else {
            vec![]
        };

        let result = PaginatedBlocks {
            blocks: page,
            total,
            offset,
            limit,
            has_more: end < total,
            forked_from: parsed.forked_from,
            entrypoint: parsed.entrypoint,
        };
        Ok(Json(serde_json::to_value(result).unwrap()))
    } else {
        // Legacy format — existing behavior
        let limit = query.limit.unwrap_or(100);
        let offset = query.offset.unwrap_or(0);
        let result = if query.raw {
            claude_view_core::parse_session_paginated_with_raw(&path, limit, offset).await?
        } else {
            claude_view_core::parse_session_paginated(&path, limit, offset).await?
        };
        Ok(Json(serde_json::to_value(result).unwrap()))
    }
}
/// GET /api/sessions/:id/subagents/:agent_id/messages — Paginated blocks for a sub-agent.
///
/// Resolves the parent session's JSONL path, then derives the sub-agent's JSONL
/// file using the same path convention as the terminal WebSocket handler.
/// Returns `PaginatedBlocks` — same shape as the parent messages endpoint.
#[utoipa::path(get, path = "/api/sessions/{id}/subagents/{agent_id}/messages", tag = "sessions",
    params(
        ("id" = String, Path, description = "Parent session ID"),
        ("agent_id" = String, Path, description = "Sub-agent ID (alphanumeric)"),
        SessionMessagesQuery,
    ),
    responses(
        (status = 200, description = "Paginated sub-agent blocks", body = serde_json::Value),
        (status = 400, description = "Invalid agent ID"),
        (status = 404, description = "Session or sub-agent not found"),
    )
)]
pub async fn get_subagent_messages(
    State(state): State<Arc<AppState>>,
    Path((session_id, agent_id)): Path<(String, String)>,
    Query(query): Query<SessionMessagesQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    // Validate agent_id (same check as terminal WS handler)
    if agent_id.is_empty()
        || !agent_id.chars().all(|c| c.is_ascii_alphanumeric())
        || agent_id.len() > 64
    {
        return Err(ApiError::BadRequest(format!(
            "Invalid agent ID: '{}'",
            agent_id
        )));
    }

    // Resolve parent → sub-agent JSONL path
    let parent_path = resolve_session_file_path(&state, &session_id).await?;
    let subagent_path = crate::live::subagent_file::resolve_subagent_path(&parent_path, &agent_id);

    if !subagent_path.exists() {
        return Err(ApiError::NotFound(format!(
            "Sub-agent '{}' JSONL not found for session '{}'",
            agent_id, session_id
        )));
    }

    // Parse JSONL → blocks → paginate
    let content = tokio::fs::read_to_string(&subagent_path)
        .await
        .map_err(|e| ApiError::Internal(format!("Read error: {e}")))?;

    let parsed = claude_view_core::block_accumulator::parse_session(&content);
    let blocks = parsed.blocks;

    let total = blocks.len();
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(50);
    let end = std::cmp::min(offset + limit, total);
    let page: Vec<_> = if offset < total {
        blocks.into_iter().skip(offset).take(limit).collect()
    } else {
        vec![]
    };

    let result = PaginatedBlocks {
        blocks: page,
        total,
        offset,
        limit,
        has_more: end < total,
        forked_from: parsed.forked_from,
        entrypoint: parsed.entrypoint,
    };
    Ok(Json(serde_json::to_value(result).unwrap()))
}

/// GET /api/sessions/:id/rich — Parse JSONL on demand via `SessionAccumulator` and return
/// rich session data (tokens, cost, cache status, sub-agents, progress items, etc.).
///
/// This endpoint bridges historical sessions with the same rich data shape used by
/// Live Monitor, enabling a unified session detail view.
#[utoipa::path(get, path = "/api/sessions/{id}/rich", tag = "sessions",
    params(("id" = String, Path, description = "Session ID")),
    responses(
        (status = 200, description = "Rich parsed session data with tokens, cost, and sub-agents", body = serde_json::Value),
        (status = 404, description = "Session not found"),
    )
)]
pub async fn get_session_rich(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> ApiResult<Json<claude_view_core::accumulator::RichSessionData>> {
    // 1. Resolve JSONL file path (DB → live session fallback)
    let path = resolve_session_file_path(&state, &session_id).await?;

    // 2. Arc-clone the pricing table (cheap, no lock needed — pricing is immutable)
    let pricing = state.pricing.clone();

    // 3. Parse JSONL through SessionAccumulator (blocking I/O → spawn_blocking)
    let mut rich_data =
        tokio::task::spawn_blocking(move || SessionAccumulator::from_file(&path, &pricing))
            .await
            .map_err(|e| ApiError::Internal(format!("Join error: {e}")))?
            .map_err(|e| ApiError::Internal(format!("Parse error: {e}")))?;

    // Historical sessions are loaded from file — any subagent still marked
    // Running can't actually be running (the parent session is over).
    // Mark them as Error so the UI doesn't show a green "running" dot.
    crate::live::mutation::apply_lifecycle::finalize_orphaned_subagents(
        &mut rich_data.sub_agents,
        chrono::Utc::now().timestamp(),
    );

    Ok(Json(rich_data))
}

// ============================================================================
// Block merge helpers
// ============================================================================

/// Extract timestamp from a ConversationBlock, if available.
///
/// Only Progress, User, and Assistant blocks carry timestamps.
/// Other variants (Interaction, TurnBoundary, Notice, System) have no
/// timestamp field — returns None so the merge algorithm skips them
/// as insertion points rather than misplacing them.
fn block_timestamp(block: &claude_view_core::ConversationBlock) -> Option<f64> {
    use claude_view_core::ConversationBlock;
    match block {
        ConversationBlock::Progress(b) => Some(b.ts),
        ConversationBlock::User(b) => Some(b.timestamp),
        ConversationBlock::Assistant(b) => b.timestamp,
        ConversationBlock::Interaction(_)
        | ConversationBlock::TurnBoundary(_)
        | ConversationBlock::Notice(_)
        | ConversationBlock::System(_)
        | ConversationBlock::TeamTranscript(_) => None,
    }
}

/// Merge hook ProgressBlocks into an existing block list by timestamp.
///
/// Uses a stable sort on the combined list so that hook events (Channel B)
/// interleave correctly even when assistant block timestamps (= API message
/// creation time) are earlier than the hook events that fire during that
/// turn's tool execution.
///
/// Blocks without a timestamp (TurnBoundary, Notice, System) keep their
/// original relative position via the `original_index` tie-breaker.
pub(super) fn merge_hook_blocks_by_timestamp(
    blocks: Vec<claude_view_core::ConversationBlock>,
    hook_blocks: Vec<claude_view_core::ConversationBlock>,
) -> Vec<claude_view_core::ConversationBlock> {
    if hook_blocks.is_empty() {
        return blocks;
    }

    let blocks_len = blocks.len();
    let total = blocks_len + hook_blocks.len();
    let mut merged: Vec<(f64, usize, claude_view_core::ConversationBlock)> =
        Vec::with_capacity(total);

    // Original blocks get indices [0..blocks_len) — preserves relative order
    // for blocks with no timestamp (system, turn_boundary, notice).
    for (i, block) in blocks.into_iter().enumerate() {
        let ts = block_timestamp(&block).unwrap_or(0.0);
        merged.push((ts, i, block));
    }

    // Hook blocks get indices [blocks_len..total) — sorts them after
    // any original block at the exact same timestamp.
    for (i, block) in hook_blocks.into_iter().enumerate() {
        let ts = block_timestamp(&block).unwrap_or(0.0);
        merged.push((ts, blocks_len + i, block));
    }

    // Stable sort: primary key = timestamp, secondary = original_index.
    // f64 comparison: 0.0 timestamps (no-ts blocks) sort to the front,
    // which is correct — system/notice blocks appear before content.
    merged.sort_by(|a, b| {
        a.0.partial_cmp(&b.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.1.cmp(&b.1))
    });

    merged.into_iter().map(|(_, _, block)| block).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use claude_view_core::hook_to_block::make_hook_progress_block;

    #[test]
    fn merge_hook_blocks_preserves_order_and_inserts_by_timestamp() {
        // Create some blocks with timestamps
        let user1 = claude_view_core::ConversationBlock::User(claude_view_core::UserBlock {
            id: "user-1".into(),
            text: "hello".into(),
            timestamp: 100.0,
            status: None,
            local_id: None,
            pending: None,
            permission_mode: None,
            parent_uuid: None,
            is_sidechain: None,
            agent_id: None,
            images: vec![],
            raw_json: None,
        });
        let user2 = claude_view_core::ConversationBlock::User(claude_view_core::UserBlock {
            id: "user-2".into(),
            text: "world".into(),
            timestamp: 300.0,
            status: None,
            local_id: None,
            pending: None,
            permission_mode: None,
            parent_uuid: None,
            is_sidechain: None,
            agent_id: None,
            images: vec![],
            raw_json: None,
        });

        let hook1 =
            make_hook_progress_block("hook-db-50-0".into(), 50.0, "Start", None, "Starting");
        let hook2 = make_hook_progress_block(
            "hook-db-200-1".into(),
            200.0,
            "PreToolUse",
            Some("Bash"),
            "Running",
        );
        let hook3 = make_hook_progress_block("hook-db-400-2".into(), 400.0, "Stop", None, "Done");

        let blocks = vec![user1, user2];
        let hook_blocks = vec![hook1, hook2, hook3];

        let merged = merge_hook_blocks_by_timestamp(blocks, hook_blocks);

        // Expected order: hook@50, user@100, hook@200, user@300, hook@400
        assert_eq!(merged.len(), 5);
        assert_eq!(merged[0].id(), "hook-db-50-0");
        assert_eq!(merged[1].id(), "user-1");
        assert_eq!(merged[2].id(), "hook-db-200-1");
        assert_eq!(merged[3].id(), "user-2");
        assert_eq!(merged[4].id(), "hook-db-400-2");
    }

    /// Regression: hook events from tool execution have timestamps LATER than
    /// the assistant block that started the turn (assistant ts = API message
    /// creation time; hook ts = actual tool execution time).
    ///
    /// The old positional-insertion merge placed hooks with ts=318 before the
    /// assistant at ts=217, creating 100+ second backward time jumps.
    /// The sort-based merge places everything in strict chronological order.
    #[test]
    fn merge_hook_blocks_handles_hooks_during_assistant_tool_execution() {
        // Assistant starts a long-running turn at ts=100
        let assistant =
            claude_view_core::ConversationBlock::Assistant(claude_view_core::AssistantBlock {
                id: "asst-1".into(),
                segments: vec![],
                thinking: None,
                streaming: false,
                timestamp: Some(100.0),
                parent_uuid: None,
                is_sidechain: None,
                agent_id: None,
                raw_json: None,
            });
        // User's tool_result comes back
        let user_result = claude_view_core::ConversationBlock::User(claude_view_core::UserBlock {
            id: "user-result".into(),
            text: "tool output".into(),
            timestamp: 105.0,
            status: None,
            local_id: None,
            pending: None,
            permission_mode: None,
            parent_uuid: None,
            is_sidechain: None,
            agent_id: None,
            images: vec![],
            raw_json: None,
        });
        // Next assistant message
        let assistant2 =
            claude_view_core::ConversationBlock::Assistant(claude_view_core::AssistantBlock {
                id: "asst-2".into(),
                segments: vec![],
                thinking: None,
                streaming: false,
                timestamp: Some(200.0),
                parent_uuid: None,
                is_sidechain: None,
                agent_id: None,
                raw_json: None,
            });

        // Hook events fire DURING the assistant's tool execution:
        // PreToolUse at ts=102 (before tool_result at 105 — no issue)
        // PostToolUse at ts=150 (AFTER assistant ts=100 — was the bug)
        // PreToolUse at ts=160 (second tool call)
        // PostToolUse at ts=190 (still before asst-2 at 200)
        let hook_pre1 = make_hook_progress_block(
            "hook-102".into(),
            102.0,
            "PreToolUse",
            Some("Bash"),
            "Running git status",
        );
        let hook_post1 = make_hook_progress_block(
            "hook-150".into(),
            150.0,
            "PostToolUse",
            Some("Bash"),
            "Completed",
        );
        let hook_pre2 = make_hook_progress_block(
            "hook-160".into(),
            160.0,
            "PreToolUse",
            Some("Agent"),
            "Spawning subagent",
        );
        let hook_post2 = make_hook_progress_block(
            "hook-190".into(),
            190.0,
            "PostToolUse",
            Some("Agent"),
            "Agent done",
        );

        let blocks = vec![assistant, user_result, assistant2];
        let hook_blocks = vec![hook_pre1, hook_post1, hook_pre2, hook_post2];

        let merged = merge_hook_blocks_by_timestamp(blocks, hook_blocks);

        // Strict chronological order — no backward time jumps
        assert_eq!(merged.len(), 7);
        assert_eq!(merged[0].id(), "asst-1"); // ts=100
        assert_eq!(merged[1].id(), "hook-102"); // ts=102
        assert_eq!(merged[2].id(), "user-result"); // ts=105
        assert_eq!(merged[3].id(), "hook-150"); // ts=150
        assert_eq!(merged[4].id(), "hook-160"); // ts=160
        assert_eq!(merged[5].id(), "hook-190"); // ts=190
        assert_eq!(merged[6].id(), "asst-2"); // ts=200

        // Verify no timestamp goes backwards
        let mut prev_ts = 0.0f64;
        for block in &merged {
            if let Some(ts) = block_timestamp(block) {
                assert!(
                    ts >= prev_ts,
                    "Timestamp went backwards: {} < {} at block {}",
                    ts,
                    prev_ts,
                    block.id()
                );
                prev_ts = ts;
            }
        }
    }
}
