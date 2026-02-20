// crates/core/src/live_parser.rs
//! Incremental JSONL tail parser for live session monitoring.
//!
//! Reads only the new bytes appended since the last poll, using byte offsets
//! for efficiency. SIMD-accelerated pre-filtering via `memchr` avoids
//! deserialising lines that lack interesting keys.

use memchr::memmem;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::progress::{RawTaskCreate, RawTaskIdAssignment, RawTaskUpdate, RawTodoItem};

/// Byte offset + timestamp for incremental tailing.
pub struct TailState {
    pub offset: u64,
    pub last_modified: std::time::SystemTime,
}

/// Extracted from a Task tool_use block on an assistant line.
#[derive(Debug, Clone)]
pub struct SubAgentSpawn {
    pub tool_use_id: String,
    pub agent_type: String,
    pub description: String,
}

/// Extracted from a `type: "progress"` line with `data.type: "agent_progress"`.
#[derive(Debug, Clone)]
pub struct SubAgentProgress {
    /// Links back to the Task spawn's `tool_use_id`.
    pub parent_tool_use_id: String,
    /// 7-char agent ID (available before completion!).
    pub agent_id: String,
    /// Current tool the sub-agent is using (e.g., "Read", "Grep", "Edit").
    /// Extracted from the latest `tool_use` block in `data.message.content`.
    pub current_tool: Option<String>,
}

/// Extracted from a `<task-notification>` on a user line (background agent completion).
#[derive(Debug, Clone)]
pub struct SubAgentNotification {
    /// The agent ID from `<task-id>`.
    pub agent_id: String,
    /// The completion status: "completed", "failed", or "killed".
    pub status: String,
}

/// Extracted from a toolUseResult on a user line (Task completion).
#[derive(Debug, Clone)]
pub struct SubAgentResult {
    pub tool_use_id: String,
    /// 7-char short hash from `toolUseResult.agentId` (e.g., "a33bda6").
    pub agent_id: Option<String>,
    pub status: String,  // "completed", "error", etc.
    pub total_duration_ms: Option<u64>,
    /// Number of tool calls from `toolUseResult.totalToolUseCount`.
    pub total_tool_use_count: Option<u32>,
    pub usage_input_tokens: Option<u64>,
    pub usage_output_tokens: Option<u64>,
    pub usage_cache_read_tokens: Option<u64>,
    pub usage_cache_creation_tokens: Option<u64>,
}

/// A single parsed JSONL line from the session log.
#[derive(Debug, Clone)]
pub struct LiveLine {
    pub line_type: LineType,
    pub role: Option<String>,
    pub content_preview: String,
    pub tool_names: Vec<String>,
    pub model: Option<String>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub cache_creation_tokens: Option<u64>,
    pub cache_creation_5m_tokens: Option<u64>,
    pub cache_creation_1hr_tokens: Option<u64>,
    pub timestamp: Option<String>,
    pub stop_reason: Option<String>,
    /// Git branch extracted from user-type JSONL lines.
    pub git_branch: Option<String>,
    /// Whether this is a meta/system message (not real user content).
    pub is_meta: bool,
    /// Whether this line's content array contains a `tool_result` block,
    /// indicating it's a continuation after tool execution (not a new user turn).
    pub is_tool_result_continuation: bool,
    /// Whether this user-type line starts with a system-injected prefix
    /// (e.g. `<local-command-caveat>`, `<task-notification>`, continuation markers).
    pub has_system_prefix: bool,
    /// If this assistant line contains a Task tool_use, the spawn info.
    /// Vec because a single assistant message can spawn multiple sub-agents.
    pub sub_agent_spawns: Vec<SubAgentSpawn>,
    /// If this user line has a `toolUseResult` (Task completion), the result info.
    pub sub_agent_result: Option<SubAgentResult>,
    /// If this is a progress line with agent_progress data.
    pub sub_agent_progress: Option<SubAgentProgress>,
    /// If this user line contains a `<task-notification>` for a background agent.
    pub sub_agent_notification: Option<SubAgentNotification>,
    /// Full replacement todo list from TodoWrite tool_use on this assistant line.
    pub todo_write: Option<Vec<RawTodoItem>>,
    /// TaskCreate calls on this assistant line (Vec: one message can create multiple tasks).
    pub task_creates: Vec<RawTaskCreate>,
    /// TaskUpdate calls on this assistant line.
    pub task_updates: Vec<RawTaskUpdate>,
    /// Task ID assignments from toolUseResult on this user line.
    pub task_id_assignments: Vec<RawTaskIdAssignment>,
}

/// Broad classification of a JSONL line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineType {
    User,
    Assistant,
    System,
    Progress,
    Summary,
    Result,
    Other,
}

/// Pre-compiled SIMD substring finders. Build once at startup via
/// [`TailFinders::new`] and share across polls.
pub struct TailFinders {
    pub type_user: memmem::Finder<'static>,
    pub type_assistant: memmem::Finder<'static>,
    pub type_system: memmem::Finder<'static>,
    pub type_progress: memmem::Finder<'static>,
    pub type_summary: memmem::Finder<'static>,
    pub type_result: memmem::Finder<'static>,
    pub content_key: memmem::Finder<'static>,
    pub model_key: memmem::Finder<'static>,
    pub usage_key: memmem::Finder<'static>,
    pub tool_use_key: memmem::Finder<'static>,
    pub name_key: memmem::Finder<'static>,
    pub stop_reason_key: memmem::Finder<'static>,
    pub task_name_key: memmem::Finder<'static>,
    pub tool_use_result_key: memmem::Finder<'static>,
    pub agent_progress_key: memmem::Finder<'static>,
    pub todo_write_key: memmem::Finder<'static>,
    pub task_create_key: memmem::Finder<'static>,
    pub task_update_key: memmem::Finder<'static>,
    pub task_notification_key: memmem::Finder<'static>,
}

