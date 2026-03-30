//! Unified process scanning oracle.
//!
//! Single background task that owns one `sysinfo::System` instance (reused
//! across ticks for CPU delta tracking). Publishes snapshots via `tokio::watch`
//! so both the Live Monitor and System Monitor can read from the same data
//! without duplicating expensive system calls.
//!
//! # Cadences
//! - **Every 2s:** CPU/memory/disk refresh → `ResourceData`
//! - **Every 5th tick (10s):** Process tree classification → `ProcessTreeSnapshot`
//! - **Every 5th tick (10s):** Claude process detection → `ClaudeProcesses`
//!
//! # Why tokio::watch?
//! Latest-value semantics: slow consumers skip intermediate snapshots.
//! No queue backlog, no lag. The `System` object is never shared — only the
//! computed output is published.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use sysinfo::{Disks, ProcessesToUpdate, System};
use tokio::sync::watch;

use super::monitor::{normalize_process_name, ProcessGroup, ResourceSnapshot, SessionResource};
use super::process::{detect_claude_processes_with_sys, ClaudeProcess};
use super::process_tree::ProcessTreeSnapshot;

/// Snapshot produced by the oracle on every 2s tick.
#[derive(Debug, Clone)]
pub struct OracleSnapshot {
    /// Full resource data (CPU, memory, disk, top processes).
    /// Does NOT include session_resources — consumers join that themselves.
    pub resource: ResourceData,
    /// Claude-specific processes. Updated every 5th tick (10s).
    /// `None` on ticks where detection was skipped.
    pub claude_processes: Option<Arc<ClaudeProcesses>>,
    /// Process tree classification. Updated every 5th tick (10s).
    pub process_tree: Option<ProcessTreeSnapshot>,
    /// Component-level resource breakdown. Updated every 5th tick (10s).
    pub component_snapshot: Option<super::process_tree::component_types::ComponentSnapshot>,
    /// When this snapshot was taken.
    pub scanned_at: Instant,
    /// Monotonic tick counter.
    pub tick: u32,
    /// Wall-clock Unix timestamp (seconds) of the last successful oracle update.
    /// Used by consumers to detect stale snapshots (e.g. if the oracle panics).
    /// 0 means "never updated".
    pub last_updated_at: i64,
}

/// Raw resource data computed from the System object.
#[derive(Debug, Clone)]
pub struct ResourceData {
    pub timestamp: i64,
    pub cpu_percent: f32,
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
    pub disk_used_bytes: u64,
    pub disk_total_bytes: u64,
    pub top_processes: Vec<ProcessGroup>,
    /// Per-PID CPU/memory for ALL processes — consumers join against session PIDs.
    pub process_resources: HashMap<u32, ProcessResourceEntry>,
}

/// Per-process resource entry for session resource lookups.
#[derive(Debug, Clone)]
pub struct ProcessResourceEntry {
    pub cpu_percent: f32,
    pub memory_bytes: u64,
}

/// Claude processes detected on the system.
#[derive(Debug, Clone)]
pub struct ClaudeProcesses {
    pub processes: HashMap<u32, ClaudeProcess>,
    pub count: u32,
}

/// Public handle to the oracle — consumers hold a watch::Receiver.
pub type OracleReceiver = watch::Receiver<Arc<OracleSnapshot>>;

/// Create a dummy oracle receiver for tests — no background task spawned.
///
/// Returns a watch receiver with empty initial data. Used by test `AppState`
/// constructors to avoid spawning N background threads doing `refresh_processes`
/// every 2 seconds across the entire test suite.
pub fn stub() -> OracleReceiver {
    let initial = Arc::new(OracleSnapshot {
        resource: ResourceData {
            timestamp: 0,
            cpu_percent: 0.0,
            memory_used_bytes: 0,
            memory_total_bytes: 0,
            disk_used_bytes: 0,
            disk_total_bytes: 0,
            top_processes: Vec::new(),
            process_resources: HashMap::new(),
        },
        claude_processes: None,
        process_tree: None,
        component_snapshot: None,
        scanned_at: Instant::now(),
        tick: 0,
        last_updated_at: 0,
    });
    watch::channel(initial).1
}

