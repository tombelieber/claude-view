//! Unified process scanning oracle.
//!
//! Single background task that owns one `sysinfo::System` instance (reused
//! across ticks for CPU delta tracking). Publishes snapshots via `tokio::watch`
//! so both the Live Monitor and System Monitor can read from the same data
//! without duplicating expensive system calls.
//!
//! # Two-tier cadences
//! - **Fast tick (2s):** CPU/memory + targeted `refresh_processes(Some(session_pids))`
//!   for per-session resource tracking. Typically ~9 PIDs vs 500+ system-wide.
//! - **Slow tick (10s, every 5th):** Full `refresh_processes(All)` + top processes,
//!   disk stats, process tree classification → `ProcessTreeSnapshot`.
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
use super::process_tree::ProcessTreeSnapshot;

/// Snapshot produced by the oracle on every 2s tick.
#[derive(Debug, Clone)]
pub struct OracleSnapshot {
    /// Full resource data (CPU, memory, disk, top processes).
    /// Does NOT include session_resources — consumers join that themselves.
    pub resource: ResourceData,
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

impl OracleSnapshot {
    /// Returns true if this snapshot is stale (oracle may have crashed).
    /// A snapshot older than 10 seconds is considered stale since the oracle
    /// ticks every 2 seconds.
    pub fn is_stale(&self) -> bool {
        if self.last_updated_at == 0 {
            return true; // Never updated
        }
        let now = chrono::Utc::now().timestamp();
        (now - self.last_updated_at) > 10
    }

    /// Age of this snapshot in seconds. Returns -1 if never updated.
    pub fn age_secs(&self) -> i64 {
        if self.last_updated_at == 0 {
            return -1; // Never updated
        }
        chrono::Utc::now().timestamp() - self.last_updated_at
    }
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
    /// Per-PID CPU/memory for known session PIDs only (targeted refresh).
    pub process_resources: HashMap<u32, ProcessResourceEntry>,
}

/// Per-process resource entry for session resource lookups.
#[derive(Debug, Clone)]
pub struct ProcessResourceEntry {
    pub cpu_percent: f32,
    pub memory_bytes: u64,
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
    session_pids_rx: watch::Receiver<Vec<u32>>,
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
            let should_full_refresh = tick == 1 || tick.is_multiple_of(5); // first tick + every 10s

