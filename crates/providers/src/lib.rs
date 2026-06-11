// crates/providers/src/lib.rs
//
// Multi-provider session ingestion. See README.md for layout and the
// attribution note (format knowledge derived from agentsview, MIT).

pub mod catalog;
pub mod discover;
pub mod kind;
pub mod model;
pub mod parsers;
pub mod util;

pub use catalog::ForeignCatalog;
pub use discover::{provider_for, registry, DiscoveredSession, Provider};
pub use kind::ProviderKind;
pub use model::{ForeignSession, ForeignSessionMeta, ForeignUsage, UsageTotals};
