//! JSONL snapshot extraction -- builds hash -> file_path mapping.

use std::collections::HashMap;
use std::path::Path;

use super::helpers::parse_backup_filename;

/// Extract a hash -> file_path mapping from `file-history-snapshot` entries in a JSONL file.
///
/// Scans the JSONL file for lines with `"file-history-snapshot"` type, extracts
/// `snapshot.trackedFileBackups`, and builds a map from the hash portion of each
/// `backupFileName` (stripping the `@vN` suffix) to the original file path.
///
/// All snapshots are processed; later entries overwrite earlier ones for the same hash,
/// reflecting the most recent file path (files may be renamed between snapshots).
pub fn extract_file_path_map(jsonl_path: &Path) -> HashMap<String, String> {
    use memchr::memmem;
    use std::io::{BufRead, BufReader};

    let mut map = HashMap::new();

    let file = match std::fs::File::open(jsonl_path) {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!(
                path = %jsonl_path.display(),
                error = %e,
                "Failed to open JSONL file for file-path extraction"
            );
            return map;
        }
    };

    let reader = BufReader::new(file);
    let finder = memmem::Finder::new(b"file-history-snapshot");

    for (line_idx, line) in reader.lines().enumerate() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                tracing::debug!(
                    line_number = line_idx + 1,
                    error = %e,
                    "Failed to read line in JSONL file"
                );
                continue;
            }
        };

        // SIMD-accelerated pre-filter: skip lines that can't be file-history-snapshot
        if finder.find(line.as_bytes()).is_none() {
            continue;
        }

        let value: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                tracing::debug!(
                    line_number = line_idx + 1,
                    error = %e,
                    "Failed to parse JSON on line that matched file-history-snapshot pre-filter"
                );
                continue;
            }
        };

        if value.get("type").and_then(|t| t.as_str()) != Some("file-history-snapshot") {
            continue;
        }

        let tracked = match value
            .get("snapshot")
            .and_then(|s| s.get("trackedFileBackups"))
            .and_then(|t| t.as_object())
        {
            Some(obj) => obj,
            None => continue,
        };

        for (file_path, backup_info) in tracked {
            let backup_file_name = match backup_info.get("backupFileName").and_then(|b| b.as_str())
            {
                Some(name) => name,
                None => continue,
            };

            // Extract hash by stripping @vN suffix
            if let Some((hash, _version)) = parse_backup_filename(backup_file_name) {
                map.insert(hash, file_path.clone());
            }
        }
    }

    map
}
