// crates/providers/src/parsers/kiro_ide/exec_log.rs
//
// New-format execution logs: `<root>/<ws-hash>/414d…63e9/<file>` JSON docs
// holding the assistant's actions for one executionId. The workspace path
// (hashed into <ws-hash>) comes from the sibling sessions.json index; the
// executionId is sniffed from each file's first 1 KiB so indexing stays
// cheap even for multi-megabyte logs.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Exec-log dir for a session JSON:
/// `<root>/<hex(sha256(ws))[:32]>/414d1636299d2b9e4ce7e17fb11f63e9/`.
pub(super) fn dir_for(session_path: &Path) -> Option<PathBuf> {
    let ws_session_dir = session_path.parent()?;
    let ws_path = super::first_workspace_dir(&ws_session_dir.join("sessions.json"))?;
    let root = ws_session_dir.parent()?.parent()?;
    Some(root.join(super::hash32(&ws_path)).join(super::EXEC_SUBDIR))
}

/// Map executionId → exec-log file for every log in the dir.
pub(super) fn build_index(dir: PathBuf) -> HashMap<String, PathBuf> {
    let mut idx = HashMap::new();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return idx;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if path.is_dir() || name.starts_with('.') {
            continue;
        }
        if let Some(exec_id) = sniff_execution_id(&path) {
            idx.insert(exec_id, path);
        }
    }
    idx
}

/// Extract the `"executionId"` value from the first 1 KiB of a file without
/// parsing the (potentially huge) JSON document.
pub(super) fn sniff_execution_id(path: &Path) -> Option<String> {
    use std::io::Read;
    let mut f = std::fs::File::open(path).ok()?;
    let mut buf = [0u8; 1024];
    let n = f.read(&mut buf).ok()?;
    if n == 0 {
        return None;
    }
    let head = String::from_utf8_lossy(&buf[..n]).into_owned();
    let i = head.find("\"executionId\"")?;
    let rest = &head[i + "\"executionId\"".len()..];
    let open = rest.find('"')?; // skips `: `
    let rest = &rest[open + 1..];
    let close = rest.find('"')?;
    let id = &rest[..close];
    (!id.is_empty()).then(|| id.to_string())
}
