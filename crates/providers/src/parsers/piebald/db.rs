// crates/providers/src/parsers/piebald/db.rs
//
// Read-only access to Piebald's app.db. Columns/joins mirror the Go source
// exactly; the per-part detail lookups are deliberately collapsed into one
// query per table per chat, grouped in maps (the Go version is N+1).

use rusqlite::{Connection, OpenFlags};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Open read-only with a short busy timeout — the app may be writing.
/// No WAL/journal pragmas: a read-only connection must not touch the mode.
pub(super) fn open_ro(db: &Path) -> anyhow::Result<Connection> {
    let conn = Connection::open_with_flags(
        db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.busy_timeout(Duration::from_millis(3000))?;
    Ok(conn)
}

/// Virtual session path `<db>#<chatID>`.
pub(super) fn virtual_path(db: &Path, chat_id: i64) -> PathBuf {
    PathBuf::from(format!("{}#{chat_id}", db.display()))
}

/// Lightweight `(chat id, updated_at)` listing for discovery: live chats
/// with at least one message.
pub(super) fn list_chat_stubs(conn: &Connection) -> rusqlite::Result<Vec<(i64, String)>> {
    let mut stmt = conn.prepare(
        "SELECT id, COALESCE(updated_at, created_at, '') FROM chats \
         WHERE COALESCE(is_deleted, 0) = 0 AND message_count > 0",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
    rows.collect()
}

pub(super) struct ChatRow {
    pub(super) id: i64,
    pub(super) title: String,
    pub(super) created_at: String,
    pub(super) updated_at: String,
    pub(super) current_directory: String,
    pub(super) worktree_path: String,
    pub(super) branch_name: String,
    pub(super) project_directory: String,
    pub(super) project_name: String,
}

/// Load one chat (by id, deleted excluded) or every live contentful chat.
pub(super) fn load_chats(conn: &Connection, chat_id: Option<i64>) -> anyhow::Result<Vec<ChatRow>> {
    const COLS: &str = "SELECT c.id, COALESCE(c.title, ''), COALESCE(c.created_at, ''), \
         COALESCE(c.updated_at, c.created_at, ''), COALESCE(c.current_directory, ''), \
         COALESCE(c.worktree_path, ''), COALESCE(c.branch_name, ''), \
         COALESCE(p.directory, ''), COALESCE(p.name, '') \
         FROM chats c LEFT JOIN projects p ON p.id = c.project_id";
    let map = |r: &rusqlite::Row<'_>| -> rusqlite::Result<ChatRow> {
        Ok(ChatRow {
            id: r.get(0)?,
            title: r.get(1)?,
            created_at: r.get(2)?,
            updated_at: r.get(3)?,
            current_directory: r.get(4)?,
            worktree_path: r.get(5)?,
            branch_name: r.get(6)?,
            project_directory: r.get(7)?,
            project_name: r.get(8)?,
        })
    };
    let mut out = Vec::new();
    match chat_id {
        Some(id) => {
            let mut stmt = conn.prepare(&format!(
                "{COLS} WHERE c.id = ?1 AND COALESCE(c.is_deleted, 0) = 0"
            ))?;
            for row in stmt.query_map([id], map)? {
                out.push(row?);
            }
        }
        None => {
            let mut stmt = conn.prepare(&format!(
                "{COLS} WHERE COALESCE(c.is_deleted, 0) = 0 AND c.message_count > 0 \
                 ORDER BY COALESCE(c.updated_at, c.created_at)"
            ))?;
            for row in stmt.query_map([], map)? {
                out.push(row?);
            }
        }
    }
    Ok(out)
}

pub(super) struct MsgRow {
    pub(super) id: i64,
    pub(super) parent_message_id: Option<i64>,
    pub(super) enabled: bool,
    pub(super) role: String,
    pub(super) model: String,
    pub(super) created_at: String,
    /// Token columns are nullable: NULL means ABSENT, not zero.
    pub(super) input_tokens: Option<i64>,
    pub(super) output_tokens: Option<i64>,
    pub(super) reasoning_tokens: Option<i64>,
    pub(super) cache_read_tokens: Option<i64>,
    pub(super) cache_write_tokens: Option<i64>,
}

pub(super) fn load_messages(conn: &Connection, chat_id: i64) -> anyhow::Result<Vec<MsgRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, parent_message_id, enabled, COALESCE(role, ''), COALESCE(model, ''), \
         COALESCE(created_at, ''), input_tokens, output_tokens, reasoning_tokens, \
         cache_read_tokens, cache_write_tokens \
         FROM messages WHERE parent_chat_id = ?1 ORDER BY created_at, id",
    )?;
    let rows = stmt.query_map([chat_id], |r| {
        Ok(MsgRow {
            id: r.get(0)?,
            parent_message_id: r.get(1)?,
            enabled: r.get::<_, Option<i64>>(2)?.unwrap_or(1) != 0,
            role: r.get(3)?,
            model: r.get(4)?,
            created_at: r.get(5)?,
            input_tokens: r.get(6)?,
            output_tokens: r.get(7)?,
            reasoning_tokens: r.get(8)?,
            cache_read_tokens: r.get(9)?,
            cache_write_tokens: r.get(10)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub(super) struct ToolCallRow {
    pub(super) tool_use_id: String,
    pub(super) tool_name: String,
    pub(super) tool_input: String,
    pub(super) tool_result: Option<String>,
    pub(super) tool_error: Option<String>,
    pub(super) tool_state: String,
}

/// All part-level detail for one chat, batched and grouped.
#[derive(Default)]
pub(super) struct ChatParts {
    /// message id → ordered `(part id, part_type)` rows.
    pub(super) by_message: HashMap<i64, Vec<(i64, String)>>,
    /// text part id → is_thinking. Row presence = the detail row exists.
    pub(super) thinking: HashMap<i64, bool>,
    /// text part id → content (chunks concatenated with NO separator).
    pub(super) text: HashMap<i64, String>,
    /// tool part id → tool call detail. sub_agent_chat_id is deliberately
    /// not selected (no subagent-link field in the normalized model).
    pub(super) tools: HashMap<i64, ToolCallRow>,
}

pub(super) fn load_parts(conn: &Connection, chat_id: i64) -> anyhow::Result<ChatParts> {
    let mut parts = ChatParts::default();

    let mut stmt = conn.prepare(
        "SELECT mp.parent_chat_message_id, mp.id, COALESCE(mp.part_type, '') \
         FROM message_parts mp JOIN messages m ON m.id = mp.parent_chat_message_id \
         WHERE m.parent_chat_id = ?1 ORDER BY mp.part_index, mp.id",
    )?;
    let rows = stmt.query_map([chat_id], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, String>(2)?,
        ))
    })?;
    for row in rows {
        let (msg_id, part_id, part_type) = row?;
        parts
            .by_message
            .entry(msg_id)
            .or_default()
            .push((part_id, part_type));
    }

    let mut stmt = conn.prepare(
        "SELECT mpt.message_part_id, COALESCE(mpt.is_thinking, 0) \
         FROM message_part_text mpt \
         JOIN message_parts mp ON mp.id = mpt.message_part_id \
         JOIN messages m ON m.id = mp.parent_chat_message_id \
         WHERE m.parent_chat_id = ?1",
    )?;
    let rows = stmt.query_map([chat_id], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, bool>(1)?))
    })?;
    for row in rows {
        let (part_id, is_thinking) = row?;
        parts.thinking.insert(part_id, is_thinking);
    }

    // Text content is CHUNKED across node rows: concatenate in
    // (node_index, id) order with NO separator.
    let mut stmt = conn.prepare(
        "SELECT mcn.parent_text_part_id, COALESCE(mnt.content, '') \
         FROM message_content_nodes mcn \
         JOIN message_node_text mnt ON mnt.node_id = mcn.id \
         JOIN message_parts mp ON mp.id = mcn.parent_text_part_id \
         JOIN messages m ON m.id = mp.parent_chat_message_id \
         WHERE m.parent_chat_id = ?1 AND mcn.node_type = 'text' \
         ORDER BY mcn.parent_text_part_id, mcn.node_index, mcn.id",
    )?;
    let rows = stmt.query_map([chat_id], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (part_id, chunk) = row?;
        parts.text.entry(part_id).or_default().push_str(&chunk);
    }

    let mut stmt = conn.prepare(
        "SELECT mptc.message_part_id, COALESCE(mptc.provider_tool_use_id, ''), \
         COALESCE(mptc.tool_name, ''), COALESCE(mptc.tool_input, ''), \
         mptc.tool_result, mptc.tool_error, COALESCE(mptc.tool_state, '') \
         FROM message_part_tool_call mptc \
         JOIN message_parts mp ON mp.id = mptc.message_part_id \
         JOIN messages m ON m.id = mp.parent_chat_message_id \
         WHERE m.parent_chat_id = ?1",
    )?;
    let rows = stmt.query_map([chat_id], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            ToolCallRow {
                tool_use_id: r.get(1)?,
                tool_name: r.get(2)?,
                tool_input: r.get(3)?,
                tool_result: r.get(4)?,
                tool_error: r.get(5)?,
                tool_state: r.get(6)?,
            },
        ))
    })?;
    for row in rows {
        let (part_id, tc) = row?;
        parts.tools.insert(part_id, tc);
    }

    Ok(parts)
}
