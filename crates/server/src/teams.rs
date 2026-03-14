// crates/server/src/teams.rs
//! Teams data parser for ~/.claude/teams/.
//!
//! Reads team configs and inbox messages from the filesystem.
//! No file watching — teams are ephemeral (1–44 min bursts).

use memchr::memmem;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use ts_rs::TS;

// ============================================================================
// API Response Types (generated to TypeScript via ts-rs)
// ============================================================================

#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct TeamSummary {
    pub name: String,
    pub description: String,
    #[ts(type = "number")]
    pub created_at: i64,
    pub lead_session_id: String,
    #[ts(type = "number")]
    pub member_count: u32,
    #[ts(type = "number")]
    pub message_count: u32,
    #[ts(type = "number | null")]
    pub duration_estimate_secs: Option<u32>,
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct TeamDetail {
    pub name: String,
    pub description: String,
    #[ts(type = "number")]
    pub created_at: i64,
    pub lead_session_id: String,
    pub members: Vec<TeamMember>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct TeamMember {
    pub agent_id: String,
    pub name: String,
    pub agent_type: String,
    pub model: String,
    pub prompt: Option<String>,
    pub color: String,
    pub backend_type: Option<String>,
    pub cwd: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct InboxMessage {
    pub from: String,
    pub text: String,
    pub timestamp: String,
    pub message_type: InboxMessageType,
    pub read: bool,
    pub color: Option<String>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub enum InboxMessageType {
    PlainText,
    TaskAssignment,
    IdleNotification,
    ShutdownRequest,
    ShutdownApproved,
}

// ============================================================================
// JSONL Fallback Index
// ============================================================================

/// Reference to a session JSONL file that contains data for a team.
/// Used when the filesystem team directory (`~/.claude/teams/<name>/`) no longer exists.
#[derive(Debug, Clone)]
pub struct TeamJSONLRef {
    pub session_id: String,
    pub jsonl_path: std::path::PathBuf,
}

/// Index type: team_name → list of JSONL refs (a team may appear across multiple sessions).
pub type TeamJSONLIndex = HashMap<String, Vec<TeamJSONLRef>>;

// ============================================================================
// Raw deserialization types (match on-disk JSON shape)
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawTeamConfig {
    name: String,
    description: String,
    created_at: i64,
    lead_session_id: String,
    members: Vec<RawTeamMember>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)] // Fields deserialized from on-disk JSON but not all are mapped to API types
struct RawTeamMember {
    agent_id: String,
    name: String,
    agent_type: String,
    model: String,
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    color: String,
    #[serde(default)]
    backend_type: Option<String>,
    #[serde(default)]
    plan_mode_required: bool,
    #[serde(default)]
    cwd: String,
    #[serde(default)]
    joined_at: i64,
    #[serde(default)]
    tmux_pane_id: String,
    #[serde(default)]
    subscriptions: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawInboxMessage {
    from: String,
    text: String,
    #[serde(default)]
    timestamp: String,
    #[serde(default)]
    read: bool,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    summary: Option<String>,
}

// ============================================================================
// JSONL Index Scanner
// ============================================================================

/// Scan all JSONL files under `claude_dir/projects/` to build a team → JSONL path index.
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

        // Extract session ID from filename stem (e.g. "b4c61369-....jsonl" → "b4c61369-...")
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

// ============================================================================
// JSONL Reconstruction
// ============================================================================

/// Deterministic color palette for team members when color is not in JSONL.
const FALLBACK_COLORS: &[&str] = &[
    "#3b82f6", // blue
    "#ef4444", // red
    "#22c55e", // green
    "#f59e0b", // amber
    "#8b5cf6", // violet
    "#ec4899", // pink
    "#06b6d4", // cyan
    "#f97316", // orange
];

/// Generate a deterministic color from a member name.
fn deterministic_color(name: &str) -> &'static str {
    let hash = name
        .bytes()
        .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
    FALLBACK_COLORS[(hash as usize) % FALLBACK_COLORS.len()]
}

/// Reconstruct a `TeamDetail` from JSONL session files.
///
/// Scans the referenced JSONL files for:
/// - `TeamCreate` tool_use → team name + description
/// - `Agent`/`Task` spawns with matching `input.team_name` → members
/// - First timestamp with matching `teamName` → `created_at`
///
/// Returns `None` if no TeamCreate for the given team is found.
fn reconstruct_team_from_jsonl(team_name: &str, refs: &[TeamJSONLRef]) -> Option<TeamDetail> {
    let team_name_finder = memmem::Finder::new(team_name.as_bytes());
    let team_create_finder = memmem::Finder::new(b"\"TeamCreate\"");

    let mut description = String::new();
    let mut lead_session_id = String::new();
    let mut created_at: i64 = 0;
    let mut members = Vec::new();
    let mut found_create = false;

    for r in refs {
        let Ok(content) = std::fs::read_to_string(&r.jsonl_path) else {
            continue;
        };

        for line in content.lines() {
            // SIMD pre-filter: must mention the team name
            if team_name_finder.find(line.as_bytes()).is_none() {
                continue;
            }

            let Ok(parsed) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };

            // Verify this line is for our team.
            // Real TeamCreate assistant messages do NOT carry a top-level "teamName" —
            // the team name only appears inside message.content[].input.team_name.
            // We allow those through and let the inner block check confirm the match.
            let line_team = parsed
                .get("teamName")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let is_team_create_line = team_create_finder.find(line.as_bytes()).is_some();
            if !is_team_create_line && line_team != team_name {
                continue;
            }

            // Extract created_at from first line that belongs to our team
            if created_at == 0 && (line_team == team_name || is_team_create_line) {
                if let Some(ts) = parsed.get("timestamp").and_then(|v| v.as_str()) {
                    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts) {
                        created_at = dt.timestamp_millis();
                    }
                }
                lead_session_id = r.session_id.clone();
            }

            let content_blocks = parsed
                .get("message")
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_array());

            let Some(blocks) = content_blocks else {
                continue;
            };

            for block in blocks {
                let tool_name = block.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let input = block.get("input");

                // TeamCreate → description
                if tool_name == "TeamCreate" && is_team_create_line {
                    if let Some(inp) = input {
                        let inp_team = inp.get("team_name").and_then(|v| v.as_str()).unwrap_or("");
                        if inp_team == team_name {
                            description = inp
                                .get("description")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            found_create = true;
                        }
                    }
                }

                // Agent/Task spawn with team_name → team member
                if tool_name == "Agent" || tool_name == "Task" {
                    if let Some(inp) = input {
                        let spawn_team =
                            inp.get("team_name").and_then(|v| v.as_str()).unwrap_or("");
                        if spawn_team == team_name {
                            let member_name = inp
                                .get("name")
                                .or_else(|| inp.get("description"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("unnamed")
                                .to_string();
                            let agent_type = inp
                                .get("subagent_type")
                                .and_then(|v| v.as_str())
                                .unwrap_or(tool_name)
                                .to_string();
                            // Prefer explicit model from Agent input, fall back to message.model
                            let model = inp
                                .get("model")
                                .and_then(|v| v.as_str())
                                .or_else(|| {
                                    parsed
                                        .get("message")
                                        .and_then(|m| m.get("model"))
                                        .and_then(|v| v.as_str())
                                })
                                .unwrap_or("unknown")
                                .to_string();
                            let prompt = inp
                                .get("prompt")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            let tool_use_id = block
                                .get("id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();

                            members.push(TeamMember {
                                agent_id: tool_use_id,
                                name: member_name.clone(),
                                agent_type,
                                model,
                                prompt,
                                color: deterministic_color(&member_name).to_string(),
                                backend_type: None,
                                cwd: String::new(),
                            });
                        }
                    }
                }
            }
        }
    }

    if !found_create {
        return None;
    }

    Some(TeamDetail {
        name: team_name.to_string(),
        description,
        created_at,
        lead_session_id,
        members,
    })
}

/// Reconstruct both TeamDetail and inbox in a single file-read pass.
///
/// Combines `reconstruct_team_from_jsonl` and `reconstruct_inbox_from_jsonl`
/// to avoid reading each JSONL file twice when both are needed (e.g. summaries()).
/// Returns `None` when no TeamCreate is found (same semantics as reconstruct_team).
fn reconstruct_team_and_inbox_from_jsonl(
    team_name: &str,
    refs: &[TeamJSONLRef],
) -> Option<(TeamDetail, Vec<InboxMessage>)> {
    let team_name_finder = memmem::Finder::new(team_name.as_bytes());
    let team_create_finder = memmem::Finder::new(b"\"TeamCreate\"");
    let send_msg_finder = memmem::Finder::new(b"\"SendMessage\"");

    let mut description = String::new();
    let mut lead_session_id = String::new();
    let mut created_at: i64 = 0;
    let mut members = Vec::new();
    let mut messages = Vec::new();
    let mut found_create = false;

    for r in refs {
        let Ok(content) = std::fs::read_to_string(&r.jsonl_path) else {
            continue;
        };

        for line in content.lines() {
            if team_name_finder.find(line.as_bytes()).is_none() {
                continue;
            }

            let Ok(parsed) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };

            // Allow TeamCreate lines through even without top-level teamName —
            // real Claude Code JSONL omits teamName on the creation event itself.
            let line_team = parsed
                .get("teamName")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let is_team_create_line = team_create_finder.find(line.as_bytes()).is_some();
            if !is_team_create_line && line_team != team_name {
                continue;
            }

            if created_at == 0 && (line_team == team_name || is_team_create_line) {
                if let Some(ts) = parsed.get("timestamp").and_then(|v| v.as_str()) {
                    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts) {
                        created_at = dt.timestamp_millis();
                    }
                }
                lead_session_id = r.session_id.clone();
            }

            let timestamp = parsed
                .get("timestamp")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let blocks = parsed
                .get("message")
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_array());

            let Some(blocks) = blocks else {
                continue;
            };

            for block in blocks {
                let tool_name = block.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let input = block.get("input");

                if tool_name == "TeamCreate" && is_team_create_line {
                    if let Some(inp) = input {
                        let inp_team = inp.get("team_name").and_then(|v| v.as_str()).unwrap_or("");
                        if inp_team == team_name {
                            description = inp
                                .get("description")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            found_create = true;
                        }
                    }
                }

                if tool_name == "Agent" || tool_name == "Task" {
                    if let Some(inp) = input {
                        let spawn_team =
                            inp.get("team_name").and_then(|v| v.as_str()).unwrap_or("");
                        if spawn_team == team_name {
                            let member_name = inp
                                .get("name")
                                .or_else(|| inp.get("description"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("unnamed")
                                .to_string();
                            let agent_type = inp
                                .get("subagent_type")
                                .and_then(|v| v.as_str())
                                .unwrap_or(tool_name)
                                .to_string();
                            let model = inp
                                .get("model")
                                .and_then(|v| v.as_str())
                                .or_else(|| {
                                    parsed
                                        .get("message")
                                        .and_then(|m| m.get("model"))
                                        .and_then(|v| v.as_str())
                                })
                                .unwrap_or("unknown")
                                .to_string();
                            let prompt = inp
                                .get("prompt")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            let tool_use_id = block
                                .get("id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            members.push(TeamMember {
                                agent_id: tool_use_id,
                                name: member_name.clone(),
                                agent_type,
                                model,
                                prompt,
                                color: deterministic_color(&member_name).to_string(),
                                backend_type: None,
                                cwd: String::new(),
                            });
                        }
                    }
                }

                if tool_name == "SendMessage" && send_msg_finder.find(line.as_bytes()).is_some() {
                    if let Some(inp) = input {
                        let msg_type = inp
                            .get("type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("message");
                        let content_text = inp
                            .get("content")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let summary = inp
                            .get("summary")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        let message_type = match msg_type {
                            "shutdown_request" => InboxMessageType::ShutdownRequest,
                            "shutdown_approved" => InboxMessageType::ShutdownApproved,
                            "task_assignment" => InboxMessageType::TaskAssignment,
                            "idle_notification" => InboxMessageType::IdleNotification,
                            _ => InboxMessageType::PlainText,
                        };
                        messages.push(InboxMessage {
                            from: "team-lead".to_string(),
                            text: content_text,
                            timestamp: timestamp.clone(),
                            message_type,
                            read: true,
                            color: None,
                            summary,
                        });
                    }
                }
            }
        }
    }

    if !found_create {
        return None;
    }

    messages.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

    let detail = TeamDetail {
        name: team_name.to_string(),
        description,
        created_at,
        lead_session_id,
        members,
    };
    Some((detail, messages))
}

/// Reconstruct inbox messages from SendMessage tool_use calls in JSONL.
///
/// SendMessage calls in the lead session JSONL represent outbound messages
/// (team-lead → member). These are the only messages available after the
/// filesystem team directory is deleted.
fn reconstruct_inbox_from_jsonl(team_name: &str, refs: &[TeamJSONLRef]) -> Vec<InboxMessage> {
    let team_name_finder = memmem::Finder::new(team_name.as_bytes());
    let send_msg_finder = memmem::Finder::new(b"\"SendMessage\"");

    let mut messages = Vec::new();

    for r in refs {
        let Ok(content) = std::fs::read_to_string(&r.jsonl_path) else {
            continue;
        };

        for line in content.lines() {
            // SIMD pre-filter: must mention team name AND SendMessage
            if team_name_finder.find(line.as_bytes()).is_none() {
                continue;
            }
            if send_msg_finder.find(line.as_bytes()).is_none() {
                continue;
            }

            let Ok(parsed) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };

            let line_team = parsed
                .get("teamName")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if line_team != team_name {
                continue;
            }

            let timestamp = parsed
                .get("timestamp")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let blocks = parsed
                .get("message")
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_array());

            let Some(blocks) = blocks else {
                continue;
            };

            for block in blocks {
                if block.get("name").and_then(|n| n.as_str()) != Some("SendMessage") {
                    continue;
                }
                let Some(input) = block.get("input") else {
                    continue;
                };

                let msg_type = input
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("message");
                let content_text = input
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let summary = input
                    .get("summary")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let message_type = match msg_type {
                    "shutdown_request" => InboxMessageType::ShutdownRequest,
                    "shutdown_approved" => InboxMessageType::ShutdownApproved,
                    "task_assignment" => InboxMessageType::TaskAssignment,
                    "idle_notification" => InboxMessageType::IdleNotification,
                    _ => InboxMessageType::PlainText,
                };

                messages.push(InboxMessage {
                    from: "team-lead".to_string(),
                    text: content_text,
                    timestamp: timestamp.clone(),
                    message_type,
                    read: true, // Historical messages are always "read"
                    color: None,
                    summary,
                });
            }
        }
    }

    messages.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    messages
}

