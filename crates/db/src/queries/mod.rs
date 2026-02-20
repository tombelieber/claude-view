// crates/db/src/queries/mod.rs
// Session CRUD operations for the claude-view SQLite database.

pub(crate) mod row_types;
mod classification;
mod dashboard;
pub mod hook_events;
mod invocables;
mod models;
mod sessions;
mod system;
mod ai_generation;
pub mod facets;
mod fluency;
pub mod reports;
mod types;

pub use dashboard::ActivityPoint;
pub use dashboard::SessionFilterParams;
pub use types::*;

// Re-export _tx functions for indexer_parallel.rs (crate::queries::*_tx paths)
pub use row_types::{
    batch_insert_invocations_tx, batch_insert_turns_tx, batch_upsert_models_tx,
    update_session_deep_fields_tx,
};
