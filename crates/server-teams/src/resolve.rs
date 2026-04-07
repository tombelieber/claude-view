//! Team member session resolution and cost computation.
//!
//! Resolves team member names to spawn metadata (session IDs, models),
//! discovers subagent sidechains, and computes per-member cost breakdowns.

use super::types::{
    JsonlLine, ResolvedMemberInfo, TeamCostBreakdown, TeamDetail, TeamMemberCost,
    TeamMemberSidechain,
};
use claude_view_core::pricing::{ModelPricing, TokenUsage};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// Scan the lead session's JSONL to resolve team member names -> spawn metadata.
///
/// Parses `toolUseResult` objects with `status: "teammate_spawned"` and
/// matching `team_name`. Each result carries `name`, `agent_id`, `model`, and
/// `tmux_pane_id` directly.
pub fn resolve_team_member_sessions(
    lead_jsonl_path: &Path,
    team_name: &str,
) -> HashMap<String, ResolvedMemberInfo> {
    let Ok(content) = std::fs::read_to_string(lead_jsonl_path) else {
        return HashMap::new();
    };

    let mut members: HashMap<String, ResolvedMemberInfo> = HashMap::new();

    for line in content.lines() {
        let Ok(parsed) = serde_json::from_str::<JsonlLine>(line) else {
            continue;
        };
        let Some(spawn) = parsed.tool_use_result else {
            continue;
        };
        if spawn.status != "teammate_spawned" || spawn.team_name != team_name {
            continue;
        }
        if spawn.name.is_empty() || spawn.agent_id.is_empty() {
            continue;
        }

        members.insert(
            spawn.name.clone(),
            ResolvedMemberInfo {
                agent_id: spawn.agent_id,
                model: spawn.model,
                in_process: spawn.tmux_pane_id == "in-process",
            },
        );
    }

    tracing::debug!(
        "Resolved {} team member(s) for '{}'",
        members.len(),
        team_name,
    );

    members
}