// ============================================================================
// Parser
// ============================================================================

/// Classify an inbox message by attempting to parse its text as JSON.
fn classify_message(text: &str) -> InboxMessageType {
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(text) {
        match parsed.get("type").and_then(|t| t.as_str()) {
            Some("task_assignment") => InboxMessageType::TaskAssignment,
            Some("idle_notification") => InboxMessageType::IdleNotification,
            Some("shutdown_request") => InboxMessageType::ShutdownRequest,
            Some("shutdown_approved") => InboxMessageType::ShutdownApproved,
            _ => InboxMessageType::PlainText,
        }
    } else {
        InboxMessageType::PlainText
    }
}

/// Load all inbox messages for a team directory, sorted chronologically.
fn load_inbox(team_dir: &Path) -> Vec<InboxMessage> {
    let inbox_dir = team_dir.join("inboxes");
    let Ok(entries) = std::fs::read_dir(&inbox_dir) else {
        return Vec::new();
    };

    let mut messages = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(raw_msgs): Result<Vec<RawInboxMessage>, _> = serde_json::from_str(&content) else {
            continue;
        };
        for raw in raw_msgs {
            let message_type = classify_message(&raw.text);
            messages.push(InboxMessage {
                from: raw.from,
                text: raw.text,
                timestamp: raw.timestamp,
                message_type,
                read: raw.read,
                color: raw.color,
                summary: raw.summary,
            });
        }
    }
    messages.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    messages
}

