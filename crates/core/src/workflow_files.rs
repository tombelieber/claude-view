//! Read-only scanners for Claude Code dynamic workflow artifacts.
//!
//! Claude Code owns these files. claude-view treats them as untrusted input:
//! parse JSON/JSONL defensively, never execute workflow scripts, cap reads and
//! previews, redact secret-like content before it leaves the process, and reject
//! identifier/path traversal before reading detail artifacts.
//!
//! Decomposed by concern: [`types`] (wire structs), [`discovery`] (locate runs),
//! [`runs`] (parse a run), [`agents`]/[`journal`] (per-agent + runtime events),
//! [`claude_home`] (the `~/.claude` browser), [`preview`] (truncate + redact),
//! [`ids`] (validation), [`fsjson`] (capped reads + JSON access).

use std::path::{Path, PathBuf};

mod agents;
mod claude_home;
mod discovery;
mod fsjson;
mod ids;
mod journal;
mod preview;
mod runs;
mod types;

#[cfg(test)]
mod tests;

pub use claude_home::scan_claude_home_entries;
pub use types::*;

use agents::{
    canonical_agent_id_from_path, find_agent_jsonl, first_event_preview, last_assistant_preview,
    read_agent_events, read_agent_meta_preview,
};
use discovery::{discover_workflow_artifacts, find_workflow_artifact};
use ids::{ids_match, validate_agent_id, validate_run_id, validate_session_id};
use journal::read_journal_events;
use runs::build_run_from_artifact;

// Bounds — every read and preview is capped so a hostile artifact cannot OOM or
// flood the server. Centralized here; submodules reference them via `super::`.
pub(crate) const MAX_LIST_PREVIEW_CHARS: usize = 280;
pub(crate) const MAX_DETAIL_TEXT_CHARS: usize = 16_000;
pub(crate) const MAX_AGENT_EVENT_CHARS: usize = 800;
pub(crate) const MAX_AGENT_EVENTS: usize = 120;
pub(crate) const MAX_JOURNAL_EVENTS: usize = 80;
pub(crate) const MAX_CLAUDE_HOME_PREVIEW_CHARS: usize = 2_400;
pub(crate) const MAX_CLAUDE_HOME_WALK: usize = 2_000;
pub(crate) const MAX_SYNTH_PHASES: usize = 256;
pub(crate) const MAX_READ_BYTES: usize = 4_000_000;
/// Hard cap on runs returned by a single list scan (newest-first), so the
/// endpoint stays bounded no matter how deep the session history is.
pub(crate) const MAX_WORKFLOW_RUNS: usize = 2_000;

/// Resolve Claude home. `CLAUDE_HOME` is honored for tests and advanced users;
/// otherwise this returns `~/.claude`.
pub fn claude_home_dir() -> Option<PathBuf> {
    std::env::var_os("CLAUDE_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".claude")))
}

pub fn scan_workflow_runs(claude_home: &Path) -> WorkflowScanResult {
    let artifacts = discover_workflow_artifacts(claude_home);
    let mut result = WorkflowScanResult::default();

    for artifact in artifacts {
        match build_run_from_artifact(claude_home, &artifact, false) {
            Some(parsed) => result.runs.push(parsed.summary),
            None => result.warnings.push(format!(
                "Skipped malformed workflow artifact {} / {}",
                artifact.session_id, artifact.run_id
            )),
        }
    }

    result.runs.sort_by(|a, b| {
        b.start_time
            .or(b.updated_at)
            .unwrap_or(0)
            .cmp(&a.start_time.or(a.updated_at).unwrap_or(0))
            .then_with(|| a.workflow_name.cmp(&b.workflow_name))
    });

    if result.runs.len() > MAX_WORKFLOW_RUNS {
        let dropped = result.runs.len() - MAX_WORKFLOW_RUNS;
        result.runs.truncate(MAX_WORKFLOW_RUNS);
        result.warnings.push(format!(
            "Workflow run list capped at {MAX_WORKFLOW_RUNS}; {dropped} older runs not shown"
        ));
    }
    result
}

pub fn get_workflow_run(
    claude_home: &Path,
    session_id: &str,
    run_id: &str,
) -> Result<Option<WorkflowRunDetail>, WorkflowArtifactError> {
    validate_session_id(session_id)?;
    validate_run_id(run_id)?;

    let Some(artifact) = find_workflow_artifact(claude_home, session_id, run_id) else {
        return Ok(None);
    };
    let Some(parsed) = build_run_from_artifact(claude_home, &artifact, true) else {
        return Ok(None);
    };
    let journal = artifact
        .run_dir
        .as_ref()
        .map(|run_dir| read_journal_events(&run_dir.join("journal.jsonl")))
        .unwrap_or_default();
    let artifact_relative_path = artifact
        .summary_path
        .as_ref()
        .or(artifact.run_dir.as_ref())
        .and_then(|path| path.strip_prefix(claude_home).ok())
        .map(|path| path.to_string_lossy().to_string());

    Ok(Some(WorkflowRunDetail {
        summary: parsed.summary,
        phases: parsed.phases,
        agents: parsed.agents,
        script: parsed.script,
        result: parsed.result,
        journal,
        artifact_relative_path,
    }))
}

pub fn get_workflow_agent(
    claude_home: &Path,
    session_id: &str,
    run_id: &str,
    agent_id: &str,
) -> Result<Option<WorkflowAgentDetail>, WorkflowArtifactError> {
    validate_session_id(session_id)?;
    validate_run_id(run_id)?;
    validate_agent_id(agent_id)?;

    // Single targeted lookup — reused for both the run summary and the run dir.
    let Some(artifact) = find_workflow_artifact(claude_home, session_id, run_id) else {
        return Ok(None);
    };
    let Some(parsed) = build_run_from_artifact(claude_home, &artifact, true) else {
        return Ok(None);
    };
    let Some(run_dir) = artifact.run_dir.as_ref() else {
        return Ok(None);
    };
    let Some(agent_file) = find_agent_jsonl(run_dir, agent_id) else {
        return Ok(None);
    };

    let canonical_id = canonical_agent_id_from_path(&agent_file).unwrap_or_else(|| {
        agent_id
            .strip_prefix("agent-")
            .unwrap_or(agent_id)
            .to_string()
    });
    let summary = parsed
        .agents
        .iter()
        .find(|agent| ids_match(&agent.agent_id, &canonical_id))
        .cloned()
        .unwrap_or_else(|| WorkflowAgentSummary {
            agent_id: canonical_id.clone(),
            label: None,
            phase_index: None,
            phase_title: None,
            model: None,
            state: "unknown".to_string(),
            started_at: None,
            queued_at: None,
            last_progress_at: None,
            tokens: 0,
            tool_calls: 0,
            duration_ms: None,
            prompt_preview: None,
            result_preview: None,
            events_available: true,
        });

    let meta_preview = read_agent_meta_preview(run_dir, &canonical_id);
    let events = read_agent_events(&agent_file);
    let prompt_preview = summary
        .prompt_preview
        .clone()
        .or_else(|| first_event_preview(&events, "user"));
    let result_preview = summary
        .result_preview
        .clone()
        .or_else(|| last_assistant_preview(&events));

    Ok(Some(WorkflowAgentDetail {
        summary,
        prompt_preview,
        result_preview,
        events,
        meta_preview,
    }))
}
