//! Filesystem + JSON access helpers shared across the workflow scanners.
//!
//! All artifact files are untrusted Claude-Code-owned input, so reads are
//! byte-capped and JSON field access is defensive (never panics).

use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::time::UNIX_EPOCH;

use serde_json::Value;

use super::MAX_READ_BYTES;

/// Read a file as UTF-8 (lossy), capped at `MAX_READ_BYTES`. Returns `None` on
/// any IO error. The cap bounds memory before downstream `.lines().take(..)` /
/// char truncation, so a pathologically large artifact cannot OOM the server.
pub(crate) fn read_text_capped(path: &Path) -> Option<String> {
    let file = File::open(path).ok()?;
    let mut buf = Vec::new();
    file.take(MAX_READ_BYTES as u64)
        .read_to_end(&mut buf)
        .ok()?;
    Some(String::from_utf8_lossy(&buf).into_owned())
}

pub(crate) fn json_string(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

pub(crate) fn json_u64(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(|v| {
        v.as_u64()
            .or_else(|| v.as_i64().and_then(|n| (n >= 0).then_some(n as u64)))
            .or_else(|| v.as_f64().and_then(|n| (n >= 0.0).then_some(n as u64)))
    })
}

pub(crate) fn json_i64(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(|v| {
        v.as_i64()
            .or_else(|| v.as_u64().and_then(|n| i64::try_from(n).ok()))
            .or_else(|| v.as_f64().map(|n| n as i64))
    })
}

pub(crate) fn file_modified_ms(path: &Path) -> Option<i64> {
    std::fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|mtime| mtime.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as i64)
}

/// True only when `path` exists and canonicalizes to a location inside `root`.
/// Canonicalization resolves symlinks, so this rejects symlink escapes too.
pub(crate) fn is_existing_path_under(root: &Path, path: &Path) -> bool {
    let (Ok(root), Ok(path)) = (root.canonicalize(), path.canonicalize()) else {
        return false;
    };
    path.starts_with(root)
}