/// Parse a single team directory into a TeamDetail + inbox messages.
fn parse_team(team_dir: &Path) -> Option<(TeamDetail, Vec<InboxMessage>)> {
    let config_path = team_dir.join("config.json");
    let content = std::fs::read_to_string(&config_path).ok()?;
    let raw: RawTeamConfig = serde_json::from_str(&content).ok()?;

    let members = raw
        .members
        .into_iter()
        .map(|m| TeamMember {
            agent_id: m.agent_id,
            name: m.name,
            agent_type: m.agent_type,
            model: m.model,
            prompt: m.prompt,
            color: m.color,
            backend_type: m.backend_type,
            cwd: m.cwd,
        })
        .collect();

    let detail = TeamDetail {
        name: raw.name,
        description: raw.description,
        created_at: raw.created_at,
        lead_session_id: raw.lead_session_id,
        members,
    };

    let inbox = load_inbox(team_dir);
    Some((detail, inbox))
}

/// Live-reloading teams store.
///
/// Re-scans `~/.claude/teams/` on every public method call. The directory
/// typically has <10 entries, each config.json <5KB — total I/O is
/// microseconds, far cheaper than any staleness/cache-invalidation bug.
pub struct TeamsStore {
    /// Path to `~/.claude` (or equivalent). `None` for empty/test stores.
    claude_dir: Option<std::path::PathBuf>,
    /// Eagerly loaded snapshot (used for the initial log message at startup).
    pub teams: HashMap<String, TeamDetail>,
    pub inboxes: HashMap<String, Vec<InboxMessage>>,
    /// JSONL fallback index: team_name → JSONL file refs.
    /// Populated at startup by scanning all session JSONL files.
    /// Used when a team's filesystem directory no longer exists.
    jsonl_index: TeamJSONLIndex,
}

