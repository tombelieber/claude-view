//! Process kill/cleanup endpoints.
//!
//! - `POST /api/processes/{pid}/kill`   — SIGTERM/SIGKILL a single process
//! - `POST /api/processes/cleanup`       — Batch SIGTERM of multiple processes
//!
//! Validation: PID must exist, start_time must match (prevents recycled-PID
//! attacks), and the server's own PID is always rejected. The "Claude-related"
//! classification is NOT re-checked here — the monitor's persistent System
//! instance sees full command strings that a fresh System::new() may miss on
//! macOS (SIP/empty-cmd edge case). If the frontend shows a process, it was
//! already validated as Claude-related by the monitor.

use axum::{
    extract::{Path, State},
    response::Json,
    routing::post,
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use sysinfo::{ProcessesToUpdate, System};
use ts_rs::TS;

use crate::state::AppState;

// =============================================================================
// Request / Response types
// =============================================================================

#[derive(Deserialize)]
pub struct KillProcessRequest {
    pub start_time: i64,
    pub force: bool,
}

#[derive(Debug, Serialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(export, export_to = "../../../../../apps/web/src/types/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct KillProcessResponse {
    pub killed: bool,
    pub pid: u32,
    pub error: Option<String>,
}

#[derive(Deserialize)]
pub struct KillTarget {
    pub pid: u32,
    pub start_time: i64,
}

#[derive(Deserialize)]
pub struct CleanupRequest {
    pub targets: Vec<KillTarget>,
}

#[derive(Debug, Serialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(export, export_to = "../../../../../apps/web/src/types/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CleanupResponse {
    pub killed: Vec<u32>,
    pub failed: Vec<KillFailure>,
}

#[derive(Debug, Serialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(export, export_to = "../../../../../apps/web/src/types/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct KillFailure {
    pub pid: u32,
    pub reason: String,
}

// =============================================================================
// Router
// =============================================================================

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/processes/cleanup", post(cleanup_processes))
        .route("/processes/{pid}/kill", post(kill_process))
}

// =============================================================================
// Handlers
// =============================================================================

async fn kill_process(
    State(_state): State<Arc<AppState>>,
    Path(pid): Path<u32>,
    Json(req): Json<KillProcessRequest>,
) -> Json<KillProcessResponse> {
    let result =
        tokio::task::spawn_blocking(move || validate_and_kill(pid, req.start_time, req.force))
            .await
            .unwrap_or_else(|e| Err(format!("internal error: {e}")));

    match result {
        Ok(()) => Json(KillProcessResponse {
            killed: true,
            pid,
            error: None,
        }),
        Err(msg) => Json(KillProcessResponse {
            killed: false,
            pid,
            error: Some(msg),
        }),
    }
}

async fn cleanup_processes(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<CleanupRequest>,
) -> Json<CleanupResponse> {
    let targets = req.targets;

    let result = tokio::task::spawn_blocking(move || {
        let mut sys = System::new();
        sys.refresh_processes(ProcessesToUpdate::All, true);

        let own_pid = std::process::id();

        let mut killed = Vec::new();
        let mut failed = Vec::new();

        for target in &targets {
            match validate_pid_in_system(&sys, target.pid, target.start_time, own_pid) {
                Ok(()) => {
                    let signal = libc::SIGTERM;
                    let result = unsafe { libc::kill(target.pid as i32, signal) };
                    if result == 0 {
                        killed.push(target.pid);
                    } else {
                        let errno = std::io::Error::last_os_error();
                        failed.push(KillFailure {
                            pid: target.pid,
                            reason: format!("SIGTERM failed: {errno}"),
                        });
                    }
                }
                Err(reason) => {
                    failed.push(KillFailure {
                        pid: target.pid,
                        reason,
                    });
                }
            }
        }

        CleanupResponse { killed, failed }
    })
    .await
    .unwrap_or_else(|e| CleanupResponse {
        killed: vec![],
        failed: vec![KillFailure {
            pid: 0,
            reason: format!("internal error: {e}"),
        }],
    });

    Json(result)
}

// =============================================================================
// Validation
// =============================================================================

fn validate_and_kill(pid: u32, start_time: i64, force: bool) -> Result<(), String> {
    let own_pid = std::process::id();
    if pid == own_pid {
        return Err("cannot kill own process".to_string());
    }

    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);

    validate_pid_in_system(&sys, pid, start_time, own_pid)?;

    let signal = if force { libc::SIGKILL } else { libc::SIGTERM };
    let result = unsafe { libc::kill(pid as i32, signal) };
    if result == 0 {
        Ok(())
    } else {
        let errno = std::io::Error::last_os_error();
        Err(format!(
            "{} failed: {errno}",
            if force { "SIGKILL" } else { "SIGTERM" }
        ))
    }
}

/// Validate that a PID is safe to kill: exists, start_time matches, not self.
fn validate_pid_in_system(
    sys: &System,
    pid: u32,
    start_time: i64,
    own_pid: u32,
) -> Result<(), String> {
    if pid == own_pid {
        return Err("cannot kill own process".to_string());
    }

    let sysinfo_pid = sysinfo::Pid::from_u32(pid);
    let process = sys
        .process(sysinfo_pid)
        .ok_or_else(|| format!("process {pid} not found"))?;

    if process.start_time() as i64 != start_time {
        return Err(format!(
            "PID {pid} start_time mismatch (expected {start_time}, got {}): PID may have been recycled",
            process.start_time()
        ));
    }

    Ok(())
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kill_response_serializes_to_camel_case() {
        let resp = KillProcessResponse {
            killed: true,
            pid: 1234,
            error: None,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["killed"], true);
        assert_eq!(json["pid"], 1234);
        assert!(json.get("pid").is_some());
    }

    #[test]
    fn test_cleanup_response_serializes_correctly() {
        let resp = CleanupResponse {
            killed: vec![100, 200],
            failed: vec![KillFailure {
                pid: 300,
                reason: "not found".into(),
            }],
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["killed"][0], 100);
        assert_eq!(json["failed"][0]["pid"], 300);
        assert_eq!(json["failed"][0]["reason"], "not found");
    }

    #[test]
    fn test_validate_rejects_own_pid() {
        let own_pid = std::process::id();
        let mut sys = System::new();
        sys.refresh_processes(ProcessesToUpdate::All, true);
        let result = validate_pid_in_system(&sys, own_pid, 0, own_pid);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cannot kill own process"));
    }

    #[test]
    fn test_validate_rejects_nonexistent_pid() {
        let mut sys = System::new();
        sys.refresh_processes(ProcessesToUpdate::All, true);
        let result = validate_pid_in_system(&sys, 4_000_000, 0, 9999);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_validate_rejects_start_time_mismatch() {
        let own_pid = std::process::id();
        let mut sys = System::new();
        sys.refresh_processes(ProcessesToUpdate::All, true);

        let sysinfo_pid = sysinfo::Pid::from_u32(own_pid);
        let real_start_time = sys
            .process(sysinfo_pid)
            .map(|p| p.start_time() as i64)
            .expect("test process must be visible to sysinfo");

        let wrong_start_time = real_start_time + 9999;
        let fake_own = own_pid + 999_999;

        let result = validate_pid_in_system(&sys, own_pid, wrong_start_time, fake_own);

        assert!(result.is_err(), "mismatched start_time must be rejected");
        let err_msg = result.unwrap_err();
        assert!(
            err_msg.contains("recycled"),
            "error message must mention PID recycling, got: {err_msg}"
        );
        assert!(err_msg.contains(&own_pid.to_string()));
        assert!(err_msg.contains(&wrong_start_time.to_string()));
    }
}
