//! Teams data parser for ~/.claude/teams/.
//!
//! Reads team configs and inbox messages from the filesystem.
//! No file watching -- teams are ephemeral (1-44 min bursts).

pub mod jsonl_index;
pub mod jsonl_reconstruct;
pub mod parser;
pub mod resolve;
pub mod snapshot;
pub mod store;
mod tests;
mod types;

// Re-export all public items so `crate::teams::*` paths remain unchanged.
pub use jsonl_index::build_team_jsonl_index;
pub use resolve::{build_team_cost, resolve_team_member_sessions, resolve_team_sidechains};
pub use snapshot::snapshot_team;
pub use store::TeamsStore;
pub use types::{
    InboxMessage, InboxMessageType, ResolvedMemberInfo, TeamCostBreakdown, TeamDetail,
    TeamJSONLIndex, TeamJSONLRef, TeamMember, TeamMemberCost, TeamMemberSidechain, TeamSummary,
};
