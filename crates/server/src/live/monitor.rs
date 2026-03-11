//! System resource monitoring for the System Monitor page.
//!
//! Provides periodic snapshots of CPU, memory, disk, and per-session resource
//! usage. Uses a lazy observer pattern: polling starts only when the first SSE
//! client connects and stops when the last one disconnects.

use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use sysinfo::{Disks, ProcessesToUpdate, System};
use tokio::sync::broadcast;
use ts_rs::TS;

use crate::live::manager::LiveSessionMap;

// =============================================================================
// Data Structs
// =============================================================================

/// Static system information that doesn't change between snapshots.
#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(export, export_to = "../../../../../apps/web/src/types/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    /// Machine hostname.
    pub hostname: String,
    /// Operating system name (e.g. "macOS", "Linux").
    pub os: String,
    /// OS version string.
    pub os_version: String,
    /// CPU architecture (e.g. "aarch64", "x86_64").
    pub arch: String,
    /// Number of physical CPU cores.
    pub cpu_core_count: usize,
    /// Total physical memory in bytes.
    #[ts(type = "number")]
    pub total_memory_bytes: u64,
}

/// A single process group — aggregated by normalized process name.
#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(export, export_to = "../../../../../apps/web/src/types/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ProcessGroup {
    /// Normalized display name (e.g. "Google Chrome", not "Google Chrome Helper (Renderer)").
    pub name: String,
    /// Number of OS processes in this group.
    pub process_count: u32,
    /// Total CPU usage across all processes in this group (0.0–N cores).
    pub cpu_percent: f32,
    /// Total resident memory in bytes.
    #[ts(type = "number")]
    pub memory_bytes: u64,
}

/// Per-session resource snapshot (CPU + memory for the Claude process).
#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(export, export_to = "../../../../../apps/web/src/types/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct SessionResource {
    /// Session UUID (matches LiveSession.id).
    pub session_id: String,
    /// PID of the Claude process.
    pub pid: u32,
    /// CPU usage of this process (0.0–100.0 per core).
    pub cpu_percent: f32,
    /// Resident memory in bytes.
    #[ts(type = "number")]
    pub memory_bytes: u64,
}

/// A periodic resource snapshot broadcast to SSE clients.
#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(export, export_to = "../../../../../apps/web/src/types/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSnapshot {
    /// Unix timestamp (seconds) when this snapshot was taken.
    #[ts(type = "number")]
    pub timestamp: i64,
    /// Overall CPU usage (0.0–100.0).
    pub cpu_percent: f32,
    /// Used memory in bytes.
    #[ts(type = "number")]
    pub memory_used_bytes: u64,
    /// Total memory in bytes.
    #[ts(type = "number")]
    pub memory_total_bytes: u64,
    /// Used disk space in bytes (sum of all mounted volumes).
    #[ts(type = "number")]
    pub disk_used_bytes: u64,
    /// Total disk space in bytes.
    #[ts(type = "number")]
    pub disk_total_bytes: u64,
    /// Top processes by CPU+memory, grouped by normalized name.
    pub top_processes: Vec<ProcessGroup>,
    /// Per-session resource usage for active Claude sessions.
    pub session_resources: Vec<SessionResource>,
}

// =============================================================================
// Process Name Normalization
// =============================================================================

/// Normalize a process name for grouping.
///
/// Strips macOS helper suffixes like "Google Chrome Helper (Renderer)" → "Google Chrome".
pub fn normalize_process_name(name: &str) -> String {
    // Strip " Helper (XXX)" suffix (Chrome, Electron apps)
    if let Some(base) = name.strip_suffix(')') {
        if let Some(pos) = base.rfind(" Helper") {
            return name[..pos].to_string();
        }
    }
    // Strip bare " Helper" suffix
    if let Some(base) = name.strip_suffix(" Helper") {
        return base.to_string();
    }
    name.to_string()
}

// =============================================================================
// System Info Collection
// =============================================================================

/// Collect static system information. Called once at SSE connect time.
pub fn collect_system_info() -> SystemInfo {
    SystemInfo {
        hostname: System::host_name().unwrap_or_else(|| "unknown".into()),
        os: System::name().unwrap_or_else(|| "unknown".into()),
        os_version: System::os_version().unwrap_or_else(|| "unknown".into()),
        arch: std::env::consts::ARCH.to_string(),
        cpu_core_count: {
            let sys = System::new();
            sys.physical_core_count().unwrap_or(1)
        },
        total_memory_bytes: {
            let mut sys = System::new();
            sys.refresh_memory();
            sys.total_memory()
        },
    }
}

