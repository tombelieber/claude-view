// crates/server/src/teams.rs
//! Teams data parser for ~/.claude/teams/.
//!
//! Reads team configs and inbox messages from the filesystem.
//! No file watching — teams are ephemeral (1–44 min bursts).

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

/// In-memory store of all teams, loaded once at startup.
pub struct TeamsStore {
    pub teams: HashMap<String, TeamDetail>,
    pub inboxes: HashMap<String, Vec<InboxMessage>>,
}

impl TeamsStore {
    /// Create an empty store (used by test constructors that don't need real teams).
    pub fn empty() -> Self {
        Self {
            teams: HashMap::new(),
            inboxes: HashMap::new(),
        }
    }

    /// Scan ~/.claude/teams/ and load all teams with valid config.json.
    pub fn load(claude_dir: &Path) -> Self {
        let teams_dir = claude_dir.join("teams");
        let mut teams = HashMap::new();
        let mut inboxes = HashMap::new();

        let Ok(entries) = std::fs::read_dir(&teams_dir) else {
            tracing::debug!("No teams directory found at {:?}", teams_dir);
            return Self { teams, inboxes };
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if let Some((detail, inbox)) = parse_team(&path) {
                tracing::info!(
                    "Loaded team '{}': {} members, {} inbox messages",
                    detail.name,
                    detail.members.len(),
                    inbox.len()
                );
                let name = detail.name.clone();
                teams.insert(name.clone(), detail);
                inboxes.insert(name, inbox);
            }
        }

        tracing::info!("Loaded {} teams from {:?}", teams.len(), teams_dir);
        Self { teams, inboxes }
    }

    /// Build summary list for the /api/teams index endpoint.
    pub fn summaries(&self) -> Vec<TeamSummary> {
        let mut summaries: Vec<_> = self
            .teams
            .values()
            .map(|t| {
                let inbox = self.inboxes.get(&t.name);
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
        summaries.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        summaries
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
}
