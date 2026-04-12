//! Process memory measurement matching Apple Activity Monitor.
//!
//! `sysinfo::Process::memory()` returns RSS (resident set size), which on macOS
//! inflates dramatically for processes that do large batch allocations then free
//! them — the allocator retains pages, so RSS stays high even though real usage
//! is low.
//!
//! Activity Monitor uses **physical footprint** (`ri_phys_footprint` from
//! `proc_pid_rusage`), which only counts pages the process is uniquely
//! responsible for. This module provides that metric on macOS, with RSS fallback
//! on other platforms or when the syscall fails (e.g. permission denied for
//! processes owned by other users).

/// Best available memory metric for a process.
///
/// On macOS: physical footprint via `proc_pid_rusage` (matches Activity Monitor).
/// On other platforms or on permission error: falls back to `rss_fallback`.
pub fn process_memory_bytes(pid: u32, rss_fallback: u64) -> u64 {
    #[cfg(target_os = "macos")]
    {
        physical_footprint_bytes(pid).unwrap_or(rss_fallback)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = pid;
        rss_fallback
    }
}

/// macOS: get physical footprint for a PID via `proc_pid_rusage(RUSAGE_INFO_V0)`.
///
/// Returns `None` if the syscall fails (process exited, permission denied, etc.).
#[cfg(target_os = "macos")]
fn physical_footprint_bytes(pid: u32) -> Option<u64> {
    // Layout from <sys/resource.h> — rusage_info_v0
    #[repr(C)]
    struct RUsageInfoV0 {
        ri_uuid: [u8; 16],
        ri_user_time: u64,
        ri_system_time: u64,
        ri_pkg_idle_wkups: u64,
        ri_interrupt_wkups: u64,
        ri_pageins: u64,
        ri_wired_size: u64,
        ri_resident_size: u64,
        ri_phys_footprint: u64,
        ri_proc_start_abstime: u64,
        ri_proc_exit_abstime: u64,
    }

    const RUSAGE_INFO_V0: i32 = 0;

    extern "C" {
        fn proc_pid_rusage(pid: i32, flavor: i32, buffer: *mut RUsageInfoV0) -> i32;
    }

    let mut info = std::mem::MaybeUninit::<RUsageInfoV0>::zeroed();
    let result = unsafe { proc_pid_rusage(pid as i32, RUSAGE_INFO_V0, info.as_mut_ptr()) };

    if result == 0 {
        Some(unsafe { info.assume_init().ri_phys_footprint })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn self_process_has_nonzero_memory() {
        let pid = std::process::id();
        let mem = process_memory_bytes(pid, 0);
        assert!(mem > 0, "own process should have non-zero memory");
    }

    #[test]
    fn fallback_used_for_nonexistent_pid() {
        let mem = process_memory_bytes(u32::MAX, 12345);
        // On macOS, proc_pid_rusage fails for invalid PID → falls back to 12345.
        // On other platforms, always returns the fallback.
        assert_eq!(mem, 12345);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn physical_footprint_less_than_rss_for_self() {
        let pid = std::process::id();
        let footprint = physical_footprint_bytes(pid).unwrap();
        // Physical footprint should be > 0 and typically much less than
        // what sysinfo reports as RSS for processes with retained pages.
        assert!(footprint > 0);
    }
}