/// Internal: scan the teams directory and return (teams, inboxes).
fn scan_teams_dir(
    claude_dir: &Path,
) -> (
    HashMap<String, TeamDetail>,
    HashMap<String, Vec<InboxMessage>>,
) {
    let teams_dir = claude_dir.join("teams");
    let mut teams = HashMap::new();
    let mut inboxes = HashMap::new();

    let Ok(entries) = std::fs::read_dir(&teams_dir) else {
        return (teams, inboxes);
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if let Some((detail, inbox)) = parse_team(&path) {
            let name = detail.name.clone();
            teams.insert(name.clone(), detail);
            inboxes.insert(name, inbox);
        }
    }

    (teams, inboxes)
}

impl TeamsStore {
    /// Create an empty store (used by test constructors that don't need real teams).
    pub fn empty() -> Self {
        Self {
            claude_dir: None,
            teams: HashMap::new(),
            inboxes: HashMap::new(),
            jsonl_index: HashMap::new(),
        }
    }

    /// Scan ~/.claude/teams/ and build the JSONL fallback index.
    /// Delegates to `load_with_index()` — the existing call at `lib.rs:234`
    /// automatically builds the JSONL index at startup without changes.
    pub fn load(claude_dir: &Path) -> Self {
        Self::load_with_index(claude_dir)
    }

    /// Scan ~/.claude/teams/ for filesystem teams and build the JSONL fallback index.
    /// Subsequent calls to `summaries()`, `get()`, etc. will re-scan disk live.
    pub fn load_with_index(claude_dir: &Path) -> Self {
        let (teams, inboxes) = scan_teams_dir(claude_dir);
        let jsonl_index = build_team_jsonl_index(claude_dir);

        for (name, detail) in &teams {
            let msg_count = inboxes.get(name).map_or(0, |i| i.len());
            tracing::info!(
                "Loaded team '{}': {} members, {} inbox messages",
                name,
                detail.members.len(),
                msg_count,
            );
        }

        let jsonl_only: Vec<_> = jsonl_index
            .keys()
            .filter(|name| !teams.contains_key(*name))
            .collect();
        if !jsonl_only.is_empty() {
            tracing::info!(
                "Found {} teams in JSONL only (filesystem deleted): {:?}",
                jsonl_only.len(),
                jsonl_only,
            );
        }

        tracing::info!(
            "Loaded {} teams from disk + {} in JSONL index",
            teams.len(),
            jsonl_index.len(),
        );

        Self {
            claude_dir: Some(claude_dir.to_path_buf()),
            teams,
            inboxes,
            jsonl_index,
        }
    }

    /// Re-scan disk and update the in-memory snapshot.
    fn refresh(
        &self,
    ) -> (
        HashMap<String, TeamDetail>,
        HashMap<String, Vec<InboxMessage>>,
    ) {
        match &self.claude_dir {
            Some(dir) => scan_teams_dir(dir),
            None => (HashMap::new(), HashMap::new()),
        }
    }

    /// Build summary list for the /api/teams index endpoint.
    /// Re-scans the teams directory to pick up teams created after server start.
    pub fn summaries(&self) -> Vec<TeamSummary> {
        let (teams, inboxes) = self.refresh();
        let mut summaries: Vec<_> = teams
            .values()
            .map(|t| {
                let inbox = inboxes.get(&t.name);
                let msg_count = inbox.map_or(0, |i| i.len() as u32);
                let mut models: Vec<_> = t.members.iter().map(|m| m.model.clone()).collect();
                models.sort();
                models.dedup();

                // Estimate duration from first to last message timestamp.
                let duration = inbox.filter(|i| !i.is_empty()).and_then(|i| {
                    let first = chrono::DateTime::parse_from_rfc3339(&i[0].timestamp).ok()?;
                    let last =
                        chrono::DateTime::parse_from_rfc3339(&i[i.len() - 1].timestamp).ok()?;
                    Some((last - first).num_seconds().max(0) as u32)
                });

                TeamSummary {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    created_at: t.created_at,
                    lead_session_id: t.lead_session_id.clone(),
                    member_count: t.members.len() as u32,
                    message_count: msg_count,
                    duration_estimate_secs: duration,
                    models,
                }
            })
            .collect();
        // Add JSONL-only teams (not on filesystem).
        // Uses combined single-pass reconstruction to avoid reading each JSONL file twice.
        for (team_name, refs) in &self.jsonl_index {
            if teams.contains_key(team_name) {
                continue; // Already have this team from filesystem
            }
            if let Some((detail, inbox)) = reconstruct_team_and_inbox_from_jsonl(team_name, refs) {
                let msg_count = inbox.len() as u32;
                let mut models: Vec<_> = detail.members.iter().map(|m| m.model.clone()).collect();
                models.sort();
                models.dedup();

                let duration = if inbox.len() >= 2 {
                    let first = chrono::DateTime::parse_from_rfc3339(&inbox[0].timestamp).ok();
                    let last =
                        chrono::DateTime::parse_from_rfc3339(&inbox[inbox.len() - 1].timestamp)
                            .ok();
                    first
                        .zip(last)
                        .map(|(f, l)| (l - f).num_seconds().max(0) as u32)
                } else {
                    None
                };

                summaries.push(TeamSummary {
                    name: detail.name,
                    description: detail.description,
                    created_at: detail.created_at,
                    lead_session_id: detail.lead_session_id,
                    member_count: detail.members.len() as u32,
                    message_count: msg_count,
                    duration_estimate_secs: duration,
                    models,
                });
            }
        }

        summaries.sort_by_key(|s| std::cmp::Reverse(s.created_at));
        summaries
    }

    /// Look up a team by name (re-scans disk, falls back to JSONL index).
    pub fn get(&self, name: &str) -> Option<TeamDetail> {
        let (teams, _) = self.refresh();
        // Filesystem first
        if let Some(detail) = teams.get(name) {
            return Some(detail.clone());
        }
        // JSONL fallback — reconstruct from session logs
        if let Some(refs) = self.jsonl_index.get(name) {
            return reconstruct_team_from_jsonl(name, refs);
        }
        None
    }

