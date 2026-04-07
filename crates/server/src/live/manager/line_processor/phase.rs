//! Phase classification: deterministic shipping rule, activity signal
//! accumulation, conversation turn buffering, and dirty-signal dispatch.

use crate::local_llm::client::{ConversationTurn, Role};
use claude_view_core::phase::scheduler::Priority;
use claude_view_core::phase::{is_shipping_cmd, PhaseLabel, SessionPhase, MAX_PHASE_LABELS};

use super::super::accumulator::SessionAccumulator;

pub(super) fn process_phase_classification(
    line: &claude_view_core::live_parser::LiveLine,
    acc: &mut SessionAccumulator,
    session_id: &str,
    dirty_tx: &tokio::sync::mpsc::Sender<super::super::super::drain_loop::DirtySignal>,
) {
    // Deterministic shipping rule (pre-LLM shortcut)
    for cmd in &line.bash_commands {
        if is_shipping_cmd(cmd) {
            acc.stabilizer.lock_shipping();
            acc.phase_labels.push(PhaseLabel {
                phase: SessionPhase::Shipping,
                confidence: 1.0,
                scope: None,
            });
            if acc.phase_labels.len() > MAX_PHASE_LABELS {
                acc.phase_labels.remove(0);
            }
            break;
        }
    }

    // Accumulate activity signals for classify context
    for cmd in &line.bash_commands {
        if acc.recent_bash_commands.len() >= 5 {
            acc.recent_bash_commands.pop_front();
        }
        acc.recent_bash_commands.push_back(cmd.clone());
    }
    for file in &line.edited_files {
        if acc.recent_edited_files.len() >= 5 {
            acc.recent_edited_files.pop_front();
        }
        acc.recent_edited_files.push_back(file.clone());
    }

    // Accumulate conversation turn (user/assistant only)
    if line.role.as_deref() == Some("assistant") || line.role.as_deref() == Some("user") {
        let role = if line.role.as_deref() == Some("user") {
            Role::User
        } else {
            Role::Assistant
        };
        let turn = ConversationTurn {
            role,
            text: line.content_extended.clone(),
            tools: line.tool_names.clone(),
        };
        if acc.message_buf.len() >= 15 {
            acc.message_buf.pop_front();
        }
        acc.message_buf.push_back(turn);
    }

    // Mark session dirty on ANY line -- drain loop handles debounce/scheduling.
    // Context is built from accumulator at drain time (freshest data).
    let priority = if acc.phase_labels.is_empty() {
        Priority::New
    } else if acc.stabilizer.displayed_phase().is_none() {
        Priority::Transition
    } else {
        Priority::Steady
    };
    let signal = if line.role.as_deref() == Some("user") {
        super::super::super::drain_loop::DirtySignal::UserTurn(session_id.to_string(), priority)
    } else {
        super::super::super::drain_loop::DirtySignal::Activity(session_id.to_string(), priority)
    };
    let _ = dirty_tx.try_send(signal);
}
