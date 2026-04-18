// crates/db/src/queries/mod.rs
// Session CRUD operations for the claude-view SQLite database.

mod ai_generation;
mod catalog;
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
mod seed;
pub mod sessions;
pub mod settings;
pub mod stats;
mod system;
mod types;

pub use dashboard::ActivityPoint;
pub use dashboard::{ActivitySummaryRow, ProjectActivityRow, RichActivityResponse};
pub use search_prefilter::SearchPrefilter;
// Phase 3 PR 3.a: catalog-shape reads consumed by `SessionCatalogAdapter`
// (crates/core/src/session_catalog.rs). `StatsHeader` stays private — it's
// still an internal indexer_v2 type.
pub use stats::{CatalogFilter, CatalogSort, StatsCatalogRow};
pub use types::*;

// Re-export _tx functions used by the unified indexing pipeline.
pub use row_types::{batch_insert_invocations_tx, batch_insert_turns_tx, batch_upsert_models_tx};
