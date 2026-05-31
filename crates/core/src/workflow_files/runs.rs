//! Turn a discovered [`WorkflowArtifact`] into a parsed run: summary metadata,
//! phases, agents, and (on demand) script/result bodies.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde_json::Value;

use super::agents::{find_agent_jsonl, scan_agent_files};
use super::fsjson::{
    file_modified_ms, is_existing_path_under, json_i64, json_string, json_u64, read_text_capped,
};
use super::ids::ids_match;
use super::journal::read_journal_events;
use super::preview::{preview_value, safe_preview, truncate};
use super::types::{
    ParsedRun, WorkflowAgentSummary, WorkflowArtifact, WorkflowPhaseSummary, WorkflowRunSummary,
};
use super::{MAX_DETAIL_TEXT_CHARS, MAX_LIST_PREVIEW_CHARS, MAX_SYNTH_PHASES};

pub(crate) fn build_run_from_artifact(
    claude_home: &Path,
    artifact: &WorkflowArtifact,
    include_detail: bool,
) -> Option<ParsedRun> {
    if let Some(summary_path) = artifact.summary_path.as_ref() {
        let raw = read_text_capped(summary_path)?;
        let value: Value = match serde_json::from_str(&raw) {
            Ok(value) => value,
            Err(err) => {
                tracing::warn!(
                    path = %summary_path.display(),
                    error = %err,
                    "Skipping malformed workflow summary JSON"
                );
                return None;
            }
        };
        Some(parsed_summary_run(
            claude_home,
            artifact,
            summary_path,
            &value,
            include_detail,
        ))
    } else {
        Some(stub_run_from_subagents(artifact, include_detail))
    }
}

fn parsed_summary_run(
    claude_home: &Path,
    artifact: &WorkflowArtifact,
    summary_path: &Path,
    value: &Value,
    include_detail: bool,
) -> ParsedRun {
    let workflow_name = json_string(value, "workflowName")
        .or_else(|| json_string(value, "name"))
        .unwrap_or_else(|| artifact.run_id.clone());
    let status = json_string(value, "status").unwrap_or_else(|| "unknown".to_string());
    let summary_text =
        json_string(value, "summary").map(|v| safe_preview(&v, MAX_DETAIL_TEXT_CHARS));
    let result = value
        .get("result")
        .map(|v| preview_value(v, MAX_DETAIL_TEXT_CHARS));
    let script = read_script_text(claude_home, summary_path, value, include_detail);
    let agents = parse_agents(value, artifact.run_dir.as_deref());
    let phases = parse_phases(value, &agents);
    let updated_at = file_modified_ms(summary_path).or_else(|| {
        artifact
            .run_dir
            .as_ref()
            .and_then(|run_dir| file_modified_ms(run_dir))
    });

    let agent_count = json_u64(value, "agentCount")
        .map(|v| v as u32)
        .unwrap_or(agents.len() as u32);
    let phase_count = phases.len() as u32;
    let has_journal = artifact
        .run_dir
        .as_ref()
        .map(|run_dir| run_dir.join("journal.jsonl").is_file())
        .unwrap_or(false);
    let summary = WorkflowRunSummary {
        session_id: artifact.session_id.clone(),
        run_id: artifact.run_id.clone(),
        project_dir: artifact.project_dir.clone(),
        workflow_name,
        status,
        summary: summary_text
            .clone()
            .map(|s| truncate(&s, MAX_LIST_PREVIEW_CHARS)),
        default_model: json_string(value, "defaultModel"),
        start_time: json_i64(value, "startTime"),
        duration_ms: json_u64(value, "durationMs"),
        total_tokens: json_u64(value, "totalTokens").unwrap_or_else(|| {
            agents
                .iter()
                .map(|agent| agent.tokens)
                .fold(0_u64, |acc, tokens| acc.saturating_add(tokens))
        }),
        total_tool_calls: json_u64(value, "totalToolCalls").unwrap_or_else(|| {
            agents
                .iter()
                .map(|agent| agent.tool_calls)
                .fold(0_u64, |acc, count| acc.saturating_add(count))
        }),
        agent_count,
        phase_count,
        updated_at,
        script_preview: script.as_ref().map(|s| truncate(s, MAX_LIST_PREVIEW_CHARS)),
        result_preview: result.as_ref().map(|s| truncate(s, MAX_LIST_PREVIEW_CHARS)),
        has_summary_json: true,
        has_journal,
    };

    ParsedRun {
        summary,
        phases,
        agents,
        script: if include_detail { script } else { None },
        result: if include_detail { result } else { None },
    }
}

