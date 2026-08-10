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
fn parses_claude_code_agent_jsonl_as_structured_events() {
    let tmp = fixture_home();
    let workflows = session_dir(tmp.path()).join("workflows");
    let run_dir = session_dir(tmp.path())
        .join("subagents")
        .join("workflows")
        .join("wf_tools");
    fs::create_dir_all(&workflows).unwrap();
    fs::create_dir_all(&run_dir).unwrap();
    fs::write(
        workflows.join("wf_tools.json"),
        serde_json::json!({
            "runId": "wf_tools",
            "workflowName": "Tools",
            "status": "completed",
            "workflowProgress": [{
                "type": "workflow_agent",
                "agentId": "abc",
                "state": "completed",
                "lastToolName": "Bash",
                "lastToolSummary": "python3 /repo/scripts/check.py"
            }]
        })
        .to_string(),
    )
    .unwrap();
    fs::write(
        run_dir.join("agent-abc.jsonl"),
        [
            serde_json::json!({
                "type": "assistant",
                "message": {
                    "role": "assistant",
                    "content": [
                        {"type": "text", "text": "I will inspect and update files."},
                        {
                            "type": "tool_use",
                            "id": "toolu_read",
                            "name": "Read",
                            "input": {"file_path": "/repo/src/main.rs"}
                        },
                        {
                            "type": "tool_use",
                            "id": "toolu_write",
                            "name": "Write",
                            "input": {
                                "file_path": "/repo/out.txt",
                                "content": "AUTH_TOKEN=supersecretvalue123\nlarge body that should not dominate the preview"
                            }
                        },
                        {
                            "type": "tool_use",
                            "id": "toolu_read_again",
                            "name": "Read",
                            "input": {"file_path": "/repo/src/lib.rs"}
                        },
                        {
                            "type": "tool_use",
                            "id": "toolu_bash",
                            "name": "Bash",
                            "input": {
                                "command": "python3 /repo/scripts/query_case_punishments.py --case-dir /tmp/case-04 --api-token secretvalue123",
                                "description": "Query punishments for the case"
                            }
                        }
                    ]
                },
                "timestamp": "2026-03-11T10:00:00.000Z"
            })
            .to_string(),
            serde_json::json!({
                "type": "assistant",
                "message": {
                    "role": "assistant",
                    "content": [
                        {"type": "thinking", "thinking": "Compare the two files before editing."}
                    ]
                },
                "timestamp": "2026-03-11T10:00:01.000Z"
            })
            .to_string(),
            serde_json::json!({
                "type": "user",
                "message": {
                    "role": "user",
                    "content": [
                        {
                            "type": "tool_result",
                            "tool_use_id": "toolu_read",
                            "content": "fn main() {}\nfn helper() {}"
                        }
                    ]
                },
                "timestamp": "2026-03-11T10:00:02.000Z"
            })
            .to_string(),
        ]
        .join("\n"),
    )
    .unwrap();

    let agent = get_workflow_agent(tmp.path(), "sess-1", "wf_tools", "abc")
        .unwrap()
        .unwrap();
    assert_eq!(agent.events.len(), 7);
    assert_eq!(agent.summary.last_tool_name.as_deref(), Some("Bash"));
    assert_eq!(
        agent.summary.last_tool_summary.as_deref(),
        Some("python3 /repo/scripts/check.py")
    );

    let text = &agent.events[0];
    assert_eq!(text.kind, "message");
    assert_eq!(text.preview, "I will inspect and update files.");

    let read = &agent.events[1];
    assert_eq!(read.kind, "tool_use");
    assert_eq!(read.role.as_deref(), Some("assistant"));
    assert_eq!(read.tool_use_id.as_deref(), Some("toolu_read"));
    assert_eq!(read.tool_names, vec!["Read"]);
    assert!(read.timestamp.is_some(), "ISO timestamp was not parsed");
    assert_eq!(
        read.tool_input_preview.as_deref(),
        Some("Read: file_path=/repo/src/main.rs")
    );

    let write = &agent.events[2];
    assert_eq!(write.kind, "tool_use");
    assert_eq!(write.tool_use_id.as_deref(), Some("toolu_write"));
    assert_eq!(write.tool_names, vec!["Write"]);
    let input_preview = write.tool_input_preview.as_deref().unwrap();
    assert!(input_preview.contains("/repo/out.txt"));
    assert!(
        !input_preview.contains("supersecretvalue123"),
        "tool input leaked secret content: {input_preview}"
    );
    assert!(
        !input_preview.contains("large body that should not dominate"),
        "Write content dominated the input preview: {input_preview}"
    );

    let read_again = &agent.events[3];
    assert_eq!(read_again.kind, "tool_use");
    assert_eq!(read_again.tool_use_id.as_deref(), Some("toolu_read_again"));
    assert_eq!(read_again.tool_names, vec!["Read"]);
    assert_eq!(
        read_again.tool_input_preview.as_deref(),
        Some("Read: file_path=/repo/src/lib.rs")
    );

    let bash = &agent.events[4];
    assert_eq!(bash.kind, "tool_use");
    assert_eq!(bash.tool_use_id.as_deref(), Some("toolu_bash"));
    assert_eq!(bash.tool_names, vec!["Bash"]);
    assert_eq!(
        bash.tool_input_preview.as_deref(),
        Some("Bash: python3 /repo/scripts/query_case_punishments.py --case-dir /tmp/case-04 --api-token [redacted]")
    );
    assert!(
        !bash.preview.contains("secretvalue123"),
        "Bash command preview leaked CLI token: {}",
        bash.preview
    );

    let thinking = &agent.events[5];
    assert_eq!(thinking.kind, "thinking");
    assert_eq!(thinking.preview, "Compare the two files before editing.");
    assert!(
        !thinking.preview.contains("\"type\""),
        "thinking preview dumped raw JSON: {}",
        thinking.preview
    );

    let tool_result = &agent.events[6];
    assert_eq!(tool_result.kind, "tool_result");
    assert_eq!(tool_result.tool_use_id.as_deref(), Some("toolu_read"));
    assert_eq!(tool_result.tool_names, vec!["Read"]);
    assert_eq!(
        tool_result.tool_result_preview.as_deref(),
        Some("fn main() {}\nfn helper() {}")
    );
    assert!(
        !tool_result.preview.contains("\"tool_use_id\""),
        "tool_result preview dumped raw JSON: {}",
        tool_result.preview
    );
}

