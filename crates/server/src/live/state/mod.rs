//! Live session state types and status derivation for Live Monitor.
//!
//! Provides real-time session status tracking by analyzing the last JSONL line,
//! file modification time, and process presence.
//!
//! Decomposed into domain-specific submodules:
//! - `agent` — AgentState, AgentStateGroup, status_from_agent_state
//! - `classify` — LiveSessionAction, classify_live_session
//! - `core` — LiveSession, SessionStatus, ControlBinding, snapshots
//! - `event` — SessionEvent, HookEvent, append helpers
//! - `field_types` — ToolUsed, VerifiedFile, FileSourceKind
//! - `hook_fields` — HookFields
//! - `jsonl_fields` — JsonlFields
//! - `statusline_fields` — StatuslineFields, StatuslineDebugEntry

pub mod agent;
pub mod classify;
pub mod core;
pub mod event;
pub mod field_types;
pub mod hook_fields;
pub mod jsonl_fields;
pub mod statusline_fields;

// ---- Re-exports for backward compatibility ----
// All public items re-exported at the same paths as the original flat module.

// agent
pub use agent::{status_from_agent_state, AgentState, AgentStateGroup};

// classify
pub use classify::{classify_live_session, LiveSessionAction};

// core
#[cfg(test)]
pub(crate) use core::test_live_session;
pub use core::{ControlBinding, LiveSession, SessionSnapshot, SessionStatus, SnapshotEntry};

// event
pub(crate) use event::{append_capped_hook_event, MAX_HOOK_EVENTS_PER_SESSION};
pub use event::{HookEvent, SessionEvent};

// field_types
pub use field_types::{FileSourceKind, ToolUsed, VerifiedFile};

// hook_fields
pub use hook_fields::HookFields;

// jsonl_fields
pub use jsonl_fields::JsonlFields;

// statusline_fields
pub use statusline_fields::{StatuslineDebugEntry, StatuslineFields, MAX_STATUSLINE_DEBUG_ENTRIES};
