// crates/core/src/parser/helpers.rs
//! Shared helper functions for attaching common fields and raw JSON to messages.

use crate::types::Message;

/// Attach common fields (timestamp, uuid, parent_uuid) to a message.
///
/// Takes references so the same Option values can be used across multiple
/// branches within the user content match arms.
pub(super) fn attach_common_fields(
    mut message: Message,
    timestamp: &Option<String>,
    uuid: &Option<String>,
    parent_uuid: &Option<String>,
) -> Message {
    if let Some(ts) = timestamp {
        message = message.with_timestamp(ts.clone());
    }
    if let Some(u) = uuid {
        message = message.with_uuid(u.clone());
    }
    if let Some(pu) = parent_uuid {
        message = message.with_parent_uuid(pu.clone());
    }
    message
}

/// Conditionally attach the full raw JSON value to a message.
///
/// When `include_raw` is true, clones the JSONL `Value` into `message.raw_json`.
/// On the default path (`include_raw = false`) this is a no-op — zero allocation cost.
pub(super) fn maybe_attach_raw(
    message: Message,
    value: &serde_json::Value,
    include_raw: bool,
) -> Message {
    if include_raw {
        message.with_raw_json(value.clone())
    } else {
        message
    }
}
