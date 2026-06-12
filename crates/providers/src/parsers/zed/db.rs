// crates/providers/src/parsers/zed/db.rs
//
// SQLite access for Zed's threads.db. Read-only with a 3 s busy timeout
// (Zed holds the DB live); top-level threads only.

use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub(super) struct ThreadRow {
    pub(super) id: String,
    pub(super) summary: String,
    pub(super) updated_at: String,
    pub(super) data_type: String,
    pub(super) data: Vec<u8>,
    pub(super) folder_paths: String,
    pub(super) created_at: String,
}

pub(super) fn open_read_only(db: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open_with_flags(
        db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.busy_timeout(Duration::from_millis(3000))?;
    Ok(conn)
}

/// Top-level thread ids + updated_at, cheap enough for discovery (no data
/// blobs are read).
pub(super) fn list_thread_metas(conn: &Connection) -> Vec<(String, String)> {
    let Ok(mut stmt) = conn.prepare(
        "SELECT id, COALESCE(updated_at, '') FROM threads \
         WHERE COALESCE(parent_id, '') = '' ORDER BY updated_at, id",
    ) else {
        return Vec::new();
    };
    let Ok(rows) = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
    else {
        return Vec::new();
    };
    rows.flatten()
        .filter(|(id, _)| is_valid_session_id(id))
        .collect()
}

/// Load top-level thread rows, optionally filtered to one id. Older Zed DBs
/// may lack the `folder_paths` column — fall back to selecting an empty
/// string so those parse without project/cwd info instead of erroring.
pub(super) fn load_threads(
    conn: &Connection,
    raw_id: Option<&str>,
) -> rusqlite::Result<Vec<ThreadRow>> {
    match load_threads_cols(conn, raw_id, "COALESCE(folder_paths, '')") {
        Ok(rows) => Ok(rows),
        Err(_) => load_threads_cols(conn, raw_id, "''"),
    }
}

fn load_threads_cols(
    conn: &Connection,
    raw_id: Option<&str>,
    folder_col: &str,
) -> rusqlite::Result<Vec<ThreadRow>> {
    let filter = if raw_id.is_some() { "AND id = ?1 " } else { "" };
    let sql = format!(
        "SELECT id, COALESCE(summary, ''), COALESCE(updated_at, ''), \
                COALESCE(data_type, ''), data, {folder_col}, \
                COALESCE(created_at, '') \
         FROM threads WHERE COALESCE(parent_id, '') = '' {filter}\
         ORDER BY updated_at, id"
    );
    let mut stmt = conn.prepare(&sql)?;
    let map_row = |r: &rusqlite::Row| -> rusqlite::Result<ThreadRow> {
        Ok(ThreadRow {
            id: r.get(0)?,
            summary: r.get(1)?,
            updated_at: r.get(2)?,
            data_type: r.get(3)?,
            data: r.get::<_, Option<Vec<u8>>>(4)?.unwrap_or_default(),
            folder_paths: r.get(5)?,
            created_at: r.get(6)?,
        })
    };
    let rows = match raw_id {
        Some(id) => stmt.query_map([id], map_row)?.flatten().collect(),
        None => stmt.query_map([], map_row)?.flatten().collect(),
    };
    Ok(rows)
}

pub(super) fn virtual_path(db: &Path, raw_id: &str) -> PathBuf {
    PathBuf::from(format!("{}#{}", db.display(), raw_id))
}

/// Alphanumeric / dash / underscore only — rejects path-like input.
pub(super) fn is_valid_session_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}
