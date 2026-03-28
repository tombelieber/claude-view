//! Assembles component-level resource metrics from sidecar + oMLX + self.

use std::sync::atomic::Ordering;

use sysinfo::System;

use super::omlx_lifecycle::OmlxStatus;
use super::process_tree::component_types::{
    ComponentDetails, ComponentKind, ComponentSnapshot, ComponentStatus,
};
use super::gpu_memory;
use crate::sidecar::SidecarManager;

/// Collect component status snapshot.
///
/// Called by the process oracle every 10s (same cadence as process_tree).
/// Uses the already-refreshed `sysinfo::System` to look up PID metrics.
pub fn collect(
    sys: &System,
    sidecar: &SidecarManager,
    omlx_status: &OmlxStatus,
) -> ComponentSnapshot {
    let mut components = Vec::with_capacity(3);

    // --- Self (claude-view server) ---
    let self_pid = std::process::id();
    let (self_cpu, self_mem) = pid_metrics(sys, Some(self_pid));
    components.push(ComponentStatus {
        name: "claude-view".into(),
        kind: ComponentKind::ChildProcess,
        enabled: true,
        running: true,
        pid: Some(self_pid),
        cpu_percent: self_cpu,
        memory_bytes: self_mem,
        vram_bytes: None,
        details: ComponentDetails::Server,
    });

    // --- Agent SDK Sidecar ---
    let sidecar_pid = sidecar.child_pid();
    let (sidecar_cpu, sidecar_mem) = pid_metrics(sys, sidecar_pid);
    components.push(ComponentStatus {
        name: "agent-sdk-sidecar".into(),
        kind: ComponentKind::ChildProcess,
        enabled: true, // v1: always enabled
        running: sidecar_pid.is_some(),
        pid: sidecar_pid,
        cpu_percent: sidecar_cpu,
        memory_bytes: sidecar_mem,
        vram_bytes: None,
        details: ComponentDetails::Sidecar {
            session_count: if sidecar_pid.is_some() {
                fetch_session_count(sidecar)
            } else {
                None
            },
        },
    });

    // --- oMLX ---
    let omlx_healthy = omlx_status.ready.load(Ordering::Acquire);
    // PID cached by omlx_lifecycle at startup — no lsof on the 10s hot path
    let omlx_pid = if omlx_healthy { omlx_status.pid() } else { None };
    let (omlx_cpu, omlx_mem) = pid_metrics(sys, omlx_pid);
    components.push(ComponentStatus {
        name: "omlx-qwen".into(),
        kind: ComponentKind::ExternalService,
        enabled: true, // v1: always enabled
        running: omlx_healthy && omlx_pid.is_some(),
        pid: omlx_pid,
        cpu_percent: omlx_cpu,
        memory_bytes: omlx_mem,
        vram_bytes: if omlx_healthy { gpu_memory::gpu_alloc_bytes() } else { None },
        details: ComponentDetails::Omlx {
            model_id: super::omlx_lifecycle::EXPECTED_MODEL_SUBSTRING.into(),
            port: omlx_status.port,
            healthy: omlx_healthy,
        },
    });

    let build_mode = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    }
    .to_owned();

    ComponentSnapshot {
        components,
        build_mode,
        total_vram_bytes: gpu_memory::total_gpu_memory_bytes(),
    }
}

/// Fetch active session count from sidecar HTTP API.
/// Returns None if sidecar is not running or request fails.
/// Uses a 1-second timeout — collector must not block the oracle.
fn fetch_session_count(sidecar: &SidecarManager) -> Option<u32> {
    let url = format!("{}/api/sidecar/sessions", sidecar.base_url());
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(1))
        .build()
        .ok()?;
    let resp = client.get(&url).send().ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body: serde_json::Value = resp.json().ok()?;
    body.as_array().map(|arr| arr.len() as u32)
}

/// Look up CPU and memory for a PID from the already-refreshed System.
fn pid_metrics(sys: &System, pid: Option<u32>) -> (f32, u64) {
    let Some(pid) = pid else {
        return (0.0, 0);
    };
    let spid = sysinfo::Pid::from_u32(pid);
    match sys.process(spid) {
        Some(proc) => (proc.cpu_usage(), proc.memory()),
        None => (0.0, 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sysinfo::ProcessesToUpdate;

    #[test]
    fn pid_metrics_returns_zero_for_none() {
        let sys = System::new_all();
        let (cpu, mem) = pid_metrics(&sys, None);
        assert_eq!(cpu, 0.0);
        assert_eq!(mem, 0);
    }

    #[test]
    fn pid_metrics_returns_data_for_self() {
        let mut sys = System::new_all();
        std::thread::sleep(std::time::Duration::from_millis(200));
        sys.refresh_processes(ProcessesToUpdate::All, true);
        let self_pid = std::process::id();
        let (cpu, mem) = pid_metrics(&sys, Some(self_pid));
        // We are running, so memory should be > 0
        assert!(mem > 0, "self process memory should be > 0");
        // CPU can be 0 on first sample, that's OK
        let _ = cpu;
    }

}
