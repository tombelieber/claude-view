//! Server startup scaffolding — port detection, install telemetry,
//! static-asset discovery, periodic background tasks, and the CQRS
//! shadow gauge + drift samplers.
//!
//! Each concern lives in its own sibling module so the top-level
//! `main.rs` stays focused on runtime orchestration (CLI dispatch,
//! observability init, HTTP serve). Re-exports below preserve the
//! existing `crate::startup::*` call sites.

pub mod background;
pub mod cqrs;
pub mod install;
pub mod paths;
pub mod port;

pub use cqrs::{
    run_drift_sampler_once, run_sampler_once, spawn_cqrs_sampler, spawn_drift_detector,
};
