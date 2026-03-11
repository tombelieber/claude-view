// crates/db/src/queries/mod.rs
// Session CRUD operations for the claude-view SQLite database.

mod ai_generation;
mod classification;
mod dashboard;
pub mod facets;
mod fluency;
pub mod hook_events;
mod invocables;
mod models;
pub mod reports;
pub(crate) mod row_types;
pub mod search_prefilter;
pub mod sessions;
pub mod settings;
mod system;
mod types;

pub use dashboard::ActivityPoint;
pub use dashboard::SessionFilterParams;
pub use search_prefilter::SearchPrefilter;
pub use types::*;

// Re-export _tx functions for indexer_parallel.rs (crate::queries::*_tx paths)
#[allow(deprecated)]
pub use row_types::{
    batch_insert_invocations_tx, batch_insert_turns_tx, batch_upsert_models_tx,
    update_session_deep_fields_tx,
};
