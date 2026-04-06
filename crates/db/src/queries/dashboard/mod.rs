// crates/db/src/queries/dashboard/mod.rs
// Dashboard statistics, project summaries, and paginated session queries.

mod activity;
mod project_listing;
mod session_queries;
mod stats;
mod types;

pub use types::{
    ActivityPoint, ActivitySummaryRow, ProjectActivityRow, RichActivityResponse,
    SessionFilterParams,
};
