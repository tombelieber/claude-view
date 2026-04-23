//! Server startup scaffolding — port detection, install telemetry,
//! static-asset discovery, periodic background tasks, and the CQRS
//! shadow gauge + drift samplers.
//!
//! Each concern lives in its own sibling module so the top-level
//! `main.rs` stays focused on runtime orchestration. Re-exports below
//! preserve the existing `crate::startup::*` call sites.

pub mod auth;
pub mod background;
pub mod bootstrap;
pub mod cli_dispatch;
pub mod cqrs;
pub mod data_dir;
pub mod indexer;
pub mod install;
pub mod observability;
pub mod paths;
pub mod platform;
pub mod port;
pub mod search;
pub mod serve;
pub mod server_bind;
pub mod tasks;
pub mod tui;

pub use cqrs::{
    run_drift_sampler_once, run_sampler_once, spawn_cqrs_sampler, spawn_drift_detector,
};
