//! The Claude Home browser: read-only metadata + safe previews for a fixed set
//! of `~/.claude` areas. Sensitive areas (session-env, shell-snapshots,
//! file-history) are metadata-only — their contents are never previewed.

use std::path::Path;
use std::time::UNIX_EPOCH;

use walkdir::WalkDir;

use super::fsjson::read_text_capped;
use super::preview::{redact_secret_like_text, truncate};
use super::types::ClaudeHomeEntry;
use super::{MAX_CLAUDE_HOME_PREVIEW_CHARS, MAX_CLAUDE_HOME_WALK};

/// `(kind, relative dir, metadata_only)` — the areas surfaced by the browser.
const AREAS: &[(&str, &str, bool)] = &[
    ("hooks", "hooks", false),
    ("rules", "rules", false),
    ("jobs", "jobs", false),
    ("workflow-definitions", "workflows", false),
    ("session-env", "session-env", true),
    ("shell-snapshots", "shell-snapshots", true),
    ("file-history", "file-history", true),
];

pub fn scan_claude_home_entries(claude_home: &Path) -> Vec<ClaudeHomeEntry> {
    let mut entries = Vec::new();
    for &(kind, rel, metadata_only) in AREAS {
        let path = claude_home.join(rel);
        if !path.exists() {
            continue;
        }
        push_claude_home_entry(&mut entries, claude_home, kind, &path, metadata_only);

        if !metadata_only && path.is_dir() {
            for entry in WalkDir::new(&path)
                .min_depth(1)
                .max_depth(2)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|entry| entry.file_type().is_file())
                .take(200)
            {
                push_claude_home_entry(&mut entries, claude_home, kind, entry.path(), false);
            }
        }
    }
    entries.sort_by(|a, b| {
        a.kind
            .cmp(&b.kind)
            .then_with(|| a.relative_path.cmp(&b.relative_path))
    });
    entries
}

fn push_claude_home_entry(
    entries: &mut Vec<ClaudeHomeEntry>,
    claude_home: &Path,
    kind: &str,
    path: &Path,
    metadata_only: bool,
) {
    let Ok(metadata) = std::fs::metadata(path) else {
        return;
    };
    let is_directory = metadata.is_dir();
    let (item_count, size_bytes) = if is_directory {
        bounded_dir_stats(path)
    } else {
        (0, metadata.len())
    };
    let modified_at = metadata
        .modified()
        .ok()
        .and_then(|mtime| mtime.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as i64);
    let relative_path = path
        .strip_prefix(claude_home)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(kind)
        .to_string();
    let (preview, preview_truncated) = if metadata_only || is_directory {
        (None, false)
    } else {
        preview_safe_file(path, MAX_CLAUDE_HOME_PREVIEW_CHARS)
    };

    entries.push(ClaudeHomeEntry {
        kind: kind.to_string(),
        name,
        relative_path,
        path: path.to_string_lossy().to_string(),
        is_directory,
        item_count,
        size_bytes,
        modified_at,
        preview,
        preview_truncated,
        metadata_only,
    });
}

fn bounded_dir_stats(path: &Path) -> (u64, u64) {
    let mut count = 0_u64;
    let mut size = 0_u64;
    for entry in WalkDir::new(path)
        .min_depth(1)
        .max_depth(4)
        .into_iter()
        .filter_map(Result::ok)
        .take(MAX_CLAUDE_HOME_WALK)
    {
        count += 1;
        if entry.file_type().is_file() {
            if let Ok(metadata) = entry.metadata() {
                size = size.saturating_add(metadata.len());
            }
        }
    }
    (count, size)
}

fn preview_safe_file(path: &Path, limit: usize) -> (Option<String>, bool) {
    let safe_ext = matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("json" | "jsonl" | "js" | "ts" | "py" | "sh" | "md" | "txt" | "yaml" | "yml")
    );
    if !safe_ext {
        return (None, false);
    }
    let Some(raw) = read_text_capped(path) else {
        return (None, false);
    };
    let truncated = raw.chars().count() > limit;
    (
        Some(redact_secret_like_text(&truncate(&raw, limit))),
        truncated,
    )
}
