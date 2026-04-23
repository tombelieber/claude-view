// crates/server/src/lib.rs
//! Claude View server library.
//!
//! This crate provides the Axum-based HTTP server for the claude-view application.
//! It serves a REST API for listing Claude Code projects and retrieving session data.

pub mod app_factory;
pub mod auth;
pub mod backfill;
pub mod cli;
pub use claude_view_server_types::cache;
pub use claude_view_server_types::classify_state;
pub mod crypto;
pub use claude_view_server_types::error;
pub use claude_view_server_types::facet_ingest;
pub mod file_tracker;
pub use claude_view_server_insights as insights;
pub use claude_view_server_jobs as jobs;
pub use claude_view_server_types::git_sync_state;
pub use claude_view_server_types::indexing_state;
pub mod live;
pub mod local_llm;
pub use claude_view_server_types::metrics;
pub mod openapi;
pub mod routes;
pub mod search_migration;
pub mod search_service;
pub mod session_catalog_adapter;
pub mod share_serializer;
pub use claude_view_server_sidecar as sidecar;
pub mod startup;
pub mod state;
pub mod supabase_proxy;
pub use claude_view_server_teams as teams;
pub mod telemetry;
pub use claude_view_server_types::terminal_state;
pub mod time_range;
pub mod webhook_engine;

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
