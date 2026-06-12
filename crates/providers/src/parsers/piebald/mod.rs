// crates/providers/src/parsers/piebald/mod.rs
//
// Piebald — desktop agent app storing chats in a normalized SQLite DB at
// `<root>/app.db` (macOS root: ~/Library/Application Support/piebald).
//
// Schema (ported from agentsview's piebald.go, verified against its
// fixtures): chats → messages → message_parts, with per-part detail in
// message_part_text (+ message_content_nodes → message_node_text, text
// CHUNKED across node rows) and message_part_tool_call. See db.rs for the
// exact columns and joins.
//
// Messages form a DAG via parent_message_id (edits/retries create sibling
// children). With a single root, the MAIN path follows the last ENABLED
// child at each branch point; every bypassed child spawns a fork child
// session with raw id `<chatID>-<firstRowID>` (recursive, so nested forks
// all surface). Multiple roots → one flat session. See branches.rs.
//
// Deliberate deviations from the Go source:
// - Part/text/tool detail is BATCHED (one query per table per chat) instead
//   of Go's per-part N+1 lookups.
// - Fork parent/relationship metadata (ParentSessionID/RelFork) is dropped:
//   ForeignSessionMeta has no such fields; forks surface as plain sessions.
// - sub_agent_chat_id is ignored (no subagent-link field in the model).
// - A part missing its detail row is skipped + counted in malformed_lines
//   instead of failing the whole chat.

mod branches;
mod db;
mod message;
#[cfg(test)]
mod tests;

use crate::discover::{split_virtual_path, DiscoveredSession, Provider};
use crate::kind::ProviderKind;
use crate::model::{ForeignSession, ForeignSessionMeta};
use crate::util::{project_from_cwd, time};
use claude_view_types::block_types::ConversationBlock;
use rusqlite::Connection;
use std::path::Path;

pub struct PiebaldProvider;

impl Provider for PiebaldProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Piebald
    }

    fn discover(&self, root: &Path) -> Vec<DiscoveredSession> {
        let db_path = root.join("app.db");
        if !db_path.is_file() {
            return Vec::new();
        }
        let Ok(conn) = db::open_ro(&db_path) else {
            return Vec::new();
        };
        let stubs = match db::list_chat_stubs(&conn) {
            Ok(stubs) => stubs,
            Err(e) => {
                tracing::debug!(db = %db_path.display(), error = %e, "piebald discovery failed");
                return Vec::new();
            }
        };
        stubs
            .into_iter()
            .map(|(chat_id, updated_at)| DiscoveredSession {
                id: ProviderKind::Piebald.session_id(&chat_id.to_string()),
                provider: ProviderKind::Piebald,
                path: db::virtual_path(&db_path, chat_id),
                project_hint: None,
                // The catalog fingerprints virtual paths with discovery-time
                // data: updated_at is the per-chat change signal (mirrors the
                // Go FileMtime); a chat inside the shared DB file has no
                // meaningful byte size of its own.
                mtime: time::parse_timestamp(&updated_at, false).unwrap_or(0.0),
                size_bytes: 0,
            })
            .collect()
    }

    fn parse(&self, path: &Path) -> anyhow::Result<Vec<ForeignSession>> {
        let (db_path, chat_filter) = match split_virtual_path(path) {
            Some((db, raw)) => {
                let id: i64 = raw
                    .parse()
                    .map_err(|_| anyhow::anyhow!("invalid piebald chat id {raw:?}"))?;
                (db, Some(id))
            }
            // A bare path is the DB itself: parse every chat (the Go
            // ParsePiebaldDB shape).
            None => (path.to_path_buf(), None),
        };
        let conn = db::open_ro(&db_path)?;
        let chats = db::load_chats(&conn, chat_filter)?;
        if chats.is_empty() {
            if let Some(id) = chat_filter {
                anyhow::bail!("piebald chat {id} not found in {}", db_path.display());
            }
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for chat in &chats {
            match build_chat_sessions(&conn, chat, &db_path) {
                Ok(mut sessions) => out.append(&mut sessions),
                // A whole-DB sweep tolerates one broken chat (Go parity);
                // a directly-requested chat propagates its error.
                Err(e) if chat_filter.is_none() => {
                    tracing::warn!(chat = chat.id, error = %e, "piebald chat failed; skipping");
                }
                Err(e) => return Err(e),
            }
        }
        Ok(out)
    }
}

/// One chat → its main session plus any fork branch sessions.
fn build_chat_sessions(
    conn: &Connection,
    chat: &db::ChatRow,
    db_path: &Path,
) -> anyhow::Result<Vec<ForeignSession>> {
    let msgs = db::load_messages(conn, chat.id)?;
    if msgs.is_empty() {
        return Ok(Vec::new());
    }
    let parts = db::load_parts(conn, chat.id)?;
    Ok(branches::split_branches(&msgs)
        .iter()
        .filter_map(|b| build_branch(chat, &msgs, &parts, b, db_path))
        .collect())
}

fn build_branch(
    chat: &db::ChatRow,
    msgs: &[db::MsgRow],
    parts: &db::ChatParts,
    branch: &branches::Branch,
    db_path: &Path,
) -> Option<ForeignSession> {
    let raw_id = match branch.first_row_id {
        None => chat.id.to_string(),
        Some(first) => format!("{}-{first}", chat.id),
    };
    // Forks share their chat's virtual path (Go parity).
    let mut meta = ForeignSessionMeta::new(
        ProviderKind::Piebald,
        &raw_id,
        db::virtual_path(db_path, chat.id),
    );
    meta.title = message::non_empty(&chat.title);
    meta.git_branch = message::non_empty(&chat.branch_name);
    let cwd = [
        chat.worktree_path.as_str(),
        chat.current_directory.as_str(),
        chat.project_directory.as_str(),
    ]
    .into_iter()
    .find(|s| !s.is_empty());
    meta.cwd = cwd.map(str::to_string);
    meta.project = if !chat.project_name.is_empty() {
        chat.project_name.clone()
    } else if !chat.project_directory.is_empty() {
        project_from_cwd(&chat.project_directory)
    } else if let Some(c) = cwd {
        project_from_cwd(c)
    } else {
        "piebald".to_string()
    };

    let mut out_blocks: Vec<ConversationBlock> = Vec::new();
    for &idx in &branch.rows {
        message::append_message(&mut out_blocks, &mut meta, &raw_id, &msgs[idx], parts);
    }
    // Sessions with zero contentful messages are non-interactive noise.
    if meta.message_count == 0 {
        return None;
    }
    // The chat-level envelope bounds the MAIN branch (Go uses the chat's
    // created/updated directly); forks span only their own messages.
    if branch.first_row_id.is_none() {
        for raw in [&chat.created_at, &chat.updated_at] {
            if let Some(ts) = time::parse_timestamp(raw, false) {
                meta.observe_timestamp(ts);
            }
        }
    }
    Some(ForeignSession {
        meta,
        blocks: out_blocks,
    })
}
