use super::types::{ProcessTreeSnapshot, ProcessTreeTotals, RawProcessInfo};
use sysinfo::System;

pub(super) fn collect_raw_processes(_sys: &System, _own_pid: u32) -> Vec<RawProcessInfo> {
    vec![]
}

pub(super) fn classify_process_list(
    _processes: &[RawProcessInfo],
    _own_pid: u32,
) -> ProcessTreeSnapshot {
    ProcessTreeSnapshot {
        timestamp: 0,
        ecosystem: vec![],
        children: vec![],
        totals: ProcessTreeTotals {
            ecosystem_cpu: 0.0,
            ecosystem_memory: 0,
            ecosystem_count: 0,
            child_cpu: 0.0,
            child_memory: 0,
            child_count: 0,
            unparented_count: 0,
            unparented_memory: 0,
        },
    }
}
