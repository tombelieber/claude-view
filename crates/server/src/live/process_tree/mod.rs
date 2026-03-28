//! Process tree classifier for the System Monitor page.
//!
//! Scans ALL Claude-related OS processes (CLI, VS Code extension, Desktop app,
//! orphaned snapshots, child processes) — no deduplication. PID is the atom.
//!
//! # Classification algorithm
//! Three-pass: collect raw data → classify each PID → aggregate descendants.
//!
//! # macOS note
//! sysinfo may return empty `cmd` for some processes. When `name` looks Claude-like
//! but `cmd` is empty, fall back to `ps -p <pid> -o command=` (same pattern as
//! `get_cwd_via_lsof` in `process.rs`).

mod classifier;
pub mod component_types;
pub mod helpers;
pub mod sysctl_cmd;
pub mod types;

use sysinfo::System;
pub use types::{
    ClassifiedProcess, EcosystemTag, ProcessCategory, ProcessTreeSnapshot, ProcessTreeTotals,
    Staleness,
};

/// Classify all Claude-related processes visible in `sys`.
///
/// IMPORTANT: `sys` must be already refreshed (call `collect_snapshot` first,
/// which calls `sys.refresh_processes()`). Do NOT call `sys.refresh_*` here —
/// this is a read-only pass on already-fresh data.
pub fn classify_processes(sys: &System) -> ProcessTreeSnapshot {
    let own_pid = std::process::id();
    let raw = classifier::collect_raw_processes(sys, own_pid);
    classifier::classify_process_list(&raw, own_pid)
}