/// Start the process oracle background task.
///
/// Returns a `watch::Receiver` for consumers and the subscriber count for
/// the monitor SSE (used to pause/resume the oracle when no one is listening).
///
/// The oracle runs continuously (not paused when monitor has no subscribers)
/// because the LiveSessionManager always needs process data for reconciliation.
pub fn start_oracle(
    sidecar: Arc<crate::sidecar::SidecarManager>,
    omlx_status: Arc<crate::local_llm::LlmStatus>,
) -> OracleReceiver {
    let initial = Arc::new(OracleSnapshot {
        resource: ResourceData {
            timestamp: chrono::Utc::now().timestamp(),
            cpu_percent: 0.0,
            memory_used_bytes: 0,
            memory_total_bytes: 0,
            disk_used_bytes: 0,
            disk_total_bytes: 0,
            top_processes: Vec::new(),
            process_resources: HashMap::new(),
        },
        claude_processes: None,
        process_tree: None,
        component_snapshot: None,
        scanned_at: Instant::now(),
        tick: 0,
        last_updated_at: 0,
    });

    let (tx, rx) = watch::channel(initial);

    tokio::task::spawn(async move {
        tracing::info!("process_oracle: started");

        // The System instance persists across ticks — critical for CPU delta tracking.
        // Skip the 200ms CPU baseline sleep — first tick CPU reads 0% but components
        // load ~200ms faster. Second tick (2s later) has accurate CPU deltas.
        let mut sys = System::new_all();

        let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));
        let mut tick: u32 = 0;
        let mut tree_cache = super::process_tree::ProcessTreeCache::new();

        loop {
            interval.tick().await;
            tick = tick.wrapping_add(1);
            let should_classify = tick == 1 || tick.is_multiple_of(5); // first tick + every 10s

            // All sysinfo calls happen on a blocking thread.
            let sidecar_ref = sidecar.clone();
            let omlx_ref = omlx_status.clone();
            let mut sys_moved = std::mem::take(&mut sys);
            let mut cache_moved = std::mem::replace(
                &mut tree_cache,
                super::process_tree::ProcessTreeCache::new(),
            );
            let result = tokio::task::spawn_blocking(move || {
                let snapshot = collect_oracle_snapshot(
                    &mut sys_moved,
                    tick,
                    should_classify,
                    &mut cache_moved,
                );
                let component_snapshot = if should_classify {
                    Some(super::component_collector::collect(
                        &sys_moved,
                        &sidecar_ref,
                        &omlx_ref,
                    ))
                } else {
                    None
                };
                (snapshot, component_snapshot, sys_moved, cache_moved)
            })
            .await;

            match result {
                Ok((mut snapshot, component_snapshot, sys_back, cache_back)) => {
                    sys = sys_back;
                    tree_cache = cache_back;
                    snapshot.component_snapshot = component_snapshot;
                    let _ = tx.send(Arc::new(snapshot));
                }
                Err(e) => {
                    tracing::error!("process_oracle: blocking task panicked: {e}");
                    sys = System::new_all();
                    tree_cache = super::process_tree::ProcessTreeCache::new();
                }
            }
        }
    });

    rx
}

