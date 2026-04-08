//! Per-route dependency traits for Interface Segregation.
//!
//! Each route module should bound its handlers on the narrowest trait
//! that covers its actual needs, rather than taking full `AppState`.
//!
//! Convention:
//!   - `DbDeps` for routes that only need the database
//!   - Future: `LiveDeps`, `SearchDeps`, etc. when route crates are extracted (Phase 3)

use claude_view_db::Database;

/// Minimal dependency: database-only routes.
/// Used by: coaching, contributions, reports, turns, insights, etc.
pub trait DbDeps: Send + Sync + 'static {
    fn db(&self) -> &Database;
}
