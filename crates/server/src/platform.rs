//! Cross-platform process termination.
//!
//! Routes through `sysinfo` instead of raw `libc::kill` so the server builds and
//! runs on Windows, where there is no `kill(2)`.
//!
//! ponytail: Windows has no SIGTERM. `Process::kill_with(Signal::Term)` returns
//! `None` there, so graceful termination degrades to the hard kill
//! (`TerminateProcess`) — the only termination Windows offers. Unix behaviour is
//! unchanged: SIGTERM for graceful, SIGKILL for forced.

use sysinfo::{Pid, ProcessesToUpdate, Signal, System};

/// Terminate `pid` using an already-refreshed `System`.
pub(crate) fn terminate_in(sys: &System, pid: u32, force: bool) -> Result<(), String> {
    let Some(proc) = sys.process(Pid::from_u32(pid)) else {
        return Err(format!("pid {pid} not found"));
    };
    let killed = if force {
        proc.kill()
    } else {
        proc.kill_with(Signal::Term).unwrap_or_else(|| proc.kill())
    };
    if killed {
        Ok(())
    } else {
        Err(format!(
            "{} failed for pid {pid}",
            if force { "kill" } else { "terminate" }
        ))
    }
}

/// Terminate `pid`, refreshing only that process first.
pub(crate) fn terminate_pid(pid: u32, force: bool) -> Result<(), String> {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::Some(&[Pid::from_u32(pid)]), true);
    terminate_in(&sys, pid, force)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_pid_reports_not_found() {
        // PID 0 is never a killable user process on any supported platform.
        let err = terminate_pid(0, false).unwrap_err();
        assert!(err.contains("not found"), "unexpected error: {err}");
    }

    #[test]
    fn terminates_a_real_child_process() {
        let mut child = if cfg!(windows) {
            std::process::Command::new("cmd")
                .args(["/C", "ping -n 30 127.0.0.1 > NUL"])
                .spawn()
                .expect("spawn child")
        } else {
            std::process::Command::new("sleep")
                .arg("30")
                .spawn()
                .expect("spawn child")
        };

        terminate_pid(child.id(), true).expect("terminate should succeed");

        // The child must actually die, not just report success.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                _ if std::time::Instant::now() >= deadline => {
                    let _ = child.kill();
                    panic!("child survived terminate_pid");
                }
                _ => std::thread::sleep(std::time::Duration::from_millis(50)),
            }
        }
    }
}
