// crates/providers/src/parsers/opencode/sqlite.rs
//
// Legacy SQLite backend (<root>/opencode.db): project/session/message/part
// tables with JSON `data` columns and epoch-ms time columns. Sessions are
// addressed by virtual path `<db>#<sessionID>`. The DB is opened read-only
// with a 3s busy timeout (a live OpenCode may hold a write lock).

use super::build::{self, MessageRow, PartRow, SessionRow};
use crate::model::ForeignSession;
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

fn open_db(path: &Path) -> anyhow::Result<Connection> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.busy_timeout(Duration::from_millis(3000))?;
    Ok(conn)
}

/// Discovery-grade listing: (raw session id, time_updated epoch-ms). Reads
/// only the session table — no transcript parsing.
pub(super) fn list_sessions(db_path: &Path) -> anyhow::Result<Vec<(String, i64)>> {
    let conn = open_db(db_path)?;
    let mut stmt = conn.prepare("SELECT id, time_updated FROM session")?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub(super) fn parse_session(
    db_path: &Path,
    raw_id: &str,
    source_path: &Path,
) -> anyhow::Result<Vec<ForeignSession>> {
    let conn = open_db(db_path)?;
    let (project_id, title, created_ms, updated_ms) = conn.query_row(
        "SELECT project_id, COALESCE(title, ''), time_created, time_updated
         FROM session WHERE id = ?1",
        [raw_id],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
            ))
        },
    )?;
    let worktree: String = conn
        .query_row(
            "SELECT worktree FROM project WHERE id = ?1",
            [&project_id],
            |r| r.get(0),
        )
        .unwrap_or_default();

    let mut malformed = 0u32;
    let msgs = load_messages(&conn, raw_id, &mut malformed)?;
    let parts = load_parts(&conn, raw_id, &mut malformed)?;
    let row = SessionRow {
        id: raw_id.to_string(),
        title,
        directory: worktree,
        created_ms,
        updated_ms,
    };
    Ok(
        build::build_session(row, source_path.to_path_buf(), msgs, parts, malformed)
            .into_iter()
            .collect(),
    )
}

fn load_messages(
    conn: &Connection,
    session_id: &str,
    malformed: &mut u32,
) -> anyhow::Result<Vec<MessageRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, data, time_created FROM message
         WHERE session_id = ?1 ORDER BY time_created",
    )?;
    let mut rows = stmt.query([session_id])?;
    let mut msgs = Vec::new();
    while let Some(r) = rows.next()? {
        let id: String = r.get(0)?;
        let data: String = r.get(1)?;
        let sort_time_ms: i64 = r.get(2)?;
        match serde_json::from_str::<Value>(&data) {
            Ok(data) => msgs.push(MessageRow {
                id,
                data,
                sort_time_ms,
            }),
            Err(_) => *malformed += 1,
        }
    }
    Ok(msgs)
}

fn load_parts(
    conn: &Connection,
    session_id: &str,
    malformed: &mut u32,
) -> anyhow::Result<HashMap<String, Vec<PartRow>>> {
    let mut stmt = conn.prepare(
        "SELECT id, message_id, COALESCE(data, '{}'), time_created FROM part
         WHERE session_id = ?1 ORDER BY time_created",
    )?;
    let mut rows = stmt.query([session_id])?;
    let mut parts: HashMap<String, Vec<PartRow>> = HashMap::new();
    while let Some(r) = rows.next()? {
        let id: String = r.get(0)?;
        let message_id: String = r.get(1)?;
        let data: String = r.get(2)?;
        let sort_time_ms: i64 = r.get(3)?;
        match serde_json::from_str::<Value>(&data) {
            Ok(data) => parts.entry(message_id).or_default().push(PartRow {
                id,
                data,
                sort_time_ms,
            }),
            Err(_) => *malformed += 1,
        }
    }
    Ok(parts)
}
