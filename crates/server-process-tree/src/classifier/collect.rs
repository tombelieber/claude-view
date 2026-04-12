use sysinfo::System;

use super::super::sysctl_cmd;
use super::super::types::RawProcessInfo;

// =============================================================================
// Pass 1: Collect raw process data
// =============================================================================

pub fn collect_raw_processes(sys: &System, own_pid: u32) -> Vec<RawProcessInfo> {
    // Pass 1: collect all processes, track which need ps fallback.
    struct Partial {
        pid: u32,
        ppid: u32,
        name: String,
        command: String,
        cpu_percent: f32,
        memory_bytes: u64,
        start_time: i64,
        needs_ps: bool,
    }
    let mut partials = Vec::new();
    let mut need_ps: Vec<u32> = Vec::new();

    for (pid, process) in sys.processes() {
        let pid_u32 = pid.as_u32();
        let ppid = process.parent().map(|p| p.as_u32()).unwrap_or(0);
        let name = process.name().to_string_lossy().to_string();

        let command: String = process
            .cmd()
            .iter()
            .map(|s| s.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ");

        // macOS fallback: sysinfo may return empty cmd for SIP-restricted processes.
        let needs_ps = command.is_empty()
            && (name.to_ascii_lowercase().contains("claude") || pid_u32 == own_pid);
        if needs_ps {
            need_ps.push(pid_u32);
        }

        partials.push(Partial {
            pid: pid_u32,
            ppid,
            name,
            command,
            cpu_percent: process.cpu_usage(),
            memory_bytes: crate::proc_memory::process_memory_bytes(pid_u32, process.memory()),
            start_time: process.start_time() as i64,
            needs_ps,
        });
    }

    // Pass 2: resolve commands via sysctl(KERN_PROCARGS2) for all PIDs that need it.
    let ps_results = if need_ps.is_empty() {
        std::collections::HashMap::new()
    } else {
        sysctl_cmd::batch_get_command(&need_ps)
    };

    // Pass 3: assemble with resolved commands.
    partials
        .into_iter()
        .map(|p| {
            let command = if p.needs_ps {
                ps_results.get(&p.pid).cloned().unwrap_or(p.command)
            } else {
                p.command
            };
            RawProcessInfo {
                pid: p.pid,
                ppid: p.ppid,
                name: p.name,
                command,
                cpu_percent: p.cpu_percent,
                memory_bytes: p.memory_bytes,
                start_time: p.start_time,
            }
        })
        .collect()
}
