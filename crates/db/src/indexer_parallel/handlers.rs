// crates/db/src/indexer_parallel/handlers.rs
// Handler functions for dispatched JSONL line parsing: assistant (typed + Value) and system lines.

use super::cost::normalize_model_id;
use super::helpers::extract_tool_input_file_path;
use super::serde_types::*;
use super::types::*;

pub(crate) fn usage_block_has_token_payload(usage: &UsageBlock) -> bool {
    usage.input_tokens.is_some()
        || usage.output_tokens.is_some()
        || usage.cache_read_input_tokens.is_some()
        || usage.cache_creation_input_tokens.is_some()
        || usage
            .cache_creation
            .as_ref()
            .and_then(|c| c.ephemeral_5m_input_tokens)
            .is_some()
        || usage
            .cache_creation
            .as_ref()
            .and_then(|c| c.ephemeral_1h_input_tokens)
            .is_some()
}

pub(crate) fn value_usage_has_token_payload(usage: Option<&serde_json::Value>) -> bool {
    usage
        .map(|u| {
            u.get("input_tokens").and_then(|v| v.as_u64()).is_some()
                || u.get("output_tokens").and_then(|v| v.as_u64()).is_some()
                || u.get("cache_read_input_tokens")
                    .and_then(|v| v.as_u64())
                    .is_some()
                || u.get("cache_creation_input_tokens")
                    .and_then(|v| v.as_u64())
                    .is_some()
                || u.get("cache_creation")
                    .and_then(|c| c.get("ephemeral_5m_input_tokens"))
                    .and_then(|v| v.as_u64())
                    .is_some()
                || u.get("cache_creation")
                    .and_then(|c| c.get("ephemeral_1h_input_tokens"))
                    .and_then(|v| v.as_u64())
                    .is_some()
        })
        .unwrap_or(false)
}

pub(crate) fn should_count_api_call(
    msg_id: Option<&str>,
    req_id: Option<&str>,
    seen_api_calls: &mut std::collections::HashSet<String>,
) -> bool {
    match (msg_id, req_id) {
        (Some(mid), Some(rid)) => {
            let key = format!("{}:{}", mid, rid);
            seen_api_calls.insert(key)
        }
        _ => true, // no IDs available -- count each legacy line
    }
}

pub(crate) fn should_count_usage_block(
    msg_id: Option<&str>,
    req_id: Option<&str>,
    has_usage_payload: bool,
    seen_api_calls: &mut std::collections::HashSet<String>,
) -> bool {
    if !has_usage_payload {
        return false;
    }
    match (msg_id, req_id) {
        (Some(mid), Some(rid)) => {
            let key = format!("{}:{}", mid, rid);
            seen_api_calls.insert(key)
        }
        _ => true, // no IDs available -- count it (legacy data)
    }
}

