// crates/providers/src/parsers/zed/mod.rs
//
// Zed editor agent threads — one shared SQLite DB at
// `<root>/threads/threads.db` (root = `~/Library/Application Support/Zed`
// on macOS, `~/.local/share/zed` elsewhere).
//
// Schema (from the agentsview census, verified against fixtures):
//   threads(id TEXT PK, summary, updated_at, data_type, data BLOB,
//           parent_id, folder_paths, created_at)
// `data` is a JSON doc, zstd-compressed when data_type='zstd' (''|'json' →
// raw). Top-level threads only (`COALESCE(parent_id,'')=''`); child threads
// are continuations and are excluded, matching the Go source.
//
// Doc shape:
//   { model: { model },
//     request_token_usage: { <req-id>: { input_tokens, output_tokens } },
//     messages: [ {User:{content}} | {Agent:{content, tool_results}} ] }
// Content blocks are Rust-enum-style keyed objects — 'Text' (string or
// {text}), 'Thinking':{text}, 'ToolUse':{id,name,input|raw_input} — found
// via a recursive walk over arbitrary nesting (ported from zedWalk).
// tool_results is a map keyed by tool_use_id with an 'output' string or a
// 'content' array of Text blocks.
//
// Discovery emits virtual paths `<dbPath>#<threadId>` (mtime = updated_at,
// size 0); parse accepts either a virtual path or the bare DB path. The DB
// is opened read-only with a 3 s busy timeout because Zed holds it live.
//
// Module layout: `db` (SQLite access), `doc` (thread doc → session),
// `walk` (recursive content-block extraction).

mod db;
mod doc;
mod walk;

#[cfg(test)]
mod tests;

use crate::discover::{split_virtual_path, DiscoveredSession, Provider};
use crate::kind::ProviderKind;
use crate::model::ForeignSession;
use crate::util::time;
use std::path::Path;

pub struct ZedProvider;

impl Provider for ZedProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Zed
    }

    fn discover(&self, root: &Path) -> Vec<DiscoveredSession> {
        let db_path = root.join("threads").join("threads.db");
        if !db_path.is_file() {
            return Vec::new();
        }
        let Ok(conn) = db::open_read_only(&db_path) else {
            return Vec::new();
        };
        db::list_thread_metas(&conn)
            .into_iter()
            .map(|(id, updated_at)| DiscoveredSession {
                id: ProviderKind::Zed.session_id(&id),
                provider: ProviderKind::Zed,
                path: db::virtual_path(&db_path, &id),
                project_hint: None,
                mtime: time::parse_timestamp(&updated_at, false).unwrap_or(0.0),
                size_bytes: 0,
            })
            .collect()
    }

    fn parse(&self, path: &Path) -> anyhow::Result<Vec<ForeignSession>> {
        let (db_path, raw_id) = match split_virtual_path(path) {
            Some((db_path, id)) => (db_path, Some(id)),
            None => (path.to_path_buf(), None),
        };
        if let Some(id) = &raw_id {
            if !db::is_valid_session_id(id) {
                anyhow::bail!("invalid Zed thread id: {id}");
            }
        }
        let conn = db::open_read_only(&db_path)?;
        let rows = db::load_threads(&conn, raw_id.as_deref())?;
        Ok(rows
            .into_iter()
            .filter_map(|row| doc::build_session(row, &db_path))
            .collect())
    }
}