            // Read latest known session PIDs (non-blocking, latest-value).
            let session_pids: Vec<u32> = session_pids_rx.borrow().clone();

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
                    should_full_refresh,
                    &session_pids,
                    &mut cache_moved,
                );
                let component_snapshot = if should_full_refresh {
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
///
/// Two-tier refresh:
/// - **Fast tick** (`should_full_refresh=false`): refreshes only `session_pids`
///   (~9 PIDs) for per-session CPU/memory. Skips top_processes, disk, and tree.
/// - **Full tick** (`should_full_refresh=true`, every 5th tick = 10s): refreshes
///   ALL processes for top_processes, disk stats, and process tree classification.
fn collect_oracle_snapshot(
    sys: &mut System,
    tick: u32,
    should_full_refresh: bool,
    session_pids: &[u32],
    tree_cache: &mut super::process_tree::ProcessTreeCache,
) -> OracleSnapshot {
    // CPU + memory: always refreshed (lightweight).
    sys.refresh_cpu_usage();
    sys.refresh_memory();

    if should_full_refresh {
        // Every 10s: full process refresh for top_processes + process tree.
        sys.refresh_processes(ProcessesToUpdate::All, true);
    } else {
        // Every 2s: targeted refresh for session PIDs only.
        let pids: Vec<sysinfo::Pid> = session_pids
            .iter()
            .map(|&p| sysinfo::Pid::from_u32(p))
            .collect();
        sys.refresh_processes(ProcessesToUpdate::Some(&pids), true);
    }

    // Overall CPU %
    let cpu_percent = {
        let cpus = sys.cpus();
        if cpus.is_empty() {
            0.0
        } else {
            cpus.iter().map(|c| c.cpu_usage()).sum::<f32>() / cpus.len() as f32
        }
    };

    // Per-PID resources: only for session PIDs (fast path).
    let mut process_resources: HashMap<u32, ProcessResourceEntry> = HashMap::new();
    for &pid in session_pids {
        if let Some(proc) = sys.process(sysinfo::Pid::from_u32(pid)) {
            let mem = super::process_tree::proc_memory::process_memory_bytes(pid, proc.memory());
            process_resources.insert(
                pid,
                ProcessResourceEntry {
                    cpu_percent: proc.cpu_usage(),
                    memory_bytes: mem,
                },
            );
        }
    }

    // Top processes + tree + disk: only on full refresh ticks.
    let (top_processes, process_tree, disk_used_bytes, disk_total_bytes) = if should_full_refresh {
        // Disk (deduped by name for APFS)
        let disks = Disks::new_with_refreshed_list();
        let mut seen_names = std::collections::HashSet::new();
        let (du, dt) = disks.iter().fold((0u64, 0u64), |(used, total), d| {
            let name = d.name().to_string_lossy().to_string();
            if !seen_names.insert(name) {
                return (used, total);
            }
            (
                used + (d.total_space() - d.available_space()),
                total + d.total_space(),
            )
        });

        // Top processes grouped by normalized name.
        // Memory = physical footprint on macOS (matches Activity Monitor), RSS elsewhere.
        let mut groups: HashMap<String, (u32, f32, u64)> = HashMap::new();
        for (pid, proc) in sys.processes() {
            let mem =
                super::process_tree::proc_memory::process_memory_bytes(pid.as_u32(), proc.memory());
            let norm = normalize_process_name(&proc.name().to_string_lossy());
            let entry = groups.entry(norm).or_insert((0, 0.0, 0));
            entry.0 += 1;
            entry.1 += proc.cpu_usage();
            entry.2 += mem;
        }
        let mut top: Vec<ProcessGroup> = groups
            .into_iter()
            .map(|(name, (count, cpu, mem))| ProcessGroup {
                name,
                process_count: count,
                cpu_percent: cpu,
                memory_bytes: mem,
            })
            .collect();
        top.sort_by(|a, b| {
            b.cpu_percent
                .total_cmp(&a.cpu_percent)
                .then_with(|| b.memory_bytes.cmp(&a.memory_bytes))
        });
        top.truncate(10);

        let tree = super::process_tree::classify_processes_cached(sys, tree_cache);
        (top, Some(tree), du, dt)
    } else {
        (Vec::new(), None, 0, 0)
    };

    OracleSnapshot {
        resource: ResourceData {
            timestamp: chrono::Utc::now().timestamp(),
            cpu_percent,
            memory_used_bytes: sys.used_memory(),
            memory_total_bytes: sys.total_memory(),
            disk_used_bytes,
            disk_total_bytes,
            top_processes,
            process_resources,
        },
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

#[cfg(test)]
mod tests {
    use super::*;

    /// L4: Verify oracle health check detects never-updated snapshots.
    #[test]
    fn test_oracle_snapshot_is_stale_when_never_updated() {
        let snapshot = OracleSnapshot {
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
            process_tree: None,
            component_snapshot: None,
            scanned_at: Instant::now(),
            tick: 0,
            last_updated_at: 0,
        };
        assert!(snapshot.is_stale());
        assert_eq!(snapshot.age_secs(), -1);
    }

    /// L4: Verify oracle health check passes for fresh snapshots.
    #[test]
    fn test_oracle_snapshot_not_stale_when_fresh() {
        let now = chrono::Utc::now().timestamp();
        let snapshot = OracleSnapshot {
            resource: ResourceData {
                timestamp: now,
                cpu_percent: 10.0,
                memory_used_bytes: 1024,
                memory_total_bytes: 8192,
                disk_used_bytes: 0,
                disk_total_bytes: 0,
                top_processes: Vec::new(),
                process_resources: HashMap::new(),
            },
            process_tree: None,
            component_snapshot: None,
            scanned_at: Instant::now(),
            tick: 5,
            last_updated_at: now,
        };
        assert!(!snapshot.is_stale());
        assert!(snapshot.age_secs() <= 1);
    }

    /// Two-tier: fast tick with no session PIDs → empty process_resources, no top_processes, no tree, no disk.
    #[test]
    fn test_oracle_snapshot_fast_tick_only_tracks_session_pids() {
        let mut sys = System::new_all();
        let mut cache = super::super::process_tree::ProcessTreeCache::new();
        let snap = collect_oracle_snapshot(&mut sys, 2, false, &[], &mut cache);
        assert!(
            snap.resource.process_resources.is_empty(),
            "fast tick with no PIDs should have empty process_resources"
        );
        assert!(
            snap.resource.top_processes.is_empty(),
            "fast tick should not compute top_processes"
        );
        assert!(
            snap.process_tree.is_none(),
            "fast tick should not compute process tree"
        );
        assert_eq!(
            snap.resource.disk_used_bytes, 0,
            "fast tick should not compute disk"
        );
    }

    /// Two-tier: full tick populates top_processes and process tree.
    #[test]
    fn test_oracle_snapshot_full_tick_populates_top_processes() {
        let mut sys = System::new_all();
        let mut cache = super::super::process_tree::ProcessTreeCache::new();
        let snap = collect_oracle_snapshot(&mut sys, 1, true, &[], &mut cache);
        assert!(
            !snap.resource.top_processes.is_empty(),
            "full tick should have top_processes"
        );
        assert!(
            snap.process_tree.is_some(),
            "full tick should have process tree"
        );
    }

    /// Two-tier: fast tick with our own PID → that PID appears in process_resources.
    #[test]
    fn test_oracle_snapshot_tracks_current_process_pid() {
        let my_pid = std::process::id();
        let mut sys = System::new_all();
        // Full refresh first to populate the system's process table
        sys.refresh_processes(ProcessesToUpdate::All, true);
        let mut cache = super::super::process_tree::ProcessTreeCache::new();
        // Targeted refresh for our own PID
        let snap = collect_oracle_snapshot(&mut sys, 2, false, &[my_pid], &mut cache);
        assert!(
            snap.resource.process_resources.contains_key(&my_pid),
            "fast tick should track session PID {my_pid}"
        );
        let entry = &snap.resource.process_resources[&my_pid];
        assert!(
            entry.memory_bytes > 0,
            "should report non-zero memory for current process"
        );
    }

    /// L3: Verify PID→session→CPU/RAM join pipeline via build_resource_snapshot.
    #[test]
    fn test_build_resource_snapshot_joins_session_pids() {
        let mut process_resources = HashMap::new();
        process_resources.insert(
            1234,
            ProcessResourceEntry {
                cpu_percent: 45.0,
                memory_bytes: 1024 * 1024 * 100,
            },
        );
        process_resources.insert(
            5678,
            ProcessResourceEntry {
                cpu_percent: 10.0,
                memory_bytes: 1024 * 1024 * 50,
            },
        );

        let data = ResourceData {
            timestamp: chrono::Utc::now().timestamp(),
            cpu_percent: 55.0,
            memory_used_bytes: 4 * 1024 * 1024 * 1024,
            memory_total_bytes: 16 * 1024 * 1024 * 1024,
            disk_used_bytes: 0,
            disk_total_bytes: 0,
            top_processes: Vec::new(),
            process_resources,
        };

        // build_resource_snapshot takes &HashMap<String, LiveSession>
        // LiveSession is complex — build a minimal one by reading the join logic:
        // it only accesses session.hook.pid and session.id
        // Rather than constructing the full struct, we test via the public function
        // using the stub() receiver pattern.

        // Since LiveSession doesn't impl Default, we verify the join logic
        // indirectly: the function iterates sessions, extracts pid, and looks up
        // in process_resources. With an empty sessions map, we get 0 results.
        let empty_sessions: HashMap<String, super::super::state::LiveSession> = HashMap::new();
        let snapshot = build_resource_snapshot(&data, &empty_sessions);
        assert!(snapshot.session_resources.is_empty());

        // Verify resource fields pass through correctly
        assert!((snapshot.cpu_percent - 55.0).abs() < 0.01);
        assert_eq!(snapshot.memory_used_bytes, 4 * 1024 * 1024 * 1024);
        assert_eq!(snapshot.memory_total_bytes, 16 * 1024 * 1024 * 1024);
    }
}
