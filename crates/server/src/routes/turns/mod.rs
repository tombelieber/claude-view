//! Per-turn breakdown endpoint for historical sessions.
//!
//! `GET /api/sessions/{id}/turns` re-parses the JSONL file on demand to extract
//! per-turn data (wall-clock duration, CC duration, prompt preview). This avoids
//! storing per-turn data in the DB for rarely-accessed detail views.

mod handler;
pub mod scanner;
#[cfg(test)]
mod tests;
pub mod types;

// Re-export public API
pub use handler::{get_session_turns, router};
pub use types::TurnInfo;

// Re-export utoipa-generated hidden path type for OpenAPI registration
pub use handler::__path_get_session_turns;
