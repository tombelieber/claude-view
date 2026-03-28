//! Shared helper functions used across manager submodules.
//!
//! Path extraction, timestamp parsing, snapshot I/O, and hook event construction.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use tracing::warn;

use claude_view_core::live_parser::HookProgressData;

use crate::live::state::{AgentState, AgentStateGroup, HookEvent, SessionSnapshot, SnapshotEntry};

// =============================================================================
// Path extraction helpers
// =============================================================================

/// Extract the session ID from a JSONL file path.
///
/// Path format: `~/.claude/projects/{encoded-project-dir}/{session-uuid}.jsonl`
/// Session ID = filename without the `.jsonl` extension.
pub(super) fn extract_session_id(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string()
}

/// Extract project info from a JSONL file path.
///
/// Returns `(encoded_project_name, display_name, decoded_project_path, resolved_cwd)`.
pub(super) fn extract_project_info(
    path: &Path,
    cached_cwd: Option<&str>,
) -> (String, String, String, Option<String>) {
    let project_encoded = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    // Use cached cwd if available, else resolve from JSONL on disk.
    let cwd = cached_cwd.map(|s| s.to_string()).or_else(|| {
        path.parent()
            .and_then(claude_view_core::resolve_cwd_for_project)
    });

    let resolved = claude_view_core::discovery::resolve_project_path_with_cwd(
        &project_encoded,
        cwd.as_deref(),
    );

    (
        project_encoded,
        resolved.display_name,
        resolved.full_path,
        cwd,
    )
}

// =============================================================================
// Timestamp helpers
// =============================================================================

/// Calculate seconds since a Unix timestamp.
pub(super) fn seconds_since_modified_from_timestamp(last_activity_at: i64) -> u64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before Unix epoch")
        .as_secs() as i64;

    (now - last_activity_at).max(0) as u64
}

/// Parse an ISO 8601 timestamp string to a Unix epoch second.
pub(super) fn parse_timestamp_to_unix(ts: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(ts)
        .ok()
        .map(|dt| dt.timestamp())
        .or_else(|| {
            chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%S")
                .ok()
                .map(|ndt| ndt.and_utc().timestamp())
        })
}

// =============================================================================
// Snapshot I/O
// =============================================================================

/// Path to the PID snapshot file for server restart recovery.
pub(super) fn pid_snapshot_path() -> Option<PathBuf> {
    Some(
        dirs::home_dir()?
            .join(".claude")
            .join("live-monitor-pids.json"),
    )
}

/// Save the extended session snapshot to disk atomically.
pub(super) fn save_session_snapshot(path: &Path, snapshot: &SessionSnapshot) {
    let content = match serde_json::to_string(snapshot) {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to serialize session snapshot: {}", e);
            return;
        }
    };
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, &content).is_ok() {
        if let Err(e) = std::fs::rename(&tmp, path) {
            tracing::error!(error = %e, "failed to persist session snapshot");
        }
    }
}

/// Load the session snapshot from disk, handling v1->v2 migration.
pub(super) fn load_session_snapshot(path: &Path) -> SessionSnapshot {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => {
            return SessionSnapshot {
                version: 2,
                sessions: HashMap::new(),
            }
        }
    };
    load_session_snapshot_from_str(&content)
}

/// Parse a snapshot string, auto-detecting v1 (bare pid map) vs v2 (structured).
pub(super) fn load_session_snapshot_from_str(content: &str) -> SessionSnapshot {
    // Try v2 first
    if let Ok(snapshot) = serde_json::from_str::<SessionSnapshot>(content) {
        return snapshot;
    }
    // Fall back to v1: { "session_id": pid, ... }
    if let Ok(v1) = serde_json::from_str::<HashMap<String, u32>>(content) {
        let sessions = v1
            .into_iter()
            .map(|(id, pid)| {
                (
                    id,
                    SnapshotEntry {
                        pid,
                        status: "working".to_string(),
                        agent_state: AgentState {
                            group: AgentStateGroup::Autonomous,
                            state: "recovered".into(),
                            label: "Recovered from restart".into(),
                            context: None,
                        },
                        last_activity_at: 0,
                        control_id: None,
                    },
                )
            })
            .collect();
        return SessionSnapshot {
            version: 2,
            sessions,
        };
    }
    SessionSnapshot {
        version: 2,
        sessions: HashMap::new(),
    }
}

// =============================================================================
// Hook event helpers (Channel A: JSONL-derived events)
// =============================================================================

/// Wraps `parse_timestamp_to_unix` for Option<String> input.
/// Returns 0 on failure -- never SystemTime::now() (would break historical replay dedup).
pub(super) fn timestamp_string_to_unix(ts: &Option<String>) -> i64 {
    ts.as_deref().and_then(parse_timestamp_to_unix).unwrap_or(0)
}

pub(super) fn resolve_hook_event_from_progress(
    hp: &HookProgressData,
    ts: &Option<String>,
) -> HookEvent {
    let group = match hp.hook_event.as_str() {
        "SessionStart" => {
            if hp.source.as_deref() == Some("compact") {
                "autonomous"
            } else {
                "needs_you"
            }
        }
        "PreToolUse" => match hp.tool_name.as_deref() {
            Some("AskUserQuestion") | Some("EnterPlanMode") | Some("ExitPlanMode") => "needs_you",
            _ => "autonomous",
        },
        "PostToolUse" => "autonomous",
        "PostToolUseFailure" => "autonomous",
        "Stop" => "needs_you",
        _ => "autonomous",
    };
    let label = match &hp.tool_name {
        Some(tool) => format!("{}: {}", hp.hook_event, tool),
        None => hp.hook_event.clone(),
    };
    HookEvent {
        timestamp: timestamp_string_to_unix(ts),
        event_name: hp.hook_event.clone(),
        tool_name: hp.tool_name.clone(),
        label,
        group: group.to_string(),
        context: None,
        source: "hook_progress".to_string(),
    }
}

pub(super) fn make_synthesized_event(
    ts: &Option<String>,
    event_name: &str,
    tool_name: Option<&str>,
    group: &str,
) -> HookEvent {
    HookEvent {
        timestamp: timestamp_string_to_unix(ts),
        event_name: event_name.to_string(),
        tool_name: tool_name.map(|s| s.to_string()),
        label: event_name.to_string(),
        group: group.to_string(),
        context: None,
        source: "synthesized".to_string(),
    }
}
