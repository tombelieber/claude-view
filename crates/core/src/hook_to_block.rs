//! Convert hook event fields into a ConversationBlock::Progress(Hook).

use crate::block_types::{
    ConversationBlock, HookProgress, ProgressBlock, ProgressData, ProgressVariant,
};
use crate::category::ActionCategory;

/// Build a `ConversationBlock::Progress` with `HookProgress` data from raw fields.
///
/// Callers provide their own `id` (e.g. `hook-{timestamp}-{index}`) and `ts`.
/// `tool_name` falls back to `event_name` for the `hookName` display field.
/// `command` is always empty — Channel B does not carry command text.
pub fn make_hook_progress_block(
    id: String,
    ts: f64,
    event_name: &str,
    tool_name: Option<&str>,
    label: &str,
) -> ConversationBlock {
    ConversationBlock::Progress(ProgressBlock {
        id,
        variant: ProgressVariant::Hook,
        category: ActionCategory::Hook,
        data: ProgressData::Hook(HookProgress {
            hook_event: event_name.to_string(),
            hook_name: tool_name.unwrap_or(event_name).to_string(),
            command: String::new(),
            status_message: label.to_string(),
        }),
        ts,
        parent_tool_use_id: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_hook_progress_block_with_tool_name() {
        let block = make_hook_progress_block(
            "hook-123-0".into(),
            1234567890.0,
            "PreToolUse",
            Some("Bash"),
            "Running: git status",
        );
        assert!(matches!(block, ConversationBlock::Progress(_)));
        let ConversationBlock::Progress(pb) = &block else {
            panic!("expected Progress");
        };
        assert_eq!(pb.id, "hook-123-0");
        assert_eq!(pb.variant, ProgressVariant::Hook);
        assert_eq!(pb.ts, 1234567890.0);
        let ProgressData::Hook(hp) = &pb.data else {
            panic!("expected Hook");
        };
        assert_eq!(hp.hook_event, "PreToolUse");
        assert_eq!(hp.hook_name, "Bash");
        assert_eq!(hp.command, "");
        assert_eq!(hp.status_message, "Running: git status");
    }

    #[test]
    fn falls_back_to_event_name_when_no_tool_name() {
        let block =
            make_hook_progress_block("hook-456-0".into(), 100.0, "Stop", None, "Session ending");
        let ConversationBlock::Progress(pb) = &block else {
            panic!("expected Progress");
        };
        let ProgressData::Hook(hp) = &pb.data else {
            panic!("expected Hook");
        };
        assert_eq!(hp.hook_name, "Stop");
    }
}
