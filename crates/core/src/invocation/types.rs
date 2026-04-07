// crates/core/src/invocation/types.rs
//
// Public types for tool_use classification results.

use crate::registry::InvocableKind;

/// Result of classifying a tool_use call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClassifyResult {
    /// Successfully matched to a known invocable.
    Valid {
        invocable_id: String,
        kind: InvocableKind,
    },
    /// Recognized tool pattern but failed validation.
    Rejected { raw_value: String, reason: String },
    /// Unknown tool, silently discard.
    Ignored,
}

/// Raw tool_use data extracted from a JSONL line, for downstream processing.
#[derive(Debug, Clone)]
pub struct RawToolUse {
    pub name: String,
    pub input: Option<serde_json::Value>,
}
