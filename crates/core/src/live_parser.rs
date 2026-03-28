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
use std::sync::atomic::{AtomicU64, Ordering};

use crate::progress::{RawTaskCreate, RawTaskIdAssignment, RawTaskUpdate, RawTodoItem};

/// Pattern for absolute file paths in plain text. No lookbehinds (regex-lite doesn't support them).
/// URL exclusion is handled by SIMD pre-filter: skip lines containing "://".
/// Matches paths like `/etc/hosts` (no extension) and `/src/auth.rs` (with extension).
/// Rejects bare directories like `/directory/` (trailing slash).
const PASTED_PATH_PATTERN: &str = r"(?:^|\s)(\/(?:[\w.-]+\/)*[\w.-]+)(?:\s|$|[,;:!?)])";

static PROGRESS_MESSAGE_CONTENT_FALLBACK_COUNT: AtomicU64 = AtomicU64::new(0);

/// Number of times progress parsing fell back from
/// `data.message.message.content` to `data.message.content`.
pub fn progress_message_content_fallback_count() -> u64 {
    PROGRESS_MESSAGE_CONTENT_FALLBACK_COUNT.load(Ordering::Relaxed)
}

/// Reset fallback counter. Primarily useful for tests.
pub fn reset_progress_message_content_fallback_count() {
    PROGRESS_MESSAGE_CONTENT_FALLBACK_COUNT.store(0, Ordering::Relaxed);
}

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
    /// Present when this spawn is a team member (from `input.team_name`).
    /// Used by accumulator/manager to skip adding to sub_agents[].
    pub team_name: Option<String>,
    /// Model alias or full ID from `input.model` (e.g., "haiku", "sonnet", "claude-haiku-4-5-20251001").
    /// None when omitted (sub-agent inherits parent model).
    pub model: Option<String>,
}

/// Extracted from a `type: "progress"` line with `data.type: "agent_progress"`.
#[derive(Debug, Clone)]
pub struct SubAgentProgress {
    /// Links back to the Task spawn's `tool_use_id`.
    pub parent_tool_use_id: String,
    /// Agent ID (available before completion!).
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
    /// Alphanumeric agent ID from `toolUseResult.agentId`.
    pub agent_id: Option<String>,
    pub status: String, // "completed", "error", etc.
    pub total_duration_ms: Option<u64>,
    /// Number of tool calls from `toolUseResult.totalToolUseCount`.
    pub total_tool_use_count: Option<u32>,
    pub usage_input_tokens: Option<u64>,
    pub usage_output_tokens: Option<u64>,
    pub usage_cache_read_tokens: Option<u64>,
    pub usage_cache_creation_tokens: Option<u64>,
    /// Model used by the sub-agent, from `toolUseResult.model`.
    /// None if not present in the result payload.
    pub model: Option<String>,
}

/// Extracted from a `type: "progress"` line with `data.type: "hook_progress"`.
#[derive(Debug, Clone)]
pub struct HookProgressData {
    pub hook_event: String,
    pub tool_name: Option<String>,
    pub source: Option<String>,
}

/// A single parsed JSONL line from the session log.
#[derive(Debug, Clone)]
pub struct LiveLine {
    pub line_type: LineType,
    pub role: Option<String>,
    pub content_preview: String,
    pub content_extended: String,
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
    /// Current working directory extracted from user-type JSONL lines.
    pub cwd: Option<String>,
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
    /// Skill names extracted from Skill tool_use blocks (from `input.skill`).
    pub skill_names: Vec<String>,
    /// Bash commands extracted from Bash tool_use blocks (from `input.command`, truncated to 200 chars).
    pub bash_commands: Vec<String>,
    /// File paths extracted from Edit/Write tool_use blocks (from `input.file_path`).
    pub edited_files: Vec<String>,
    /// Whether this is a system message with subtype "compact_boundary".
    pub is_compact_boundary: bool,
    /// Filename extracted from `<ide_opened_file>` tag, if present.
    pub ide_file: Option<String>,
    /// `message.id` from the API response (for dedup: one API response = multiple JSONL lines).
    pub message_id: Option<String>,
    /// `requestId` from the JSONL entry (for dedup: combined with message_id).
    pub request_id: Option<String>,
    /// If this is a hook_progress line, the extracted event data.
    pub hook_progress: Option<HookProgressData>,
    /// Session slug from top-level JSONL field (e.g. "async-greeting-dewdrop").
    /// Present on every line; extracted once by accumulator.
    pub slug: Option<String>,
    /// Team name from top-level `teamName` JSONL field.
    /// Present on every line after a `TeamCreate` tool_use succeeds.
    /// Set-once by accumulator (first non-None wins).
    pub team_name: Option<String>,
    /// Files referenced with `@filename` syntax in user messages.
    pub at_files: Vec<String>,
    /// Absolute file paths pasted in user messages (detected via regex, URL-filtered).
    pub pasted_paths: Vec<String>,
}

/// Broad classification of a JSONL line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineType {
    User,
    Assistant,
    System,
    Progress,
    Other,
}

