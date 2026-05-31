use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use super::*;

fn fixture_home() -> TempDir {
    TempDir::new().unwrap()
}

fn session_dir(home: &Path) -> PathBuf {
    home.join("projects").join("proj-a").join("sess-1")
}

#[test]
fn parses_completed_workflow_summary_json() {
    let tmp = fixture_home();
    let workflows = session_dir(tmp.path()).join("workflows");
    let scripts = workflows.join("scripts");
    let run_dir = session_dir(tmp.path())
        .join("subagents")
        .join("workflows")
        .join("wf_done");
    fs::create_dir_all(&scripts).unwrap();
    fs::create_dir_all(&run_dir).unwrap();
    fs::write(
        workflows.join("wf_done.json"),
        serde_json::json!({
            "runId": "wf_done",
            "workflowName": "Ship plan",
            "status": "completed",
            "summary": "Design and verify",
            "defaultModel": "claude-opus-4-1",
            "startTime": 1780247179758_i64,
            "durationMs": 42,
            "totalTokens": 1234,
            "totalToolCalls": 7,
            "agentCount": 1,
            "phases": [{"title": "Map", "detail": "Parallel discovery"}],
            "workflowProgress": [{
                "type": "workflow_agent",
                "agentId": "abc",
                "label": "Discovery",
                "phaseIndex": 1,
                "phaseTitle": "Map",
                "model": "claude",
                "state": "completed",
                "tokens": 12,
                "toolCalls": 3,
                "resultPreview": "Done"
            }],
            "result": {"ok": true}
        })
        .to_string(),
    )
    .unwrap();
    fs::write(
        scripts.join("ship-plan-wf_done.js"),
        "phase('Map')\nreturn {}",
    )
    .unwrap();
    fs::write(run_dir.join("agent-abc.jsonl"), "{}\n").unwrap();

    let scan = scan_workflow_runs(tmp.path());
    assert_eq!(scan.warnings, Vec::<String>::new());
    assert_eq!(scan.runs.len(), 1);
    assert_eq!(scan.runs[0].workflow_name, "Ship plan");
    assert_eq!(scan.runs[0].total_tokens, 1234);

    let detail = get_workflow_run(tmp.path(), "sess-1", "wf_done")
        .unwrap()
        .unwrap();
    assert_eq!(detail.phases[0].agent_count, 1);
    assert!(detail.script.unwrap().contains("phase('Map')"));
    assert!(detail.agents[0].events_available);
}

#[test]
fn infers_partial_run_from_subagent_directory() {
    let tmp = fixture_home();
    let run_dir = session_dir(tmp.path())
        .join("subagents")
        .join("workflows")
        .join("wf_partial");
    fs::create_dir_all(&run_dir).unwrap();
    fs::write(
        run_dir.join("journal.jsonl"),
        r#"{"type":"started","agentId":"abc"}"#,
    )
    .unwrap();
    fs::write(
        run_dir.join("agent-abc.jsonl"),
        r#"{"type":"user","message":{"role":"user","content":"hello"}}"#,
    )
    .unwrap();

    let scan = scan_workflow_runs(tmp.path());
    assert_eq!(scan.runs.len(), 1);
    assert_eq!(scan.runs[0].status, "running");
    assert!(!scan.runs[0].has_summary_json);
    assert_eq!(scan.runs[0].agent_count, 1);
}

#[test]
fn subagent_only_run_with_result_event_is_completed() {
    let tmp = fixture_home();
    let run_dir = session_dir(tmp.path())
        .join("subagents")
        .join("workflows")
        .join("wf_finished");
    fs::create_dir_all(&run_dir).unwrap();
    fs::write(
        run_dir.join("journal.jsonl"),
        "{\"type\":\"started\",\"agentId\":\"abc\"}\n{\"type\":\"result\",\"result\":\"all done\"}\n",
    )
    .unwrap();

    let scan = scan_workflow_runs(tmp.path());
    assert_eq!(scan.runs.len(), 1);
    assert_eq!(scan.runs[0].status, "completed");
    assert!(scan.runs[0].start_time.is_none());
}

#[test]
fn skips_malformed_json_and_jsonl_rows() {
    let tmp = fixture_home();
    let workflows = session_dir(tmp.path()).join("workflows");
    let run_dir = session_dir(tmp.path())
        .join("subagents")
        .join("workflows")
        .join("wf_good");
    fs::create_dir_all(&workflows).unwrap();
    fs::create_dir_all(&run_dir).unwrap();
    fs::write(workflows.join("wf_bad.json"), "{not-json").unwrap();
    fs::write(
        workflows.join("wf_good.json"),
        r#"{"runId":"wf_good","workflowName":"Good","status":"completed"}"#,
    )
    .unwrap();
    fs::write(
        run_dir.join("agent-abc.jsonl"),
        "bad\n{\"message\":{\"role\":\"assistant\",\"content\":\"ok\"}}\n",
    )
    .unwrap();

    let scan = scan_workflow_runs(tmp.path());
    assert_eq!(scan.runs.len(), 1);
    assert_eq!(scan.warnings.len(), 1);
    let agent = get_workflow_agent(tmp.path(), "sess-1", "wf_good", "abc")
        .unwrap()
        .unwrap();
    assert_eq!(agent.events.len(), 1);
    assert_eq!(agent.events[0].preview, "ok");
}

#[test]
fn missing_directories_return_empty_results() {
    let tmp = fixture_home();
    let scan = scan_workflow_runs(tmp.path());
    assert!(scan.runs.is_empty());
    assert!(scan.warnings.is_empty());
}

