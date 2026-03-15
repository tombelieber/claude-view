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

    // Sanity: total must be > 0
    assert!(
        snap.disk_total_bytes > 0,
        "disk_total_bytes must be non-zero"
    );

    // used ≤ total (basic invariant)
    assert!(
        snap.disk_used_bytes <= snap.disk_total_bytes,
        "disk_used_bytes ({}) must not exceed disk_total_bytes ({})",
        snap.disk_used_bytes,
        snap.disk_total_bytes,
    );

    // Regression guard: on a typical machine, total should be < 20 TB.
    // If dedup fails, APFS double-counting inflates this to 2-4x the real size.
    // 20 TB is generous enough for any real workstation.
    let twenty_tb = 20_u64 * 1024 * 1024 * 1024 * 1024;
    assert!(
        snap.disk_total_bytes < twenty_tb,
        "disk_total_bytes ({} bytes = {} TB) looks inflated — dedup may be broken",
        snap.disk_total_bytes,
        snap.disk_total_bytes / (1024 * 1024 * 1024 * 1024),
    );
}