impl TailFinders {
    /// Create all finders once. The needles are `'static` string slices.
    pub fn new() -> Self {
        Self {
            type_user: memmem::Finder::new(b"\"user\""),
            type_assistant: memmem::Finder::new(b"\"assistant\""),
            type_system: memmem::Finder::new(b"\"system\""),
            type_progress: memmem::Finder::new(b"\"progress\""),
            type_summary: memmem::Finder::new(b"\"summary\""),
            type_result: memmem::Finder::new(b"\"result\""),
            content_key: memmem::Finder::new(b"\"content\""),
            model_key: memmem::Finder::new(b"\"model\""),
            usage_key: memmem::Finder::new(b"\"usage\""),
            tool_use_key: memmem::Finder::new(b"\"tool_use\""),
            name_key: memmem::Finder::new(b"\"name\""),
            stop_reason_key: memmem::Finder::new(b"\"stop_reason\""),
            task_name_key: memmem::Finder::new(b"\"name\":\"Task\""),
            tool_use_result_key: memmem::Finder::new(b"\"toolUseResult\""),
            agent_progress_key: memmem::Finder::new(b"\"agent_progress\""),
            todo_write_key: memmem::Finder::new(b"\"name\":\"TodoWrite\""),
            task_create_key: memmem::Finder::new(b"\"name\":\"TaskCreate\""),
            task_update_key: memmem::Finder::new(b"\"name\":\"TaskUpdate\""),
            task_notification_key: memmem::Finder::new(b"<task-notification>"),
        }
    }
}

impl Default for TailFinders {
    fn default() -> Self {
        Self::new()
    }
}

/// Read new JSONL lines appended since `offset`.
///
/// Returns the parsed lines and the new byte offset to pass on the next call.
/// This function uses synchronous I/O and should be called from
/// `tokio::task::spawn_blocking`.
pub fn parse_tail(
    path: &Path,
    offset: u64,
    finders: &TailFinders,
) -> std::io::Result<(Vec<LiveLine>, u64)> {
    let mut file = File::open(path)?;
    let metadata = file.metadata()?;
    let file_len = metadata.len();

    if offset > file_len {
        // File was replaced (new file smaller than stored offset).
        // Reset to start and read the entire new file.
        tracing::warn!(
            path = %path.display(),
            old_offset = offset,
            new_file_len = file_len,
            "File replaced (offset > size) — resetting to start"
        );
        return parse_tail(path, 0, finders);
    }
    if offset == file_len {
        return Ok((Vec::new(), offset));
    }

    file.seek(SeekFrom::Start(offset))?;

    let to_read = (file_len - offset) as usize;
    let mut buf = vec![0u8; to_read];
    file.read_exact(&mut buf)?;

    // Find the last newline — anything after it is a partial write and must be
    // excluded so we don't try to parse an incomplete JSON object.
    let last_newline = buf.iter().rposition(|&b| b == b'\n');
    let (complete, new_offset) = match last_newline {
        Some(pos) => (&buf[..=pos], offset + pos as u64 + 1),
        None => {
            // No complete line yet
            return Ok((Vec::new(), offset));
        }
    };

    let mut lines = Vec::new();
    for raw_line in complete.split(|&b| b == b'\n') {
        if raw_line.is_empty() {
            continue;
        }
        let line = parse_single_line(raw_line, finders);
        lines.push(line);
    }

    Ok((lines, new_offset))
}

