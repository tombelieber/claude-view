//! Locate workflow artifacts under `~/.claude/projects/<project>/<session>/`.
//!
//! A run is described by an optional `workflows/<run_id>.json` summary and/or an
//! optional `subagents/workflows/<run_id>/` directory of per-agent JSONL files.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use super::ids::{validate_run_id, validate_session_id};
use super::types::WorkflowArtifact;

/// Full scan: every run across every project/session (drives the run list).
pub(crate) fn discover_workflow_artifacts(claude_home: &Path) -> Vec<WorkflowArtifact> {
    let projects_dir = claude_home.join("projects");
    if !projects_dir.is_dir() {
        return Vec::new();
    }

    let mut artifacts: BTreeMap<(String, String, String), WorkflowArtifact> = BTreeMap::new();
    let project_entries = match fs::read_dir(&projects_dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    for project_entry in project_entries.flatten() {
        let project_path = project_entry.path();
        if !project_path.is_dir() {
            continue;
        }
        let project_dir = project_entry.file_name().to_string_lossy().to_string();
        let session_entries = match fs::read_dir(&project_path) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for session_entry in session_entries.flatten() {
            let session_path = session_entry.path();
            if !session_path.is_dir() {
                continue;
            }
            let session_id = session_entry.file_name().to_string_lossy().to_string();
            if validate_session_id(&session_id).is_err() {
                continue;
            }

            let workflows_dir = session_path.join("workflows");
            if workflows_dir.is_dir() {
                if let Ok(entries) = fs::read_dir(&workflows_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file() && is_workflow_summary_path(&path) {
                            let Some(run_id) = path
                                .file_stem()
                                .and_then(|stem| stem.to_str())
                                .map(str::to_string)
                            else {
                                continue;
                            };
                            if validate_run_id(&run_id).is_err() {
                                continue;
                            }
                            let key = (project_dir.clone(), session_id.clone(), run_id.clone());
                            artifacts
                                .entry(key)
                                .and_modify(|artifact| artifact.summary_path = Some(path.clone()))
                                .or_insert_with(|| WorkflowArtifact {
                                    project_dir: project_dir.clone(),
                                    session_id: session_id.clone(),
                                    run_id,
                                    summary_path: Some(path.clone()),
                                    run_dir: None,
                                });
                        }
                    }
                }
            }

            let subagent_root = session_path.join("subagents").join("workflows");
            if subagent_root.is_dir() {
                if let Ok(entries) = fs::read_dir(&subagent_root) {
                    for entry in entries.flatten() {
                        let run_dir = entry.path();
                        if !run_dir.is_dir() {
                            continue;
                        }
                        let run_id = entry.file_name().to_string_lossy().to_string();
                        if validate_run_id(&run_id).is_err() {
                            continue;
                        }
                        let key = (project_dir.clone(), session_id.clone(), run_id.clone());
                        artifacts
                            .entry(key)
                            .and_modify(|artifact| artifact.run_dir = Some(run_dir.clone()))
                            .or_insert_with(|| WorkflowArtifact {
                                project_dir: project_dir.clone(),
                                session_id: session_id.clone(),
                                run_id,
                                summary_path: None,
                                run_dir: Some(run_dir.clone()),
                            });
                    }
                }
            }
        }
    }
    artifacts.into_values().collect()
}

/// Targeted lookup for a single run. Only stats the relevant session directory
/// instead of walking the whole projects tree. Callers MUST validate
/// `session_id`/`run_id` first (the public entry points do).
pub(crate) fn find_workflow_artifact(
    claude_home: &Path,
    session_id: &str,
    run_id: &str,
) -> Option<WorkflowArtifact> {
    let projects_dir = claude_home.join("projects");
    let project_entries = fs::read_dir(&projects_dir).ok()?;
    for project_entry in project_entries.flatten() {
        let project_path = project_entry.path();
        if !project_path.is_dir() {
            continue;
        }
        let session_path = project_path.join(session_id);
        if !session_path.is_dir() {
            continue;
        }
        let summary_path = {
            let candidate = session_path
                .join("workflows")
                .join(format!("{run_id}.json"));
            (candidate.is_file() && is_workflow_summary_path(&candidate)).then_some(candidate)
        };
        let run_dir = {
            let candidate = session_path
                .join("subagents")
                .join("workflows")
                .join(run_id);
            candidate.is_dir().then_some(candidate)
        };
        if summary_path.is_some() || run_dir.is_some() {
            return Some(WorkflowArtifact {
                project_dir: project_entry.file_name().to_string_lossy().to_string(),
                session_id: session_id.to_string(),
                run_id: run_id.to_string(),
                summary_path,
                run_dir,
            });
        }
    }
    None
}

pub(crate) fn is_workflow_summary_path(path: &Path) -> bool {
    path.extension().and_then(|ext| ext.to_str()) == Some("json")
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.starts_with("wf_"))
            .unwrap_or(false)
}
