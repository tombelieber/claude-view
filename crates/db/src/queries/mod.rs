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
mod stats;
mod system;
mod types;

pub use dashboard::ActivityPoint;
pub use dashboard::{ActivitySummaryRow, ProjectActivityRow, RichActivityResponse};
pub use search_prefilter::SearchPrefilter;
// `stats::StatsHeader` is intentionally not re-exported in PR 2.1 — the
// only caller (indexer_v2 in PR 2.2) lives inside `claude_view_db` itself
// and reaches it via `crate::queries::stats`. Promoting to a public
// re-export will happen when an external crate first needs the type.
pub use types::*;

// Re-export _tx functions used by the unified indexing pipeline.
pub use row_types::{batch_insert_invocations_tx, batch_insert_turns_tx, batch_upsert_models_tx};
