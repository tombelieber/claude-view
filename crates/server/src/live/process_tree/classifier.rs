use std::collections::{HashMap, HashSet};
use sysinfo::System;

use super::helpers::{
    aggregate_descendants, compute_staleness, get_command_via_ps, truncate_command,
};
use super::types::{
    ClassifiedProcess, EcosystemTag, ProcessCategory, ProcessTreeSnapshot, ProcessTreeTotals,
    RawProcessInfo,
};

// =============================================================================
// Pass 1: Collect raw process data
// =============================================================================

pub(super) fn collect_raw_processes(sys: &System, own_pid: u32) -> Vec<RawProcessInfo> {
    let mut result = Vec::new();

    for (pid, process) in sys.processes() {
        let pid_u32 = pid.as_u32();
        let ppid = process.parent().map(|p| p.as_u32()).unwrap_or(0);
        let name = process.name().to_string_lossy().to_string();

        let mut command: String = process
            .cmd()
            .iter()
            .map(|s| s.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ");

        // macOS fallback: sysinfo may return empty cmd for SIP-restricted processes.
        if command.is_empty()
            && (name.to_ascii_lowercase().contains("claude") || pid_u32 == own_pid)
        {
            if let Some(full_cmd) = get_command_via_ps(pid_u32) {
                command = full_cmd;
            }
        }

        let start_time = process.start_time() as i64;

        result.push(RawProcessInfo {
            pid: pid_u32,
            ppid,
            name,
            command,
            cpu_percent: process.cpu_usage(),
            memory_bytes: process.memory(),
            start_time,
        });
    }

    result
}

// =============================================================================
// Pass 2 + 3: Classify and aggregate
// =============================================================================

pub(super) fn classify_process_list(
    processes: &[RawProcessInfo],
    own_pid: u32,
) -> ProcessTreeSnapshot {
    let now_secs = chrono::Utc::now().timestamp().max(0) as u64;

    let mut ecosystem_pids: HashSet<u32> = HashSet::new();
    let mut classified: HashMap<u32, ClassifiedProcess> = HashMap::new();
    let pid_to_raw: HashMap<u32, &RawProcessInfo> = processes.iter().map(|p| (p.pid, p)).collect();

    // ORDERING IS CRITICAL: rules 1-3 are specific path matches that must fire
    // before rule 4 (broad name == "claude" check).
    for proc in processes {
        let cmd = &proc.command;
        let name = &proc.name;

        let tag = if cmd.contains(".vscode/extensions/anthropic.claude-code") {
            Some(EcosystemTag::Ide)
        } else if cmd.contains("Claude.app/Contents") {
            Some(EcosystemTag::Desktop)
        } else if cmd.contains("claude-view") || proc.pid == own_pid {
            Some(EcosystemTag::Self_)
        } else if name == "claude" {
            Some(EcosystemTag::Cli)
        } else {
            None
        };

        if let Some(tag) = tag {
            let is_unparented = proc.ppid == 1;
            let start_time_u64 = proc.start_time.max(0) as u64;
            let uptime_secs = now_secs.saturating_sub(start_time_u64);
            let staleness = compute_staleness(proc.cpu_percent, proc.ppid, uptime_secs);

            let cp = ClassifiedProcess {
                pid: proc.pid,
                ppid: proc.ppid,
                name: proc.name.clone(),
                command: truncate_command(&proc.command),
                category: ProcessCategory::ClaudeEcosystem,
                ecosystem_tag: Some(tag),
                cpu_percent: proc.cpu_percent,
                memory_bytes: proc.memory_bytes,
                uptime_secs,
                start_time: proc.start_time,
                is_unparented,
                staleness,
                descendant_count: 0,
                descendant_cpu: 0.0,
                descendant_memory: 0,
                descendants: vec![],
                is_self: proc.pid == own_pid,
            };
            ecosystem_pids.insert(proc.pid);
            classified.insert(proc.pid, cp);
        }
    }

    // Second sub-pass: identify child processes (rules 5-6).
    let mut parent_to_children: HashMap<u32, Vec<u32>> = HashMap::new();
    for proc in processes {
        if !classified.contains_key(&proc.pid) {
            parent_to_children
                .entry(proc.ppid)
                .or_default()
                .push(proc.pid);
        }
    }

    let ecosystem_pid_list: Vec<u32> = ecosystem_pids.iter().copied().collect();
    let mut child_classified: HashMap<u32, ClassifiedProcess> = HashMap::new();

    for &eco_pid in &ecosystem_pid_list {
        if let Some(direct_children) = parent_to_children.get(&eco_pid) {
            for &child_pid in direct_children {
                if let Some(raw) = pid_to_raw.get(&child_pid) {
                    let start_time_u64 = raw.start_time.max(0) as u64;
                    let uptime_secs = now_secs.saturating_sub(start_time_u64);
                    let staleness = compute_staleness(raw.cpu_percent, raw.ppid, uptime_secs);
                    let descendants = build_descendants(
                        child_pid,
                        &parent_to_children,
                        &pid_to_raw,
                        now_secs,
                        own_pid,
                    );
                    let (desc_count, desc_cpu, desc_mem) = aggregate_descendants(&descendants);

                    let cp = ClassifiedProcess {
                        pid: raw.pid,
                        ppid: raw.ppid,
                        name: raw.name.clone(),
                        command: truncate_command(&raw.command),
                        category: ProcessCategory::ChildProcess,
                        ecosystem_tag: None,
                        cpu_percent: raw.cpu_percent,
                        memory_bytes: raw.memory_bytes,
                        uptime_secs,
                        start_time: raw.start_time,
                        is_unparented: raw.ppid == 1,
                        staleness,
                        descendant_count: desc_count,
                        descendant_cpu: desc_cpu,
                        descendant_memory: desc_mem,
                        descendants,
                        is_self: raw.pid == own_pid,
                    };
                    child_classified.insert(child_pid, cp);
                }
            }
        }
    }

    // Pass 3: Aggregate
    let mut ecosystem: Vec<ClassifiedProcess> = Vec::new();
    let mut children: Vec<ClassifiedProcess> = Vec::new();
    let mut totals = ProcessTreeTotals {
        ecosystem_cpu: 0.0,
        ecosystem_memory: 0,
        ecosystem_count: 0,
        child_cpu: 0.0,
        child_memory: 0,
        child_count: 0,
        unparented_count: 0,
        unparented_memory: 0,
    };

    for &eco_pid in &ecosystem_pid_list {
        if let Some(mut eco_proc) = classified.remove(&eco_pid) {
            let direct_child_pids: Vec<u32> = child_classified
                .keys()
                .filter(|&&pid| {
                    pid_to_raw
                        .get(&pid)
                        .map(|r| r.ppid == eco_pid)
                        .unwrap_or(false)
                })
                .copied()
                .collect();

            let mut desc_count = 0u32;
            let mut desc_cpu = 0.0f32;
            let mut desc_mem = 0u64;
            let mut direct_descendants = Vec::new();

            for &cpid in &direct_child_pids {
                if let Some(child) = child_classified.remove(&cpid) {
                    desc_count += 1 + child.descendant_count;
                    desc_cpu += child.cpu_percent + child.descendant_cpu;
                    desc_mem += child.memory_bytes + child.descendant_memory;
                    direct_descendants.push(child);
                }
            }

            eco_proc.descendant_count = desc_count;
            eco_proc.descendant_cpu = desc_cpu;
            eco_proc.descendant_memory = desc_mem;
            eco_proc.descendants = direct_descendants;

            totals.ecosystem_count += 1;
            totals.ecosystem_cpu += eco_proc.cpu_percent;
            totals.ecosystem_memory += eco_proc.memory_bytes;
            if eco_proc.is_unparented {
                totals.unparented_count += 1;
                totals.unparented_memory += eco_proc.memory_bytes;
            }

            ecosystem.push(eco_proc);
        }
    }

    for (_, child) in child_classified {
        totals.child_count += 1;
        totals.child_cpu += child.cpu_percent;
        totals.child_memory += child.memory_bytes;
        if child.is_unparented {
            totals.unparented_count += 1;
            totals.unparented_memory += child.memory_bytes;
        }
        children.push(child);
    }

    // Sort: MUST use total_cmp, NEVER partial_cmp (project hard rule -- NaN panics)
    ecosystem.sort_by(|a, b| b.cpu_percent.total_cmp(&a.cpu_percent));
    children.sort_by(|a, b| {
        a.ppid
            .cmp(&b.ppid) // ascending: group children by parent PID
            .then_with(|| b.cpu_percent.total_cmp(&a.cpu_percent))
    });

    ProcessTreeSnapshot {
        timestamp: chrono::Utc::now().timestamp(),
        ecosystem,
        children,
        totals,
    }
}

fn build_descendants(
    parent_pid: u32,
    parent_to_children: &HashMap<u32, Vec<u32>>,
    pid_to_raw: &HashMap<u32, &RawProcessInfo>,
    now_secs: u64,
    own_pid: u32,
) -> Vec<ClassifiedProcess> {
    let Some(child_pids) = parent_to_children.get(&parent_pid) else {
        return vec![];
    };

    child_pids
        .iter()
        .filter_map(|&cpid| {
            let raw = pid_to_raw.get(&cpid)?;
            let start_time_u64 = raw.start_time.max(0) as u64;
            let uptime_secs = now_secs.saturating_sub(start_time_u64);
            let staleness = compute_staleness(raw.cpu_percent, raw.ppid, uptime_secs);
            let descendants =
                build_descendants(cpid, parent_to_children, pid_to_raw, now_secs, own_pid);
            let (desc_count, desc_cpu, desc_mem) = aggregate_descendants(&descendants);

            Some(ClassifiedProcess {
                pid: raw.pid,
                ppid: raw.ppid,
                name: raw.name.clone(),
                command: truncate_command(&raw.command),
                category: ProcessCategory::ChildProcess,
                ecosystem_tag: None,
                cpu_percent: raw.cpu_percent,
                memory_bytes: raw.memory_bytes,
                uptime_secs,
                start_time: raw.start_time,
                is_unparented: raw.ppid == 1,
                staleness,
                descendant_count: desc_count,
                descendant_cpu: desc_cpu,
                descendant_memory: desc_mem,
                descendants,
                is_self: raw.pid == own_pid,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::process_tree::types::Staleness;

    fn make_raw(
        pid: u32,
        ppid: u32,
        name: &str,
        command: &str,
        cpu: f32,
        mem: u64,
        start_time: i64,
    ) -> RawProcessInfo {
        RawProcessInfo {
            pid,
            ppid,
            name: name.into(),
            command: command.into(),
            cpu_percent: cpu,
            memory_bytes: mem,
            start_time,
        }
    }

    #[test]
    fn classify_cli_process() {
        let processes = vec![make_raw(
            100,
            99,
            "claude",
            "/usr/local/bin/claude --verbose",
            5.0,
            200_000_000,
            1_700_000_000,
        )];
        let snap = classify_process_list(&processes, 9999);
        assert_eq!(snap.ecosystem.len(), 1);
        let proc = &snap.ecosystem[0];
        assert!(matches!(proc.ecosystem_tag, Some(EcosystemTag::Cli)));
        assert_eq!(proc.pid, 100);
    }

    #[test]
    fn classify_ide_process() {
        let processes = vec![make_raw(
            200,
            150,
            "node",
            "/Users/test/.vscode/extensions/anthropic.claude-code-1.0.0/node_modules/.bin/claude",
            2.0,
            100_000_000,
            1_700_000_000,
        )];
        let snap = classify_process_list(&processes, 9999);
        assert_eq!(snap.ecosystem.len(), 1);
        assert!(matches!(
            snap.ecosystem[0].ecosystem_tag,
            Some(EcosystemTag::Ide)
        ));
    }

    #[test]
    fn classify_desktop_process() {
        let processes = vec![make_raw(
            300,
            1,
            "Claude",
            "/Applications/Claude.app/Contents/MacOS/Claude",
            1.0,
            500_000_000,
            1_700_000_000,
        )];
        let snap = classify_process_list(&processes, 9999);
        assert_eq!(snap.ecosystem.len(), 1);
        assert!(matches!(
            snap.ecosystem[0].ecosystem_tag,
            Some(EcosystemTag::Desktop)
        ));
    }

    #[test]
    fn classify_self_process() {
        let own_pid = 777u32;
        let processes = vec![make_raw(
            own_pid,
            1,
            "claude-view",
            "/Users/test/.cargo/bin/claude-view serve",
            0.5,
            50_000_000,
            1_700_000_000,
        )];
        let snap = classify_process_list(&processes, own_pid);
        assert_eq!(snap.ecosystem.len(), 1);
        let proc = &snap.ecosystem[0];
        assert!(matches!(proc.ecosystem_tag, Some(EcosystemTag::Self_)));
        assert!(proc.is_self);
    }

    #[test]
    fn classifier_ordering_claude_view_not_cli() {
        let own_pid = 888u32;
        let processes = vec![make_raw(
            own_pid,
            1,
            "claude-view",
            "/Users/test/.local/bin/claude-view",
            0.1,
            10_000_000,
            1_700_000_000,
        )];
        let snap = classify_process_list(&processes, own_pid);
        assert_eq!(snap.ecosystem.len(), 1);
        assert!(matches!(
            snap.ecosystem[0].ecosystem_tag,
            Some(EcosystemTag::Self_)
        ));
        assert!(!matches!(
            snap.ecosystem[0].ecosystem_tag,
            Some(EcosystemTag::Cli)
        ));
    }

    #[test]
    fn unparented_detection() {
        let processes = vec![make_raw(
            501,
            1,
            "claude",
            "/usr/local/bin/claude",
            0.0,
            5_000_000,
            1_700_000_000,
        )];
        let snap = classify_process_list(&processes, 9999);
        assert_eq!(snap.ecosystem.len(), 1);
        assert!(snap.ecosystem[0].is_unparented);
    }

    #[test]
    fn staleness_active_high_cpu() {
        let now = chrono::Utc::now().timestamp();
        let processes = vec![make_raw(
            600,
            1,
            "claude",
            "/usr/local/bin/claude",
            5.0,
            100_000_000,
            now - 7200,
        )];
        let snap = classify_process_list(&processes, 9999);
        assert!(matches!(snap.ecosystem[0].staleness, Staleness::Active));
    }

    #[test]
    fn staleness_likely_stale() {
        let now = chrono::Utc::now().timestamp();
        let processes = vec![make_raw(
            700,
            1,
            "claude",
            "/usr/local/bin/claude",
            0.0,
            10_000_000,
            now - 600,
        )];
        let snap = classify_process_list(&processes, 9999);
        let proc = &snap.ecosystem[0];
        assert!(proc.is_unparented);
        assert!(matches!(proc.staleness, Staleness::LikelyStale));
    }

    #[test]
    fn staleness_idle_has_parent() {
        let now = chrono::Utc::now().timestamp();
        let processes = vec![make_raw(
            800,
            500,
            "claude",
            "/usr/local/bin/claude",
            0.0,
            10_000_000,
            now - 600,
        )];
        let snap = classify_process_list(&processes, 9999);
        assert!(matches!(snap.ecosystem[0].staleness, Staleness::Idle));
    }

    #[test]
    fn descendant_aggregation() {
        let processes = vec![
            make_raw(
                1000,
                1,
                "claude",
                "/usr/local/bin/claude",
                5.0,
                200_000_000,
                1_700_000_000,
            ),
            make_raw(
                1001,
                1000,
                "node",
                "/usr/local/bin/node server.js",
                10.0,
                100_000_000,
                1_700_000_001,
            ),
            make_raw(
                1002,
                1000,
                "cargo",
                "/usr/local/bin/cargo build",
                50.0,
                300_000_000,
                1_700_000_002,
            ),
            make_raw(
                1003,
                1000,
                "bun",
                "/usr/local/bin/bun run dev",
                15.0,
                150_000_000,
                1_700_000_003,
            ),
        ];
        let snap = classify_process_list(&processes, 9999);

        assert_eq!(snap.ecosystem.len(), 1);
        // Children are now nested inside ecosystem descendants (not flat)
        assert_eq!(snap.children.len(), 0);

        let parent = &snap.ecosystem[0];
        assert_eq!(parent.descendant_count, 3);
        assert!((parent.descendant_cpu - 75.0).abs() < 0.1);
        assert_eq!(parent.descendant_memory, 550_000_000);
        // Direct children are accessible via descendants
        assert_eq!(parent.descendants.len(), 3);
        let child_names: Vec<&str> = parent.descendants.iter().map(|c| c.name.as_str()).collect();
        assert!(child_names.contains(&"node"));
        assert!(child_names.contains(&"cargo"));
        assert!(child_names.contains(&"bun"));
    }

    #[test]
    fn totals_aggregation() {
        let processes = vec![
            make_raw(
                2000,
                99,
                "claude",
                "/usr/local/bin/claude",
                10.0,
                400_000_000,
                1_700_000_000,
            ),
            make_raw(
                2001,
                2000,
                "node",
                "/usr/local/bin/node",
                5.0,
                100_000_000,
                1_700_000_001,
            ),
        ];
        let snap = classify_process_list(&processes, 9999);
        assert_eq!(snap.totals.ecosystem_count, 1);
        assert!((snap.totals.ecosystem_cpu - 10.0).abs() < 0.1);
        assert_eq!(snap.totals.ecosystem_memory, 400_000_000);
        // Children are now nested inside ecosystem descendants
        assert_eq!(snap.totals.child_count, 0);
        assert_eq!(snap.ecosystem[0].descendants.len(), 1);
        assert_eq!(snap.ecosystem[0].descendants[0].name, "node");
    }

    #[test]
    fn test_classify_empty_process_list() {
        let processes: Vec<RawProcessInfo> = vec![];
        let snap = classify_process_list(&processes, 9999);

        assert!(snap.ecosystem.is_empty());
        assert!(snap.children.is_empty());
        assert_eq!(snap.totals.ecosystem_count, 0);
        assert!((snap.totals.ecosystem_cpu - 0.0).abs() < f32::EPSILON);
        assert_eq!(snap.totals.ecosystem_memory, 0);
        assert_eq!(snap.totals.child_count, 0);
        assert_eq!(snap.totals.unparented_count, 0);
        assert!(snap.timestamp > 0);
    }

    #[test]
    fn test_classify_nan_cpu_does_not_panic() {
        let processes = vec![
            make_raw(
                100,
                99,
                "claude",
                "/usr/local/bin/claude",
                f32::NAN,
                200_000_000,
                1_700_000_000,
            ),
            make_raw(
                200,
                99,
                "claude",
                "/usr/local/bin/claude --verbose",
                5.0,
                100_000_000,
                1_700_000_000,
            ),
            make_raw(
                300,
                99,
                "claude",
                "/usr/local/bin/claude chat",
                f32::NAN,
                50_000_000,
                1_700_000_000,
            ),
        ];
        let snap = classify_process_list(&processes, 9999);
        assert_eq!(snap.ecosystem.len(), 3);
        let cpu_values: Vec<f32> = snap.ecosystem.iter().map(|p| p.cpu_percent).collect();
        assert_eq!(cpu_values.len(), 3);
    }

    #[test]
    fn test_is_self_flag_set_for_real_own_pid() {
        let real_own_pid = std::process::id();
        let processes = vec![make_raw(
            real_own_pid,
            1,
            "claude-view",
            "/Users/test/.cargo/bin/claude-view serve",
            0.5,
            50_000_000,
            chrono::Utc::now().timestamp() - 100,
        )];
        let snap = classify_process_list(&processes, real_own_pid);
        assert_eq!(snap.ecosystem.len(), 1);
        assert!(
            snap.ecosystem[0].is_self,
            "is_self must be true for PID {} (the real server PID)",
            real_own_pid
        );
    }

    #[test]
    fn test_classifier_does_not_include_non_claude_processes() {
        let processes = vec![
            make_raw(
                100,
                1,
                "claude",
                "/usr/local/bin/claude",
                5.0,
                200_000_000,
                1_700_000_000,
            ),
            make_raw(200, 1, "bash", "/bin/bash", 0.1, 5_000_000, 1_700_000_000),
            make_raw(201, 1, "zsh", "/bin/zsh", 0.1, 5_000_000, 1_700_000_000),
            make_raw(
                202,
                1,
                "Finder",
                "/System/Library/CoreServices/Finder.app/Contents/MacOS/Finder",
                1.0,
                50_000_000,
                1_700_000_000,
            ),
            make_raw(
                203,
                1,
                "loginwindow",
                "/System/Library/CoreServices/loginwindow.app/Contents/MacOS/loginwindow",
                0.0,
                20_000_000,
                1_700_000_000,
            ),
            make_raw(
                204,
                1,
                "node",
                "/usr/local/bin/node server.js",
                10.0,
                300_000_000,
                1_700_000_000,
            ),
            make_raw(
                205,
                1,
                "cargo",
                "/usr/local/bin/cargo build",
                25.0,
                500_000_000,
                1_700_000_000,
            ),
            make_raw(
                206,
                1,
                "WindowServer",
                "/System/Library/PrivateFrameworks/SkyLight.framework/..",
                3.0,
                100_000_000,
                1_700_000_000,
            ),
        ];
        let snap = classify_process_list(&processes, 9999);

        assert_eq!(
            snap.ecosystem.len(),
            1,
            "only 'claude' should be in ecosystem, got: {:?}",
            snap.ecosystem.iter().map(|p| &p.name).collect::<Vec<_>>()
        );
        assert_eq!(snap.ecosystem[0].pid, 100);

        assert!(
            snap.children.is_empty(),
            "system processes with PPID=1 must not appear as children, got: {:?}",
            snap.children.iter().map(|p| &p.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_classifier_child_requires_ecosystem_parent() {
        let processes = vec![
            make_raw(
                100,
                1,
                "claude",
                "/usr/local/bin/claude",
                5.0,
                200_000_000,
                1_700_000_000,
            ),
            make_raw(
                101,
                100,
                "node",
                "/usr/local/bin/node dev-server.js",
                10.0,
                100_000_000,
                1_700_000_001,
            ),
            make_raw(
                202,
                1,
                "node",
                "/usr/local/bin/node unrelated-server.js",
                10.0,
                100_000_000,
                1_700_000_001,
            ),
        ];
        let snap = classify_process_list(&processes, 9999);

        assert_eq!(snap.ecosystem.len(), 1);
        // PID 101 (PPID=100) is now nested inside ecosystem[0].descendants
        assert_eq!(snap.children.len(), 0);
        assert_eq!(
            snap.ecosystem[0].descendants.len(),
            1,
            "only PID 101 (PPID=100) should be a descendant of claude, got: {:?}",
            snap.ecosystem[0]
                .descendants
                .iter()
                .map(|p| (p.pid, p.ppid))
                .collect::<Vec<_>>()
        );
        assert_eq!(snap.ecosystem[0].descendants[0].pid, 101);
    }
}
