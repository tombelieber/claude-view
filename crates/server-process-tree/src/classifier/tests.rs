use super::pipeline::classify_process_list;
use crate::types::{EcosystemTag, RawProcessInfo, Staleness};

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

#[test]
fn sidecar_child_gets_sidecar_tag() {
    let own_pid = 5000u32;
    let processes = vec![
        make_raw(
            own_pid,
            1,
            "claude-view",
            "/usr/local/bin/claude-view serve",
            0.5,
            50_000_000,
            1_700_000_000,
        ),
        make_raw(
            5001,
            own_pid,
            "node",
            "node /home/user/.cache/claude-view/bin/sidecar/dist/index.js",
            0.0,
            188_000_000,
            1_700_000_001,
        ),
    ];
    let snap = classify_process_list(&processes, own_pid);
    // Sidecar stays as a child (descendant) of Self_, not promoted to ecosystem
    assert_eq!(snap.ecosystem.len(), 1);
    assert!(matches!(
        snap.ecosystem[0].ecosystem_tag,
        Some(EcosystemTag::Self_)
    ));
    assert_eq!(snap.ecosystem[0].descendants.len(), 1);
    let sidecar = &snap.ecosystem[0].descendants[0];
    assert_eq!(sidecar.pid, 5001);
    assert!(
        matches!(sidecar.ecosystem_tag, Some(EcosystemTag::Sidecar)),
        "sidecar child must get Sidecar tag, got: {:?}",
        sidecar.ecosystem_tag
    );
}

#[test]
fn sidecar_command_not_matched_as_self() {
    // sidecar/dist/index.js contains "claude-view" in the path —
    // must NOT be classified as Self_ ecosystem process.
    let processes = vec![make_raw(
        5001,
        5000,
        "node",
        "node /home/user/.cache/claude-view/bin/sidecar/dist/index.js",
        0.0,
        188_000_000,
        1_700_000_000,
    )];
    let snap = classify_process_list(&processes, 9999);
    // Must NOT appear as ecosystem (not Self_, not anything)
    assert!(
        snap.ecosystem.is_empty(),
        "sidecar must not be classified as ecosystem process"
    );
}