// =============================================================================
// Snapshot Collection
// =============================================================================

/// Collect a single resource snapshot. Runs on a blocking thread.
pub fn collect_snapshot(
    sys: &mut System,
    live_sessions: &HashMap<String, crate::live::state::LiveSession>,
) -> ResourceSnapshot {
    sys.refresh_cpu_usage();
    sys.refresh_memory();
    sys.refresh_processes(ProcessesToUpdate::All, true);

    // Overall CPU: average across all CPUs
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

    // Disk
    let disks = Disks::new_with_refreshed_list();
    let (disk_used_bytes, disk_total_bytes) =
        disks.iter().fold((0u64, 0u64), |(used, total), d| {
            (
                used + (d.total_space() - d.available_space()),
                total + d.total_space(),
            )
        });

    // Top processes — group by normalized name, take top 10
    let mut groups: HashMap<String, (u32, f32, u64)> = HashMap::new();
    for proc in sys.processes().values() {
        let raw_name = proc.name().to_string_lossy().to_string();
        let norm = normalize_process_name(&raw_name);
        let entry = groups.entry(norm).or_insert((0, 0.0, 0));
        entry.0 += 1;
        entry.1 += proc.cpu_usage();
        entry.2 += proc.memory();
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
    // Sort by CPU desc, then memory desc
    top_processes.sort_by(|a, b| {
        b.cpu_percent
            .total_cmp(&a.cpu_percent)
            .then_with(|| b.memory_bytes.cmp(&a.memory_bytes))
    });
    top_processes.truncate(10);

    // Per-session resources
    let session_resources: Vec<SessionResource> = live_sessions
        .values()
        .filter_map(|session| {
            let pid = session.pid?;
            let sysinfo_pid = sysinfo::Pid::from_u32(pid);
            let proc = sys.process(sysinfo_pid)?;
            Some(SessionResource {
                session_id: session.id.clone(),
                pid,
                cpu_percent: proc.cpu_usage(),
                memory_bytes: proc.memory(),
            })
        })
        .collect();

    ResourceSnapshot {
        timestamp: chrono::Utc::now().timestamp(),
        cpu_percent,
        memory_used_bytes,
        memory_total_bytes,
        disk_used_bytes,
        disk_total_bytes,
        top_processes,
        session_resources,
    }
}

// =============================================================================
// Lazy Observer — Polling Task
// =============================================================================

/// Start the polling task that collects snapshots every 2 seconds.
///
/// Returns immediately. The task runs until the subscriber count drops to 0.
pub fn start_polling_task(
    tx: broadcast::Sender<ResourceSnapshot>,
    subscriber_count: Arc<AtomicUsize>,
    live_sessions: LiveSessionMap,
) {
    tokio::task::spawn(async move {
        tracing::info!("monitor: polling task started");

        // sysinfo::System must be reused across calls for CPU delta tracking
        let mut sys = System::new_all();
        // Initial CPU measurement baseline — first reading is always 0
        std::thread::sleep(std::time::Duration::from_millis(200));
        sys.refresh_cpu_usage();

        let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));
        loop {
            interval.tick().await;

            // Stop polling when no subscribers remain
            if subscriber_count.load(Ordering::Relaxed) == 0 {
                tracing::info!("monitor: no subscribers, stopping polling task");
                break;
            }

            // Snapshot on blocking thread (sysinfo does syscalls)
            let sessions_clone = {
                let map = live_sessions.read().await;
                map.clone()
            };

            let mut sys_moved = std::mem::take(&mut sys);
            let (snapshot, sys_back) = tokio::task::spawn_blocking(move || {
                let snap = collect_snapshot(&mut sys_moved, &sessions_clone);
                (snap, sys_moved)
            })
            .await
            .unwrap_or_else(|e| {
                tracing::error!("monitor: blocking task panicked: {e}");
                (
                    ResourceSnapshot {
                        timestamp: chrono::Utc::now().timestamp(),
                        cpu_percent: 0.0,
                        memory_used_bytes: 0,
                        memory_total_bytes: 0,
                        disk_used_bytes: 0,
                        disk_total_bytes: 0,
                        top_processes: Vec::new(),
                        session_resources: Vec::new(),
                    },
                    System::new(),
                )
            });
            sys = sys_back;

            // Broadcast — ignore error (no receivers is fine, count will hit 0 next tick)
            let _ = tx.send(snapshot);
        }
    });
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_chrome_helper_suffixes() {
        assert_eq!(
            normalize_process_name("Google Chrome Helper (Renderer)"),
            "Google Chrome"
        );
        assert_eq!(
            normalize_process_name("Google Chrome Helper (GPU)"),
            "Google Chrome"
        );
        assert_eq!(
            normalize_process_name("Google Chrome Helper"),
            "Google Chrome"
        );
        assert_eq!(normalize_process_name("Code"), "Code");
    }

    #[test]
    fn normalize_preserves_plain_names() {
        assert_eq!(normalize_process_name("firefox"), "firefox");
        assert_eq!(normalize_process_name("node"), "node");
        assert_eq!(normalize_process_name("claude-view"), "claude-view");
    }

    #[test]
    fn normalize_strips_electron_helper_suffixes() {
        assert_eq!(normalize_process_name("Cursor Helper (Renderer)"), "Cursor");
        assert_eq!(normalize_process_name("Slack Helper (GPU)"), "Slack");
    }

    #[test]
    fn collect_system_info_returns_nonempty_hostname() {
        let info = collect_system_info();
        assert!(!info.hostname.is_empty());
        assert!(info.cpu_core_count > 0);
    }

    #[test]
    fn collect_system_info_has_nonzero_memory() {
        let info = collect_system_info();
        assert!(info.total_memory_bytes > 0);
    }

    #[test]
    fn collect_system_info_has_valid_arch() {
        let info = collect_system_info();
        assert!(
            ["aarch64", "x86_64", "x86", "arm"].contains(&info.arch.as_str()),
            "unexpected arch: {}",
            info.arch
        );
    }

    #[test]
    fn collect_snapshot_returns_valid_data() {
        let mut sys = System::new_all();
        // Need a short sleep for CPU to have a baseline
        std::thread::sleep(std::time::Duration::from_millis(200));
        let sessions = HashMap::new();
        let snap = collect_snapshot(&mut sys, &sessions);

        assert!(snap.timestamp > 0);
        assert!(snap.memory_total_bytes > 0);
        assert!(snap.memory_used_bytes > 0);
        assert!(snap.memory_used_bytes <= snap.memory_total_bytes);
        assert!(snap.disk_total_bytes > 0);
    }

    #[test]
    fn collect_snapshot_top_processes_capped_at_10() {
        let mut sys = System::new_all();
        std::thread::sleep(std::time::Duration::from_millis(200));
        let sessions = HashMap::new();
        let snap = collect_snapshot(&mut sys, &sessions);
        assert!(snap.top_processes.len() <= 10);
    }

    #[test]
    fn resource_snapshot_serializes_to_camel_case() {
        let snap = ResourceSnapshot {
            timestamp: 1700000000,
            cpu_percent: 42.5,
            memory_used_bytes: 8_000_000_000,
            memory_total_bytes: 16_000_000_000,
            disk_used_bytes: 100_000_000_000,
            disk_total_bytes: 500_000_000_000,
            top_processes: vec![],
            session_resources: vec![],
        };
        let json = serde_json::to_value(&snap).unwrap();
        assert_eq!(json["cpuPercent"], 42.5);
        assert_eq!(json["memoryUsedBytes"], 8_000_000_000u64);
        assert!(json.get("cpu_percent").is_none(), "should use camelCase");
    }

    #[test]
    fn system_info_serializes_to_camel_case() {
        let info = SystemInfo {
            hostname: "test-host".into(),
            os: "macOS".into(),
            os_version: "15.0".into(),
            arch: "aarch64".into(),
            cpu_core_count: 10,
            total_memory_bytes: 16_000_000_000,
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["cpuCoreCount"], 10);
        assert_eq!(json["totalMemoryBytes"], 16_000_000_000u64);
    }

    #[test]
    fn process_group_serializes_correctly() {
        let pg = ProcessGroup {
            name: "Google Chrome".into(),
            process_count: 15,
            cpu_percent: 12.5,
            memory_bytes: 2_000_000_000,
        };
        let json = serde_json::to_value(&pg).unwrap();
        assert_eq!(json["name"], "Google Chrome");
        assert_eq!(json["processCount"], 15);
    }

    #[test]
    fn session_resource_serializes_correctly() {
        let sr = SessionResource {
            session_id: "abc-123".into(),
            pid: 12345,
            cpu_percent: 25.0,
            memory_bytes: 500_000_000,
        };
        let json = serde_json::to_value(&sr).unwrap();
        assert_eq!(json["sessionId"], "abc-123");
        assert_eq!(json["pid"], 12345);
    }
}