/// Pre-compiled SIMD substring finders. Build once at startup via
/// [`TailFinders::new`] and share across polls.
pub struct TailFinders {
    pub content_key: memmem::Finder<'static>,
    pub model_key: memmem::Finder<'static>,
    pub usage_key: memmem::Finder<'static>,
    pub tool_use_key: memmem::Finder<'static>,
    pub name_key: memmem::Finder<'static>,
    pub stop_reason_key: memmem::Finder<'static>,
    pub task_name_key: memmem::Finder<'static>,
    pub agent_name_key: memmem::Finder<'static>,
    pub tool_use_result_key: memmem::Finder<'static>,
    pub agent_progress_key: memmem::Finder<'static>,
    pub todo_write_key: memmem::Finder<'static>,
    pub task_create_key: memmem::Finder<'static>,
    pub task_update_key: memmem::Finder<'static>,
    pub task_notification_key: memmem::Finder<'static>,
    pub compact_boundary_key: memmem::Finder<'static>,
    pub hook_progress_key: memmem::Finder<'static>,
    pub at_file_key: memmem::Finder<'static>,
}

impl TailFinders {
    /// Create all finders once. The needles are `'static` string slices.
    pub fn new() -> Self {
        Self {
            content_key: memmem::Finder::new(b"\"content\""),
            model_key: memmem::Finder::new(b"\"model\""),
            usage_key: memmem::Finder::new(b"\"usage\""),
            tool_use_key: memmem::Finder::new(b"\"tool_use\""),
            name_key: memmem::Finder::new(b"\"name\""),
            stop_reason_key: memmem::Finder::new(b"\"stop_reason\""),
            task_name_key: memmem::Finder::new(b"\"name\":\"Task\""),
            agent_name_key: memmem::Finder::new(b"\"name\":\"Agent\""),
            tool_use_result_key: memmem::Finder::new(b"\"toolUseResult\""),
            agent_progress_key: memmem::Finder::new(b"\"agent_progress\""),
            todo_write_key: memmem::Finder::new(b"\"name\":\"TodoWrite\""),
            task_create_key: memmem::Finder::new(b"\"name\":\"TaskCreate\""),
            task_update_key: memmem::Finder::new(b"\"name\":\"TaskUpdate\""),
            task_notification_key: memmem::Finder::new(b"<task-notification>"),
            compact_boundary_key: memmem::Finder::new(b"\"compact_boundary\""),
            hook_progress_key: memmem::Finder::new(b"\"hook_progress\""),
            at_file_key: memmem::Finder::new(b"@"),
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

#[derive(Debug, Default, Clone)]
struct ToolUseResultPayload {
    agent_id: Option<String>,
    status: Option<String>,
    total_duration_ms: Option<u64>,
    total_tool_use_count: Option<u32>,
    usage_input_tokens: Option<u64>,
    usage_output_tokens: Option<u64>,
    usage_cache_read_tokens: Option<u64>,
    usage_cache_creation_tokens: Option<u64>,
    model: Option<String>,
}

impl ToolUseResultPayload {
    fn has_data(&self) -> bool {
        self.agent_id.is_some()
            || self.status.is_some()
            || self.total_duration_ms.is_some()
            || self.total_tool_use_count.is_some()
            || self.usage_input_tokens.is_some()
            || self.usage_output_tokens.is_some()
            || self.usage_cache_read_tokens.is_some()
            || self.usage_cache_creation_tokens.is_some()
            || self.model.is_some()
    }

    fn merge(&mut self, other: ToolUseResultPayload) {
        if other.agent_id.is_some() && self.agent_id.is_none() {
            self.agent_id = other.agent_id;
        }
        if other.status.is_some() {
            self.status = other.status;
        }
        if other.total_duration_ms.is_some() && self.total_duration_ms.is_none() {
            self.total_duration_ms = other.total_duration_ms;
        }
        if other.total_tool_use_count.is_some() && self.total_tool_use_count.is_none() {
            self.total_tool_use_count = other.total_tool_use_count;
        }
        if other.usage_input_tokens.is_some() && self.usage_input_tokens.is_none() {
            self.usage_input_tokens = other.usage_input_tokens;
        }
        if other.usage_output_tokens.is_some() && self.usage_output_tokens.is_none() {
            self.usage_output_tokens = other.usage_output_tokens;
        }
        if other.usage_cache_read_tokens.is_some() && self.usage_cache_read_tokens.is_none() {
            self.usage_cache_read_tokens = other.usage_cache_read_tokens;
        }
        if other.usage_cache_creation_tokens.is_some() && self.usage_cache_creation_tokens.is_none()
        {
            self.usage_cache_creation_tokens = other.usage_cache_creation_tokens;
        }
        if other.model.is_some() && self.model.is_none() {
            self.model = other.model;
        }
    }
}

fn parse_tool_use_result_payload(tur: &serde_json::Value) -> Option<ToolUseResultPayload> {
    match tur {
        serde_json::Value::Object(obj) => {
            let has_known_key = obj.contains_key("status")
                || obj.contains_key("agentId")
                || obj.contains_key("totalDurationMs")
                || obj.contains_key("totalToolUseCount")
                || obj.contains_key("usage")
                || obj.contains_key("model");
            if !has_known_key {
                return None;
            }

            let status = obj
                .get("status")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .or_else(|| Some("completed".to_string()));
            let usage = obj.get("usage");

            Some(ToolUseResultPayload {
                agent_id: obj
                    .get("agentId")
                    .or_else(|| obj.get("agent_id")) // team spawns use snake_case
                    .and_then(|v| v.as_str())
                    .map(String::from),
                status,
                total_duration_ms: obj.get("totalDurationMs").and_then(|v| v.as_u64()),
                total_tool_use_count: obj
                    .get("totalToolUseCount")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u32),
                usage_input_tokens: usage
                    .and_then(|u| u.get("input_tokens"))
                    .and_then(|v| v.as_u64()),
                usage_output_tokens: usage
                    .and_then(|u| u.get("output_tokens"))
                    .and_then(|v| v.as_u64()),
                usage_cache_read_tokens: usage
                    .and_then(|u| u.get("cache_read_input_tokens"))
                    .and_then(|v| v.as_u64()),
                usage_cache_creation_tokens: usage
                    .and_then(|u| u.get("cache_creation_input_tokens"))
                    .and_then(|v| v.as_u64()),
                model: obj.get("model").and_then(|v| v.as_str()).map(String::from),
            })
        }
        serde_json::Value::String(status) => {
            let status = status.trim();
            if status.is_empty() {
                None
            } else {
                Some(ToolUseResultPayload {
                    status: Some(status.to_string()),
                    ..ToolUseResultPayload::default()
                })
            }
        }
        serde_json::Value::Array(items) => {
            let mut merged = ToolUseResultPayload::default();
            for item in items {
                if let Some(parsed) = parse_tool_use_result_payload(item) {
                    merged.merge(parsed);
                }
            }
            if merged.has_data() {
                Some(merged)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn progress_content_blocks(data: &serde_json::Value) -> (Option<&Vec<serde_json::Value>>, bool) {
    let primary = data
        .get("message")
        .and_then(|m| m.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_array());
    if primary.is_some() {
        return (primary, false);
    }

    let fallback = data
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_array());
    (fallback, fallback.is_some())
}

fn json_value_kind(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

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
        Err(_) => {
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
    // NOTE: .as_str() returns None for array content, defaulting to "" (false).
    // This is safe because system-prefix messages (<command-name>, <task-notification>,
    // <local-command-stdout>) always use string content in Claude Code JSONL — never
    // inside content arrays.
    let has_system_prefix = if line_type == LineType::User {
        let raw = content_source
            .get("content")
            .and_then(|c| c.as_str())
            .unwrap_or("");
        let c = raw.trim_start();
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

    // Extract message.id and requestId for content-block dedup.
    // Claude Code writes one JSONL line per content block (thinking, text, tool_use),
    // each carrying the full message-level usage. We need these IDs to avoid counting
    // tokens/cost multiple times for the same API response.
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
    // Claude Code renamed the tool from "Task" to "Agent" — detect both.
    let mut sub_agent_spawns = Vec::new();
    if line_type == LineType::Assistant
        && (finders.task_name_key.find(raw).is_some() || finders.agent_name_key.find(raw).is_some())
    {
        // Already have `parsed` from JSON parse above
        if let Some(content) = msg
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_array())
        {
            for block in content {
                if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                    let tool_name = block.get("name").and_then(|n| n.as_str()).unwrap_or("");
                    if tool_name != "Task" && tool_name != "Agent" {
                        continue;
                    }
                    let tool_use_id = block
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let input = block.get("input");
                    // Agent tool uses input.name as display name, Task uses input.description
                    let description = input
                        .and_then(|i| i.get("name").or_else(|| i.get("description")))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let agent_type = input
                        .and_then(|i| i.get("subagent_type"))
                        .and_then(|v| v.as_str())
                        .unwrap_or(tool_name)
                        .to_string();
                    let spawn_team_name = input
                        .and_then(|i| i.get("team_name"))
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    let spawn_model = input
                        .and_then(|i| i.get("model"))
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    if !tool_use_id.is_empty() {
                        sub_agent_spawns.push(SubAgentSpawn {
                            tool_use_id,
                            agent_type,
                            description,
                            team_name: spawn_team_name,
                            model: spawn_model,
                        });
                    }
                }
            }
        }
    }

    // --- Sub-agent completion detection (user lines with toolUseResult) ---
    let sub_agent_result =
        if line_type == LineType::User && finders.tool_use_result_key.find(raw).is_some() {
            // Extract toolUseResult from top-level (NOT inside message.content)
            parsed.get("toolUseResult").and_then(|tur| {
                let parsed_tur = match parse_tool_use_result_payload(tur) {
                    Some(parsed) => parsed,
                    None => {
                        tracing::debug!(
                            kind = json_value_kind(tur),
                            "Ignoring unsupported toolUseResult variant"
                        );
                        return None;
                    }
                };

                // Find the matching tool_use_id from the tool_result block in content
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
                    })?;
                Some(SubAgentResult {
                    tool_use_id,
                    agent_id: parsed_tur.agent_id,
                    status: parsed_tur.status.unwrap_or_else(|| "completed".to_string()),
                    total_duration_ms: parsed_tur.total_duration_ms,
                    total_tool_use_count: parsed_tur.total_tool_use_count,
                    usage_input_tokens: parsed_tur.usage_input_tokens,
                    usage_output_tokens: parsed_tur.usage_output_tokens,
                    usage_cache_read_tokens: parsed_tur.usage_cache_read_tokens,
                    usage_cache_creation_tokens: parsed_tur.usage_cache_creation_tokens,
                    model: parsed_tur.model,
                })
            })
        } else {
            None
        };

    // --- Sub-agent progress detection (progress lines with agent_progress) ---
    // SIMD pre-filter on "agent_progress" + exact LineType::Progress check.
    let sub_agent_progress =
        if line_type == LineType::Progress && finders.agent_progress_key.find(raw).is_some() {
            parsed.get("data").and_then(|data| {
                if data.get("type").and_then(|t| t.as_str()) != Some("agent_progress") {
                    return None;
                }
                let parent_tool_use_id = parsed
                    .get("parentToolUseID")
                    .and_then(|v| v.as_str())
                    .map(String::from)?;
                let agent_id = data
                    .get("agentId")
                    .and_then(|v| v.as_str())
                    .map(String::from)?;
                // Primary: data.message.message.content[*]
                // Fallback: data.message.content[*]
                let (progress_blocks, used_fallback) = progress_content_blocks(data);
                if used_fallback {
                    PROGRESS_MESSAGE_CONTENT_FALLBACK_COUNT.fetch_add(1, Ordering::Relaxed);
                }

                let current_tool = progress_blocks.and_then(|blocks| {
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

    // --- Hook progress detection (progress lines with hook_progress) ---
    let mut result_hook_progress = None;
    if line_type == LineType::Progress && finders.hook_progress_key.find(raw).is_some() {
        if let Some(hook_event) = parsed.pointer("/data/hookEvent").and_then(|v| v.as_str()) {
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
            result_hook_progress = Some(HookProgressData {
                hook_event: hook_event.to_string(),
                tool_name,
                source,
            });
        }
    }

    // --- TodoWrite detection (assistant lines with TodoWrite tool_use) ---
    let todo_write =
        if line_type == LineType::Assistant && finders.todo_write_key.find(raw).is_some() {
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
                                    todos
                                        .iter()
                                        .filter_map(|item| {
                                            Some(RawTodoItem {
                                                content: item
                                                    .get("content")
                                                    .and_then(|v| v.as_str())?
                                                    .to_string(),
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
                                        .collect::<Vec<_>>()
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
    }

    // --- TaskUpdate detection (assistant lines with TaskUpdate tool_use) ---
    let mut task_updates = Vec::new();
    if line_type == LineType::Assistant && finders.task_update_key.find(raw).is_some() {
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
    }

    // --- TaskIdAssignment detection (user lines with toolUseResult containing task.id) ---
    let mut task_id_assignments = Vec::new();
    if line_type == LineType::User && finders.tool_use_result_key.find(raw).is_some() {
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
                task_id_assignments.push(RawTaskIdAssignment {
                    tool_use_id,
                    task_id: task_id.to_string(),
                });
            }
        }
    }

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
    }
}

/// Strip XML noise tags from user message content and extract IDE file context.
///
/// Returns `(clean_text, ide_file)` where:
/// - `clean_text` is the content with all noise tags removed, trimmed
/// - `ide_file` is the last path component from `<ide_opened_file>` if present
///
/// Tags stripped: system-reminder, ide_opened_file, ide_selection, command-name,
/// command-args, command-message, local-command-stdout, local-command-caveat,
/// task-notification, user-prompt-submit-hook.
///
/// NOTE: The IDE filename extraction regex ("the file\s+(\S+)\s+in the IDE") is
/// coupled to Claude Code's current hook output format. If the format changes,
/// extraction gracefully degrades to `None`.
pub(crate) fn strip_noise_tags(content: &str) -> (String, Option<String>) {
    use regex_lite::Regex;
    use std::sync::OnceLock;

    // `regex-lite` does NOT support backreferences (\1), so each tag must be
    // enumerated with its explicit closing tag. Uses OnceLock to match codebase
    // convention (see cli.rs, sync.rs, metrics.rs).
    static NOISE_TAGS: OnceLock<Regex> = OnceLock::new();
    let noise_re = NOISE_TAGS.get_or_init(|| {
        Regex::new(concat!(
            r"(?s)<system-reminder>.*?</system-reminder>\s*",
            r"|<ide_selection>.*?</ide_selection>\s*",
            r"|<command-name>.*?</command-name>\s*",
            r"|<command-args>.*?</command-args>\s*",
            r"|<command-message>.*?</command-message>\s*",
            r"|<local-command-stdout>.*?</local-command-stdout>\s*",
            r"|<local-command-caveat>.*?</local-command-caveat>\s*",
            r"|<task-notification>.*?</task-notification>\s*",
            r"|<user-prompt-submit-hook>.*?</user-prompt-submit-hook>\s*",
        ))
        .unwrap()
    });

    // Separate regex for ide_opened_file to extract filename before stripping
    static IDE_FILE_TAG: OnceLock<Regex> = OnceLock::new();
    let ide_tag_re = IDE_FILE_TAG
        .get_or_init(|| Regex::new(r"(?s)<ide_opened_file>.*?</ide_opened_file>\s*").unwrap());

    static IDE_FILE_PATH: OnceLock<Regex> = OnceLock::new();
    let ide_path_re =
        IDE_FILE_PATH.get_or_init(|| Regex::new(r"the file\s+(\S+)\s+in the IDE").unwrap());

    // Extract IDE file before stripping
    let ide_file = ide_tag_re.find(content).and_then(|m| {
        ide_path_re.captures(m.as_str()).and_then(|caps| {
            caps.get(1).map(|p| {
                let path = p.as_str();
                // Extract last path component (filename)
                path.rsplit('/').next().unwrap_or(path).to_string()
            })
        })
    });

    // Strip all noise tags
    let cleaned = noise_re.replace_all(content, "");
    // Strip ide_opened_file tag
    let cleaned = ide_tag_re.replace_all(&cleaned, "");
    let cleaned = cleaned.trim().to_string();

    (cleaned, ide_file)
}

/// Extract content preview (truncated to 200 chars), tool_use names,
/// skill names (from Skill tool_use `input.skill`), and whether the
/// content array contains a `tool_result` block.
/// Content extraction result from `extract_content_and_tools`.
type ContentExtraction = (
    String,         // content_preview
    String,         // content_extended (500-char truncation for phase classifier)
    Vec<String>,    // tool_names
    Vec<String>,    // skill_names
    Vec<String>,    // bash_commands
    Vec<String>,    // edited_files
    bool,           // has_tool_result
    Option<String>, // ide_file
    Vec<String>,    // at_files
);

fn extract_content_and_tools(
    parsed: &serde_json::Value,
    finders: &TailFinders,
) -> ContentExtraction {
    use std::sync::OnceLock;
    static AT_FILE_RE: OnceLock<regex_lite::Regex> = OnceLock::new();
    let at_file_re = AT_FILE_RE
        .get_or_init(|| regex_lite::Regex::new(r"(?:^|\s)@([\w./-]+\.\w{1,15})").unwrap());

    let mut preview = String::new();
    let mut extended = String::new();
    let mut tool_names = Vec::new();
    let mut skill_names = Vec::new();
    let mut bash_commands = Vec::new();
    let mut edited_files = Vec::new();
    let mut has_tool_result = false;
    let mut ide_file: Option<String> = None;
    let mut at_files: Vec<String> = Vec::new();

    match parsed.get("content") {
        Some(serde_json::Value::String(s)) => {
            // Extract @file references from raw string before noise stripping
            if finders.at_file_key.find(s.as_bytes()).is_some() {
                for caps in at_file_re.captures_iter(s) {
                    if let Some(m) = caps.get(1) {
                        at_files.push(m.as_str().to_string());
                    }
                }
            }
            let (stripped, file) = strip_noise_tags(s);
            preview = truncate_str(&stripped, 200);
            extended = truncate_str(&stripped, 500);
            ide_file = file;
        }
        Some(serde_json::Value::Array(blocks)) => {
            for block in blocks {
                match block.get("type").and_then(|t| t.as_str()) {
                    Some("text") => {
                        if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                            // Extract @file references from raw text before noise stripping
                            if finders.at_file_key.find(text.as_bytes()).is_some() {
                                for caps in at_file_re.captures_iter(text) {
                                    if let Some(m) = caps.get(1) {
                                        at_files.push(m.as_str().to_string());
                                    }
                                }
                            }
                            if preview.is_empty() {
                                let (stripped, file) = strip_noise_tags(text);
                                preview = truncate_str(&stripped, 200);
                                extended = truncate_str(&stripped, 500);
                                if ide_file.is_none() {
                                    ide_file = file;
                                }
                            }
                        }
                    }
                    Some("tool_use") => {
                        if let Some(name) = block.get("name").and_then(|n| n.as_str()) {
                            tool_names.push(name.to_string());
                            let input = block.get("input");
                            match name {
                                "Skill" => {
                                    if let Some(skill) =
                                        input.and_then(|i| i.get("skill")).and_then(|s| s.as_str())
                                    {
                                        if !skill.is_empty() {
                                            skill_names.push(skill.to_string());
                                        }
                                    }
                                }
                                "Bash" => {
                                    if let Some(cmd) = input
                                        .and_then(|i| i.get("command"))
                                        .and_then(|s| s.as_str())
                                    {
                                        if !cmd.is_empty() {
                                            bash_commands.push(truncate_str(cmd, 200).to_string());
                                        }
                                    }
                                }
                                "Edit" | "Write" => {
                                    if let Some(fp) = input
                                        .and_then(|i| i.get("file_path"))
                                        .and_then(|s| s.as_str())
                                    {
                                        if !fp.is_empty() {
                                            edited_files.push(fp.to_string());
                                        }
                                    }
                                }
                                _ => {}
                            }
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

    (
        preview,
        extended,
        tool_names,
        skill_names,
        bash_commands,
        edited_files,
        has_tool_result,
        ide_file,
        at_files,
    )
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
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"Hello world"}}}}"#
        )
        .unwrap();
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
            writeln!(
                f,
                r#"{{"type":"user","message":{{"role":"user","content":"hello"}}}}"#
            )
            .unwrap();
            writeln!(f, r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"text","text":"hi"}}]}}}}"#).unwrap();
        }

        let finders = TailFinders::new();
        let (lines, offset) = parse_tail(&path, 0, &finders).unwrap();
        assert!(!lines.is_empty());
        assert!(offset > 0);

        // "Replace" the file with smaller content (simulates log rotation)
        {
            let mut f = File::create(&path).unwrap();
            writeln!(
                f,
                r#"{{"type":"user","message":{{"role":"user","content":"new session"}}}}"#
            )
            .unwrap();
        }

        // Old offset is larger than new file — should reset and read from start
        let (lines2, offset2) = parse_tail(&path, offset, &finders).unwrap();
        assert!(
            !lines2.is_empty(),
            "Should read new content after file replacement"
        );
        assert!(offset2 > 0);
    }

    #[test]
    fn test_parse_tail_incremental() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("incremental.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"first"}}}}"#
        )
        .unwrap();
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
        writeln!(
            f,
            r#"{{"type":"assistant","message":{{"role":"assistant","content":"second"}}}}"#
        )
        .unwrap();
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
    fn test_parse_cwd_extraction() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cwd.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"hello"}},"cwd":"/Users/u/dev/repo/.worktrees/feat","gitBranch":"main","timestamp":"2026-01-15T10:30:00Z"}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(
            lines[0].cwd.as_deref(),
            Some("/Users/u/dev/repo/.worktrees/feat")
        );
        assert_eq!(lines[0].git_branch.as_deref(), Some("main"));
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
        writeln!(f, r#"{{"type":"user","message":{{"role":"user"}}}}"#).unwrap();
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

    // -------------------------------------------------------------------------
    // Skill name extraction from Skill tool_use
    // -------------------------------------------------------------------------

    #[test]
    fn test_skill_name_extracted_from_skill_tool_use() {
        let finders = TailFinders::new();
        let raw = br#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Let me commit."},{"type":"tool_use","id":"toolu_01XYZ","name":"Skill","input":{"skill":"commit","args":"-m 'fix bug'"}}]}}"#;
        let line = parse_single_line(raw, &finders);
        assert_eq!(line.tool_names, vec!["Skill"]);
        assert_eq!(line.skill_names, vec!["commit"]);
    }

    #[test]
    fn test_multiple_skill_invocations() {
        let finders = TailFinders::new();
        let raw = br#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_01A","name":"Skill","input":{"skill":"commit"}},{"type":"tool_use","id":"toolu_01B","name":"Skill","input":{"skill":"review-pr","args":"123"}}]}}"#;
        let line = parse_single_line(raw, &finders);
        assert_eq!(line.tool_names, vec!["Skill", "Skill"]);
        assert_eq!(line.skill_names, vec!["commit", "review-pr"]);
    }

    #[test]
    fn test_skill_name_empty_ignored() {
        let finders = TailFinders::new();
        let raw = br#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_01A","name":"Skill","input":{"skill":""}}]}}"#;
        let line = parse_single_line(raw, &finders);
        assert_eq!(line.tool_names, vec!["Skill"]);
        assert!(
            line.skill_names.is_empty(),
            "Empty skill name should be ignored"
        );
    }

    #[test]
    fn test_skill_name_missing_input() {
        let finders = TailFinders::new();
        let raw = br#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_01A","name":"Skill","input":{}}]}}"#;
        let line = parse_single_line(raw, &finders);
        assert_eq!(line.tool_names, vec!["Skill"]);
        assert!(
            line.skill_names.is_empty(),
            "Missing skill field should produce no skill_names"
        );
    }

    #[test]
    fn test_non_skill_tool_use_no_skill_names() {
        let finders = TailFinders::new();
        let raw = br#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_01A","name":"Bash","input":{"command":"ls"}},{"type":"tool_use","id":"toolu_01B","name":"mcp__plugin_playwright_playwright__browser_navigate","input":{"url":"http://example.com"}}]}}"#;
        let line = parse_single_line(raw, &finders);
        assert_eq!(
            line.tool_names,
            vec![
                "Bash",
                "mcp__plugin_playwright_playwright__browser_navigate"
            ]
        );
        assert!(
            line.skill_names.is_empty(),
            "Non-Skill tools should produce no skill_names"
        );
    }

    #[test]
    fn test_mixed_skill_and_mcp_tools() {
        let finders = TailFinders::new();
        let raw = br#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_01A","name":"mcp__chrome-devtools__take_screenshot","input":{}},{"type":"tool_use","id":"toolu_01B","name":"Skill","input":{"skill":"pdf"}}]}}"#;
        let line = parse_single_line(raw, &finders);
        assert_eq!(
            line.tool_names,
            vec!["mcp__chrome-devtools__take_screenshot", "Skill"]
        );
        assert_eq!(line.skill_names, vec!["pdf"]);
    }

    // -------------------------------------------------------------------------
    // strip_noise_tags
    // -------------------------------------------------------------------------

    #[test]
    fn test_strip_noise_tags_system_reminder() {
        let input = "<system-reminder>SessionStart:startup hook success: Success</system-reminder>fix the bug";
        let (clean, file) = strip_noise_tags(input);
        assert_eq!(clean, "fix the bug");
        assert!(file.is_none());
    }

    #[test]
    fn test_strip_noise_tags_ide_opened_file() {
        let input = "<ide_opened_file>The user opened the file /Users/me/project/src/auth.rs in the IDE. This may or may not be related to the current task.</ide_opened_file> continue";
        let (clean, file) = strip_noise_tags(input);
        assert_eq!(clean, "continue");
        assert_eq!(file.as_deref(), Some("auth.rs"));
    }

    #[test]
    fn test_strip_noise_tags_multiple_tags() {
        let input = "<system-reminder>hook data</system-reminder><ide_opened_file>The user opened the file /path/to/main.rs in the IDE.</ide_opened_file><ide_selection>some code</ide_selection> do the thing";
        let (clean, file) = strip_noise_tags(input);
        assert_eq!(clean, "do the thing");
        assert_eq!(file.as_deref(), Some("main.rs"));
    }

    #[test]
    fn test_strip_noise_tags_no_tags() {
        let input = "just a normal message";
        let (clean, file) = strip_noise_tags(input);
        assert_eq!(clean, "just a normal message");
        assert!(file.is_none());
    }

    #[test]
    fn test_strip_noise_tags_only_tags() {
        let input = "<system-reminder>hook stuff</system-reminder>";
        let (clean, file) = strip_noise_tags(input);
        assert_eq!(clean, "");
        assert!(file.is_none());
    }

    #[test]
    fn test_strip_noise_tags_command_tags() {
        let input = "<command-name>/clear</command-name><command-message>Clearing context</command-message> hello";
        let (clean, file) = strip_noise_tags(input);
        assert_eq!(clean, "hello");
        assert!(file.is_none());
    }

    #[test]
    fn test_strip_noise_tags_nested_path() {
        let input = "<ide_opened_file>The user opened the file /deep/nested/path/to/component.tsx in the IDE.</ide_opened_file>review this";
        let (clean, file) = strip_noise_tags(input);
        assert_eq!(clean, "review this");
        assert_eq!(file.as_deref(), Some("component.tsx"));
    }

    #[test]
    fn test_strip_noise_tags_user_prompt_submit_hook() {
        let input = "<user-prompt-submit-hook>hook output</user-prompt-submit-hook>fix tests";
        let (clean, file) = strip_noise_tags(input);
        assert_eq!(clean, "fix tests");
        assert!(file.is_none());
    }

    #[test]
    fn test_strip_noise_tags_whitespace_between_text() {
        let input = "hello <system-reminder>hook data</system-reminder> world";
        let (clean, file) = strip_noise_tags(input);
        assert_eq!(clean, "hello world");
        assert!(file.is_none());
    }

    #[test]
    fn test_parse_tail_strips_noise_tags_from_preview() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tags.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"<system-reminder>hook data</system-reminder>fix the bug"}}}}"#
        ).unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].content_preview, "fix the bug");
        assert!(lines[0].ide_file.is_none());
    }

    #[test]
    fn test_parse_tail_extracts_ide_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ide.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"<ide_opened_file>The user opened the file /src/auth.rs in the IDE.</ide_opened_file> continue"}}}}"#
        ).unwrap();
        f.flush().unwrap();

        let finders = TailFinders::new();
        let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].content_preview, "continue");
        assert_eq!(lines[0].ide_file.as_deref(), Some("auth.rs"));
    }

    #[test]
    fn test_hook_progress_pre_tool_use() {
        let line = br#"{"type":"progress","timestamp":"2026-03-07T12:00:00Z","data":{"type":"hook_progress","hookEvent":"PreToolUse","hookName":"PreToolUse:Read","hookId":"h1","command":"curl ...","status":"success"}}"#;
        let finders = TailFinders::new();
        let result = parse_single_line(line, &finders);
        assert_eq!(result.line_type, LineType::Progress);
        let hp = result.hook_progress.expect("hook_progress should be Some");
        assert_eq!(hp.hook_event, "PreToolUse");
        assert_eq!(hp.tool_name, Some("Read".to_string()));
        assert_eq!(hp.source, None);
    }

    #[test]
    fn test_hook_progress_session_start_compact() {
        let line = br#"{"type":"progress","timestamp":"2026-03-07T12:00:00Z","data":{"type":"hook_progress","hookEvent":"SessionStart","hookName":"SessionStart:compact","hookId":"h2","command":"curl ...","status":"success"}}"#;
        let finders = TailFinders::new();
        let result = parse_single_line(line, &finders);
        let hp = result.hook_progress.expect("hook_progress should be Some");
        assert_eq!(hp.hook_event, "SessionStart");
        assert_eq!(hp.tool_name, None);
        assert_eq!(hp.source, Some("compact".to_string()));
    }

    #[test]
    fn test_hook_progress_stop_no_colon() {
        let line = br#"{"type":"progress","timestamp":"2026-03-07T12:00:00Z","data":{"type":"hook_progress","hookEvent":"Stop","hookName":"Stop","hookId":"h3","command":"curl ...","status":"success"}}"#;
        let finders = TailFinders::new();
        let result = parse_single_line(line, &finders);
        let hp = result.hook_progress.expect("hook_progress should be Some");
        assert_eq!(hp.hook_event, "Stop");
        assert_eq!(hp.tool_name, None);
        assert_eq!(hp.source, None);
    }

    #[test]
    fn test_hook_progress_malformed_json() {
        let line = br#"{"type":"progress","data":{"type":"hook_progress","broken"#;
        let finders = TailFinders::new();
        let result = parse_single_line(line, &finders);
        assert!(
            result.hook_progress.is_none(),
            "Malformed JSON should be None"
        );
    }

    #[test]
    fn test_sub_agent_spawn_agent_tool() {
        // Claude Code >= 0.10 renamed "Task" to "Agent" and uses input.name for display.
        let line = br#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_01ABC","name":"Agent","input":{"name":"Gate 1: Code Quality","description":"General code review","subagent_type":"code-reviewer","prompt":"Review code","run_in_background":true}}]},"timestamp":"2026-03-08T10:00:00Z"}"#;
        let finders = TailFinders::new();
        let result = parse_single_line(line, &finders);
        assert_eq!(result.sub_agent_spawns.len(), 1);
        let spawn = &result.sub_agent_spawns[0];
        assert_eq!(spawn.tool_use_id, "toolu_01ABC");
        assert_eq!(spawn.agent_type, "code-reviewer");
        // Should prefer input.name over input.description
        assert_eq!(spawn.description, "Gate 1: Code Quality");
    }

    #[test]
    fn test_sub_agent_spawn_legacy_task_tool() {
        // Pre-0.10 format uses "Task" tool name with input.description.
        let line = br#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_02DEF","name":"Task","input":{"description":"Search codebase","subagent_type":"Explore"}}]},"timestamp":"2026-02-20T10:00:00Z"}"#;
        let finders = TailFinders::new();
        let result = parse_single_line(line, &finders);
        assert_eq!(result.sub_agent_spawns.len(), 1);
        let spawn = &result.sub_agent_spawns[0];
        assert_eq!(spawn.tool_use_id, "toolu_02DEF");
        assert_eq!(spawn.agent_type, "Explore");
        assert_eq!(spawn.description, "Search codebase");
    }

    #[test]
    fn test_sub_agent_spawn_agent_without_name_field() {
        // Agent tool without input.name — should fall back to input.description.
        let line = br#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_03GHI","name":"Agent","input":{"description":"Audit the codebase","prompt":"Do audit"}}]},"timestamp":"2026-03-08T11:00:00Z"}"#;
        let finders = TailFinders::new();
        let result = parse_single_line(line, &finders);
        assert_eq!(result.sub_agent_spawns.len(), 1);
        let spawn = &result.sub_agent_spawns[0];
        assert_eq!(spawn.tool_use_id, "toolu_03GHI");
        assert_eq!(spawn.agent_type, "Agent"); // no subagent_type → falls back to tool name
        assert_eq!(spawn.description, "Audit the codebase");
    }

    #[test]
    fn test_hook_progress_empty_hook_name() {
        let line = br#"{"type":"progress","timestamp":"2026-03-07T12:00:00Z","data":{"type":"hook_progress","hookEvent":"PreToolUse","hookName":"","hookId":"h4","command":"curl ...","status":"success"}}"#;
        let finders = TailFinders::new();
        let result = parse_single_line(line, &finders);
        let hp = result.hook_progress.expect("hook_progress should be Some");
        assert_eq!(hp.hook_event, "PreToolUse");
        assert_eq!(hp.tool_name, None);
        assert_eq!(hp.source, None);
    }

    #[test]
    fn test_slug_extraction() {
        let finders = TailFinders::new();
        let line = br#"{"type":"user","slug":"async-greeting-dewdrop","message":{"role":"user","content":"hello"}}"#;
        let parsed = parse_single_line(line, &finders);
        assert_eq!(parsed.slug.as_deref(), Some("async-greeting-dewdrop"));
    }

    #[test]
    fn test_slug_missing() {
        let finders = TailFinders::new();
        let line = br#"{"type":"user","message":{"role":"user","content":"hello"}}"#;
        let parsed = parse_single_line(line, &finders);
        assert!(parsed.slug.is_none());
    }

    // -------------------------------------------------------------------------
    // Team name extraction from top-level `teamName` JSONL field
    // -------------------------------------------------------------------------

    #[test]
    fn test_team_name_from_top_level_field() {
        // Real data: after TeamCreate succeeds, every subsequent JSONL line has top-level `teamName`
        let line = br#"{"type":"assistant","teamName":"demo-team","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_01AG","name":"Agent","input":{"name":"agent-sysinfo","description":"System info agent","prompt":"..."}}]},"timestamp":"2026-03-11T10:00:00Z"}"#;
        let finders = TailFinders::new();
        let result = parse_single_line(line, &finders);
        assert_eq!(result.team_name.as_deref(), Some("demo-team"));
        assert_eq!(result.sub_agent_spawns.len(), 1);
        // This spawn has no input.team_name — just a regular Agent spawn within a team session
        assert!(result.sub_agent_spawns[0].team_name.is_none());
    }

    #[test]
    fn test_spawn_with_input_team_name() {
        // Real data: team member spawn has team_name in input (from Agent tool)
        let line = br#"{"type":"assistant","teamName":"nvda-demo","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_015Y","name":"Agent","input":{"description":"NVDA stock researcher","name":"researcher","team_name":"nvda-demo","subagent_type":"general-purpose","prompt":"...","run_in_background":true}}]},"timestamp":"2026-03-10T19:04:44Z"}"#;
        let finders = TailFinders::new();
        let result = parse_single_line(line, &finders);
        assert_eq!(result.team_name.as_deref(), Some("nvda-demo"));
        assert_eq!(result.sub_agent_spawns.len(), 1);
        // team_name extracted from input.team_name — this is the discriminator
        assert_eq!(
            result.sub_agent_spawns[0].team_name.as_deref(),
            Some("nvda-demo")
        );
    }

    #[test]
    fn test_no_team_name_without_top_level_field() {
        // Regular sub-agent spawn without any team context
        let line = br#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_01REG","name":"Agent","input":{"name":"Search auth","description":"Search auth code","subagent_type":"Explore"}}]},"timestamp":"2026-03-08T10:00:00Z"}"#;
        let finders = TailFinders::new();
        let result = parse_single_line(line, &finders);
        assert_eq!(result.sub_agent_spawns.len(), 1);
        assert!(result.team_name.is_none());
        assert!(result.sub_agent_spawns[0].team_name.is_none());
    }

    #[test]
    fn pasted_path_regex_matches_absolute_paths() {
        let re = regex_lite::Regex::new(PASTED_PATH_PATTERN).unwrap();
        let cases = vec![
            (
                "look at /Users/dev/project/src/auth.rs",
                Some("/Users/dev/project/src/auth.rs"),
            ),
            ("/etc/hosts is the file", Some("/etc/hosts")),
            ("check /tmp/test.txt, please", Some("/tmp/test.txt")),
            ("no path here", None),
            ("relative/path.rs not matched", None),
            ("just a /directory/ not matched", None), // no file extension
        ];
        for (input, expected) in cases {
            let found = re
                .captures(input)
                .map(|c| c.get(1).unwrap().as_str().to_string());
            assert_eq!(found.as_deref(), expected, "input: {input}");
        }
    }

    #[test]
    fn pasted_path_skips_urls() {
        // Lines containing :// should be pre-filtered before regex application
        let line = "see https://github.com/foo/bar.rs for details";
        assert!(line.contains("://"), "pre-filter should skip this line");
        // The regex itself does NOT need a lookbehind — the pre-filter handles it
    }

    #[test]
    fn pasted_path_regex_compiles_with_regex_lite() {
        // This test ensures we never accidentally use regex features unsupported by regex-lite
        let re = regex_lite::Regex::new(PASTED_PATH_PATTERN);
        assert!(
            re.is_ok(),
            "regex must compile with regex-lite (no lookbehinds, no Unicode classes)"
        );
    }
}