fn stub_run_from_subagents(artifact: &WorkflowArtifact, include_detail: bool) -> ParsedRun {
    let run_dir = artifact.run_dir.as_ref();
    let has_journal = run_dir
        .map(|dir| dir.join("journal.jsonl").is_file())
        .unwrap_or(false);
    let journal = run_dir
        .map(|dir| read_journal_events(&dir.join("journal.jsonl")))
        .unwrap_or_default();
    // A journal `result` row means the run reached a terminal state; otherwise a
    // present journal means it is still streaming, and no journal means unknown.
    let status = if journal.iter().any(|event| event.kind == "result") {
        "completed"
    } else if has_journal {
        "running"
    } else {
        "unknown"
    };
    let agents = run_dir.map(|dir| scan_agent_files(dir)).unwrap_or_default();
    let phases = phase_summaries_from_agents(&agents, Vec::new());
    let updated_at = run_dir.and_then(|dir| file_modified_ms(dir));
    let result = journal
        .iter()
        .rev()
        .find(|event| event.kind == "result")
        .and_then(|event| event.preview.clone());

    ParsedRun {
        summary: WorkflowRunSummary {
            session_id: artifact.session_id.clone(),
            run_id: artifact.run_id.clone(),
            project_dir: artifact.project_dir.clone(),
            workflow_name: artifact.run_id.clone(),
            status: status.to_string(),
            summary: None,
            default_model: None,
            // We only know the file mtime, not a real start time — surface it as
            // `updated_at` and leave `start_time` unset rather than conflating them.
            start_time: None,
            duration_ms: None,
            total_tokens: agents.iter().map(|agent| agent.tokens).sum(),
            total_tool_calls: agents.iter().map(|agent| agent.tool_calls).sum(),
            agent_count: agents.len() as u32,
            phase_count: phases.len() as u32,
            updated_at,
            script_preview: None,
            result_preview: result
                .as_ref()
                .map(|text| truncate(text, MAX_LIST_PREVIEW_CHARS)),
            has_summary_json: false,
            has_journal,
        },
        phases,
        agents,
        script: None,
        result: if include_detail { result } else { None },
    }
}

fn parse_phases(value: &Value, agents: &[WorkflowAgentSummary]) -> Vec<WorkflowPhaseSummary> {
    let mut phases = Vec::new();
    if let Some(raw_phases) = value.get("phases").and_then(Value::as_array) {
        for (idx, raw_phase) in raw_phases.iter().take(MAX_SYNTH_PHASES).enumerate() {
            phases.push(WorkflowPhaseSummary {
                index: idx as u32,
                title: json_string(raw_phase, "title")
                    .or_else(|| json_string(raw_phase, "name"))
                    .unwrap_or_else(|| format!("Phase {}", idx + 1)),
                detail: json_string(raw_phase, "detail")
                    .or_else(|| json_string(raw_phase, "description")),
                agent_count: 0,
                completed_agent_count: 0,
                token_count: 0,
                tool_call_count: 0,
                duration_ms: json_u64(raw_phase, "durationMs"),
            });
        }
    }
    phase_summaries_from_agents(agents, phases)
}

fn phase_summaries_from_agents(
    agents: &[WorkflowAgentSummary],
    mut phases: Vec<WorkflowPhaseSummary>,
) -> Vec<WorkflowPhaseSummary> {
    let mut seen_phase_titles: HashSet<String> = phases.iter().map(|p| p.title.clone()).collect();
    for agent in agents {
        if let Some(index) = agent.phase_index {
            let index = index as usize;
            // Cap synthesized phases: a malicious/garbage phaseIndex must not drive
            // an unbounded allocation (DoS). Out-of-range agents simply go uncounted.
            if index < MAX_SYNTH_PHASES {
                while phases.len() <= index {
                    let next_index = phases.len() as u32;
                    phases.push(WorkflowPhaseSummary {
                        index: next_index,
                        title: format!("Phase {}", next_index + 1),
                        detail: None,
                        agent_count: 0,
                        completed_agent_count: 0,
                        token_count: 0,
                        tool_call_count: 0,
                        duration_ms: None,
                    });
                }
            }
        } else if let Some(title) = agent.phase_title.as_ref() {
            if phases.len() < MAX_SYNTH_PHASES && seen_phase_titles.insert(title.clone()) {
                phases.push(WorkflowPhaseSummary {
                    index: phases.len() as u32,
                    title: title.clone(),
                    detail: None,
                    agent_count: 0,
                    completed_agent_count: 0,
                    token_count: 0,
                    tool_call_count: 0,
                    duration_ms: None,
                });
            }
        }
    }

    let title_to_index: HashMap<String, usize> = phases
        .iter()
        .enumerate()
        .map(|(idx, phase)| (phase.title.clone(), idx))
        .collect();
    for agent in agents {
        let target = agent
            .phase_index
            .and_then(|idx| phases.get(idx as usize).map(|_| idx as usize))
            .or_else(|| {
                agent
                    .phase_title
                    .as_ref()
                    .and_then(|title| title_to_index.get(title).copied())
            });
        if let Some(index) = target {
            let phase = &mut phases[index];
            phase.agent_count += 1;
            if matches!(agent.state.as_str(), "completed" | "done" | "success") {
                phase.completed_agent_count += 1;
            }
            phase.token_count = phase.token_count.saturating_add(agent.tokens);
            phase.tool_call_count = phase.tool_call_count.saturating_add(agent.tool_calls);
            if let Some(duration) = agent.duration_ms {
                phase.duration_ms = Some(phase.duration_ms.unwrap_or(0).saturating_add(duration));
            }
        }
    }
    phases
}