/// Classify and extract fields from a single JSONL line.
///
/// Claude Code JSONL wraps API messages inside a `"message"` field:
/// ```json
/// {"type": "assistant", "message": {"role": "assistant", "model": "...", "usage": {...}, "content": [...]}}
/// {"type": "user", "message": {"role": "user", "content": "..."}, "gitBranch": "...", "isMeta": true}
/// ```
/// We check both the top level and the nested `message` object for each field.
fn parse_single_line(raw: &[u8], finders: &TailFinders) -> LiveLine {
    // Fast classification via SIMD substring search (avoids JSON parse for
    // lines that don't contain the keys we care about).
    // Check result/progress/summary BEFORE user/assistant because these lines
    // may contain nested "role":"assistant" which would match the assistant finder.
    let line_type = if finders.type_result.find(raw).is_some()
        && finders.type_progress.find(raw).is_none()
    {
        // "result" appears in result lines AND in toolUseResult lines.
        // Disambiguate: if line has "result" but NOT "user", it's a
        // top-level result line. If it has both "result" and "user", it's
        // a toolUseResult on a user line → classify as User below.
        if finders.type_user.find(raw).is_none() {
            LineType::Result
        } else {
            LineType::User
        }
    } else if finders.type_progress.find(raw).is_some() {
        LineType::Progress
    } else if finders.type_summary.find(raw).is_some() {
        LineType::Summary
    } else if finders.type_user.find(raw).is_some() {
        LineType::User
    } else if finders.type_assistant.find(raw).is_some() {
        LineType::Assistant
    } else if finders.type_system.find(raw).is_some() {
        LineType::System
    } else {
        LineType::Other
    };

    // Parse JSON to extract structured fields
    let parsed: serde_json::Value = match serde_json::from_slice(raw) {
        Ok(v) => v,
        Err(_) => {
            return LiveLine {
                line_type,
                role: None,
                content_preview: String::new(),
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
            };
        }
    };

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
    let (content_preview, tool_names, is_tool_result) =
        extract_content_and_tools(content_source, finders);

    // Detect system-injected prefixes in user-type lines.
    // These are not real user prompts — they're tool result continuations,
    // command outputs, or session continuation markers injected by Claude Code.
    let has_system_prefix = if line_type == LineType::User {
        let c = content_preview.trim_start();
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
    // Format: <task-notification>\n<task-id>AGENT_ID</task-id>\n<status>STATUS</status>\n...
    // Parse from the full content (not content_preview) because the notification
    // can appear deep into the content string, well past the 200-char preview limit.
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

    // Check if this is a meta message (system prompts, hooks, etc.)
    let is_meta = parsed
        .get("isMeta")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // --- Sub-agent spawn detection (assistant lines with Task tool_use) ---
    let mut sub_agent_spawns = Vec::new();
    if line_type == LineType::Assistant && finders.task_name_key.find(raw).is_some() {
        // Already have `parsed` from JSON parse above
        if let Some(content) = msg.and_then(|m| m.get("content")).and_then(|c| c.as_array()) {
            for block in content {
                if block.get("type").and_then(|t| t.as_str()) == Some("tool_use")
                    && block.get("name").and_then(|n| n.as_str()) == Some("Task")
                {
                    let tool_use_id = block.get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let input = block.get("input");
                    let description = input
                        .and_then(|i| i.get("description"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let agent_type = input
                        .and_then(|i| i.get("subagent_type"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("Task")
                        .to_string();
                    if !tool_use_id.is_empty() {
                        sub_agent_spawns.push(SubAgentSpawn {
                            tool_use_id,
                            agent_type,
                            description,
                        });
                    }
                }
            }
        }
    }

    // --- Sub-agent completion detection (user lines with toolUseResult) ---
    let sub_agent_result = if line_type == LineType::User
        && finders.tool_use_result_key.find(raw).is_some()
    {
        // Extract toolUseResult from top-level (NOT inside message.content)
        parsed.get("toolUseResult").and_then(|tur| {
            // Find the matching tool_use_id from the tool_result block in content
            let tool_use_id = msg
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_array())
                .and_then(|blocks| {
                    blocks.iter().find_map(|b| {
                        if b.get("type").and_then(|t| t.as_str()) == Some("tool_result") {
                            b.get("tool_use_id").and_then(|v| v.as_str()).map(String::from)
                        } else {
                            None
                        }
                    })
                })?;

            let agent_id = tur.get("agentId")
                .and_then(|v| v.as_str())
                .map(String::from);
            let status = tur.get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("completed")
                .to_string();
            let total_duration_ms = tur.get("totalDurationMs").and_then(|v| v.as_u64());
            let total_tool_use_count = tur.get("totalToolUseCount")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32);
            let usage = tur.get("usage");
            Some(SubAgentResult {
                tool_use_id,
                agent_id,
                status,
                total_duration_ms,
                total_tool_use_count,
                usage_input_tokens: usage.and_then(|u| u.get("input_tokens")).and_then(|v| v.as_u64()),
                usage_output_tokens: usage.and_then(|u| u.get("output_tokens")).and_then(|v| v.as_u64()),
                usage_cache_read_tokens: usage.and_then(|u| u.get("cache_read_input_tokens")).and_then(|v| v.as_u64()),
                usage_cache_creation_tokens: usage.and_then(|u| u.get("cache_creation_input_tokens")).and_then(|v| v.as_u64()),
            })
        })
    } else {
        None
    };

    // --- Sub-agent progress detection (progress lines with agent_progress) ---
    // SIMD pre-filter on "agent_progress" then verify the parsed JSON `type` field
    // for an extra safety check (belt-and-suspenders with the line_type classification).
    let sub_agent_progress = if finders.agent_progress_key.find(raw).is_some()
        && parsed.get("type").and_then(|t| t.as_str()) == Some("progress")
    {
        parsed.get("data").and_then(|data| {
            if data.get("type").and_then(|t| t.as_str()) != Some("agent_progress") {
                return None;
            }
            let parent_tool_use_id = parsed.get("parentToolUseID")
                .and_then(|v| v.as_str())
                .map(String::from)?;
            let agent_id = data.get("agentId")
                .and_then(|v| v.as_str())
                .map(String::from)?;
            // Extract current tool from the latest tool_use block in message.content
            let current_tool = data.get("message")
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_array())
                .and_then(|blocks| {
                    blocks.iter().rev().find_map(|b| {
                        if b.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                            b.get("name").and_then(|n| n.as_str()).map(String::from)
                        } else {
                            None
                        }
                    })
                });
            Some(SubAgentProgress {
                parent_tool_use_id,
                agent_id,
                current_tool,
            })
        })
    } else {
        None
    };

    // --- TodoWrite detection (assistant lines with TodoWrite tool_use) ---
    let todo_write = if line_type == LineType::Assistant && finders.todo_write_key.find(raw).is_some() {
        msg.and_then(|m| m.get("content")).and_then(|c| c.as_array()).and_then(|blocks| {
            blocks.iter().find_map(|b| {
                if b.get("type").and_then(|t| t.as_str()) == Some("tool_use")
                    && b.get("name").and_then(|n| n.as_str()) == Some("TodoWrite")
                {
                    b.get("input").and_then(|i| i.get("todos")).and_then(|t| t.as_array()).map(|todos| {
                        todos.iter().filter_map(|item| {
                            Some(RawTodoItem {
                                content: item.get("content").and_then(|v| v.as_str())?.to_string(),
                                status: item.get("status").and_then(|v| v.as_str()).unwrap_or("pending").to_string(),
                                active_form: item.get("activeForm").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            })
                        }).collect::<Vec<_>>()
                    })
                } else {
                    None
                }
            })
        })
    } else {
        None
    };

    // --- TaskCreate detection (assistant lines with TaskCreate tool_use) ---
    let mut task_creates = Vec::new();
    if line_type == LineType::Assistant && finders.task_create_key.find(raw).is_some() {
        if let Some(content) = msg.and_then(|m| m.get("content")).and_then(|c| c.as_array()) {
            for block in content {
                if block.get("type").and_then(|t| t.as_str()) == Some("tool_use")
                    && block.get("name").and_then(|n| n.as_str()) == Some("TaskCreate")
                {
                    let tool_use_id = block.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let input = block.get("input");
                    if !tool_use_id.is_empty() {
                        task_creates.push(RawTaskCreate {
                            tool_use_id,
                            subject: input.and_then(|i| i.get("subject")).and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            description: input.and_then(|i| i.get("description")).and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            active_form: input.and_then(|i| i.get("activeForm")).and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        });
                    }
                }
            }
        }
    }

    // --- TaskUpdate detection (assistant lines with TaskUpdate tool_use) ---
    let mut task_updates = Vec::new();
    if line_type == LineType::Assistant && finders.task_update_key.find(raw).is_some() {
        if let Some(content) = msg.and_then(|m| m.get("content")).and_then(|c| c.as_array()) {
            for block in content {
                if block.get("type").and_then(|t| t.as_str()) == Some("tool_use")
                    && block.get("name").and_then(|n| n.as_str()) == Some("TaskUpdate")
                {
                    let input = block.get("input");
                    let task_id = input.and_then(|i| i.get("taskId")).and_then(|v| v.as_str()).unwrap_or("").to_string();
                    if !task_id.is_empty() {
                        task_updates.push(RawTaskUpdate {
                            task_id,
                            status: input.and_then(|i| i.get("status")).and_then(|v| v.as_str()).map(String::from),
                            subject: input.and_then(|i| i.get("subject")).and_then(|v| v.as_str()).map(String::from),
                            active_form: input.and_then(|i| i.get("activeForm")).and_then(|v| v.as_str()).map(String::from),
                        });
                    }
                }
            }
        }
    }

    // --- TaskIdAssignment detection (user lines with toolUseResult containing task.id) ---
    let mut task_id_assignments = Vec::new();
    if line_type == LineType::User && finders.tool_use_result_key.find(raw).is_some() {
        if let Some(task_id) = parsed.get("toolUseResult")
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
                            b.get("tool_use_id").and_then(|v| v.as_str()).map(String::from)
                        } else {
                            None
                        }
                    })
                });
            if let Some(tool_use_id) = tool_use_id {
                task_id_assignments.push(RawTaskIdAssignment {
                    tool_use_id,
                    task_id: task_id.to_string(),
                });
            }
        }
    }

    LiveLine {
        line_type,
        role,
        content_preview,
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
    }
}

