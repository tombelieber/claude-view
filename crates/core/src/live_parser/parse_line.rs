//! Main single-line parser that orchestrates extraction from all sub-modules.

use crate::progress::{RawTaskCreate, RawTaskIdAssignment, RawTaskUpdate, RawTodoItem};

use super::content::{extract_content_and_tools, extract_task_notification};
use super::finders::TailFinders;
use super::sub_agents::{
    extract_sub_agent_progress, extract_sub_agent_result, extract_sub_agent_spawns,
};
use super::types::{HookProgressData, LineType, LiveLine, PASTED_PATH_PATTERN};
use super::usage::{extract_usage, UsageTokens};

/// Classify and extract fields from a single JSONL line.
///
/// Claude Code JSONL wraps API messages inside a `"message"` field:
/// ```json
/// {"type": "assistant", "message": {"role": "assistant", "model": "...", "usage": {...}, "content": [...]}}
/// {"type": "user", "message": {"role": "user", "content": "..."}, "gitBranch": "...", "isMeta": true}
/// ```
/// We check both the top level and the nested `message` object for each field.
pub fn parse_single_line(raw: &[u8], finders: &TailFinders) -> LiveLine {
    // Parse JSON to extract structured fields
    let parsed: serde_json::Value = match serde_json::from_slice(raw) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(
                error = %e,
                preview = %String::from_utf8_lossy(&raw[..raw.len().min(200)]),
                "JSONL parse failed — returning empty stub"
            );
            return LiveLine {
                line_type: LineType::Other,
                role: None,
                content_preview: String::new(),
                content_extended: String::new(),
                tool_names: Vec::new(),
                model: None,
                input_tokens: None,
                output_tokens: None,
                cache_read_tokens: None,
                cache_creation_tokens: None,
                cache_creation_5m_tokens: None,
                cache_creation_1hr_tokens: None,
                timestamp: None,
                stop_reason: None,
                git_branch: None,
                cwd: None,
                is_meta: false,
                is_tool_result_continuation: false,
                has_system_prefix: false,
                sub_agent_spawns: Vec::new(),
                sub_agent_result: None,
                sub_agent_progress: None,
                sub_agent_notification: None,
                todo_write: None,
                task_creates: Vec::new(),
                task_updates: Vec::new(),
                task_id_assignments: Vec::new(),
                skill_names: Vec::new(),
                bash_commands: Vec::new(),
                edited_files: Vec::new(),
                is_compact_boundary: false,
                ide_file: None,
                message_id: None,
                request_id: None,
                hook_progress: None,
                slug: None,
                team_name: None,
                at_files: Vec::new(),
                pasted_paths: Vec::new(),
                entrypoint: None,
                ai_title: None,
                goal: None,
                content_byte_len: None,
            };
        }
    };

    // Classification must come from exact top-level `type`.
    let line_type = match parsed.get("type").and_then(|t| t.as_str()) {
        Some("user") => LineType::User,
        Some("assistant") => LineType::Assistant,
        Some("system") => LineType::System,
        Some("progress") => LineType::Progress,
        _ => LineType::Other,
    };

    // Extract session title from `custom-title` (user-set via /title) or `ai-title` (auto-generated).
    let ai_title = match parsed.get("type").and_then(|t| t.as_str()) {
        Some("custom-title") => parsed
            .get("customTitle")
            .and_then(|t| t.as_str())
            .map(String::from),
        Some("ai-title") => parsed
            .get("aiTitle")
            .and_then(|t| t.as_str())
            .map(String::from),
        _ => None,
    };

    // Extract the session `/goal` (the session-scoped Stop-hook condition) from whichever
    // transcript carrier it rode in on. None on the vast majority of lines.
    let goal = extract_goal(&parsed);

    // The nested message object (most fields live here in Claude Code JSONL)
    let msg = parsed.get("message");

    let role = parsed
        .get("role")
        .or_else(|| msg.and_then(|m| m.get("role")))
        .and_then(|v| v.as_str())
        .map(String::from);

    // Content can be a string OR an array of content blocks.
    // Try top-level first, then nested message.
    let content_source = if parsed.get("content").is_some() {
        &parsed
    } else if let Some(m) = msg {
        m
    } else {
        &parsed
    };
    let (
        content_preview,
        content_extended,
        tool_names,
        skill_names,
        bash_commands,
        edited_files,
        is_tool_result,
        ide_file,
        at_files,
    ) = extract_content_and_tools(content_source, finders);

    // Detect system-injected prefixes from RAW content (before stripping).
    let has_system_prefix = if line_type == LineType::User {
        let raw_content = content_source
            .get("content")
            .and_then(|c| c.as_str())
            .unwrap_or("");
        let c = raw_content.trim_start();
        c.starts_with("<local-command-caveat>")
            || c.starts_with("<local-command-stdout>")
            || c.starts_with("<command-name>/clear")
            || c.starts_with("<command-name>/context")
            || c.starts_with("This session is being continued")
            || c.starts_with("<task-notification>")
    } else {
        false
    };

    // --- Background agent completion notification ---
    let sub_agent_notification =
        if line_type == LineType::User && finders.task_notification_key.find(raw).is_some() {
            extract_task_notification(content_source)
        } else {
            None
        };

    let model = if finders.model_key.find(raw).is_some() {
        parsed
            .get("model")
            .or_else(|| msg.and_then(|m| m.get("model")))
            .and_then(|v| v.as_str())
            .map(String::from)
    } else {
        None
    };

    let UsageTokens {
        input: input_tokens,
        output: output_tokens,
        cache_read: cache_read_tokens,
        cache_creation: cache_creation_tokens,
        cache_creation_5m: cache_creation_5m_tokens,
        cache_creation_1hr: cache_creation_1hr_tokens,
    } = if finders.usage_key.find(raw).is_some() {
        // Try top-level usage, then nested message.usage
        let u = extract_usage(&parsed);
        if u.input.is_some() || u.output.is_some() {
            u
        } else if let Some(m) = msg {
            extract_usage(m)
        } else {
            UsageTokens::default()
        }
    } else {
        UsageTokens::default()
    };

    // Extract message.id and requestId for content-block dedup.
    let message_id = msg
        .and_then(|m| m.get("id"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let request_id = parsed
        .get("requestId")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let timestamp = parsed
        .get("timestamp")
        .and_then(|v| v.as_str())
        .map(String::from);

    let stop_reason = if finders.stop_reason_key.find(raw).is_some() {
        parsed
            .get("stop_reason")
            .or_else(|| msg.and_then(|m| m.get("stop_reason")))
            .and_then(|v| v.as_str())
            .map(String::from)
    } else {
        None
    };

    // Extract git branch from top-level (present on user messages)
    let git_branch = parsed
        .get("gitBranch")
        .and_then(|v| v.as_str())
        .map(String::from);

    // Extract cwd from top-level (present on user messages)
    let cwd = parsed.get("cwd").and_then(|v| v.as_str()).map(String::from);

    // Extract session slug from top-level (present on every line)
    let slug = parsed
        .get("slug")
        .and_then(|v| v.as_str())
        .map(String::from);

    // Extract entrypoint from top-level (present on the first line)
    let entrypoint = parsed
        .get("entrypoint")
        .and_then(|v| v.as_str())
        .map(String::from);

    // Extract team name from top-level (present on every line after TeamCreate)
    let team_name = parsed
        .get("teamName")
        .and_then(|v| v.as_str())
        .map(String::from);

    // Check if this is a meta message (system prompts, hooks, etc.)
    let is_meta = parsed
        .get("isMeta")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // --- Sub-agent spawn detection (assistant lines with Task/Agent tool_use) ---
    let sub_agent_spawns = if line_type == LineType::Assistant
        && (finders.task_name_key.find(raw).is_some() || finders.agent_name_key.find(raw).is_some())
    {
        extract_sub_agent_spawns(msg)
    } else {
        Vec::new()
    };

    // --- Sub-agent completion detection (user lines with toolUseResult) ---
    let sub_agent_result =
        if line_type == LineType::User && finders.tool_use_result_key.find(raw).is_some() {
            extract_sub_agent_result(&parsed, msg)
        } else {
            None
        };

    // --- Sub-agent progress detection (progress lines with agent_progress) ---
    let sub_agent_progress =
        if line_type == LineType::Progress && finders.agent_progress_key.find(raw).is_some() {
            extract_sub_agent_progress(&parsed)
        } else {
            None
        };

    // --- Hook progress detection (progress lines with hook_progress) ---
    let result_hook_progress =
        if line_type == LineType::Progress && finders.hook_progress_key.find(raw).is_some() {
            extract_hook_progress(&parsed)
        } else {
            None
        };

    // --- TodoWrite detection (assistant lines with TodoWrite tool_use) ---
    let todo_write =
        if line_type == LineType::Assistant && finders.todo_write_key.find(raw).is_some() {
            extract_todo_write(msg)
        } else {
            None
        };

    // --- TaskCreate detection (assistant lines with TaskCreate tool_use) ---
    let task_creates =
        if line_type == LineType::Assistant && finders.task_create_key.find(raw).is_some() {
            extract_task_creates(msg)
        } else {
            Vec::new()
        };

    // --- TaskUpdate detection (assistant lines with TaskUpdate tool_use) ---
    let task_updates =
        if line_type == LineType::Assistant && finders.task_update_key.find(raw).is_some() {
            extract_task_updates(msg)
        } else {
            Vec::new()
        };

    // --- TaskIdAssignment detection (user lines with toolUseResult containing task.id) ---
    let task_id_assignments =
        if line_type == LineType::User && finders.tool_use_result_key.find(raw).is_some() {
            extract_task_id_assignments(&parsed, msg)
        } else {
            Vec::new()
        };

    // --- Compact boundary detection (system lines with subtype "compact_boundary") ---
    let is_compact_boundary =
        line_type == LineType::System && finders.compact_boundary_key.find(raw).is_some();

    // --- Pasted absolute paths (user lines only, skip URLs via SIMD pre-filter) ---
    let pasted_paths = if line_type == LineType::User && !content_preview.is_empty() {
        let line_str = &content_preview;
        if !line_str.contains("://") {
            static PASTED_RE: std::sync::OnceLock<regex_lite::Regex> = std::sync::OnceLock::new();
            let re = PASTED_RE.get_or_init(|| regex_lite::Regex::new(PASTED_PATH_PATTERN).unwrap());
            re.captures_iter(line_str)
                .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
                .collect()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    let content_byte_len = if content_extended.is_empty() {
        None
    } else {
        let original_len = content_source
            .get("content")
            .and_then(|c| match c {
                serde_json::Value::String(s) => Some(s.len()),
                serde_json::Value::Array(blocks) => blocks.iter().find_map(|b| {
                    if b.get("type").and_then(|t| t.as_str()) == Some("text") {
                        b.get("text").and_then(|t| t.as_str()).map(|s| s.len())
                    } else {
                        None
                    }
                }),
                _ => None,
            })
            .unwrap_or(content_extended.len());
        Some(original_len)
    };

    LiveLine {
        line_type,
        role,
        content_preview,
        content_extended,
        tool_names,
        model,
        input_tokens,
        output_tokens,
        cache_read_tokens,
        cache_creation_tokens,
        cache_creation_5m_tokens,
        cache_creation_1hr_tokens,
        timestamp,
        stop_reason,
        git_branch,
        cwd,
        is_meta,
        is_tool_result_continuation: is_tool_result,
        has_system_prefix,
        sub_agent_spawns,
        sub_agent_result,
        sub_agent_progress,
        sub_agent_notification,
        todo_write,
        task_creates,
        task_updates,
        task_id_assignments,
        skill_names,
        bash_commands,
        edited_files,
        is_compact_boundary,
        ide_file,
        message_id,
        request_id,
        hook_progress: result_hook_progress,
        slug,
        team_name,
        at_files,
        pasted_paths,
        entrypoint,
        ai_title,
        goal,
        content_byte_len,
    }
}

/// Extract the session `/goal` — the session-scoped Stop-hook condition — from a line, if present.
///
/// `/goal` writes no file; the goal text rides into the transcript on one of three carriers,
/// each matched on an exact prefix to keep precision high:
/// 1. `type:"queue-operation"` → `content`: `"Goal set: <text>"` (typed while the agent is busy)
/// 2. `type:"attachment"` (`attachment.type:"queued_command"`) → `prompt`: `"Goal set: <text>"`
///    (auto-continuation re-injection while the goal is still active)
/// 3. `type:"user"` → `message.content`: `…condition: "<text>". Briefly acknowledge…`
///    (the universal "Stop hook is now active" directive)
///
/// The accumulator keeps the last goal seen, since goals stack / supersede.
fn extract_goal(parsed: &serde_json::Value) -> Option<String> {
    const GOAL_SET_PREFIX: &str = "Goal set: ";
    const HOOK_PREFIX: &str = "A session-scoped Stop hook is now active with condition: \"";
    const HOOK_SUFFIX: &str = "\". Briefly acknowledge";
    /// Max stored goal length in chars. Goals are a sentence or two; this only guards
    /// against a pathological transcript bloating the live-session payload.
    const MAX_GOAL_CHARS: usize = 2000;

    fn clamp(s: &str) -> Option<String> {
        let t = s.trim();
        if t.is_empty() {
            None
        } else {
            Some(t.chars().take(MAX_GOAL_CHARS).collect())
        }
    }

    match parsed.get("type").and_then(|t| t.as_str()) {
        Some("queue-operation") => parsed
            .get("content")
            .and_then(|c| c.as_str())
            .and_then(|c| c.strip_prefix(GOAL_SET_PREFIX))
            .and_then(clamp),
        Some("attachment") => parsed
            .get("attachment")
            .filter(|a| a.get("type").and_then(|t| t.as_str()) == Some("queued_command"))
            .and_then(|a| a.get("prompt"))
            .and_then(|p| p.as_str())
            .and_then(|p| p.strip_prefix(GOAL_SET_PREFIX))
            .and_then(clamp),
        Some("user") => {
            let content = parsed
                .get("message")
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_str())
                .or_else(|| parsed.get("content").and_then(|c| c.as_str()))?;
            let rest = content.strip_prefix(HOOK_PREFIX)?;
            let end = rest.find(HOOK_SUFFIX)?;
            clamp(&rest[..end])
        }
        _ => None,
    }
}

/// Extract hook progress data from a progress line.
fn extract_hook_progress(parsed: &serde_json::Value) -> Option<HookProgressData> {
    let hook_event = parsed.pointer("/data/hookEvent").and_then(|v| v.as_str())?;
    let hook_name = parsed
        .pointer("/data/hookName")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let (tool_name, source) = if let Some(pos) = hook_name.find(':') {
        let suffix = &hook_name[pos + 1..];
        if hook_event == "SessionStart" {
            (None, Some(suffix.to_string()))
        } else {
            (Some(suffix.to_string()), None)
        }
    } else {
        (None, None)
    };
    Some(HookProgressData {
        hook_event: hook_event.to_string(),
        tool_name,
        source,
    })
}

/// Extract TodoWrite items from assistant content blocks.
fn extract_todo_write(msg: Option<&serde_json::Value>) -> Option<Vec<RawTodoItem>> {
    msg.and_then(|m| m.get("content"))
        .and_then(|c| c.as_array())
        .and_then(|blocks| {
            blocks.iter().find_map(|b| {
                if b.get("type").and_then(|t| t.as_str()) == Some("tool_use")
                    && b.get("name").and_then(|n| n.as_str()) == Some("TodoWrite")
                {
                    b.get("input")
                        .and_then(|i| i.get("todos"))
                        .and_then(|t| t.as_array())
                        .map(|todos| {
                            let total = todos.len();
                            let items: Vec<_> = todos
                                .iter()
                                .filter_map(|item| {
                                    let content = item.get("content").and_then(|v| v.as_str())?;
                                    Some(RawTodoItem {
                                        content: content.to_string(),
                                        status: item
                                            .get("status")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("pending")
                                            .to_string(),
                                        active_form: item
                                            .get("activeForm")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("")
                                            .to_string(),
                                    })
                                })
                                .collect();
                            let skipped = total - items.len();
                            if skipped > 0 {
                                tracing::debug!(
                                    total,
                                    skipped,
                                    "TodoWrite items skipped due to missing content field"
                                );
                            }
                            items
                        })
                } else {
                    None
                }
            })
        })
}

/// Extract TaskCreate calls from assistant content blocks.
fn extract_task_creates(msg: Option<&serde_json::Value>) -> Vec<RawTaskCreate> {
    let mut task_creates = Vec::new();
    if let Some(content) = msg
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_array())
    {
        for block in content {
            if block.get("type").and_then(|t| t.as_str()) == Some("tool_use")
                && block.get("name").and_then(|n| n.as_str()) == Some("TaskCreate")
            {
                let tool_use_id = block
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let input = block.get("input");
                if !tool_use_id.is_empty() {
                    task_creates.push(RawTaskCreate {
                        tool_use_id,
                        subject: input
                            .and_then(|i| i.get("subject"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        description: input
                            .and_then(|i| i.get("description"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        active_form: input
                            .and_then(|i| i.get("activeForm"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                    });
                }
            }
        }
    }
    task_creates
}

/// Extract TaskUpdate calls from assistant content blocks.
fn extract_task_updates(msg: Option<&serde_json::Value>) -> Vec<RawTaskUpdate> {
    let mut task_updates = Vec::new();
    if let Some(content) = msg
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_array())
    {
        for block in content {
            if block.get("type").and_then(|t| t.as_str()) == Some("tool_use")
                && block.get("name").and_then(|n| n.as_str()) == Some("TaskUpdate")
            {
                let input = block.get("input");
                let task_id = input
                    .and_then(|i| i.get("taskId"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if !task_id.is_empty() {
                    task_updates.push(RawTaskUpdate {
                        task_id,
                        status: input
                            .and_then(|i| i.get("status"))
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        subject: input
                            .and_then(|i| i.get("subject"))
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        active_form: input
                            .and_then(|i| i.get("activeForm"))
                            .and_then(|v| v.as_str())
                            .map(String::from),
                    });
                }
            }
        }
    }
    task_updates
}

/// Extract TaskIdAssignment from user lines with toolUseResult containing task.id.
fn extract_task_id_assignments(
    parsed: &serde_json::Value,
    msg: Option<&serde_json::Value>,
) -> Vec<RawTaskIdAssignment> {
    let mut assignments = Vec::new();
    if let Some(task_id) = parsed
        .get("toolUseResult")
        .and_then(|tur| tur.get("task"))
        .and_then(|t| t.get("id"))
        .and_then(|v| v.as_str())
    {
        let tool_use_id = msg
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_array())
            .and_then(|blocks| {
                blocks.iter().find_map(|b| {
                    if b.get("type").and_then(|t| t.as_str()) == Some("tool_result") {
                        b.get("tool_use_id")
                            .and_then(|v| v.as_str())
                            .map(String::from)
                    } else {
                        None
                    }
                })
            });
        if let Some(tool_use_id) = tool_use_id {
            assignments.push(RawTaskIdAssignment {
                tool_use_id,
                task_id: task_id.to_string(),
            });
        } else {
            tracing::warn!(
                task_id = task_id,
                "Task ID found in toolUseResult but no matching tool_result block — assignment skipped"
            );
        }
    }
    assignments
}