/// Handle an assistant line parsed via typed `AssistantLine` struct.
/// Produces identical output to the Value-based path.
///
/// Takes individual mutable refs to avoid borrowing `ParseResult` while `diag`
/// (which borrows `result.diagnostics`) is also live.
#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_assistant_line(
    parsed: AssistantLine,
    byte_offset: usize,
    deep: &mut ExtendedMetadata,
    raw_invocations: &mut Vec<RawInvocation>,
    turns: &mut Vec<claude_view_core::RawTurn>,
    models_seen: &mut Vec<String>,
    diag: &mut ParseDiagnostics,
    assistant_count: &mut u32,
    tool_call_count: &mut u32,
    files_read_all: &mut Vec<String>,
    files_edited_all: &mut Vec<String>,
    first_timestamp: &mut Option<i64>,
    last_timestamp: &mut Option<i64>,
    seen_api_calls_for_count: &mut std::collections::HashSet<String>,
    seen_api_calls_for_usage: &mut std::collections::HashSet<String>,
    unique_api_call_count: &mut u32,
) {
    diag.lines_assistant += 1;
    *assistant_count += 1;

    // Extract timestamp
    let ts = parsed.timestamp.as_ref().and_then(|t| t.to_unix());
    if let Some(ts) = ts {
        diag.timestamps_extracted += 1;
        if first_timestamp.is_none() {
            *first_timestamp = Some(ts);
        }
        *last_timestamp = Some(ts);
    }

    if let Some(message) = parsed.message {
        // Dedup: count usage only once per API response.
        // If the first content block has no usage payload, wait for a later block
        // with usage instead of consuming the dedup key and dropping telemetry.
        let has_usage_payload = message
            .usage
            .as_ref()
            .is_some_and(usage_block_has_token_payload);
        let count_api_call = should_count_api_call(
            message.id.as_deref(),
            parsed.request_id.as_deref(),
            seen_api_calls_for_count,
        );
        let count_usage_block = should_count_usage_block(
            message.id.as_deref(),
            parsed.request_id.as_deref(),
            has_usage_payload,
            seen_api_calls_for_usage,
        );

        if count_api_call {
            *unique_api_call_count += 1;
        }

        // Extract token usage -- only for first content block per API response
        if count_usage_block {
            if let Some(ref usage) = message.usage {
                if let Some(v) = usage.input_tokens {
                    deep.total_input_tokens += v;
                }
                if let Some(v) = usage.output_tokens {
                    deep.total_output_tokens += v;
                }
                if let Some(v) = usage.cache_read_input_tokens {
                    deep.cache_read_tokens += v;
                }
                if let Some(v) = usage.cache_creation_input_tokens {
                    deep.cache_creation_tokens += v;
                }
            }
        }

        // Extract turn data (model + tokens) for turns table
        if let Some(ref model) = message.model {
            // M4: Normalize model ID to canonical family name
            let model_id = normalize_model_id(model);
            if !models_seen.contains(&model_id) {
                models_seen.push(model_id.clone());
            }

            let uuid = parsed.uuid.unwrap_or_default();
            let parent_uuid = parsed.parent_uuid;

            // Determine content_type from first block
            let content_type = match &message.content {
                ContentResult::Blocks(blocks) => blocks
                    .first()
                    .and_then(|b| b.block_type.as_deref())
                    .unwrap_or("text")
                    .to_string(),
                _ => "text".to_string(),
            };

            // Duplicate/no-usage blocks keep token fields as NULL (unknown/not-counted),
            // not synthetic zero.
            let (inp, outp, cr, cc, cc_5m, cc_1hr) = if count_usage_block {
                (
                    message.usage.as_ref().and_then(|u| u.input_tokens),
                    message.usage.as_ref().and_then(|u| u.output_tokens),
                    message
                        .usage
                        .as_ref()
                        .and_then(|u| u.cache_read_input_tokens),
                    message
                        .usage
                        .as_ref()
                        .and_then(|u| u.cache_creation_input_tokens),
                    message
                        .usage
                        .as_ref()
                        .and_then(|u| u.cache_creation.as_ref())
                        .and_then(|c| c.ephemeral_5m_input_tokens),
                    message
                        .usage
                        .as_ref()
                        .and_then(|u| u.cache_creation.as_ref())
                        .and_then(|c| c.ephemeral_1h_input_tokens),
                )
            } else {
                (None, None, None, None, None, None)
            };

            turns.push(claude_view_core::RawTurn {
                uuid,
                parent_uuid,
                seq: *assistant_count - 1,
                model_id,
                content_type,
                input_tokens: inp,
                output_tokens: outp,
                cache_read_tokens: cr,
                cache_creation_tokens: cc,
                cache_creation_5m_tokens: cc_5m,
                cache_creation_1hr_tokens: cc_1hr,
                service_tier: message.usage.as_ref().and_then(|u| u.service_tier.clone()),
                timestamp: ts,
            });
            diag.turns_extracted += 1;
        }

        // Extract tool_use blocks from content
        match message.content {
            ContentResult::Blocks(blocks) => {
                for block in blocks {
                    let block_type = block.block_type.as_deref().unwrap_or("");
                    match block_type {
                        "tool_use" => {
                            diag.tool_use_blocks_found += 1;
                            let name = match block.name {
                                Some(ref n) => n.as_str(),
                                None => {
                                    diag.tool_use_missing_name += 1;
                                    continue;
                                }
                            };

                            *tool_call_count += 1;

                            match name {
                                "Read" => deep.tool_counts.read += 1,
                                "Edit" => deep.tool_counts.edit += 1,
                                "Write" => deep.tool_counts.write += 1,
                                "Bash" => deep.tool_counts.bash += 1,
                                "Skill" => {
                                    if let Some(skill_name) = block
                                        .input
                                        .as_ref()
                                        .and_then(|i| i.get("skill"))
                                        .and_then(|v| v.as_str())
                                    {
                                        if !skill_name.is_empty() {
                                            deep.skills_used.push(skill_name.to_string());
                                        }
                                    }
                                }
                                _ => {}
                            }

                            // Extract file paths
                            if let Some(fp) =
                                block.input.as_ref().and_then(extract_tool_input_file_path)
                            {
                                if !fp.is_empty() {
                                    diag.file_paths_extracted += 1;
                                    deep.files_touched.push(fp.to_string());
                                    match name {
                                        "Read" => files_read_all.push(fp.to_string()),
                                        "Edit" | "Write" => files_edited_all.push(fp.to_string()),
                                        _ => {}
                                    }
                                }
                            }

                            // Store raw invocation (move input, no clone needed)
                            raw_invocations.push(RawInvocation {
                                name: name.to_string(),
                                input: block.input,
                                byte_offset,
                                timestamp: ts.unwrap_or(0),
                            });
                        }
                        "thinking" => {
                            deep.thinking_block_count += 1;
                        }
                        _ => {}
                    }
                }
            }
            ContentResult::NotArray => {
                diag.content_not_array += 1;
            }
            ContentResult::Missing => {}
        }
    }
}