/// Extract content preview (truncated to 200 chars), tool_use names, and
/// whether the content array contains a `tool_result` block.
fn extract_content_and_tools(
    parsed: &serde_json::Value,
    _finders: &TailFinders,
) -> (String, Vec<String>, bool) {
    let mut preview = String::new();
    let mut tool_names = Vec::new();
    let mut has_tool_result = false;

    match parsed.get("content") {
        Some(serde_json::Value::String(s)) => {
            preview = truncate_str(s, 200);
        }
        Some(serde_json::Value::Array(blocks)) => {
            for block in blocks {
                match block.get("type").and_then(|t| t.as_str()) {
                    Some("text") => {
                        if preview.is_empty() {
                            if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                                preview = truncate_str(text, 200);
                            }
                        }
                    }
                    Some("tool_use") => {
                        if let Some(name) = block.get("name").and_then(|n| n.as_str()) {
                            tool_names.push(name.to_string());
                        }
                    }
                    Some("tool_result") => {
                        has_tool_result = true;
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }

    (preview, tool_names, has_tool_result)
}

/// Extract a `<task-notification>` from the full content JSON value.
///
/// Walks the content field (string or array of text blocks) looking for
/// `<task-id>` and `<status>` XML tags within a `<task-notification>` block.
fn extract_task_notification(content_source: &serde_json::Value) -> Option<SubAgentNotification> {
    // Collect the full text from content (string or first text block in array)
    let full_text = match content_source.get("content") {
        Some(serde_json::Value::String(s)) => s.as_str(),
        Some(serde_json::Value::Array(blocks)) => {
            // Find the text block containing <task-notification>
            blocks
                .iter()
                .filter_map(|b| {
                    if b.get("type").and_then(|t| t.as_str()) == Some("text") {
                        b.get("text").and_then(|t| t.as_str())
                    } else {
                        None
                    }
                })
                .find(|text| text.contains("<task-notification>"))?
        }
        _ => return None,
    };

    // Find <task-notification> and extract <task-id> and <status> after it
    let tn_start = full_text.find("<task-notification>")?;
    let after_tn = &full_text[tn_start..];

    let agent_id = {
        let start = after_tn.find("<task-id>")? + "<task-id>".len();
        let end = start + after_tn[start..].find("</task-id>")?;
        let id = after_tn[start..end].trim();
        if id.is_empty() {
            return None;
        }
        id.to_string()
    };

    let status = {
        let start = after_tn.find("<status>")? + "<status>".len();
        let end = start + after_tn[start..].find("</status>")?;
        let s = after_tn[start..end].trim();
        if s.is_empty() {
            return None;
        }
        s.to_string()
    };

    Some(SubAgentNotification { agent_id, status })
}

/// Token counts extracted from a `usage` sub-object.
#[derive(Debug, Default)]
struct UsageTokens {
    pub input: Option<u64>,
    pub output: Option<u64>,
    pub cache_read: Option<u64>,
    pub cache_creation: Option<u64>,
    pub cache_creation_5m: Option<u64>,
    pub cache_creation_1hr: Option<u64>,
}

/// Extract token counts from a `usage` sub-object.
fn extract_usage(parsed: &serde_json::Value) -> UsageTokens {
    let usage = match parsed.get("usage") {
        Some(u) => u,
        None => return UsageTokens::default(),
    };

    let input = usage.get("input_tokens").and_then(|v| v.as_u64());
    let output = usage.get("output_tokens").and_then(|v| v.as_u64());
    let cache_read = usage
        .get("cache_read_input_tokens")
        .and_then(|v| v.as_u64());
    let cache_creation = usage
        .get("cache_creation_input_tokens")
        .and_then(|v| v.as_u64());

    // Extract ephemeral cache breakdown when present
    let (cache_creation_5m, cache_creation_1hr) = usage
        .get("cache_creation")
        .map(|cc| {
            let t5m = cc.get("ephemeral_5m_input_tokens").and_then(|v| v.as_u64());
            let t1h = cc.get("ephemeral_1h_input_tokens").and_then(|v| v.as_u64());
            (t5m, t1h)
        })
        .unwrap_or((None, None));

    UsageTokens {
        input,
        output,
        cache_read,
        cache_creation,
        cache_creation_5m,
        cache_creation_1hr,
    }
}

/// Truncate a string to at most `max` characters, appending "..." if trimmed.
fn truncate_str(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max).collect();
        format!("{}...", truncated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_parse_tail_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.jsonl");
        File::create(&path).unwrap();

        let finders = TailFinders::new();
        let (lines, offset) = parse_tail(&path, 0, &finders).unwrap();
        assert!(lines.is_empty());
        assert_eq!(offset, 0);
    }

    #[test]
    fn test_parse_tail_single_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("single.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(f, r#"{{"role":"user","content":"Hello world"}}"#).unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, offset) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].line_type, LineType::User);
        assert_eq!(lines[0].role.as_deref(), Some("user"));
        assert_eq!(lines[0].content_preview, "Hello world");
        assert!(offset > 0);
    }

    #[test]
    fn test_parse_tail_partial_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("partial.jsonl");
        let mut f = File::create(&path).unwrap();
        // Write without trailing newline — simulates a partial write in progress
        write!(f, r#"{{"role":"user","content":"partial"#).unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, offset) = parse_tail(&path, 0, &finders).unwrap();
        assert!(lines.is_empty());
        assert_eq!(offset, 0);
    }

    #[test]
    fn test_parse_tail_resets_on_file_replacement() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.jsonl");

        // Write initial content
        {
            let mut f = File::create(&path).unwrap();
            writeln!(f, r#"{{"type":"user","message":{{"role":"user","content":"hello"}}}}"#).unwrap();
            writeln!(f, r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"text","text":"hi"}}]}}}}"#).unwrap();
        }

        let finders = TailFinders::new();
        let (lines, offset) = parse_tail(&path, 0, &finders).unwrap();
        assert!(!lines.is_empty());
        assert!(offset > 0);

        // "Replace" the file with smaller content (simulates log rotation)
        {
            let mut f = File::create(&path).unwrap();
            writeln!(f, r#"{{"type":"user","message":{{"role":"user","content":"new session"}}}}"#).unwrap();
        }

        // Old offset is larger than new file — should reset and read from start
        let (lines2, offset2) = parse_tail(&path, offset, &finders).unwrap();
        assert!(!lines2.is_empty(), "Should read new content after file replacement");
        assert!(offset2 > 0);
    }

    #[test]
    fn test_parse_tail_incremental() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("incremental.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(f, r#"{{"role":"user","content":"first"}}"#).unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();

        // First read
        let (lines1, offset1) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines1.len(), 1);
        assert_eq!(lines1[0].content_preview, "first");

        // Append a second line
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        writeln!(f, r#"{{"role":"assistant","content":"second"}}"#).unwrap();
        f.flush().unwrap();

        // Second read from previous offset
        let (lines2, offset2) = parse_tail(&path, offset1, &finders).unwrap();
        assert_eq!(lines2.len(), 1);
        assert_eq!(lines2[0].content_preview, "second");
        assert_eq!(lines2[0].line_type, LineType::Assistant);
        assert!(offset2 > offset1);
    }

    #[test]
    fn test_parse_tail_extracts_tokens() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tokens.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"role":"assistant","content":"hi","model":"claude-opus-4-6","usage":{{"input_tokens":1000,"output_tokens":500,"cache_read_input_tokens":200,"cache_creation_input_tokens":50}},"stop_reason":"end_turn"}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        let line = &lines[0];
        assert_eq!(line.model.as_deref(), Some("claude-opus-4-6"));
        assert_eq!(line.input_tokens, Some(1000));
        assert_eq!(line.output_tokens, Some(500));
        assert_eq!(line.cache_read_tokens, Some(200));
        assert_eq!(line.cache_creation_tokens, Some(50));
        assert_eq!(line.stop_reason.as_deref(), Some("end_turn"));
    }

    #[test]
    fn test_parse_tail_content_blocks() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("blocks.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"role":"assistant","content":[{{"type":"text","text":"Hello"}},{{"type":"tool_use","name":"bash","id":"123","input":{{}}}}]}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        let line = &lines[0];
        assert_eq!(line.content_preview, "Hello");
        assert_eq!(line.tool_names, vec!["bash"]);
    }

    #[test]
    fn test_parse_tail_nested_message_format() {
        // This is the actual Claude Code JSONL format: fields nested under "message"
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"assistant","message":{{"role":"assistant","model":"claude-opus-4-6","content":[{{"type":"text","text":"Hello"}}],"usage":{{"input_tokens":1000,"output_tokens":500,"cache_read_input_tokens":200,"cache_creation_input_tokens":50}},"stop_reason":"end_turn"}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        let line = &lines[0];
        assert_eq!(line.line_type, LineType::Assistant);
        assert_eq!(line.model.as_deref(), Some("claude-opus-4-6"));
        assert_eq!(line.input_tokens, Some(1000));
        assert_eq!(line.output_tokens, Some(500));
        assert_eq!(line.cache_read_tokens, Some(200));
        assert_eq!(line.cache_creation_tokens, Some(50));
        assert_eq!(line.stop_reason.as_deref(), Some("end_turn"));
        assert_eq!(line.content_preview, "Hello");
    }

    #[test]
    fn test_parse_tail_nested_user_with_git_branch() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("user_nested.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"Fix the bug"}},"gitBranch":"feature/auth","isMeta":false,"timestamp":"2026-01-15T10:30:00Z"}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        let line = &lines[0];
        assert_eq!(line.line_type, LineType::User);
        assert_eq!(line.content_preview, "Fix the bug");
        assert_eq!(line.git_branch.as_deref(), Some("feature/auth"));
        assert!(!line.is_meta);
    }

    #[test]
    fn test_parse_tail_meta_message_flagged() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("meta.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"system prompt stuff"}},"isMeta":true,"timestamp":"2026-01-15T10:30:00Z"}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].is_meta);
    }

    #[test]
    fn test_truncation() {
        let long = "a".repeat(250);
        let result = truncate_str(&long, 200);
        // 200 chars + "..." = 203 chars
        assert_eq!(result.len(), 203);
        assert!(result.ends_with("..."));
    }

    // -------------------------------------------------------------------------
    // Turn detection: is_tool_result_continuation
    // -------------------------------------------------------------------------

    #[test]
    fn test_tool_result_continuation_true() {
        // Content array with a tool_result block should set is_tool_result_continuation = true
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tool_result.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"toolu_123","content":"file contents here"}}]}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(
            lines[0].is_tool_result_continuation,
            "Expected is_tool_result_continuation = true for content with tool_result block"
        );
    }

    #[test]
    fn test_tool_result_continuation_false_text_only() {
        // Content array with only text blocks should NOT set is_tool_result_continuation
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("text_only.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"text","text":"Hello world"}}]}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(
            !lines[0].is_tool_result_continuation,
            "Expected is_tool_result_continuation = false for text-only content"
        );
    }

    #[test]
    fn test_tool_result_continuation_false_text_and_tool_use() {
        // Content array with text + tool_use (but no tool_result) should NOT set it
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("text_tool_use.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"text","text":"Let me check"}},{{"type":"tool_use","name":"bash","id":"123","input":{{}}}}]}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(
            !lines[0].is_tool_result_continuation,
            "Expected is_tool_result_continuation = false for text + tool_use content"
        );
    }

    #[test]
    fn test_tool_result_continuation_false_string_content() {
        // String content (not array) should NOT set is_tool_result_continuation
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("string_content.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"Just a plain message"}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(
            !lines[0].is_tool_result_continuation,
            "Expected is_tool_result_continuation = false for string content"
        );
    }

    // -------------------------------------------------------------------------
    // Turn detection: has_system_prefix
    // -------------------------------------------------------------------------

    #[test]
    fn test_system_prefix_local_command_caveat() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("caveat.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"<local-command-caveat>some caveat text</local-command-caveat>"}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(
            lines[0].has_system_prefix,
            "Expected has_system_prefix = true for <local-command-caveat>"
        );
    }

    #[test]
    fn test_system_prefix_local_command_stdout() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stdout.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"<local-command-stdout>ls -la output</local-command-stdout>"}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(
            lines[0].has_system_prefix,
            "Expected has_system_prefix = true for <local-command-stdout>"
        );
    }

    #[test]
    fn test_system_prefix_command_name_clear() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("clear.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"<command-name>/clear</command-name>"}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(
            lines[0].has_system_prefix,
            "Expected has_system_prefix = true for <command-name>/clear"
        );
    }

    #[test]
    fn test_system_prefix_command_name_context() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("context.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"<command-name>/context add file.rs</command-name>"}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(
            lines[0].has_system_prefix,
            "Expected has_system_prefix = true for <command-name>/context"
        );
    }

    #[test]
    fn test_system_prefix_session_continuation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("continuation.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"This session is being continued from a previous conversation."}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(
            lines[0].has_system_prefix,
            "Expected has_system_prefix = true for session continuation marker"
        );
    }

    #[test]
    fn test_system_prefix_task_notification() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("task_notif.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"<task-notification>Task completed</task-notification>"}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(
            lines[0].has_system_prefix,
            "Expected has_system_prefix = true for <task-notification>"
        );
    }

    #[test]
    fn test_task_notification_extracts_agent_status() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notif_completed.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"<task-notification>\n<task-id>ab897bc</task-id>\n<status>completed</status>\n<summary>Agent done</summary>\n</task-notification>"}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].has_system_prefix);
        let notif = lines[0].sub_agent_notification.as_ref().unwrap();
        assert_eq!(notif.agent_id, "ab897bc");
        assert_eq!(notif.status, "completed");
    }

    #[test]
    fn test_task_notification_failed_status() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notif_failed.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"<task-notification>\n<task-id>afailed1</task-id>\n<status>failed</status>\n<summary>Agent errored</summary>\n</task-notification>"}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        let notif = lines[0].sub_agent_notification.as_ref().unwrap();
        assert_eq!(notif.agent_id, "afailed1");
        assert_eq!(notif.status, "failed");
    }

    #[test]
    fn test_task_notification_deep_in_content() {
        // Notification appearing after 300+ chars of prefix content
        // (past the 200-char content_preview truncation limit)
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notif_deep.jsonl");
        let mut f = File::create(&path).unwrap();
        let prefix = "x".repeat(400); // 400 chars of padding before notification
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"{prefix}<task-notification>\n<task-id>adeep01</task-id>\n<status>completed</status>\n<summary>Deep agent done</summary>\n</task-notification>"}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        let notif = lines[0]
            .sub_agent_notification
            .as_ref()
            .expect("notification must be extracted even past 200-char preview limit");
        assert_eq!(notif.agent_id, "adeep01");
        assert_eq!(notif.status, "completed");
    }

    #[test]
    fn test_task_notification_in_content_array() {
        // Notification inside a content array (text block), not a plain string
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notif_array.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":[{{"type":"text","text":"<task-notification>\n<task-id>aarray1</task-id>\n<status>killed</status>\n<summary>Killed agent</summary>\n</task-notification>"}}]}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        let notif = lines[0]
            .sub_agent_notification
            .as_ref()
            .expect("notification must be extracted from content array");
        assert_eq!(notif.agent_id, "aarray1");
        assert_eq!(notif.status, "killed");
    }

    #[test]
    fn test_task_notification_not_on_regular_user_message() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("normal.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"Hello world"}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert!(lines[0].sub_agent_notification.is_none());
    }

    #[test]
    fn test_system_prefix_false_normal_user_message() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("normal_user.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"Fix the bug in auth.rs"}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(
            !lines[0].has_system_prefix,
            "Expected has_system_prefix = false for normal user message"
        );
    }

    #[test]
    fn test_system_prefix_false_for_assistant_messages() {
        // has_system_prefix should only apply to User-type lines
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("assistant_with_prefix.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"assistant","message":{{"role":"assistant","content":"<local-command-caveat>this text happens to start like a prefix"}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(
            !lines[0].has_system_prefix,
            "Expected has_system_prefix = false for assistant messages even with prefix-like content"
        );
    }

    // -------------------------------------------------------------------------
    // Turn detection: edge cases
    // -------------------------------------------------------------------------

    #[test]
    fn test_turn_detection_empty_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty_content.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":""}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(!lines[0].is_tool_result_continuation);
        assert!(!lines[0].has_system_prefix);
    }

    #[test]
    fn test_turn_detection_missing_content_field() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("no_content.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user"}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(!lines[0].is_tool_result_continuation);
        assert!(!lines[0].has_system_prefix);
        assert_eq!(lines[0].content_preview, "");
    }

    #[test]
    fn test_turn_detection_empty_content_array() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty_array.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":[]}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(!lines[0].is_tool_result_continuation);
        assert!(!lines[0].has_system_prefix);
    }

    // -------------------------------------------------------------------------
    // Sub-agent progress detection
    // -------------------------------------------------------------------------

    #[test]
    fn test_progress_event_agent_activity() {
        let finders = TailFinders::new();
        let raw = br#"{"type":"progress","parentToolUseID":"toolu_01ABC","toolUseID":"agent_msg_01","data":{"type":"agent_progress","agentId":"a951849","message":{"role":"assistant","content":[{"type":"tool_use","name":"Read","input":{"file_path":"/path/to/file.rs"}}]}},"timestamp":"2026-02-16T08:34:13.134Z"}"#;
        let line = parse_single_line(raw, &finders);

        assert!(line.sub_agent_progress.is_some());
        let progress = line.sub_agent_progress.unwrap();
        assert_eq!(progress.parent_tool_use_id, "toolu_01ABC");
        assert_eq!(progress.agent_id, "a951849");
        assert_eq!(progress.current_tool, Some("Read".to_string()));
    }

    #[test]
    fn test_progress_event_no_tool_use() {
        let finders = TailFinders::new();
        // Progress event where the assistant is just thinking (no tool_use block)
        let raw = br#"{"type":"progress","parentToolUseID":"toolu_01ABC","data":{"type":"agent_progress","agentId":"a951849","message":{"role":"assistant","content":[{"type":"text","text":"Let me think..."}]}}}"#;
        let line = parse_single_line(raw, &finders);
        let progress = line.sub_agent_progress.unwrap();
        assert_eq!(progress.current_tool, None);
    }

    #[test]
    fn test_progress_event_non_agent_type() {
        let finders = TailFinders::new();
        // Progress event that isn't agent_progress (should be ignored)
        let raw = br#"{"type":"progress","data":{"type":"tool_progress","tool":"Bash"}}"#;
        let line = parse_single_line(raw, &finders);
        assert!(line.sub_agent_progress.is_none());
    }

    #[test]
    fn test_progress_event_missing_agent_id() {
        let finders = TailFinders::new();
        let raw = br#"{"type":"progress","parentToolUseID":"toolu_01ABC","data":{"type":"agent_progress","message":{"role":"assistant","content":[]}}}"#;
        let line = parse_single_line(raw, &finders);
        assert!(line.sub_agent_progress.is_none()); // agentId required
    }

    #[test]
    fn test_progress_event_multiple_tool_uses() {
        // Progress event with multiple tool_use blocks — should pick the LAST one
        let finders = TailFinders::new();
        let raw = br#"{"type":"progress","parentToolUseID":"toolu_01ABC","data":{"type":"agent_progress","agentId":"a951849","message":{"role":"assistant","content":[{"type":"tool_use","name":"Read","input":{}},{"type":"text","text":"..."},{"type":"tool_use","name":"Grep","input":{}}]}}}"#;
        let line = parse_single_line(raw, &finders);
        let progress = line.sub_agent_progress.unwrap();
        assert_eq!(progress.current_tool, Some("Grep".to_string())); // Last tool_use
    }

    #[test]
    fn test_simd_prefilter_skips_non_progress() {
        // A regular assistant line should not trigger progress extraction
        let finders = TailFinders::new();
        let raw = br#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Here is the result."}]},"timestamp":"2026-02-16T08:34:13.134Z"}"#;
        let line = parse_single_line(raw, &finders);
        assert!(line.sub_agent_progress.is_none());
    }

    #[test]
    fn test_progress_line_classified_as_progress_not_assistant() {
        let finders = TailFinders::new();
        let raw = br#"{"type":"progress","parentToolUseID":"toolu_01ABC","data":{"type":"agent_progress","agentId":"a951849","message":{"role":"assistant","content":[{"type":"tool_use","name":"Read","input":{"file_path":"/path/to/file.rs"}}]}},"timestamp":"2026-02-16T08:34:13.134Z"}"#;
        let line = parse_single_line(raw, &finders);
        assert_eq!(
            line.line_type,
            LineType::Progress,
            "Progress lines must be classified as Progress, not Assistant"
        );
    }

    #[test]
    fn test_result_line_classified_as_result() {
        let finders = TailFinders::new();
        // Claude Code writes this as the final session line
        let raw = br#"{"type":"result","subtype":"success","duration_ms":12345,"duration_api_ms":10234,"is_error":false,"num_turns":5,"session_id":"abc123"}"#;
        let line = parse_single_line(raw, &finders);
        assert_eq!(
            line.line_type,
            LineType::Result,
            "Result lines must be classified as Result"
        );
    }

    #[test]
    fn test_parse_tail_extracts_ephemeral_cache_breakdown() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ephemeral.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"assistant","message":{{"role":"assistant","content":"hi","model":"claude-opus-4-6","usage":{{"input_tokens":100,"output_tokens":50,"cache_read_input_tokens":200,"cache_creation_input_tokens":57339,"cache_creation":{{"ephemeral_5m_input_tokens":0,"ephemeral_1h_input_tokens":57339}}}}}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        let line = &lines[0];
        assert_eq!(line.cache_creation_tokens, Some(57339));
        assert_eq!(line.cache_creation_5m_tokens, Some(0));
        assert_eq!(line.cache_creation_1hr_tokens, Some(57339));
    }

    #[test]
    fn test_parse_tail_no_ephemeral_breakdown() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("no_ephemeral.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"assistant","message":{{"role":"assistant","content":"hi","usage":{{"input_tokens":100,"output_tokens":50,"cache_creation_input_tokens":1000}}}}}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        let line = &lines[0];
        assert_eq!(line.cache_creation_tokens, Some(1000));
        assert_eq!(line.cache_creation_5m_tokens, None);
        assert_eq!(line.cache_creation_1hr_tokens, None);
    }

    #[test]
    fn test_tool_use_result_not_classified_as_result() {
        let finders = TailFinders::new();
        // toolUseResult is on a user line, should be User not Result
        let raw = br#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_01ABC","content":"done"}]},"toolUseResult":{"status":"completed"}}"#;
        let line = parse_single_line(raw, &finders);
        assert_eq!(
            line.line_type,
            LineType::User,
            "toolUseResult lines must remain User, not Result"
        );
    }

    #[test]
    fn test_spawn_and_progress_same_agent() {
        // Spawn detection and progress detection produce different fields
        let finders = TailFinders::new();

        // Spawn line
        let spawn_raw = br#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"toolu_01ABC","name":"Task","input":{"description":"Search auth","subagent_type":"Explore"}}]},"timestamp":"2026-02-16T08:34:00.000Z"}"#;
        let spawn_line = parse_single_line(spawn_raw, &finders);
        assert_eq!(spawn_line.sub_agent_spawns.len(), 1);
        assert!(spawn_line.sub_agent_progress.is_none());

        // Progress line
        let progress_raw = br#"{"type":"progress","parentToolUseID":"toolu_01ABC","data":{"type":"agent_progress","agentId":"a951849","message":{"role":"assistant","content":[{"type":"tool_use","name":"Grep","input":{}}]}}}"#;
        let progress_line = parse_single_line(progress_raw, &finders);
        assert!(progress_line.sub_agent_spawns.is_empty());
        assert!(progress_line.sub_agent_progress.is_some());
    }
}