fn parse_agents(value: &Value, run_dir: Option<&Path>) -> Vec<WorkflowAgentSummary> {
    let mut agents = Vec::new();
    if let Some(progress) = value.get("workflowProgress").and_then(Value::as_array) {
        for item in progress {
            if item.get("type").and_then(Value::as_str) != Some("workflow_agent") {
                continue;
            }
            let Some(agent_id) = json_string(item, "agentId") else {
                continue;
            };
            let events_available = run_dir
                .map(|dir| find_agent_jsonl(dir, &agent_id).is_some())
                .unwrap_or(false);
            agents.push(WorkflowAgentSummary {
                agent_id,
                label: json_string(item, "label"),
                phase_index: json_u64(item, "phaseIndex").map(|v| v.saturating_sub(1) as u32),
                phase_title: json_string(item, "phaseTitle"),
                model: json_string(item, "model"),
                state: json_string(item, "state").unwrap_or_else(|| "unknown".to_string()),
                started_at: json_string(item, "startedAt"),
                queued_at: json_string(item, "queuedAt"),
                last_progress_at: json_string(item, "lastProgressAt"),
                tokens: json_u64(item, "tokens").unwrap_or(0),
                tool_calls: json_u64(item, "toolCalls").unwrap_or(0),
                duration_ms: json_u64(item, "durationMs"),
                prompt_preview: json_string(item, "promptPreview")
                    .map(|s| safe_preview(&s, MAX_DETAIL_TEXT_CHARS)),
                result_preview: json_string(item, "resultPreview")
                    .map(|s| safe_preview(&s, MAX_DETAIL_TEXT_CHARS)),
                events_available,
            });
        }
    }

    if let Some(dir) = run_dir {
        let known: HashSet<String> = agents.iter().map(|agent| agent.agent_id.clone()).collect();
        for agent in scan_agent_files(dir) {
            if !known.iter().any(|id| ids_match(id, &agent.agent_id)) {
                agents.push(agent);
            }
        }
    }
    agents
}

fn read_script_text(
    claude_home: &Path,
    summary_path: &Path,
    value: &Value,
    include_detail: bool,
) -> Option<String> {
    let limit = if include_detail {
        MAX_DETAIL_TEXT_CHARS
    } else {
        MAX_LIST_PREVIEW_CHARS
    };

    if let Some(script) = json_string(value, "script") {
        return Some(safe_preview(&script, limit));
    }

    if let Some(path_value) = json_string(value, "scriptPath") {
        let path = std::path::PathBuf::from(path_value);
        if path.is_file() && is_existing_path_under(claude_home, &path) {
            return read_text_capped(&path).map(|script| safe_preview(&script, limit));
        }
    }

    let run_id = json_string(value, "runId").or_else(|| {
        summary_path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(String::from)
    })?;
    let scripts_dir = summary_path.parent()?.join("scripts");
    if !scripts_dir.is_dir() {
        return None;
    }
    let entries = std::fs::read_dir(scripts_dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        let stem = path.file_stem().and_then(|name| name.to_str());
        // Match by run-id token boundary, not a loose substring, so wf_12 cannot
        // attach the script for wf_123. Workflow scripts are `<name>-<run_id>.js`.
        let is_match = path.extension().and_then(|ext| ext.to_str()) == Some("js")
            && stem
                .map(|stem| stem == run_id || stem.ends_with(&format!("-{run_id}")))
                .unwrap_or(false);
        if is_match && is_existing_path_under(claude_home, &path) {
            return read_text_capped(&path).map(|script| safe_preview(&script, limit));
        }
    }
    None
}