/// Handle an assistant line via the fallback Value path (for spacing variants).
#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_assistant_value(
    value: &serde_json::Value,
    byte_offset: usize,
    deep: &mut ExtendedMetadata,
    raw_invocations: &mut Vec<RawInvocation>,
    turns: &mut Vec<claude_view_core::RawTurn>,
    models_seen: &mut Vec<String>,
    diag: &mut ParseDiagnostics,
    assistant_count: u32,
    tool_call_count: &mut u32,
    files_read_all: &mut Vec<String>,
    files_edited_all: &mut Vec<String>,
    seen_api_calls_for_count: &mut std::collections::HashSet<String>,
    seen_api_calls_for_usage: &mut std::collections::HashSet<String>,
    unique_api_call_count: &mut u32,
) {
    // Dedup check -- same logic as handle_assistant_line
    let msg_id = value
        .get("message")
        .and_then(|m| m.get("id"))
        .and_then(|v| v.as_str());
    let req_id = value.get("requestId").and_then(|v| v.as_str());
    let usage_value = value.get("message").and_then(|m| m.get("usage"));
    let has_usage_payload = value_usage_has_token_payload(usage_value);
    let count_api_call = should_count_api_call(msg_id, req_id, seen_api_calls_for_count);
    let count_usage_block =
        should_count_usage_block(msg_id, req_id, has_usage_payload, seen_api_calls_for_usage);

    if count_api_call {
        *unique_api_call_count += 1;
    }

    // Extract token usage from .message.usage -- only for first block
    if count_usage_block {
        if let Some(usage) = usage_value {
            if let Some(v) = usage.get("input_tokens").and_then(|v| v.as_u64()) {
                deep.total_input_tokens += v;
            }
            if let Some(v) = usage.get("output_tokens").and_then(|v| v.as_u64()) {
                deep.total_output_tokens += v;
            }
            if let Some(v) = usage
                .get("cache_read_input_tokens")
                .and_then(|v| v.as_u64())
            {
                deep.cache_read_tokens += v;
            }
            if let Some(v) = usage
                .get("cache_creation_input_tokens")
                .and_then(|v| v.as_u64())
            {
                deep.cache_creation_tokens += v;
            }
        }
    }

    // Extract turn data (model + tokens) for turns table
    if let Some(message) = value.get("message") {
        if let Some(model) = message.get("model").and_then(|m| m.as_str()) {
            // M4: Normalize model ID to canonical family name
            let model_id = normalize_model_id(model);
            if !models_seen.contains(&model_id) {
                models_seen.push(model_id.clone());
            }

            let usage = message.get("usage");
            let uuid = value
                .get("uuid")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let parent_uuid = value
                .get("parentUuid")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let content_type = message
                .get("content")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|block| block.get("type"))
                .and_then(|t| t.as_str())
                .unwrap_or("text")
                .to_string();
            let timestamp = super::helpers::extract_timestamp_from_value(value);

            // Duplicate/no-usage blocks keep token fields as NULL (unknown/not-counted),
            // not synthetic zero.
            let (inp, outp, cr, cc, cc_5m, cc_1hr) = if count_usage_block {
                (
                    usage
                        .and_then(|u| u.get("input_tokens"))
                        .and_then(|v| v.as_u64()),
                    usage
                        .and_then(|u| u.get("output_tokens"))
                        .and_then(|v| v.as_u64()),
                    usage
                        .and_then(|u| u.get("cache_read_input_tokens"))
                        .and_then(|v| v.as_u64()),
                    usage
                        .and_then(|u| u.get("cache_creation_input_tokens"))
                        .and_then(|v| v.as_u64()),
                    usage
                        .and_then(|u| u.get("cache_creation"))
                        .and_then(|c| c.get("ephemeral_5m_input_tokens"))
                        .and_then(|v| v.as_u64()),
                    usage
                        .and_then(|u| u.get("cache_creation"))
                        .and_then(|c| c.get("ephemeral_1h_input_tokens"))
                        .and_then(|v| v.as_u64()),
                )
            } else {
                (None, None, None, None, None, None)
            };

            turns.push(claude_view_core::RawTurn {
                uuid,
                parent_uuid,
                seq: assistant_count - 1,
                model_id,
                content_type,
                input_tokens: inp,
                output_tokens: outp,
                cache_read_tokens: cr,
                cache_creation_tokens: cc,
                cache_creation_5m_tokens: cc_5m,
                cache_creation_1hr_tokens: cc_1hr,
                service_tier: usage
                    .and_then(|u| u.get("service_tier"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                timestamp,
            });
            diag.turns_extracted += 1;
        }
    }

    // Extract tool_use blocks from content
    let content = value.get("message").and_then(|m| m.get("content"));
    match content {
        Some(serde_json::Value::Array(arr)) => {
            for block in arr {
                let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                match block_type {
                    "tool_use" => {
                        diag.tool_use_blocks_found += 1;
                        let name = match block.get("name").and_then(|n| n.as_str()) {
                            Some(n) => n,
                            None => {
                                diag.tool_use_missing_name += 1;
                                continue;
                            }
                        };

                        *tool_call_count += 1;

                        let input = block.get("input");

                        match name {
                            "Read" => deep.tool_counts.read += 1,
                            "Edit" => deep.tool_counts.edit += 1,
                            "Write" => deep.tool_counts.write += 1,
                            "Bash" => deep.tool_counts.bash += 1,
                            "Skill" => {
                                if let Some(skill_name) =
                                    input.and_then(|i| i.get("skill")).and_then(|v| v.as_str())
                                {
                                    if !skill_name.is_empty() {
                                        deep.skills_used.push(skill_name.to_string());
                                    }
                                }
                            }
                            _ => {}
                        }

                        if let Some(fp) = input.and_then(extract_tool_input_file_path) {
                            if !fp.is_empty() {
                                diag.file_paths_extracted += 1;
                                deep.files_touched.push(fp.to_string());
                                match name {
                                    "Read" => files_read_all.push(fp.to_string()),
                                    "Edit" | "Write" => files_edited_all.push(fp.to_string()),
                                    _ => {}
                                }
                            }
                        }

                        let ts = value.get("timestamp").and_then(|v| v.as_i64()).unwrap_or(0);
                        raw_invocations.push(RawInvocation {
                            name: name.to_string(),
                            input: input.cloned(),
                            byte_offset,
                            timestamp: ts,
                        });
                    }
                    "thinking" => {
                        deep.thinking_block_count += 1;
                    }
                    _ => {}
                }
            }
        }
        Some(serde_json::Value::String(_)) => {
            diag.content_not_array += 1;
        }
        _ => {}
    }
}

/// Handle a system line parsed via typed `SystemLine` struct.
pub(crate) fn handle_system_line(
    parsed: SystemLine,
    deep: &mut ExtendedMetadata,
    diag: &mut ParseDiagnostics,
    first_timestamp: &mut Option<i64>,
    last_timestamp: &mut Option<i64>,
) {
    diag.lines_system += 1;

    if let Some(ts) = parsed.timestamp.as_ref().and_then(|t| t.to_unix()) {
        diag.timestamps_extracted += 1;
        if first_timestamp.is_none() {
            *first_timestamp = Some(ts);
        }
        *last_timestamp = Some(ts);
    }

    let subtype = parsed.subtype.as_deref().unwrap_or("");
    match subtype {
        "turn_duration" => {
            if let Some(ms) = parsed.duration_ms {
                deep.turn_durations_ms.push(ms);
            }
        }
        "api_error" => {
            deep.api_error_count += 1;
            if let Some(retries) = parsed.retry_attempt {
                deep.api_retry_count += retries as u32;
            }
        }
        "compact_boundary" | "microcompact_boundary" => {
            deep.compaction_count += 1;
        }
        "stop_hook_summary" => {
            if parsed.prevented_continuation == Some(true) {
                deep.hook_blocked_count += 1;
            }
        }
        _ => {}
    }
}
