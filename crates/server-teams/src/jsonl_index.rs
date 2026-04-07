//! JSONL index scanner -- builds a team-name -> JSONL-path index.
//!
//! Scans `~/.claude/projects/` for `.jsonl` files that reference team names.
//! Uses SIMD memmem pre-filter for fast file skipping.

use super::types::{TeamJSONLIndex, TeamJSONLRef};
use memchr::memmem;
use std::collections::HashMap;
use std::path::Path;

/// Scan all JSONL files under `claude_dir/projects/` to build a team -> JSONL path index.
///
/// For each `.jsonl` file, reads lines looking for a top-level `"teamName"` field.
/// Uses SIMD memmem pre-filter: files without `"teamName"` are skipped in ~microseconds.
/// Stops scanning a file once all unique team names are collected (typically <10 lines).
pub fn build_team_jsonl_index(claude_dir: &Path) -> TeamJSONLIndex {
    let projects_dir = claude_dir.join("projects");
    let mut index: TeamJSONLIndex = HashMap::new();

    let Ok(project_entries) = std::fs::read_dir(&projects_dir) else {
        return index;
    };

    let finder = memmem::Finder::new(b"\"teamName\"");

    for project_entry in project_entries.flatten() {
        let project_path = project_entry.path();
        if !project_path.is_dir() {
            continue;
        }
        scan_directory_for_teams(&project_path, &finder, &mut index);
    }

    tracing::info!(
        "Built team JSONL index: {} teams across {} session refs",
        index.len(),
        index.values().map(|v| v.len()).sum::<usize>(),
    );

    index
}

/// Scan a single directory for `.jsonl` files with team references.
fn scan_directory_for_teams(dir: &Path, finder: &memmem::Finder, index: &mut TeamJSONLIndex) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        // Only process .jsonl files (not .meta.json, not directories)
        match path.extension() {
            Some(ext) if ext == "jsonl" => {}
            _ => continue,
        }

        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };

        // SIMD pre-filter: skip entire file if no teamName reference
        if finder.find(content.as_bytes()).is_none() {
            continue;
        }

        // Extract session ID from filename stem (e.g. "b4c61369-....jsonl" -> "b4c61369-...")
        let session_id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        // Collect unique team names from this file
        let mut seen_teams = std::collections::HashSet::new();
        for line in content.lines() {
            if finder.find(line.as_bytes()).is_none() {
                continue;
            }
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(line) {
                if let Some(team_name) = parsed.get("teamName").and_then(|v| v.as_str()) {
                    if seen_teams.insert(team_name.to_string()) {
                        index
                            .entry(team_name.to_string())
                            .or_default()
                            .push(TeamJSONLRef {
                                session_id: session_id.clone(),
                                jsonl_path: path.clone(),
                            });
                    }
                }
            }
        }
    }
}