#[test]
fn rejects_traversal_and_off_root_script_paths() {
    let tmp = fixture_home();
    let workflows = session_dir(tmp.path()).join("workflows");
    fs::create_dir_all(&workflows).unwrap();
    fs::write(
        workflows.join("wf_safe.json"),
        serde_json::json!({
            "runId": "wf_safe",
            "workflowName": "Safe",
            "status": "completed",
            "scriptPath": "/etc/passwd"
        })
        .to_string(),
    )
    .unwrap();

    assert!(get_workflow_run(tmp.path(), "../sess", "wf_safe").is_err());
    assert!(get_workflow_agent(tmp.path(), "sess-1", "wf_safe", "../agent").is_err());

    let detail = get_workflow_run(tmp.path(), "sess-1", "wf_safe")
        .unwrap()
        .unwrap();
    assert_eq!(detail.script, None);
}

#[test]
fn redacts_secrets_in_run_and_agent_previews() {
    let tmp = fixture_home();
    let workflows = session_dir(tmp.path()).join("workflows");
    let run_dir = session_dir(tmp.path())
        .join("subagents")
        .join("workflows")
        .join("wf_secrets");
    fs::create_dir_all(&workflows).unwrap();
    fs::create_dir_all(&run_dir).unwrap();
    fs::write(
        workflows.join("wf_secrets.json"),
        serde_json::json!({
            "runId": "wf_secrets",
            "workflowName": "Secrets",
            "status": "completed",
            "script": "const ANTHROPIC_API_KEY = 'sk-ant-abcdefghijklmnopqrstuvwxyz0123'\nrun()",
            "workflowProgress": [{
                "type": "workflow_agent",
                "agentId": "abc",
                "state": "completed",
                "promptPreview": "export AUTH_TOKEN=supersecretvalue123",
                "resultPreview": "ok"
            }]
        })
        .to_string(),
    )
    .unwrap();
    fs::write(
        run_dir.join("agent-abc.jsonl"),
        "{\"message\":{\"role\":\"assistant\",\"content\":\"Bearer abcdefghijklmnopqrstuvwxyz\"}}\n",
    )
    .unwrap();

    let detail = get_workflow_run(tmp.path(), "sess-1", "wf_secrets")
        .unwrap()
        .unwrap();
    let script = detail.script.unwrap();
    assert!(
        !script.contains("sk-ant-abcdefghijklmnopqrstuvwxyz0123"),
        "script leaked key: {script}"
    );
    assert!(
        script.contains("[redacted]"),
        "script not redacted: {script}"
    );

    let agent = detail.agents.iter().find(|a| a.agent_id == "abc").unwrap();
    let prompt = agent.prompt_preview.clone().unwrap();
    assert!(
        !prompt.contains("supersecretvalue123"),
        "prompt leaked: {prompt}"
    );

    let agent_detail = get_workflow_agent(tmp.path(), "sess-1", "wf_secrets", "abc")
        .unwrap()
        .unwrap();
    let event = &agent_detail.events[0].preview;
    assert!(
        !event.contains("abcdefghijklmnopqrstuvwxyz"),
        "event leaked bearer token: {event}"
    );
}

#[test]
fn hostile_phase_index_does_not_allocate_unbounded() {
    let tmp = fixture_home();
    let workflows = session_dir(tmp.path()).join("workflows");
    fs::create_dir_all(&workflows).unwrap();
    fs::write(
        workflows.join("wf_phases.json"),
        serde_json::json!({
            "runId": "wf_phases",
            "workflowName": "Phases",
            "status": "running",
            "workflowProgress": [{
                "type": "workflow_agent",
                "agentId": "abc",
                "state": "running",
                "phaseIndex": 4_000_000_000u64
            }]
        })
        .to_string(),
    )
    .unwrap();

    let detail = get_workflow_run(tmp.path(), "sess-1", "wf_phases")
        .unwrap()
        .unwrap();
    assert!(
        detail.phases.len() <= MAX_SYNTH_PHASES,
        "phase synthesis was not capped: {}",
        detail.phases.len()
    );
}

#[test]
fn claude_home_entries_keep_sensitive_dirs_metadata_only() {
    let tmp = fixture_home();
    fs::create_dir_all(tmp.path().join("session-env").join("abc")).unwrap();
    fs::write(
        tmp.path().join("session-env").join("abc").join("env.json"),
        r#"{"TOKEN":"secret"}"#,
    )
    .unwrap();
    fs::create_dir_all(tmp.path().join("hooks")).unwrap();
    fs::write(tmp.path().join("hooks").join("stop.sh"), "echo stop").unwrap();
    fs::write(
        tmp.path().join("hooks").join("secret.sh"),
        "API_TOKEN=super-secret",
    )
    .unwrap();

    let entries = scan_claude_home_entries(tmp.path());
    let session_env = entries
        .iter()
        .find(|entry| entry.kind == "session-env")
        .unwrap();
    assert!(session_env.metadata_only);
    assert_eq!(session_env.preview, None);
    let hook = entries
        .iter()
        .find(|entry| entry.name == "stop.sh")
        .unwrap();
    assert_eq!(hook.preview.as_deref(), Some("echo stop"));
    let secret_hook = entries
        .iter()
        .find(|entry| entry.name == "secret.sh")
        .unwrap();
    assert_eq!(secret_hook.preview.as_deref(), Some("API_TOKEN=[redacted]"));
}