    /// Look up inbox messages for a team (re-scans disk, falls back to JSONL index).
    pub fn inbox(&self, name: &str) -> Option<Vec<InboxMessage>> {
        let (_, inboxes) = self.refresh();
        // Filesystem first
        if let Some(msgs) = inboxes.get(name) {
            return Some(msgs.clone());
        }
        // JSONL fallback — return reconstructed inbox (may be empty vec)
        if self.jsonl_index.contains_key(name) {
            let refs = &self.jsonl_index[name];
            return Some(reconstruct_inbox_from_jsonl(name, refs));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_test_team(dir: &Path) {
        let team_dir = dir.join("teams").join("test-team");
        fs::create_dir_all(team_dir.join("inboxes")).unwrap();

        let config = serde_json::json!({
            "name": "test-team",
            "description": "Test team for unit tests",
            "createdAt": 1772568545480_i64,
            "leadAgentId": "team-lead@test-team",
            "leadSessionId": "dbd14eb6-b165-4089-ba51-4995e8640d5f",
            "members": [
                {
                    "agentId": "team-lead@test-team",
                    "name": "team-lead",
                    "agentType": "team-lead",
                    "model": "claude-opus-4-6",
                    "joinedAt": 1772568545480_i64,
                    "tmuxPaneId": "",
                    "cwd": "/tmp/test",
                    "subscriptions": []
                },
                {
                    "agentId": "researcher@test-team",
                    "name": "researcher",
                    "agentType": "Explore",
                    "model": "haiku",
                    "prompt": "Research the codebase",
                    "color": "blue",
                    "planModeRequired": false,
                    "joinedAt": 1772568557410_i64,
                    "tmuxPaneId": "in-process",
                    "cwd": "/tmp/test",
                    "subscriptions": [],
                    "backendType": "in-process"
                }
            ]
        });
        fs::write(
            team_dir.join("config.json"),
            serde_json::to_string_pretty(&config).unwrap(),
        )
        .unwrap();

        let inbox = serde_json::json!([
            {
                "from": "researcher",
                "text": "# Research Report\n\nFound 3 call sites.",
                "timestamp": "2026-03-03T20:10:42.127Z",
                "read": true,
                "color": "blue"
            },
            {
                "from": "researcher",
                "text": "{\"type\":\"idle_notification\",\"from\":\"researcher\",\"timestamp\":\"2026-03-03T20:10:42.127Z\",\"idleReason\":\"available\"}",
                "timestamp": "2026-03-03T20:10:43.000Z",
                "read": true
            }
        ]);
        fs::write(
            team_dir.join("inboxes").join("team-lead.json"),
            serde_json::to_string_pretty(&inbox).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn test_load_teams() {
        let tmp = TempDir::new().unwrap();
        make_test_team(tmp.path());

        let store = TeamsStore::load(tmp.path());
        assert_eq!(store.teams.len(), 1);
        assert!(store.teams.contains_key("test-team"));

        let team = &store.teams["test-team"];
        assert_eq!(team.members.len(), 2);
        assert_eq!(team.lead_session_id, "dbd14eb6-b165-4089-ba51-4995e8640d5f");
    }

    #[test]
    fn test_inbox_parsing() {
        let tmp = TempDir::new().unwrap();
        make_test_team(tmp.path());

        let store = TeamsStore::load(tmp.path());
        let inbox = &store.inboxes["test-team"];
        assert_eq!(inbox.len(), 2);

        // First message is plain text
        assert!(matches!(inbox[0].message_type, InboxMessageType::PlainText));
        assert_eq!(inbox[0].from, "researcher");

        // Second message is idle notification
        assert!(matches!(
            inbox[1].message_type,
            InboxMessageType::IdleNotification
        ));
    }

    #[test]
    fn test_summaries() {
        let tmp = TempDir::new().unwrap();
        make_test_team(tmp.path());

        let store = TeamsStore::load(tmp.path());
        let summaries = store.summaries();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].member_count, 2);
        assert_eq!(summaries[0].message_count, 2);
        assert_eq!(summaries[0].models, vec!["claude-opus-4-6", "haiku"]);
    }

    #[test]
    fn test_skips_dir_without_config() {
        let tmp = TempDir::new().unwrap();
        let broken_dir = tmp.path().join("teams").join("no-config");
        fs::create_dir_all(broken_dir.join("inboxes")).unwrap();

        let store = TeamsStore::load(tmp.path());
        assert_eq!(store.teams.len(), 0);
    }

    #[test]
    fn test_live_reload_picks_up_new_team() {
        // Teams created AFTER TeamsStore::load() must be visible on next query.
        // This is the root cause of /teams page showing "No teams found" for
        // teams created during the current server session.
        let tmp = TempDir::new().unwrap();

        // Initial load — no teams yet
        let store = TeamsStore::load(tmp.path());
        assert_eq!(store.summaries().len(), 0);

        // Team created after initial load (simulates Claude Code /team command)
        make_test_team(tmp.path());

        // Must find the new team WITHOUT restarting
        assert_eq!(
            store.summaries().len(),
            1,
            "TeamsStore should pick up teams created after initial load"
        );
        assert!(
            store.get("test-team").is_some(),
            "Team detail should be available for newly created team"
        );
    }

    #[test]
    fn test_get_prefers_filesystem_over_jsonl() {
        let tmp = TempDir::new().unwrap();

        // Create filesystem team
        make_test_team(tmp.path());

        // Also create JSONL with different description for same team name
        let projects_dir = tmp.path().join("projects").join("test-project");
        fs::create_dir_all(&projects_dir).unwrap();
        let jsonl_path = projects_dir.join("sess-jsonl.jsonl");
        let lines = vec![
            r#"{"type":"assistant","sessionId":"sess-jsonl","teamName":"test-team","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_1","name":"TeamCreate","input":{"team_name":"test-team","description":"JSONL version"}}]},"timestamp":"2026-03-11T10:00:00.000Z"}"#,
        ];
        fs::write(&jsonl_path, lines.join("\n") + "\n").unwrap();

        let store = TeamsStore::load_with_index(tmp.path());

        // Should return filesystem version (original description), NOT JSONL version
        let detail = store.get("test-team").unwrap();
        assert_eq!(detail.description, "Test team for unit tests");
    }

    #[test]
    fn test_inbox_fallback_from_jsonl() {
        let tmp = TempDir::new().unwrap();
        let projects_dir = tmp.path().join("projects").join("test-project");
        fs::create_dir_all(&projects_dir).unwrap();

        let jsonl_path = projects_dir.join("sess-inbox.jsonl");
        let lines = vec![
            r#"{"type":"assistant","sessionId":"sess-inbox","teamName":"inbox-only","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_1","name":"TeamCreate","input":{"team_name":"inbox-only","description":"Test"}}]},"timestamp":"2026-03-11T10:00:00.000Z"}"#,
            r#"{"type":"assistant","sessionId":"sess-inbox","teamName":"inbox-only","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_2","name":"SendMessage","input":{"type":"message","recipient":"worker","content":"Hello worker"}}]},"timestamp":"2026-03-11T10:01:00.000Z"}"#,
        ];
        fs::write(&jsonl_path, lines.join("\n") + "\n").unwrap();

        let store = TeamsStore::load_with_index(tmp.path());
        let inbox = store.inbox("inbox-only");
        assert!(inbox.is_some());
        assert_eq!(inbox.unwrap().len(), 1);
    }

    #[test]
    fn test_summaries_includes_jsonl_only_teams() {
        let tmp = TempDir::new().unwrap();

        // Create one filesystem team
        make_test_team(tmp.path());

        // Create one JSONL-only team
        let projects_dir = tmp.path().join("projects").join("test-project");
        fs::create_dir_all(&projects_dir).unwrap();
        let jsonl_path = projects_dir.join("sess-summary.jsonl");
        let lines = vec![
            r#"{"type":"assistant","sessionId":"sess-summary","teamName":"jsonl-team","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_1","name":"TeamCreate","input":{"team_name":"jsonl-team","description":"JSONL-only team"}}]},"timestamp":"2026-03-11T10:00:00.000Z"}"#,
            r#"{"type":"assistant","sessionId":"sess-summary","teamName":"jsonl-team","message":{"model":"haiku","role":"assistant","content":[{"type":"tool_use","id":"toolu_2","name":"Agent","input":{"name":"agent-a","team_name":"jsonl-team","prompt":"Work"}}]},"timestamp":"2026-03-11T10:00:01.000Z"}"#,
        ];
        fs::write(&jsonl_path, lines.join("\n") + "\n").unwrap();

        let store = TeamsStore::load_with_index(tmp.path());
        let summaries = store.summaries();

        assert_eq!(summaries.len(), 2);
        let names: Vec<_> = summaries.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"test-team"));
        assert!(names.contains(&"jsonl-team"));
    }