/// Scan `{session_dir}/subagents/*.meta.json` and return sidechain info for each.
///
/// Each meta.json has `{"agentType":"js-advocate"}` and filename `agent-{hexId}.meta.json`.
/// The corresponding `.jsonl` file is read for line count, file size, model, and timestamps.
/// When `pricing` is provided, each sidechain's JSONL is also parsed via
/// `SessionAccumulator::from_file()` to compute per-sidechain cost and token usage.
/// Results are sorted by `member_name` ascending, then `line_count` descending.
pub fn resolve_team_sidechains(
    session_dir: &Path,
    pricing: &HashMap<String, ModelPricing>,
) -> Vec<TeamMemberSidechain> {
    use std::io::{BufRead, BufReader};

    #[derive(Deserialize)]
    struct Meta {
        #[serde(rename = "agentType")]
        agent_type: String,
    }

    /// Minimal struct for extracting timestamp, type, and model from JSONL entries.
    /// Only these fields are deserialized; everything else is skipped by serde.
    #[derive(Deserialize)]
    struct MinimalEntry {
        #[serde(default)]
        timestamp: Option<String>,
        #[serde(default, rename = "type")]
        entry_type: Option<String>,
        #[serde(default)]
        message: Option<MinimalMessage>,
    }

    #[derive(Deserialize)]
    struct MinimalMessage {
        #[serde(default)]
        model: Option<String>,
    }

    let subagents_dir = session_dir.join("subagents");
    let entries = match std::fs::read_dir(&subagents_dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut sidechains = Vec::new();

    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();

        // Match pattern: agent-{hexId}.meta.json
        let hex_id = match name
            .strip_prefix("agent-")
            .and_then(|s| s.strip_suffix(".meta.json"))
        {
            Some(id) => id.to_string(),
            None => continue,
        };

        // Parse meta.json for member name
        let meta_path = entry.path();
        let meta_bytes = match std::fs::read(&meta_path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let meta: Meta = match serde_json::from_slice(&meta_bytes) {
            Ok(m) => m,
            Err(_) => continue,
        };

        // Read JSONL: count lines, extract model + first/last timestamps.
        // Only parses JSON for early lines (model/timestamp) and the last line (end timestamp).
        let jsonl_path = subagents_dir.join(format!("agent-{hex_id}.jsonl"));
        let (line_count, file_size_bytes, model, started_at, ended_at) =
            match std::fs::File::open(&jsonl_path) {
                Ok(file) => {
                    let size = file.metadata().map(|m| m.len()).unwrap_or(0);
                    let reader = BufReader::new(file);

                    let mut count = 0u32;
                    let mut first_ts: Option<String> = None;
                    let mut model: Option<String> = None;
                    let mut last_raw: Option<String> = None;

                    for line_result in reader.lines() {
                        let line = match line_result {
                            Ok(l) => l,
                            Err(_) => {
                                count += 1;
                                continue;
                            }
                        };
                        count += 1;

                        // Parse early lines only: first timestamp + first assistant model
                        if first_ts.is_none() || model.is_none() {
                            if let Ok(e) = serde_json::from_str::<MinimalEntry>(&line) {
                                if first_ts.is_none() {
                                    first_ts = e.timestamp.clone();
                                }
                                if model.is_none() && e.entry_type.as_deref() == Some("assistant") {
                                    model = e.message.and_then(|m| m.model);
                                }
                            }
                        }

                        last_raw = Some(line);
                    }

                    // Parse last line for end timestamp
                    let last_ts = last_raw.and_then(|raw| {
                        serde_json::from_str::<MinimalEntry>(&raw)
                            .ok()
                            .and_then(|e| e.timestamp)
                    });

                    (count, size, model.unwrap_or_default(), first_ts, last_ts)
                }
                Err(_) => (0, 0, String::new(), None, None),
            };

        // Compute duration from ISO 8601 timestamps
        let duration_seconds = match (&started_at, &ended_at) {
            (Some(start), Some(end)) => {
                let s = chrono::DateTime::parse_from_rfc3339(start).ok();
                let e = chrono::DateTime::parse_from_rfc3339(end).ok();
                match (s, e) {
                    (Some(s), Some(e)) => e.signed_duration_since(s).num_seconds().max(0) as u32,
                    _ => 0,
                }
            }
            _ => 0,
        };

        // Compute cost via SessionAccumulator (reuses the same JSONL parsing as build_team_cost)
        let (cost_usd, tokens): (Option<f64>, Option<TokenUsage>) = {
            use claude_view_core::accumulator::SessionAccumulator;
            match SessionAccumulator::from_file(&jsonl_path, pricing) {
                Ok(rich) => (Some(rich.cost.total_usd), Some(rich.tokens)),
                Err(_) => (None, None),
            }
        };

        sidechains.push(TeamMemberSidechain {
            hex_id,
            member_name: meta.agent_type,
            line_count,
            file_size_bytes,
            model,
            started_at,
            ended_at,
            duration_seconds,
            cost_usd,
            tokens,
        });
    }

    // Sort by member_name asc, then line_count desc
    sidechains.sort_by(|a, b| {
        a.member_name
            .cmp(&b.member_name)
            .then(b.line_count.cmp(&a.line_count))
    });

    sidechains
}

/// Build a `TeamCostBreakdown` by resolving member sessions and computing costs.
///
/// `resolve_session_path` maps session_id -> JSONL file path, allowing the caller
/// to inject DB/live-session lookup logic.
pub fn build_team_cost(
    team: &TeamDetail,
    lead_jsonl_path: &Path,
    pricing: &HashMap<String, ModelPricing>,
    resolve_session_path: impl Fn(&str) -> Option<std::path::PathBuf>,
) -> TeamCostBreakdown {
    use claude_view_core::accumulator::SessionAccumulator;

    let resolved = resolve_team_member_sessions(lead_jsonl_path, &team.name);

    // Lead session cost (includes all in-process member costs)
    let lead_rich = SessionAccumulator::from_file(lead_jsonl_path, pricing).ok();
    let lead_cost_usd = lead_rich.as_ref().map(|r| r.cost.total_usd).unwrap_or(0.0);

    let mut members = Vec::with_capacity(team.members.len());
    let mut total_cost_usd = lead_cost_usd;

    for member in &team.members {
        if member.agent_type == "team-lead" {
            continue;
        }

        let info = resolved.get(&member.name);
        let in_process = info.is_some_and(|i| i.in_process)
            || member.backend_type.as_deref() == Some("in-process");
        let session_id = info.map(|i| i.agent_id.clone());

        // Use resolved model if member's model is empty (inbox-augmented members)
        let model = if !member.model.is_empty() {
            member.model.clone()
        } else {
            info.and_then(|i| i.model.clone()).unwrap_or_default()
        };

        // In-process members' cost is already in lead_cost_usd -- don't double-count
        let (cost_usd, tokens, cost) = if in_process {
            (None, None, None)
        } else {
            let rich_data = session_id
                .as_ref()
                .and_then(|sid| resolve_session_path(sid))
                .and_then(|path| SessionAccumulator::from_file(&path, pricing).ok());
            let usd = rich_data.as_ref().map(|r| r.cost.total_usd);
            if let Some(c) = usd {
                total_cost_usd += c;
            }
            (
                usd,
                rich_data.as_ref().map(|r| r.tokens.clone()),
                rich_data.map(|r| r.cost),
            )
        };

        members.push(TeamMemberCost {
            name: member.name.clone(),
            color: member.color.clone(),
            model,
            agent_type: member.agent_type.clone(),
            session_id,
            in_process,
            cost_usd,
            tokens,
            cost,
        });
    }

    TeamCostBreakdown {
        team_name: team.name.clone(),
        total_cost_usd,
        lead_cost_usd,
        members,
    }
}
