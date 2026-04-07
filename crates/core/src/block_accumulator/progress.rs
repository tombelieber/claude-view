// crates/core/src/block_accumulator/progress.rs
//
// Progress entry handler and variant builder for BlockAccumulator.
// Maps JSONL `progress` entries to typed ProgressBlock variants.

use serde_json::Value;

use super::BlockAccumulator;
use crate::block_types::*;
use crate::category::ActionCategory;

/// Helper: extract a string field with fallback to "".
pub(super) fn str_field<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(|s| s.as_str()).unwrap_or("")
}

impl BlockAccumulator {
    pub(super) fn handle_progress(&mut self, entry: &Value) {
        let data = entry.get("data");
        let data_type = data
            .and_then(|d| d.get("type"))
            .and_then(|t| t.as_str())
            .unwrap_or("");
        let ts = self.extract_timestamp(entry).unwrap_or(0.0);
        let parent_tool_use_id = entry
            .get("parentToolUseID")
            .and_then(|p| p.as_str())
            .map(String::from);

        let Some((variant, category, progress_data)) = build_progress_data(data_type, data, ts)
        else {
            return; // Unknown progress type -- skip
        };

        self.blocks.push(ConversationBlock::Progress(ProgressBlock {
            id: self.make_id("prog"),
            variant,
            category,
            data: progress_data,
            ts,
            parent_tool_use_id,
        }));
    }
}

/// Build typed progress data from a JSONL progress entry's `data` field.
fn build_progress_data(
    data_type: &str,
    data: Option<&Value>,
    _ts: f64,
) -> Option<(ProgressVariant, ActionCategory, ProgressData)> {
    let d = data?;
    match data_type {
        "bash_progress" => Some((
            ProgressVariant::Bash,
            ActionCategory::Builtin,
            ProgressData::Bash(BashProgress {
                output: str_field(d, "output").into(),
                full_output: str_field(d, "fullOutput").into(),
                elapsed_time_seconds: d
                    .get("elapsedTimeSeconds")
                    .and_then(|e| e.as_f64())
                    .unwrap_or(0.0),
                total_lines: d.get("totalLines").and_then(|t| t.as_u64()).unwrap_or(0) as u32,
                total_bytes: d.get("totalBytes").and_then(|t| t.as_u64()).unwrap_or(0),
                task_id: d.get("taskId").and_then(|t| t.as_str()).map(String::from),
            }),
        )),
        "agent_progress" => Some((
            ProgressVariant::Agent,
            ActionCategory::Agent,
            ProgressData::Agent(AgentProgress {
                prompt: str_field(d, "prompt").into(),
                agent_id: str_field(d, "agentId").into(),
                message: d.get("message").cloned(),
            }),
        )),
        "mcp_progress" => Some((
            ProgressVariant::Mcp,
            ActionCategory::Mcp,
            ProgressData::Mcp(McpProgress {
                status: str_field(d, "status").into(),
                server_name: str_field(d, "serverName").into(),
                tool_name: str_field(d, "toolName").into(),
            }),
        )),
        "hook_progress" => Some((
            ProgressVariant::Hook,
            ActionCategory::Hook,
            ProgressData::Hook(HookProgress {
                hook_event: str_field(d, "hookEvent").into(),
                hook_name: str_field(d, "hookName").into(),
                command: str_field(d, "command").into(),
                status_message: str_field(d, "statusMessage").into(),
            }),
        )),
        "waiting_for_task" => Some((
            ProgressVariant::TaskQueue,
            ActionCategory::Agent,
            ProgressData::TaskQueue(TaskQueueProgress {
                task_description: str_field(d, "taskDescription").into(),
                task_type: str_field(d, "taskType").into(),
            }),
        )),
        "search_results_received" => Some((
            ProgressVariant::Search,
            ActionCategory::Builtin,
            ProgressData::Search(SearchProgress {
                result_count: d.get("resultCount").and_then(|r| r.as_u64()).unwrap_or(0) as u32,
                query: str_field(d, "query").into(),
            }),
        )),
        "query_update" => Some((
            ProgressVariant::Query,
            ActionCategory::Builtin,
            ProgressData::Query(QueryProgress {
                query: str_field(d, "query").into(),
            }),
        )),
        _ => None,
    }
}