    #[test]
    fn test_teams_store_with_jsonl_index() {
        let tmp = TempDir::new().unwrap();
        let projects_dir = tmp.path().join("projects").join("test-project");
        fs::create_dir_all(&projects_dir).unwrap();

        let jsonl_path = projects_dir.join("sess-fallback.jsonl");
        let lines = vec![
            r#"{"type":"assistant","sessionId":"sess-fallback","teamName":"ghost-team","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_1","name":"TeamCreate","input":{"team_name":"ghost-team","description":"A team that no longer exists on disk"}}]},"timestamp":"2026-03-11T10:00:00.000Z"}"#,
            r#"{"type":"assistant","sessionId":"sess-fallback","teamName":"ghost-team","message":{"model":"haiku","role":"assistant","content":[{"type":"tool_use","id":"toolu_2","name":"Agent","input":{"name":"worker","team_name":"ghost-team","prompt":"Do stuff","subagent_type":"general-purpose"}}]},"timestamp":"2026-03-11T10:00:01.000Z"}"#,
        ];
        fs::write(&jsonl_path, lines.join("\n") + "\n").unwrap();

        // Load with JSONL index — no teams/ directory exists
        let store = TeamsStore::load_with_index(tmp.path());

        let detail = store.get("ghost-team");
        assert!(detail.is_some(), "Should reconstruct ghost-team from JSONL");
        let detail = detail.unwrap();
        assert_eq!(detail.name, "ghost-team");
        assert_eq!(detail.description, "A team that no longer exists on disk");
        assert_eq!(detail.members.len(), 1);
        assert_eq!(detail.members[0].name, "worker");
    }

    #[test]
    fn test_reconstruct_inbox_from_jsonl() {
        let tmp = TempDir::new().unwrap();
        let jsonl_path = tmp.path().join("sess-789.jsonl");

        let lines = vec![
            r#"{"type":"assistant","sessionId":"sess-789","teamName":"inbox-team","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_1","name":"SendMessage","input":{"type":"message","recipient":"analyst","summary":"Data ready","content":"Here is the analysis data."}}]},"timestamp":"2026-03-11T10:05:00.000Z"}"#,
            r#"{"type":"assistant","sessionId":"sess-789","teamName":"inbox-team","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_2","name":"SendMessage","input":{"type":"shutdown_request","recipient":"analyst","content":"All done."}}]},"timestamp":"2026-03-11T10:10:00.000Z"}"#,
        ];
        fs::write(&jsonl_path, lines.join("\n") + "\n").unwrap();

        let refs = vec![TeamJSONLRef {
            session_id: "sess-789".to_string(),
            jsonl_path,
        }];

        let inbox = reconstruct_inbox_from_jsonl("inbox-team", &refs);
        assert_eq!(inbox.len(), 2);
        assert_eq!(inbox[0].from, "team-lead");
        assert!(inbox[0].text.contains("analysis data"));
        assert!(matches!(inbox[0].message_type, InboxMessageType::PlainText));
        assert!(matches!(
            inbox[1].message_type,
            InboxMessageType::ShutdownRequest
        ));
        assert!(inbox[0].timestamp < inbox[1].timestamp);
    }

