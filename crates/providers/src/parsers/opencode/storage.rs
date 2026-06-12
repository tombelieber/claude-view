// crates/providers/src/parsers/opencode/storage.rs
//
// File-storage backend (current OpenCode): storage/session/<project>/<id>.json
// plus storage/message/<sessionID>/*.json and storage/part/<messageID>/*.json.
//
// Malformed child JSON aborts the parse (ported decision from agentsview:
// OpenCode session sync replaces the full stored transcript, so skipping a
// truncated mid-write child would silently drop previously persisted
// content until the next successful sync).

use super::build::{self, MessageRow, PartRow, SessionRow};
use crate::discover::stat_entry;
use crate::model::ForeignSession;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub(super) fn parse_session_file(path: &Path) -> anyhow::Result<Vec<ForeignSession>> {
    let raw = std::fs::read_to_string(path)?;
    let doc: Value = serde_json::from_str(&raw)
        .map_err(|e| anyhow::anyhow!("decoding opencode session file {}: {e}", path.display()))?;
    let Some(id) = doc
        .get("id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    else {
        anyhow::bail!("opencode session file {} missing id", path.display());
    };
    // <root>/storage/session/<project>/<id>.json → <root> is 4 levels up.
    let root = path
        .ancestors()
        .nth(4)
        .ok_or_else(|| anyhow::anyhow!("opencode session path too shallow: {}", path.display()))?;

    let msgs = load_messages(root, id)?;
    let parts = load_parts(root, &msgs)?;
    let row = SessionRow {
        id: id.to_string(),
        title: str_field(&doc, "title"),
        directory: str_field(&doc, "directory"),
        created_ms: build::as_ms(doc.pointer("/time/created")),
        updated_ms: build::as_ms(doc.pointer("/time/updated")),
    };
    Ok(
        build::build_session(row, path.to_path_buf(), msgs, parts, 0)
            .into_iter()
            .collect(),
    )
}

/// Composite (mtime, size) over the session file AND its message/part
/// children. New messages land as new files without touching the session
/// doc, so the session file's own stat would never invalidate the parse
/// cache (mirrors agentsview's openCodeStorageSessionMtime).
pub(super) fn composite_stat(session_path: &Path) -> Option<(f64, u64)> {
    let (mut mtime, mut size) = stat_entry(session_path)?;
    let Some(root) = session_path.ancestors().nth(4) else {
        return Some((mtime, size));
    };
    let Some(session_id) = session_path.file_stem().and_then(|s| s.to_str()) else {
        return Some((mtime, size));
    };
    let msg_dir = root.join("storage").join("message").join(session_id);
    if let Some((mt, _)) = stat_entry(&msg_dir) {
        mtime = mtime.max(mt);
    }
    let Ok(entries) = std::fs::read_dir(&msg_dir) else {
        return Some((mtime, size));
    };
    for path in entries.flatten().map(|e| e.path()) {
        if !is_json_file(&path) {
            continue;
        }
        if let Some((mt, sz)) = stat_entry(&path) {
            mtime = mtime.max(mt);
            size += sz;
        }
        let Some(message_id) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let part_dir = root.join("storage").join("part").join(message_id);
        if let Some((mt, _)) = stat_entry(&part_dir) {
            mtime = mtime.max(mt);
        }
        let Ok(part_entries) = std::fs::read_dir(&part_dir) else {
            continue;
        };
        for part_path in part_entries.flatten().map(|e| e.path()) {
            if !is_json_file(&part_path) {
                continue;
            }
            if let Some((mt, sz)) = stat_entry(&part_path) {
                mtime = mtime.max(mt);
                size += sz;
            }
        }
    }
    Some((mtime, size))
}

fn load_messages(root: &Path, session_id: &str) -> anyhow::Result<Vec<MessageRow>> {
    let dir = root.join("storage").join("message").join(session_id);
    let mut msgs = Vec::new();
    for path in json_files(&dir)? {
        let raw = std::fs::read_to_string(&path)?;
        let data: Value = serde_json::from_str(&raw).map_err(|e| {
            anyhow::anyhow!("decoding opencode message file {}: {e}", path.display())
        })?;
        let id = required_id(&data, &path, "message")?;
        let sort_time_ms = build::message_sort_time(&data);
        msgs.push(MessageRow {
            id,
            data,
            sort_time_ms,
        });
    }
    msgs.sort_by(|a, b| {
        a.sort_time_ms
            .cmp(&b.sort_time_ms)
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(msgs)
}

fn load_parts(root: &Path, msgs: &[MessageRow]) -> anyhow::Result<HashMap<String, Vec<PartRow>>> {
    let mut parts: HashMap<String, Vec<PartRow>> = HashMap::new();
    for msg in msgs {
        let dir = root.join("storage").join("part").join(&msg.id);
        for path in json_files(&dir)? {
            let raw = std::fs::read_to_string(&path)?;
            let data: Value = serde_json::from_str(&raw).map_err(|e| {
                anyhow::anyhow!("decoding opencode part file {}: {e}", path.display())
            })?;
            let id = required_id(&data, &path, "part")?;
            let sort_time_ms = build::part_sort_time(&data);
            parts.entry(msg.id.clone()).or_default().push(PartRow {
                id,
                data,
                sort_time_ms,
            });
        }
    }
    Ok(parts)
}

fn json_files(dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => {
            return Err(anyhow::anyhow!(
                "reading opencode dir {}: {e}",
                dir.display()
            ))
        }
    };
    Ok(entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| is_json_file(p))
        .collect())
}

fn is_json_file(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("json") && path.is_file()
}

fn required_id(data: &Value, path: &Path, kind: &str) -> anyhow::Result<String> {
    match data.get("id").and_then(Value::as_str) {
        Some(id) if !id.is_empty() => Ok(id.to_string()),
        _ => anyhow::bail!("opencode {kind} file {} missing id", path.display()),
    }
}

fn str_field(doc: &Value, key: &str) -> String {
    doc.get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}
