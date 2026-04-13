//! Integration test: verify disk total_bytes is sane (not double-counted).
//!
//! On macOS APFS, sysinfo returns multiple browsable volumes sharing one
//! container. The dedup logic in monitor.rs must ensure total_bytes ≤
//! the physical disk size (not 2x or 3x due to APFS volume groups).

use claude_view_server::live::monitor::collect_snapshot;
use claude_view_server::live::state::LiveSession;
use std::collections::HashMap;
use sysinfo::System;

#[test]
fn disk_total_bytes_not_inflated() {
    let mut sys = System::new_all();
    std::thread::sleep(std::time::Duration::from_millis(200));
    let sessions: HashMap<String, LiveSession> = HashMap::new();
    let snap = collect_snapshot(&mut sys, &sessions);

    // collect_snapshot always does a full refresh → disk fields are Some
    let disk_total = snap
        .disk_total_bytes
        .expect("full snapshot must have disk_total_bytes");
    let disk_used = snap
        .disk_used_bytes
        .expect("full snapshot must have disk_used_bytes");

    // Sanity: total must be > 0
    assert!(disk_total > 0, "disk_total_bytes must be non-zero");

    // used ≤ total (basic invariant)
    assert!(
        disk_used <= disk_total,
        "disk_used_bytes ({}) must not exceed disk_total_bytes ({})",
        disk_used,
        disk_total,
    );

    // Regression guard: on a typical machine, total should be < 20 TB.
    // If dedup fails, APFS double-counting inflates this to 2-4x the real size.
    // 20 TB is generous enough for any real workstation.
    let twenty_tb = 20_u64 * 1024 * 1024 * 1024 * 1024;
    assert!(
        disk_total < twenty_tb,
        "disk_total_bytes ({} bytes = {} TB) looks inflated — dedup may be broken",
        disk_total,
        disk_total / (1024 * 1024 * 1024 * 1024),
    );
}