    #[test]
    fn test_reconstruct_inbox_empty_when_no_send_messages() {
        let tmp = TempDir::new().unwrap();
        let jsonl_path = tmp.path().join("sess-empty.jsonl");

        let lines = vec![
            r#"{"type":"assistant","sessionId":"sess-empty","teamName":"no-msg-team","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_1","name":"TeamCreate","input":{"team_name":"no-msg-team","description":"Test"}}]},"timestamp":"2026-03-11T10:00:00.000Z"}"#,
        ];
        fs::write(&jsonl_path, lines.join("\n") + "\n").unwrap();

        let refs = vec![TeamJSONLRef {
            session_id: "sess-empty".to_string(),
            jsonl_path,
        }];

        let inbox = reconstruct_inbox_from_jsonl("no-msg-team", &refs);
        assert!(inbox.is_empty());
    }

    #[test]
    fn test_reconstruct_team_from_jsonl() {
        let tmp = TempDir::new().unwrap();
        let jsonl_path = tmp.path().join("sess-123.jsonl");

        let lines = vec![
            r#"{"type":"assistant","sessionId":"sess-123","teamName":"demo-team","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_1","name":"TeamCreate","input":{"team_name":"demo-team","description":"Demo research team"}}]},"timestamp":"2026-03-11T10:00:00.000Z"}"#,
            r#"{"type":"assistant","sessionId":"sess-123","teamName":"demo-team","message":{"model":"claude-sonnet-4-6","role":"assistant","content":[{"type":"tool_use","id":"toolu_2","name":"Agent","input":{"name":"researcher","team_name":"demo-team","prompt":"Research the topic","subagent_type":"Explore"}}]},"timestamp":"2026-03-11T10:00:01.000Z"}"#,
            r#"{"type":"assistant","sessionId":"sess-123","teamName":"demo-team","message":{"model":"haiku","role":"assistant","content":[{"type":"tool_use","id":"toolu_3","name":"Agent","input":{"name":"writer","team_name":"demo-team","prompt":"Write the report","subagent_type":"code-writer"}}]},"timestamp":"2026-03-11T10:00:02.000Z"}"#,
        ];
        fs::write(&jsonl_path, lines.join("\n") + "\n").unwrap();

        let refs = vec![TeamJSONLRef {
            session_id: "sess-123".to_string(),
            jsonl_path: jsonl_path.clone(),
        }];

        let detail = reconstruct_team_from_jsonl("demo-team", &refs);
        assert!(detail.is_some(), "Should reconstruct team");
        let detail = detail.unwrap();
        assert_eq!(detail.name, "demo-team");
        assert_eq!(detail.description, "Demo research team");
        assert_eq!(detail.lead_session_id, "sess-123");
        assert_eq!(detail.members.len(), 2);
        assert_eq!(detail.members[0].name, "researcher");
        assert_eq!(detail.members[0].agent_type, "Explore");
        assert_eq!(detail.members[1].name, "writer");
        assert!(!detail.members[0].color.is_empty());
        assert!(!detail.members[1].color.is_empty());
        assert_ne!(detail.members[0].color, detail.members[1].color);
    }

    #[test]
    fn test_reconstruct_ignores_non_team_agent_spawns() {
        let tmp = TempDir::new().unwrap();
        let jsonl_path = tmp.path().join("sess-456.jsonl");

        let lines = vec![
            r#"{"type":"assistant","sessionId":"sess-456","teamName":"my-team","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_1","name":"TeamCreate","input":{"team_name":"my-team","description":"Test"}}]},"timestamp":"2026-03-11T10:00:00.000Z"}"#,
            r#"{"type":"assistant","sessionId":"sess-456","teamName":"my-team","message":{"model":"opus","role":"assistant","content":[{"type":"tool_use","id":"toolu_2","name":"Agent","input":{"name":"member-a","team_name":"my-team","prompt":"Do work"}}]},"timestamp":"2026-03-11T10:00:01.000Z"}"#,
            r#"{"type":"assistant","sessionId":"sess-456","teamName":"my-team","message":{"model":"haiku","role":"assistant","content":[{"type":"tool_use","id":"toolu_3","name":"Agent","input":{"name":"helper","prompt":"Quick task"}}]},"timestamp":"2026-03-11T10:00:02.000Z"}"#,
        ];
        fs::write(&jsonl_path, lines.join("\n") + "\n").unwrap();

        let refs = vec![TeamJSONLRef {
            session_id: "sess-456".to_string(),
            jsonl_path,
        }];

        let detail = reconstruct_team_from_jsonl("my-team", &refs).unwrap();
        assert_eq!(
            detail.members.len(),
            1,
            "Only team member spawn should be included"
        );
        assert_eq!(detail.members[0].name, "member-a");
    }

    #[test]
    fn test_build_team_jsonl_index_finds_teams() {
        let tmp = TempDir::new().unwrap();
        let projects_dir = tmp.path().join("projects").join("test-project");
        std::fs::create_dir_all(&projects_dir).unwrap();

        let jsonl_path = projects_dir.join("sess-abc.jsonl");
        let lines = vec![
            r#"{"type":"user","sessionId":"sess-abc","message":{"role":"user","content":"hi"},"timestamp":"2026-03-11T10:00:00Z"}"#,
            r#"{"type":"assistant","sessionId":"sess-abc","teamName":"demo-team","message":{"role":"assistant","content":[]},"timestamp":"2026-03-11T10:00:01Z"}"#,
        ];
        std::fs::write(&jsonl_path, lines.join("\n") + "\n").unwrap();

        let index = build_team_jsonl_index(tmp.path());
        assert!(
            index.contains_key("demo-team"),
            "Should find demo-team in index"
        );
        assert_eq!(index["demo-team"].len(), 1);
        assert_eq!(index["demo-team"][0].session_id, "sess-abc");
        assert_eq!(index["demo-team"][0].jsonl_path, jsonl_path);
    }

