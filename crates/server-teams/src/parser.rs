//! Filesystem parser -- loads team config and inbox from disk.
//!
//! Reads `config.json` and `inboxes/*.json` from a team directory,
//! augmenting the member list with inbox senders not in config.

use super::jsonl_reconstruct::deterministic_color;
use super::types::{
    InboxMessage, InboxMessageType, RawInboxMessage, RawTeamConfig, TeamDetail, TeamMember,
};
use std::collections::HashSet;
use std::path::Path;

/// Classify an inbox message by attempting to parse its text as JSON.
pub(super) fn classify_message(text: &str) -> InboxMessageType {
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
///
/// After loading config.json members and inbox messages, augments the member
/// list with any inbox senders not already registered. This handles sub-agents
/// spawned by team members (e.g. debate participants spawned by debate-judge)
/// that communicate through the team inbox without being in config.json.
pub(super) fn parse_team(team_dir: &Path) -> Option<(TeamDetail, Vec<InboxMessage>)> {
    let config_path = team_dir.join("config.json");
    let content = std::fs::read_to_string(&config_path).ok()?;
    let raw: RawTeamConfig = serde_json::from_str(&content).ok()?;

    let mut members: Vec<TeamMember> = raw
        .members
        .into_iter()
        .map(|m| TeamMember {
            agent_id: m.agent_id,
            name: m.name,
            agent_type: if m.agent_type.is_empty() {
                "general-purpose".to_string()
            } else {
                m.agent_type
            },
            model: m.model,
            prompt: m.prompt,
            color: m.color,
            backend_type: m.backend_type,
            cwd: m.cwd,
        })
        .collect();

    let inbox = load_inbox(team_dir);

    // Augment members from inbox filenames (primary) and senders (fallback)
    augment_members_from_inbox_files_and_senders(&mut members, team_dir, &inbox);

    let detail = TeamDetail {
        name: raw.name,
        description: raw.description,
        created_at: raw.created_at,
        lead_session_id: raw.lead_session_id,
        members,
    };

    Some((detail, inbox))
}

/// Fill in members discovered only from inbox files or messages (not in config.json).
///
/// Primary source: inbox filenames themselves (e.g., `ts-advocate.json` -> member `ts-advocate`).
/// This is the canonical source of truth for team membership -- when an agent is spawned as
/// a teammate, a corresponding `inboxes/{agent_name}.json` is created immediately.
///
/// Fallback: inbox message senders (for sub-agents that send messages but don't have inboxes).
fn augment_members_from_inbox_files_and_senders(
    members: &mut Vec<TeamMember>,
    team_dir: &Path,
    inbox: &[InboxMessage],
) {
    let known: HashSet<String> = members.iter().map(|m| m.name.clone()).collect();

    // Primary: Discover members from inbox filenames
    let inbox_dir = team_dir.join("inboxes");
    if let Ok(entries) = std::fs::read_dir(&inbox_dir) {
        let mut inbox_agents: Vec<String> = entries
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "json") {
                    path.file_stem()
                        .and_then(|name| name.to_str().map(String::from))
                } else {
                    None
                }
            })
            .filter(|name| !known.contains(name) && !name.is_empty() && name != "team-lead")
            .collect();
        inbox_agents.sort(); // Deterministic order

        for name in inbox_agents {
            let color = deterministic_color(&name).to_string();
            members.push(TeamMember {
                agent_id: String::new(),
                name,
                agent_type: "general-purpose".to_string(),
                model: String::new(),
                prompt: None,
                color,
                backend_type: None,
                cwd: String::new(),
            });
        }
    }

    // Fallback: Discover members from message senders (sub-agents that don't have inboxes)
    let known_after_inboxes: HashSet<String> = members.iter().map(|m| m.name.clone()).collect();
    let mut seen = HashSet::new();
    for msg in inbox {
        if known_after_inboxes.contains(&msg.from) || !seen.insert(msg.from.clone()) {
            continue;
        }
        let color = msg
            .color
            .clone()
            .unwrap_or_else(|| deterministic_color(&msg.from).to_string());
        members.push(TeamMember {
            agent_id: String::new(),
            name: msg.from.clone(),
            agent_type: "general-purpose".to_string(),
            model: String::new(),
            prompt: None,
            color,
            backend_type: None,
            cwd: String::new(),
        });
    }
}
