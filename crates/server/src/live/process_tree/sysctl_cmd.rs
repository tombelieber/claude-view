//! Direct sysctl(KERN_PROCARGS2) for process command lines.
//!
//! Replaces `ps -p ... -o command=` subprocess. Same kernel API that
//! sysinfo and ps both use internally. ~10us per PID vs ~20ms for fork+exec.
//!
//! Permission: same-user or root. Claude processes are same-user, so this
//! works without elevation.

use std::collections::HashMap;

/// Get full command line for a single PID via sysctl(KERN_PROCARGS2).
/// Returns None if PID doesn't exist or is owned by another user.
pub fn get_command(pid: u32) -> Option<String> {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = pid;
        None
    }

    #[cfg(target_os = "macos")]
    {
        get_command_sysctl(pid)
    }
}

/// Batch command resolution: calls sysctl per PID (no subprocess).
/// ~10us per PID. For 50 PIDs: ~500us total vs ~20ms for one `ps` subprocess.
pub fn batch_get_command(pids: &[u32]) -> HashMap<u32, String> {
    pids.iter()
        .filter_map(|&pid| get_command(pid).map(|cmd| (pid, cmd)))
        .collect()
}

#[cfg(target_os = "macos")]
fn get_command_sysctl(pid: u32) -> Option<String> {
    use std::mem;

    unsafe {
        let mut mib: [libc::c_int; 3] = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid as libc::c_int];

        // First call: get buffer size
        let mut size: libc::size_t = 0;
        let ret = libc::sysctl(
            mib.as_mut_ptr(),
            3,
            std::ptr::null_mut(),
            &mut size,
            std::ptr::null_mut(),
            0,
        );
        if ret != 0 || size == 0 {
            return None;
        }

        // Second call: get actual data
        let mut buf: Vec<u8> = vec![0u8; size];
        let ret = libc::sysctl(
            mib.as_mut_ptr(),
            3,
            buf.as_mut_ptr() as *mut libc::c_void,
            &mut size,
            std::ptr::null_mut(),
            0,
        );
        if ret != 0 {
            return None;
        }
        buf.truncate(size);

        // Parse KERN_PROCARGS2 format:
        // [4 bytes: argc (i32)] [exec_path\0] [padding\0*] [argv[0]\0] [argv[1]\0] ...
        if buf.len() < mem::size_of::<i32>() {
            return None;
        }

        let argc = i32::from_ne_bytes(buf[..4].try_into().ok()?) as usize;
        if argc == 0 {
            return None;
        }

        // Skip past argc + exec_path (null-terminated)
        let mut pos = 4;
        while pos < buf.len() && buf[pos] != 0 {
            pos += 1;
        }
        // Skip null terminators (padding between exec_path and argv[0])
        while pos < buf.len() && buf[pos] == 0 {
            pos += 1;
        }

        // Collect argc arguments
        let mut args = Vec::with_capacity(argc);
        for _ in 0..argc {
            if pos >= buf.len() {
                break;
            }
            let start = pos;
            while pos < buf.len() && buf[pos] != 0 {
                pos += 1;
            }
            if let Ok(arg) = std::str::from_utf8(&buf[start..pos]) {
                args.push(arg.to_string());
            }
            pos += 1; // skip null terminator
        }

        if args.is_empty() {
            None
        } else {
            Some(args.join(" "))
        }
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn get_command_returns_self_process() {
        let pid = std::process::id();
        let result = get_command(pid);
        assert!(result.is_some(), "should resolve own process command");
        let cmd = result.unwrap();
        assert!(!cmd.is_empty(), "command should not be empty");
    }

    #[test]
    fn get_command_returns_none_for_nonexistent_pid() {
        let result = get_command(4_000_000);
        assert!(result.is_none());
    }

    #[test]
    fn batch_get_command_resolves_self() {
        let pid = std::process::id();
        let results = batch_get_command(&[pid]);
        assert!(results.contains_key(&pid));
        assert!(!results[&pid].is_empty());
    }

    #[test]
    fn batch_get_command_empty_input() {
        let results = batch_get_command(&[]);
        assert!(results.is_empty());
    }

    #[test]
    fn batch_get_command_skips_nonexistent() {
        let pid = std::process::id();
        let results = batch_get_command(&[pid, 4_000_001, 4_000_002]);
        assert!(results.contains_key(&pid));
        assert!(!results.contains_key(&4_000_001));
    }
}