    #[test]
    fn test_build_team_jsonl_index_multiple_teams_one_session() {
        let tmp = TempDir::new().unwrap();
        let projects_dir = tmp.path().join("projects").join("test-project");
        std::fs::create_dir_all(&projects_dir).unwrap();

        let jsonl_path = projects_dir.join("sess-multi.jsonl");
        let lines = vec![
            r#"{"type":"assistant","sessionId":"sess-multi","teamName":"team-a","message":{"role":"assistant","content":[]},"timestamp":"2026-03-11T10:00:01Z"}"#,
            r#"{"type":"assistant","sessionId":"sess-multi","teamName":"team-b","message":{"role":"assistant","content":[]},"timestamp":"2026-03-11T10:00:02Z"}"#,
        ];
        std::fs::write(&jsonl_path, lines.join("\n") + "\n").unwrap();

        let index = build_team_jsonl_index(tmp.path());
        assert!(index.contains_key("team-a"));
        assert!(index.contains_key("team-b"));
    }

    #[test]
    fn test_build_team_jsonl_index_ignores_non_jsonl_files() {
        let tmp = TempDir::new().unwrap();
        let projects_dir = tmp.path().join("projects").join("test-project");
        std::fs::create_dir_all(&projects_dir).unwrap();

        std::fs::write(
            projects_dir.join("sess-abc.meta.json"),
            r#"{"teamName":"ghost-team"}"#,
        )
        .unwrap();

        let index = build_team_jsonl_index(tmp.path());
        assert!(index.is_empty());
    }

    #[test]
    fn test_team_jsonl_ref_creation() {
        let r = TeamJSONLRef {
            session_id: "abc-123".to_string(),
            jsonl_path: std::path::PathBuf::from("/tmp/test.jsonl"),
        };
        assert_eq!(r.session_id, "abc-123");
    }

    #[test]
    fn test_classify_message() {
        assert!(matches!(
            classify_message("plain text"),
            InboxMessageType::PlainText
        ));
        assert!(matches!(
            classify_message(r#"{"type":"idle_notification","from":"x"}"#),
            InboxMessageType::IdleNotification
        ));
        assert!(matches!(
            classify_message(r#"{"type":"task_assignment","taskId":"1"}"#),
            InboxMessageType::TaskAssignment
        ));
        assert!(matches!(
            classify_message(r#"{"type":"shutdown_request","requestId":"1"}"#),
            InboxMessageType::ShutdownRequest
        ));
        assert!(matches!(
            classify_message(r#"{"type":"shutdown_approved","requestId":"1"}"#),
            InboxMessageType::ShutdownApproved
        ));
    }

    /// Regression: TeamCreate assistant messages in real Claude Code JSONL do NOT
    /// carry a top-level "teamName" field — the team name only appears inside
    /// message.content[].input.team_name. reconstruct_team_from_jsonl must still
    /// find the team without requiring a top-level teamName on that line.
    #[test]
    fn test_reconstruct_team_from_jsonl_without_toplevel_teamname() {
        let tmp = TempDir::new().unwrap();
        let jsonl_path = tmp.path().join("sess-real.jsonl");

        // Real-world shape: TeamCreate line has NO top-level teamName.
        // Subsequent Agent lines DO have teamName (real Claude Code behaviour).
        let lines = vec![
            // TeamCreate — no teamName at top level (matches real JSONL structure)
            r#"{"type":"assistant","sessionId":"sess-real","message":{"model":"claude-sonnet-4-6","role":"assistant","content":[{"type":"tool_use","id":"toolu_abc","name":"TeamCreate","input":{"team_name":"real-team","description":"A real world team"},"caller":{"type":"direct"}}]},"timestamp":"2026-03-11T10:00:00.000Z"}"#,
            // Agent spawn — has teamName (subsequent messages after team creation)
            r#"{"type":"assistant","sessionId":"sess-real","teamName":"real-team","message":{"model":"claude-sonnet-4-6","role":"assistant","content":[{"type":"tool_use","id":"toolu_def","name":"Agent","input":{"name":"worker","team_name":"real-team","prompt":"Do work","subagent_type":"general-purpose"}}]},"timestamp":"2026-03-11T10:00:01.000Z"}"#,
        ];
        std::fs::write(&jsonl_path, lines.join("\n") + "\n").unwrap();

        let refs = vec![TeamJSONLRef {
            session_id: "sess-real".to_string(),
            jsonl_path: jsonl_path.clone(),
        }];

        let result = reconstruct_team_from_jsonl("real-team", &refs);
        assert!(
            result.is_some(),
            "Should reconstruct team even when TeamCreate line has no top-level teamName"
        );
        let team = result.unwrap();
        assert_eq!(team.name, "real-team");
        assert_eq!(team.description, "A real world team");
        assert_eq!(team.members.len(), 1, "Should find Agent spawn member");
        assert_eq!(team.members[0].name, "worker");
    }

    /// Regression: reconstruct_team_and_inbox_from_jsonl (used in summaries())
    /// must also work when TeamCreate line has no top-level teamName.
    #[test]
    fn test_reconstruct_combined_without_toplevel_teamname() {
        let tmp = TempDir::new().unwrap();
        let jsonl_path = tmp.path().join("sess-combined.jsonl");

        let lines = vec![
            // TeamCreate — no teamName at top level
            r#"{"type":"assistant","sessionId":"sess-combined","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_1","name":"TeamCreate","input":{"team_name":"combo-team","description":"Combined test team"}}]},"timestamp":"2026-03-11T10:00:00.000Z"}"#,
            // SendMessage — has teamName
            r#"{"type":"assistant","sessionId":"sess-combined","teamName":"combo-team","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_2","name":"SendMessage","input":{"type":"message","recipient":"worker","content":"Go!"}}]},"timestamp":"2026-03-11T10:01:00.000Z"}"#,
        ];
        std::fs::write(&jsonl_path, lines.join("\n") + "\n").unwrap();

        let refs = vec![TeamJSONLRef {
            session_id: "sess-combined".to_string(),
            jsonl_path: jsonl_path.clone(),
        }];

        let result = reconstruct_team_and_inbox_from_jsonl("combo-team", &refs);
        assert!(
            result.is_some(),
            "summaries() path must find team without top-level teamName on TeamCreate line"
        );
        let (team, inbox) = result.unwrap();
        assert_eq!(team.name, "combo-team");
        assert_eq!(team.description, "Combined test team");
        assert_eq!(inbox.len(), 1);
    }
}