#[test]
fn redacts_and_bounds_untrusted_tool_names() {
    let tmp = fixture_home();
    let workflows = session_dir(tmp.path()).join("workflows");
    let run_dir = session_dir(tmp.path())
        .join("subagents")
        .join("workflows")
        .join("wf_tool_names");
    fs::create_dir_all(&workflows).unwrap();
    fs::create_dir_all(&run_dir).unwrap();
    fs::write(
        workflows.join("wf_tool_names.json"),
        serde_json::json!({
            "runId": "wf_tool_names",
            "workflowProgress": [{
                "type": "workflow_agent",
                "agentId": "abc",
                "state": "completed"
            }]
        })
        .to_string(),
    )
    .unwrap();
    let hostile_name = format!("Bash API_TOKEN=supersecretvalue123 {}", "x".repeat(400));
    fs::write(
        run_dir.join("agent-abc.jsonl"),
        [
            serde_json::json!({
                "type": "assistant",
                "message": {
                    "role": "assistant",
                    "content": [{
                        "type": "tool_use",
                        "id": "toolu_hostile",
                        "name": hostile_name,
                        "input": {"command": "echo ok"}
                    }]
                }
            })
            .to_string(),
            serde_json::json!({
                "type": "user",
                "message": {
                    "role": "user",
                    "content": [{
                        "type": "tool_result",
                        "tool_use_id": "toolu_hostile",
                        "content": "ok"
                    }]
                }
            })
            .to_string(),
        ]
        .join("\n"),
    )
    .unwrap();

    let agent = get_workflow_agent(tmp.path(), "sess-1", "wf_tool_names", "abc")
        .unwrap()
        .unwrap();
    let tool_use = &agent.events[0];
    assert_eq!(tool_use.kind, "tool_use");
    assert_eq!(tool_use.tool_names.len(), 1);
    assert!(
        tool_use.tool_names[0].len() <= 80,
        "tool name was not bounded: {} chars",
        tool_use.tool_names[0].len()
    );
    assert!(
        !tool_use.tool_names[0].contains("supersecretvalue123"),
        "tool name leaked a secret: {}",
        tool_use.tool_names[0]
    );
    assert!(
        !tool_use.preview.contains("supersecretvalue123"),
        "tool preview leaked a secret: {}",
        tool_use.preview
    );
    let tool_result = &agent.events[1];
    assert!(
        !tool_result.tool_names[0].contains("supersecretvalue123"),
        "tool result name leaked a secret: {}",
        tool_result.tool_names[0]
    );
}

#[test]
fn redacts_cli_secret_flags_before_tool_input_truncation() {
    let tmp = fixture_home();
    let workflows = session_dir(tmp.path()).join("workflows");
    let run_dir = session_dir(tmp.path())
        .join("subagents")
        .join("workflows")
        .join("wf_long_command");
    fs::create_dir_all(&workflows).unwrap();
    fs::create_dir_all(&run_dir).unwrap();
    fs::write(
        workflows.join("wf_long_command.json"),
        serde_json::json!({
            "runId": "wf_long_command",
            "workflowProgress": [{
                "type": "workflow_agent",
                "agentId": "abc",
                "state": "completed"
            }]
        })
        .to_string(),
    )
    .unwrap();
    let command = format!(
        "python3 /repo/scripts/run.py {} --api-token abcdefghijklmnopqrstuvwxyz",
        "x".repeat(1156)
    );
    fs::write(
        run_dir.join("agent-abc.jsonl"),
        serde_json::json!({
            "type": "assistant",
            "message": {
                "role": "assistant",
                "content": [{
                    "type": "tool_use",
                    "id": "toolu_bash",
                    "name": "Bash",
                    "input": {"command": command}
                }]
            }
        })
        .to_string(),
    )
    .unwrap();

    let agent = get_workflow_agent(tmp.path(), "sess-1", "wf_long_command", "abc")
        .unwrap()
        .unwrap();
    let preview = agent.events[0].tool_input_preview.as_deref().unwrap();
    assert!(
        !preview.contains("abcdef"),
        "tool input leaked CLI token prefix across truncation boundary: {preview}"
    );
    assert!(
        !preview.contains("--api-token ab"),
        "tool input leaked an unredacted token flag: {preview}"
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
