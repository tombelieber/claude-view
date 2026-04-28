// crates/db/src/indexer_parallel/parser/core.rs
// Full JSON parser that extracts all 7 JSONL line types.

use claude_view_core::{count_ai_lines, is_human_tool_result_content, is_system_user_content};
use memchr::memmem;

use crate::indexer_parallel::handlers::*;
use crate::indexer_parallel::helpers::*;
use crate::indexer_parallel::serde_types::*;
use crate::indexer_parallel::types::*;

/// Full JSON parser that extracts all 7 JSONL line types.
///
/// Returns a `ParseResult` containing deep metadata (tool counts, skills, token usage,
/// system metrics, progress counts, etc.) and raw tool_use invocations for downstream
/// classification.
pub fn parse_bytes(data: &[u8]) -> ParseResult {
    let mut result = ParseResult::default();
    let diag = &mut result.diagnostics;
    diag.bytes_total = data.len() as u64;

    let mut user_count = 0u32;
    let mut assistant_count = 0u32;
    let mut last_user_content: Option<String> = None;
    let mut first_user_content: Option<String> = None;
    let mut tool_call_count = 0u32;
    let mut files_read_all: Vec<String> = Vec::new();
    let mut files_edited_all: Vec<String> = Vec::new();
    let mut first_timestamp: Option<i64> = None;
    let mut last_timestamp: Option<i64> = None;

    // Dedup for API-call counting and usage-token counting.
    let mut seen_api_calls_for_count: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    let mut seen_api_calls_for_usage: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    let mut unique_api_call_count = 0u32;

    // Keep SIMD finders for string-level extraction within already-classified lines
    let content_finder = memmem::Finder::new(b"\"content\":\"");
    let text_finder = memmem::Finder::new(b"\"text\":\"");
    let skill_name_finder = memmem::Finder::new(b"\"skill\":\"");

    // SIMD type detectors -- check raw bytes before JSON parse
    let type_progress = memmem::Finder::new(b"\"type\":\"progress\"");
    let type_queue_op = memmem::Finder::new(b"\"type\":\"queue-operation\"");
    let type_file_snap = memmem::Finder::new(b"\"type\":\"file-history-snapshot\"");
    // Progress subtype detectors
    let subtype_agent = memmem::Finder::new(b"\"type\":\"agent_progress\"");
    let subtype_bash = memmem::Finder::new(b"\"type\":\"bash_progress\"");
    let subtype_hook = memmem::Finder::new(b"\"type\":\"hook_progress\"");
    let subtype_mcp = memmem::Finder::new(b"\"type\":\"mcp_progress\"");

    // Queue operation detectors
    let op_enqueue = memmem::Finder::new(b"\"operation\":\"enqueue\"");
    let op_dequeue = memmem::Finder::new(b"\"operation\":\"dequeue\"");

    // Type-dispatched parsing: SIMD pre-filter before typed struct deserialization
    let type_user = memmem::Finder::new(b"\"type\":\"user\"");
    let type_assistant = memmem::Finder::new(b"\"type\":\"assistant\"");
    let type_system = memmem::Finder::new(b"\"type\":\"system\"");
    let timestamp_finder = memmem::Finder::new(b"\"timestamp\":");

    // Phase C: LOC estimation - SIMD finders for Edit/Write tool_use blocks
    let tool_use_finder = memmem::Finder::new(b"\"type\":\"tool_use\"");

    // Turn detection: tool_result user messages are continuations, not real turns
    let tool_result_finder = memmem::Finder::new(b"\"tool_result\"");
    // Interactive tool_result markers
    let tool_result_questions_finder = memmem::Finder::new(b"\"toolUseResult\":{\"questions\"");
    let tool_result_rejected_finder =
        memmem::Finder::new(b"\"toolUseResult\":\"User rejected tool use\"");

    // gitBranch extraction
    let git_branch_finder = memmem::Finder::new(b"\"gitBranch\":\"");

    // cwd extraction from user messages
    let cwd_finder = memmem::Finder::new(b"\"cwd\":\"");

    // slug extraction (top-level field on every JSONL line)
    let slug_finder = memmem::Finder::new(b"\"slug\":\"");

    // entrypoint extraction (first line that has it: cli, claude-vscode, sdk-ts)
    let entrypoint_finder = memmem::Finder::new(b"\"entrypoint\":\"");

    // Observability trace IDs injected by claude-view (preparatory -- CLI doesn't emit yet)
    let cv_trace_id_finder = memmem::Finder::new(b"\"claude_view_trace_id\":\"");
    let cv_cli_session_id_finder = memmem::Finder::new(b"\"claude_view_cli_session_id\":\"");

    for (byte_offset, line) in split_lines_with_offsets(data) {
        if line.is_empty() {
            diag.lines_empty += 1;
            continue;
        }

        diag.lines_total += 1;

        // Extract gitBranch (appears on every JSONL line, grab from the first match)
        if result.git_branch.is_none() {
            if let Some(pos) = git_branch_finder.find(line) {
                let start = pos + b"\"gitBranch\":\"".len();
                if let Some(branch) = extract_quoted_string(&line[start..]) {
                    if !branch.is_empty() {
                        result.git_branch = Some(branch);
                    }
                }
            }
        }

        // Extract cwd (appears on user, progress, system lines -- grab first match)
        if result.cwd.is_none() {
            if let Some(pos) = cwd_finder.find(line) {
                let start = pos + b"\"cwd\":\"".len();
                if let Some(end) = line[start..].iter().position(|&b| b == b'"') {
                    if let Ok(s) = std::str::from_utf8(&line[start..start + end]) {
                        if !s.is_empty() {
                            result.cwd = Some(s.to_string());
                        }
                    }
                }
            }
        }

        // Extract slug (top-level field on every JSONL line -- grab first match)
        if result.slug.is_none() {
            if let Some(pos) = slug_finder.find(line) {
                let start = pos + b"\"slug\":\"".len();
                if let Some(slug_val) = extract_quoted_string(&line[start..]) {
                    if !slug_val.is_empty() {
                        result.slug = Some(slug_val);
                    }
                }
            }
        }

        // Extract entrypoint (first line that has it -- cli, claude-vscode, sdk-ts)
        if result.entrypoint.is_none() {
            if let Some(pos) = entrypoint_finder.find(line) {
                let start = pos + b"\"entrypoint\":\"".len();
                if let Some(ep) = extract_quoted_string(&line[start..]) {
                    if !ep.is_empty() {
                        result.entrypoint = Some(ep);
                    }
                }
            }
        }

        // Observability trace IDs (preparatory -- Claude CLI doesn't emit yet).
        // Extract from the first line that contains them.
        if result.claude_view_trace_id.is_none() {
            if let Some(pos) = cv_trace_id_finder.find(line) {
                let start = pos + b"\"claude_view_trace_id\":\"".len();
                if let Some(tid) = extract_quoted_string(&line[start..]) {
                    if !tid.is_empty() {
                        tracing::Span::current().record("trace_id", &tid);
                        result.claude_view_trace_id = Some(tid);
                    }
                }
            }
        }
        if result.claude_view_cli_session_id.is_none() {
            if let Some(pos) = cv_cli_session_id_finder.find(line) {
                let start = pos + b"\"claude_view_cli_session_id\":\"".len();
                if let Some(cid) = extract_quoted_string(&line[start..]) {
                    if !cid.is_empty() {
                        tracing::Span::current().record("cli_session_id", &cid);
                        result.claude_view_cli_session_id = Some(cid);
                    }
                }
            }
        }

        // SIMD fast path: lightweight types that don't need full JSON parse
        if type_progress.find(line).is_some() {
            diag.lines_progress += 1;
            if subtype_agent.find(line).is_some() {
                result.deep.agent_spawn_count += 1;
            } else if subtype_bash.find(line).is_some() {
                result.deep.bash_progress_count += 1;
            } else if subtype_hook.find(line).is_some() {
                result.deep.hook_progress_count += 1;
                if let Some(hook_event) = parse_hook_progress_line(line) {
                    result.deep.hook_progress_events.push(hook_event);
                }
            } else if subtype_mcp.find(line).is_some() {
                result.deep.mcp_progress_count += 1;
            }
            continue;
        }

        if type_queue_op.find(line).is_some() {
            diag.lines_queue_op += 1;
            if op_enqueue.find(line).is_some() {
                result.deep.queue_enqueue_count += 1;
            } else if op_dequeue.find(line).is_some() {
                result.deep.queue_dequeue_count += 1;
            }
            continue;
        }

        if type_file_snap.find(line).is_some() {
            diag.lines_file_snapshot += 1;
            result.deep.file_snapshot_count += 1;
            continue;
        }

        // User lines: raw-byte path (no JSON parse at all)
        if type_user.find(line).is_some() {
            diag.lines_user += 1;
            user_count += 1;
            let is_tool_result = tool_result_finder.find(line).is_some();
            let has_interactive_tool_result_marker = is_tool_result
                && (tool_result_questions_finder.find(line).is_some()
                    || tool_result_rejected_finder.find(line).is_some());
            if let Some(content) = extract_first_text_content(line, &content_finder, &text_finder) {
                let is_human_tool_result = is_tool_result
                    && (has_interactive_tool_result_marker
                        || is_human_tool_result_content(&content));
                let is_real_user_input =
                    !is_system_user_content(&content) && (!is_tool_result || is_human_tool_result);

                if first_user_content.is_none() && is_real_user_input {
                    first_user_content = Some(content.clone());
                }
                last_user_content = Some(content.clone());

                if is_real_user_input {
                    let current_ts = extract_timestamp_from_bytes(line, &timestamp_finder);
                    if let (Some(start_ts), Some(end_ts)) =
                        (result.deep.current_turn_start_ts, last_timestamp)
                    {
                        let wall_secs = (end_ts - start_ts).max(0) as u32;
                        result.deep.total_task_time_seconds += wall_secs;
                        if result
                            .deep
                            .longest_task_seconds
                            .is_none_or(|prev| wall_secs > prev)
                        {
                            result.deep.longest_task_seconds = Some(wall_secs);
                            result.deep.longest_task_preview =
                                result.deep.current_turn_prompt.take();
                        }
                    }
                    result.deep.current_turn_start_ts = current_ts;
                    result.deep.current_turn_prompt = Some(content.chars().take(60).collect());
                }
            }
            let user_ts = extract_timestamp_from_bytes(line, &timestamp_finder);
            if let Some(ts) = user_ts {
                diag.timestamps_extracted += 1;
                if first_timestamp.is_none() {
                    first_timestamp = Some(ts);
                }
                last_timestamp = Some(ts);
            }
            continue;
        }

        // Assistant lines: typed struct parse (skips text/thinking content allocation)
        if type_assistant.find(line).is_some() {
            diag.json_parse_attempts += 1;
            match serde_json::from_slice::<AssistantLine>(line) {
                Ok(parsed) => {
                    handle_assistant_line(
                        parsed,
                        byte_offset,
                        &mut result.deep,
                        &mut result.raw_invocations,
                        &mut result.turns,
                        &mut result.models_seen,
                        diag,
                        &mut assistant_count,
                        &mut tool_call_count,
                        &mut files_read_all,
                        &mut files_edited_all,
                        &mut first_timestamp,
                        &mut last_timestamp,
                        &mut seen_api_calls_for_count,
                        &mut seen_api_calls_for_usage,
                        &mut unique_api_call_count,
                    );

                    if tool_use_finder.find(line).is_some() {
                        let (added, removed) = extract_loc_from_assistant_tool_uses(line);
                        result.lines_added = result.lines_added.saturating_add(added);
                        result.lines_removed = result.lines_removed.saturating_add(removed);
                    }
                }
                Err(_) => {
                    diag.json_parse_failures += 1;
                    extract_skills_from_line(
                        line,
                        &skill_name_finder,
                        &mut result.deep.skills_used,
                    );
                }
            }
            continue;
        }

        // System lines: small flat struct
        if type_system.find(line).is_some() {
            diag.json_parse_attempts += 1;
            match serde_json::from_slice::<SystemLine>(line) {
                Ok(parsed) => {
                    handle_system_line(
                        parsed,
                        &mut result.deep,
                        diag,
                        &mut first_timestamp,
                        &mut last_timestamp,
                    );
                }
                Err(_) => {
                    diag.json_parse_failures += 1;
                }
            }
            continue;
        }

        // Fallback: full Value parse for spacing variants and unknown types
        diag.json_parse_attempts += 1;

        let value: serde_json::Value = match serde_json::from_slice(line) {
            Ok(v) => v,
            Err(_) => {
                diag.json_parse_failures += 1;
                continue;
            }
        };

        if let Some(ts) = extract_timestamp_from_value(&value) {
            diag.timestamps_extracted += 1;
            if first_timestamp.is_none() {
                first_timestamp = Some(ts);
            }
            last_timestamp = Some(ts);
        }

        let line_type = value.get("type").and_then(|t| t.as_str()).unwrap_or("");

        match line_type {
            "user" => {
                diag.lines_user += 1;
                user_count += 1;
                let is_tool_result = tool_result_finder.find(line).is_some();
                let fallback_user_ts = extract_timestamp_from_value(&value);
                if let Some(content) =
                    extract_first_text_content(line, &content_finder, &text_finder)
                {
                    let has_interactive_tool_result_marker =
                        value.get("toolUseResult").is_some_and(|tr| {
                            tr.get("questions").is_some()
                                || tr.as_str() == Some("User rejected tool use")
                        });
                    let is_human_tool_result = is_tool_result
                        && (has_interactive_tool_result_marker
                            || is_human_tool_result_content(&content));
                    let is_real_user_input = !is_system_user_content(&content)
                        && (!is_tool_result || is_human_tool_result);

                    if first_user_content.is_none() && is_real_user_input {
                        first_user_content = Some(content.clone());
                    }
                    last_user_content = Some(content.clone());

                    if is_real_user_input {
                        let current_ts = fallback_user_ts;
                        if let (Some(start_ts), Some(end_ts)) =
                            (result.deep.current_turn_start_ts, last_timestamp)
                        {
                            let wall_secs = (end_ts - start_ts).max(0) as u32;
                            result.deep.total_task_time_seconds += wall_secs;
                            if result
                                .deep
                                .longest_task_seconds
                                .is_none_or(|prev| wall_secs > prev)
                            {
                                result.deep.longest_task_seconds = Some(wall_secs);
                                result.deep.longest_task_preview =
                                    result.deep.current_turn_prompt.take();
                            }
                        }
                        result.deep.current_turn_start_ts = current_ts;
                        result.deep.current_turn_prompt = Some(content.chars().take(60).collect());
                    }
                }
            }
            "assistant" => {
                diag.lines_assistant += 1;
                assistant_count += 1;
                handle_assistant_value(
                    &value,
                    byte_offset,
                    &mut result.deep,
                    &mut result.raw_invocations,
                    &mut result.turns,
                    &mut result.models_seen,
                    diag,
                    assistant_count,
                    &mut tool_call_count,
                    &mut files_read_all,
                    &mut files_edited_all,
                    &mut seen_api_calls_for_count,
                    &mut seen_api_calls_for_usage,
                    &mut unique_api_call_count,
                );

                if tool_use_finder.find(line).is_some() {
                    let (added, removed) = extract_loc_from_assistant_tool_uses(line);
                    result.lines_added = result.lines_added.saturating_add(added);
                    result.lines_removed = result.lines_removed.saturating_add(removed);
                }
            }
            "system" => {
                diag.lines_system += 1;
                let subtype = value.get("subtype").and_then(|s| s.as_str()).unwrap_or("");
                match subtype {
                    "turn_duration" => {
                        if let Some(ms) = value.get("durationMs").and_then(|v| v.as_u64()) {
                            result.deep.turn_durations_ms.push(ms);
                        }
                    }
                    "api_error" => {
                        result.deep.api_error_count += 1;
                        if let Some(retries) = value.get("retryAttempt").and_then(|v| v.as_u64()) {
                            result.deep.api_retry_count += retries as u32;
                        }
                    }
                    "compact_boundary" | "microcompact_boundary" => {
                        result.deep.compaction_count += 1;
                    }
                    "stop_hook_summary" => {
                        if value.get("preventedContinuation").and_then(|v| v.as_bool())
                            == Some(true)
                        {
                            result.deep.hook_blocked_count += 1;
                        }
                    }
                    _ => {}
                }
            }
            "progress" => {
                diag.lines_progress += 1;
                let data_type = value
                    .get("data")
                    .and_then(|d| d.get("type"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("");
                match data_type {
                    "agent_progress" => result.deep.agent_spawn_count += 1,
                    "bash_progress" => result.deep.bash_progress_count += 1,
                    "hook_progress" => {
                        result.deep.hook_progress_count += 1;
                        if let Some(hook_event) = parse_hook_progress_line(line) {
                            result.deep.hook_progress_events.push(hook_event);
                        }
                    }
                    "mcp_progress" => result.deep.mcp_progress_count += 1,
                    _ => {}
                }
            }
            "queue-operation" => {
                diag.lines_queue_op += 1;
                match value.get("operation").and_then(|o| o.as_str()) {
                    Some("enqueue") => result.deep.queue_enqueue_count += 1,
                    Some("dequeue") => result.deep.queue_dequeue_count += 1,
                    _ => {}
                }
            }
            "file-history-snapshot" => {
                diag.lines_file_snapshot += 1;
                result.deep.file_snapshot_count += 1;
            }
            _ => {
                diag.lines_unknown_type += 1;
            }
        }
    }

    // Close the final turn at EOF using the last known timestamp
    if let (Some(start_ts), Some(end_ts)) = (result.deep.current_turn_start_ts, last_timestamp) {
        let wall_secs = (end_ts - start_ts).max(0) as u32;
        result.deep.total_task_time_seconds += wall_secs;
        if result
            .deep
            .longest_task_seconds
            .is_none_or(|prev| wall_secs > prev)
        {
            result.deep.longest_task_seconds = Some(wall_secs);
            result.deep.longest_task_preview = result.deep.current_turn_prompt.take();
        }
    }

    // Finalize metrics
    result.deep.turn_count = (user_count as usize).min(assistant_count as usize);
    result.deep.last_message = last_user_content
        .map(|c| truncate(&c, 200))
        .unwrap_or_default();
    result.deep.first_user_prompt = first_user_content.map(|c| truncate(&c, 500));
    result.deep.user_prompt_count = user_count;
    result.deep.api_call_count = unique_api_call_count;
    result.deep.tool_call_count = tool_call_count;

    // files_read: deduplicated
    let mut files_read_unique = files_read_all.clone();
    files_read_unique.sort();
    files_read_unique.dedup();
    result.deep.files_read_count = files_read_unique.len() as u32;
    result.deep.files_read = files_read_unique;

    // files_edited: store all occurrences
    result.deep.files_edited = files_edited_all.clone();
    let mut files_edited_unique = files_edited_all.clone();
    files_edited_unique.sort();
    files_edited_unique.dedup();
    result.deep.files_edited_count = files_edited_unique.len() as u32;
    result.deep.reedited_files_count = count_reedited_files(&files_edited_all);

    // AI contribution tracking
    let ai_line_count = count_ai_lines(
        result
            .raw_invocations
            .iter()
            .map(|inv| (inv.name.as_str(), &inv.input))
            .filter_map(|(name, input)| input.as_ref().map(|i| (name, i))),
    );
    result.deep.ai_lines_added = ai_line_count.lines_added;
    result.deep.ai_lines_removed = ai_line_count.lines_removed;

    // Duration
    result.deep.first_timestamp = first_timestamp;
    result.deep.last_timestamp = last_timestamp;
    result.deep.duration_seconds = match (first_timestamp, last_timestamp) {
        (Some(first), Some(last)) if last >= first => (last - first) as u32,
        _ => 0,
    };

    // Deduplicate
    result.deep.skills_used.sort();
    result.deep.skills_used.dedup();
    result.deep.files_touched.sort();
    result.deep.files_touched.dedup();
    result.models_seen.sort();
    result.models_seen.dedup();

    result
}
