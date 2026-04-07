// crates/server/src/lib.rs
//! Claude View server library.
//!
//! This crate provides the Axum-based HTTP server for the claude-view application.
//! It serves a REST API for listing Claude Code projects and retrieving session data.

pub mod app_factory;
pub mod auth;
pub mod backfill;
pub mod cache;
pub mod classify_state;
pub mod crypto;
pub mod error;
pub mod facet_ingest;
pub mod file_tracker;
pub mod git_sync_state;
pub mod indexing_state;
pub mod insights;
pub mod jobs;
pub mod live;
pub mod local_llm;
pub mod metrics;
pub mod openapi;
pub mod routes;
pub mod search_service;
pub mod share_serializer;
pub mod sidecar;
pub mod state;
pub mod teams;
pub mod telemetry;
pub mod terminal_state;
pub mod time_range;

#[cfg(test)]
mod tests;

#[cfg(test)]
#[cfg(feature = "codegen")]
mod codegen_format;

pub use error::*;
pub use facet_ingest::{FacetIngestState, IngestStatus};
pub use git_sync_state::{GitSyncPhase, GitSyncState};
pub use indexing_state::{IndexingState, IndexingStatus};
pub use live::manager::LiveSessionMap;
pub use live::state::SessionEvent;
pub use metrics::{init_metrics, record_request, record_storage, record_sync, RequestTimer};
pub use routes::api_routes;
pub use sidecar::SidecarManager;
pub use state::{
    AppState, AvailableIdesHolder, PromptIndexHolder, PromptStatsHolder, PromptTemplatesHolder,
    RegistryHolder, SearchIndexHolder, ShareConfig,
};

// Re-export all app factory functions at crate root to preserve public API.
pub use app_factory::{
    create_app, create_app_full, create_app_with_git_sync, create_app_with_indexing,
    create_app_with_indexing_and_static, create_app_with_static, create_app_with_telemetry_path,
    register_hooks,
};
