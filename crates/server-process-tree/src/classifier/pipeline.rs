use std::collections::{HashMap, HashSet};

use super::super::helpers::{aggregate_descendants, compute_staleness, truncate_command};
use super::super::types::{
    ClassifiedProcess, EcosystemTag, ProcessCategory, ProcessTreeSnapshot, ProcessTreeTotals,
    RawProcessInfo,
};
use super::rules::{is_anthropic_claude, is_claude_view_binary, is_node_running_claude};

// =============================================================================
// Pass 2 + 3: Classify and aggregate
// =============================================================================

pub fn classify_process_list(processes: &[RawProcessInfo], own_pid: u32) -> ProcessTreeSnapshot {
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
        } else if proc.pid == own_pid || is_claude_view_binary(name, cmd) {
            Some(EcosystemTag::Self_)
        } else if (name == "claude" && is_anthropic_claude(cmd))
            || is_node_running_claude(name, cmd)
        {
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

                    let child_tag = if raw.command.contains("sidecar/dist/index.js") {
                        Some(EcosystemTag::Sidecar)
                    } else {
                        None
                    };

                    let cp = ClassifiedProcess {
                        pid: raw.pid,
                        ppid: raw.ppid,
                        name: raw.name.clone(),
                        command: truncate_command(&raw.command),
                        category: ProcessCategory::ChildProcess,
                        ecosystem_tag: child_tag,
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
