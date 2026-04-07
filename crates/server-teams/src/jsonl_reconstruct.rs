//! JSONL reconstruction -- recover team data from session JSONL files.
//!
//! When the filesystem team directory (`~/.claude/teams/<name>/`) is deleted,
//! these functions reconstruct TeamDetail and InboxMessage from JSONL logs.

use super::types::{InboxMessage, InboxMessageType, TeamDetail, TeamJSONLRef, TeamMember};
use memchr::memmem;

/// Deterministic color palette for team members when color is not in JSONL.
/// Uses named colors that match the frontend DOT_COLOR_MAP / BORDER_COLOR_MAP.
const FALLBACK_COLORS: &[&str] = &["blue", "red", "green", "yellow", "purple", "orange"];

/// Generate a deterministic color from a member name.
pub(super) fn deterministic_color(name: &str) -> &'static str {
    let hash = name
        .bytes()
        .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
    FALLBACK_COLORS[(hash as usize) % FALLBACK_COLORS.len()]
}

/// Reconstruct a `TeamDetail` from JSONL session files.
///
/// Scans the referenced JSONL files for:
/// - `TeamCreate` tool_use -> team name + description
/// - `Agent`/`Task` spawns with matching `input.team_name` -> members
/// - First timestamp with matching `teamName` -> `created_at`
///
/// Returns `None` if no TeamCreate for the given team is found.
pub(super) fn reconstruct_team_from_jsonl(
    team_name: &str,
    refs: &[TeamJSONLRef],
) -> Option<TeamDetail> {
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
            // Real TeamCreate assistant messages do NOT carry a top-level "teamName" --
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

                // TeamCreate -> description
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

                // Agent/Task spawn with team_name -> team member
                if tool_name == "Agent" || tool_name == "Task" {
                    if let Some(inp) = input {
                        extract_member_from_spawn(
                            inp,
                            &parsed,
                            block,
                            team_name,
                            tool_name,
                            &mut members,
                        );
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
pub(super) fn reconstruct_team_and_inbox_from_jsonl(
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

            // Allow TeamCreate lines through even without top-level teamName --
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
                        extract_member_from_spawn(
                            inp,
                            &parsed,
                            block,
                            team_name,
                            tool_name,
                            &mut members,
                        );
                    }
                }

                if tool_name == "SendMessage" && send_msg_finder.find(line.as_bytes()).is_some() {
                    if let Some(inp) = input {
                        extract_inbox_message(inp, &timestamp, &mut messages);
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
/// (team-lead -> member). These are the only messages available after the
/// filesystem team directory is deleted.
pub(super) fn reconstruct_inbox_from_jsonl(
    team_name: &str,
    refs: &[TeamJSONLRef],
) -> Vec<InboxMessage> {
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

                extract_inbox_message(input, &timestamp, &mut messages);
            }
        }
    }

    messages.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    messages
}

// ============================================================================
// Shared extraction helpers
// ============================================================================

/// Extract a team member from an Agent/Task spawn block.
fn extract_member_from_spawn(
    inp: &serde_json::Value,
    parsed: &serde_json::Value,
    block: &serde_json::Value,
    team_name: &str,
    tool_name: &str,
    members: &mut Vec<TeamMember>,
) {
    let spawn_team = inp.get("team_name").and_then(|v| v.as_str()).unwrap_or("");
    if spawn_team != team_name {
        return;
    }
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

/// Extract an inbox message from a SendMessage input block.
fn extract_inbox_message(
    input: &serde_json::Value,
    timestamp: &str,
    messages: &mut Vec<InboxMessage>,
) {
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
        timestamp: timestamp.to_string(),
        message_type,
        read: true,
        color: None,
        summary,
    });
}
