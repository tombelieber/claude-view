use super::types::{ClassifiedProcess, Staleness};

/// Compute staleness for a process.
pub(super) fn compute_staleness(cpu_percent: f32, ppid: u32, uptime_secs: u64) -> Staleness {
    if cpu_percent > 0.1 {
        return Staleness::Active;
    }
    if uptime_secs < 60 {
        return Staleness::Active;
    }
    if ppid != 1 {
        return Staleness::Idle;
    }
    if uptime_secs < 300 {
        return Staleness::Idle;
    }
    Staleness::LikelyStale
}

/// Aggregate descendant_count, descendant_cpu, descendant_memory for a list of children.
pub(super) fn aggregate_descendants(descendants: &[ClassifiedProcess]) -> (u32, f32, u64) {
    descendants
        .iter()
        .fold((0, 0.0, 0), |(count, cpu, mem), d| {
            (
                count + 1 + d.descendant_count,
                cpu + d.cpu_percent + d.descendant_cpu,
                mem + d.memory_bytes + d.descendant_memory,
            )
        })
}

/// Truncate command string to 512 chars AFTER classification.
/// The `…` ellipsis is 3 bytes, so we slice to 509 bytes to keep total ≤ 512 bytes.
pub(super) fn truncate_command(cmd: &str) -> String {
    if cmd.len() <= 512 {
        cmd.to_string()
    } else {
        format!("{}…", &cmd[..509])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::process_tree::types::ProcessCategory;

    fn minimal_child(
        cpu: f32,
        mem: u64,
        desc_count: u32,
        desc_cpu: f32,
        desc_mem: u64,
    ) -> ClassifiedProcess {
        ClassifiedProcess {
            pid: 0,
            ppid: 0,
            name: String::new(),
            command: String::new(),
            category: ProcessCategory::ChildProcess,
            ecosystem_tag: None,
            cpu_percent: cpu,
            memory_bytes: mem,
            uptime_secs: 0,
            start_time: 0,
            is_unparented: false,
            staleness: Staleness::Active,
            descendant_count: desc_count,
            descendant_cpu: desc_cpu,
            descendant_memory: desc_mem,
            descendants: vec![],
            is_self: false,
        }
    }

    #[test]
    fn staleness_active_high_cpu() {
        assert!(matches!(compute_staleness(5.0, 1, 7200), Staleness::Active));
    }

    #[test]
    fn staleness_active_young_process() {
        assert!(matches!(compute_staleness(0.0, 1, 30), Staleness::Active));
    }

    #[test]
    fn staleness_idle_has_parent() {
        assert!(matches!(compute_staleness(0.0, 500, 600), Staleness::Idle));
    }

    #[test]
    fn staleness_idle_unparented_but_young() {
        assert!(matches!(compute_staleness(0.0, 1, 120), Staleness::Idle));
    }

    #[test]
    fn staleness_likely_stale() {
        assert!(matches!(
            compute_staleness(0.0, 1, 600),
            Staleness::LikelyStale
        ));
    }

    #[test]
    fn aggregate_empty() {
        assert_eq!(aggregate_descendants(&[]), (0, 0.0, 0));
    }

    #[test]
    fn aggregate_with_nested_descendants() {
        let children = vec![
            minimal_child(10.0, 100, 2, 5.0, 50),
            minimal_child(20.0, 200, 0, 0.0, 0),
        ];
        let (count, cpu, mem) = aggregate_descendants(&children);
        assert_eq!(count, 4);
        assert!((cpu - 35.0).abs() < 0.01);
        assert_eq!(mem, 350);
    }

    #[test]
    fn truncate_short_command() {
        let cmd = "ls -la";
        assert_eq!(truncate_command(cmd), "ls -la");
    }

    #[test]
    fn truncate_long_command() {
        let cmd = "x".repeat(600);
        let result = truncate_command(&cmd);
        assert!(result.len() < 520);
        assert!(result.ends_with('…'));
    }

    #[test]
    fn test_truncate_command_at_boundary() {
        let exactly_512 = "x".repeat(512);
        let result = truncate_command(&exactly_512);
        assert_eq!(result.len(), 512, "exactly 512 chars must not be truncated");
        assert!(!result.contains('…'), "no ellipsis when at exact boundary");

        let exactly_513 = "y".repeat(513);
        let result = truncate_command(&exactly_513);
        assert!(
            result.ends_with('…'),
            "must end with ellipsis when over 512 chars"
        );
        // 509 bytes of content + 3 bytes for '…' = 512 bytes total (≤ 512 cap)
        assert_eq!(&result[..509], &exactly_513[..509]);
        assert!(
            result.len() <= 512,
            "truncated result must not exceed 512 bytes"
        );
    }

    #[test]
    fn test_truncate_command_empty() {
        let result = truncate_command("");
        assert_eq!(result, "", "empty input must produce empty output");
    }

    #[test]
    fn test_staleness_boundary_59s_active() {
        let staleness = compute_staleness(0.0, 1, 59);
        assert!(matches!(staleness, Staleness::Active));
    }

    #[test]
    fn test_staleness_boundary_60s_with_parent_idle() {
        let staleness = compute_staleness(0.0, 500, 60);
        assert!(matches!(staleness, Staleness::Idle));
    }

    #[test]
    fn test_staleness_boundary_299s_orphan_idle() {
        let staleness = compute_staleness(0.0, 1, 299);
        assert!(matches!(staleness, Staleness::Idle));
    }

    #[test]
    fn test_staleness_boundary_300s_orphan_stale() {
        let staleness = compute_staleness(0.0, 1, 300);
        assert!(matches!(staleness, Staleness::LikelyStale));
    }

    #[test]
    fn test_staleness_high_cpu_overrides_orphan() {
        let staleness = compute_staleness(0.2, 1, 7200);
        assert!(matches!(staleness, Staleness::Active));
    }

}