/// Compute a single oracle snapshot (runs on a blocking thread).
fn collect_oracle_snapshot(
    sys: &mut System,
    tick: u32,
    should_classify: bool,
    tree_cache: &mut super::process_tree::ProcessTreeCache,
) -> OracleSnapshot {
    // Refresh all process + resource data.
    sys.refresh_cpu_usage();
    sys.refresh_memory();
    sys.refresh_processes(ProcessesToUpdate::All, true);

    // Overall CPU %
    let cpu_percent = {
        let cpus = sys.cpus();
        if cpus.is_empty() {
            0.0
        } else {
            cpus.iter().map(|c| c.cpu_usage()).sum::<f32>() / cpus.len() as f32
        }
    };

    // Memory
    let memory_used_bytes = sys.used_memory();
    let memory_total_bytes = sys.total_memory();

    // Disk (deduped by name for APFS)
    let disks = Disks::new_with_refreshed_list();
    let mut seen_names = std::collections::HashSet::new();
    let (disk_used_bytes, disk_total_bytes) =
        disks.iter().fold((0u64, 0u64), |(used, total), d| {
            let name = d.name().to_string_lossy().to_string();
            if !seen_names.insert(name) {
                return (used, total);
            }
            (
                used + (d.total_space() - d.available_space()),
                total + d.total_space(),
            )
        });

    // Top processes by CPU+memory, grouped by normalized name
    let mut groups: HashMap<String, (u32, f32, u64)> = HashMap::new();
    let mut process_resources: HashMap<u32, ProcessResourceEntry> = HashMap::new();
    for (pid, proc) in sys.processes() {
        let raw_name = proc.name().to_string_lossy().to_string();
        let norm = normalize_process_name(&raw_name);
        let entry = groups.entry(norm).or_insert((0, 0.0, 0));
        entry.0 += 1;
        entry.1 += proc.cpu_usage();
        entry.2 += proc.memory();

        // Store per-PID resources for session lookups
        process_resources.insert(
            pid.as_u32(),
            ProcessResourceEntry {
                cpu_percent: proc.cpu_usage(),
                memory_bytes: proc.memory(),
            },
        );
    }

    let mut top_processes: Vec<ProcessGroup> = groups
        .into_iter()
        .map(|(name, (count, cpu, mem))| ProcessGroup {
            name,
            process_count: count,
            cpu_percent: cpu,
            memory_bytes: mem,
        })
        .collect();
    top_processes.sort_by(|a, b| {
        b.cpu_percent
            .total_cmp(&a.cpu_percent)
            .then_with(|| b.memory_bytes.cmp(&a.memory_bytes))
    });
    top_processes.truncate(10);

    // Claude process detection + process tree classification (every 5th tick)
    let (claude_procs, process_tree) = if should_classify {
        let claude = detect_claude_processes_with_sys(sys);
        let tree = super::process_tree::classify_processes_cached(sys, tree_cache);
        (Some(Arc::new(claude)), Some(tree))
    } else {
        (None, None)
    };

    OracleSnapshot {
        resource: ResourceData {
            timestamp: chrono::Utc::now().timestamp(),
            cpu_percent,
            memory_used_bytes,
            memory_total_bytes,
            disk_used_bytes,
            disk_total_bytes,
            top_processes,
            process_resources,
        },
        claude_processes: claude_procs,
        process_tree,
        component_snapshot: None, // Filled in by oracle loop after spawn_blocking
        scanned_at: Instant::now(),
        tick,
        last_updated_at: chrono::Utc::now().timestamp(),
    }
}

/// Build a ResourceSnapshot from oracle data + live sessions.
///
/// Consumers call this to produce the final SSE-ready snapshot by joining
/// the oracle's per-PID resource data with the live session map.
pub fn build_resource_snapshot(
    data: &ResourceData,
    sessions: &HashMap<String, super::state::LiveSession>,
) -> ResourceSnapshot {
    let session_resources: Vec<SessionResource> = sessions
        .values()
        .filter_map(|session| {
            let pid = session.hook.pid?;
            let res = data.process_resources.get(&pid)?;
            Some(SessionResource {
                session_id: session.id.clone(),
                pid,
                cpu_percent: res.cpu_percent,
                memory_bytes: res.memory_bytes,
            })
        })
        .collect();

    ResourceSnapshot {
        timestamp: data.timestamp,
        cpu_percent: data.cpu_percent,
        memory_used_bytes: data.memory_used_bytes,
        memory_total_bytes: data.memory_total_bytes,
        disk_used_bytes: data.disk_used_bytes,
        disk_total_bytes: data.disk_total_bytes,
        top_processes: data.top_processes.clone(),
        session_resources,
    }
}
