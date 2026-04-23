// crates/db/src/queries/sessions/mod.rs
// Session CRUD operations: insert, update, list, and indexer state management.

mod archive;
mod indexer;
mod listing;
#[cfg(test)]
mod tests;
mod update;
mod upsert;

pub use upsert::{
    execute_upsert_parsed_session, execute_upsert_session_stats_from_parsed, UPSERT_SESSION_SQL,
    UPSERT_SESSION_STATS_FROM_PARSED_SQL,
};
