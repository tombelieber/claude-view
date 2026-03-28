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

/// Compute a hash of the (pid, ppid) set for cache invalidation.
/// If unchanged between ticks, the tree structure hasn't changed.
fn compute_pid_set_hash(sys: &System) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut pairs: Vec<(u32, u32)> = sys
        .processes()
        .iter()
        .map(|(pid, proc_)| {
            (
                pid.as_u32(),
                proc_.parent().map(|p| p.as_u32()).unwrap_or(0),
            )
        })
        .collect();
    pairs.sort_unstable();
    let mut hasher = DefaultHasher::new();
    pairs.hash(&mut hasher);
    hasher.finish()
}

/// Cached classify: skip full classification if PID set unchanged.
pub fn classify_processes_cached(
    sys: &System,
    cache: &mut ProcessTreeCache,
) -> ProcessTreeSnapshot {
    let hash = compute_pid_set_hash(sys);
    if hash == cache.last_hash {
        if let Some(ref cached) = cache.last_snapshot {
            return cached.clone();
        }
    }
    let snapshot = classify_processes(sys);
    cache.last_hash = hash;
    cache.last_snapshot = Some(snapshot.clone());
    snapshot
}

/// Cache state for process tree. Stored in the oracle loop.
pub struct ProcessTreeCache {
    last_hash: u64,
    last_snapshot: Option<ProcessTreeSnapshot>,
}

impl ProcessTreeCache {
    pub fn new() -> Self {
        Self {
            last_hash: 0,
            last_snapshot: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pid_set_hash_deterministic() {
        let sys = System::new_all();
        let h1 = compute_pid_set_hash(&sys);
        let h2 = compute_pid_set_hash(&sys);
        assert_eq!(h1, h2, "same System → same hash");
    }
}
