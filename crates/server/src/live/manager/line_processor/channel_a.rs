//! Channel A event emission from JSONL signals.

use claude_view_core::live_parser::LineType;

use crate::live::state::HookEvent;

use super::super::helpers::{make_synthesized_event, resolve_hook_event_from_progress};

pub(super) fn emit_channel_a_events(
    line: &claude_view_core::live_parser::LiveLine,
    channel_a_events: &mut Vec<HookEvent>,
) {
    // Channel A: hook_progress events from JSONL
    if let Some(ref hp) = line.hook_progress {
        channel_a_events.push(resolve_hook_event_from_progress(hp, &line.timestamp));
    }

    // Synthesized events from existing JSONL signals
    if line.line_type == LineType::User
        && !line.is_meta
        && !line.is_tool_result_continuation
        && !line.has_system_prefix
    {
        channel_a_events.push(make_synthesized_event(
            &line.timestamp,
            "UserPromptSubmit",
            None,
            "autonomous",
        ));
    }
    if line.is_compact_boundary {
        channel_a_events.push(make_synthesized_event(
            &line.timestamp,
            "PreCompact",
            None,
            "autonomous",
        ));
    }
    for spawn in &line.sub_agent_spawns {
        channel_a_events.push(make_synthesized_event(
            &line.timestamp,
            "SubagentStart",
            Some(&spawn.agent_type),
            "autonomous",
        ));
    }
    if line.sub_agent_result.is_some() {
        channel_a_events.push(make_synthesized_event(
            &line.timestamp,
            "SubagentStop",
            None,
            "autonomous",
        ));
    }
    for tu in &line.task_updates {
        if tu.status.as_deref() == Some("completed") {
            channel_a_events.push(make_synthesized_event(
                &line.timestamp,
                "TaskCompleted",
                None,
                "autonomous",
            ));
        }
    }
}
