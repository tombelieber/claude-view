//! Terminal table formatters for CLI subcommand output.
//!
//! These structs deserialize the running server's JSON responses. They
//! intentionally model the **wire format** (camelCase) instead of importing
//! the server's internal Rust types — drift then becomes a deserialize
//! failure on the canonical fixtures in `tests`, not a silent
//! `Option::None`-everything render that prints just a header.

use serde::Deserialize;

// ── Wire-format views ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorView {
    pub cpu_percent: f64,
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
    #[serde(default)]
    pub disk_used_bytes: Option<u64>,
    #[serde(default)]
    pub disk_total_bytes: Option<u64>,
    #[serde(default)]
    pub top_processes: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub session_resources: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveListView {
    #[serde(default)]
    pub sessions: Vec<LiveSessionView>,
    #[serde(default)]
    pub process_count: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveSessionView {
    pub id: String,
    #[serde(default)]
    pub project_display_name: Option<String>,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub cost: Option<LiveCostView>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveCostView {
    #[serde(default)]
    pub total_usd: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsView {
    pub total_sessions: u64,
    pub total_projects: u64,
    #[serde(default)]
    pub data_start_date: Option<i64>,
    #[serde(default)]
    pub current_week: Option<CurrentWeekView>,
    #[serde(default)]
    pub top_projects: Vec<TopProjectView>,
    #[serde(default)]
    pub top_skills: Vec<NamedCountView>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentWeekView {
    pub session_count: u64,
    pub total_tokens: u64,
    pub total_files_edited: u64,
    pub commit_count: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopProjectView {
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub name: String,
    pub session_count: u64,
}

#[derive(Debug, Deserialize)]
pub struct NamedCountView {
    pub name: String,
    pub count: u64,
}

// ── Helpers ────────────────────────────────────────────────────────────

/// Format a count with K/M/B suffixes for compact display.
pub fn format_count(n: u64) -> String {
    if n >= 1_000_000_000 {
        format!("{:.1}B", n as f64 / 1_000_000_000.0)
    } else if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn format_gb(bytes: u64) -> f64 {
    bytes as f64 / 1_073_741_824.0
}

fn format_date(unix_seconds: i64) -> String {
    chrono::DateTime::<chrono::Utc>::from_timestamp(unix_seconds, 0)
        .map(|dt| dt.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "?".to_string())
}

// ── Printers ───────────────────────────────────────────────────────────

pub fn print_monitor(data: &MonitorView) {
    println!("System Monitor");
    println!("{}", "-".repeat(50));

    let cpu_warn = if data.cpu_percent > 80.0 { " !" } else { "" };
    println!("  CPU:        {:.1}%{}", data.cpu_percent, cpu_warn);

    let mem_used_gb = format_gb(data.memory_used_bytes);
    let mem_total_gb = format_gb(data.memory_total_bytes);
    let mem_pct = if data.memory_total_bytes > 0 {
        (data.memory_used_bytes as f64 / data.memory_total_bytes as f64) * 100.0
    } else {
        0.0
    };
    let mem_warn = if mem_pct > 80.0 { " !" } else { "" };
    println!(
        "  Memory:     {:.1} / {:.1} GB ({:.0}%){}",
        mem_used_gb, mem_total_gb, mem_pct, mem_warn
    );

    if let (Some(used), Some(total)) = (data.disk_used_bytes, data.disk_total_bytes) {
        let used_gb = format_gb(used);
        let total_gb = format_gb(total);
        let pct = if total > 0 {
            (used as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        let warn = if pct > 80.0 { " !" } else { "" };
        println!(
            "  Disk:       {:.1} / {:.1} GB ({:.0}%){}",
            used_gb, total_gb, pct, warn
        );
    }

    if let Some(procs) = data.top_processes.as_ref() {
        println!("  Processes:  {} (top)", procs.len());
    }
    println!("  Sessions:   {}", data.session_resources.len());

    println!();
}

pub fn print_live(data: &LiveListView) {
    if data.sessions.is_empty() {
        println!("No live sessions.");
        return;
    }

    println!(
        "{:<10} {:<20} {:<12} {:<20} {:>8}",
        "ID", "PROJECT", "STATUS", "MODEL", "COST"
    );
    println!("{}", "-".repeat(74));

    let mut total_cost = 0.0_f64;
    for s in &data.sessions {
        let id_short = if s.id.len() > 8 { &s.id[..8] } else { &s.id };
        let project = s
            .project_display_name
            .as_deref()
            .or(s.project.as_deref())
            .unwrap_or("unknown");
        let project_trunc = if project.len() > 18 {
            format!("{}...", &project[..15])
        } else {
            project.to_string()
        };
        let status = s.status.as_deref().unwrap_or("?");
        let model = s.model.as_deref().unwrap_or("");
        let model_trunc = if model.len() > 18 {
            format!("{}...", &model[..15])
        } else {
            model.to_string()
        };
        let cost = s.cost.as_ref().map(|c| c.total_usd).unwrap_or(0.0);
        total_cost += cost;

        println!(
            "{:<10} {:<20} {:<12} {:<20} {:>7.2}",
            id_short, project_trunc, status, model_trunc, cost
        );
    }

    println!("{}", "-".repeat(74));
    println!(
        "{} session(s)  total cost: ${:.2}  ({} processes)",
        data.sessions.len(),
        total_cost,
        data.process_count
    );
    println!();
}

pub fn print_stats(data: &StatsView) {
    println!("Dashboard Statistics");
    println!("{}", "-".repeat(50));

    println!("  Total sessions:  {}", format_count(data.total_sessions));
    println!("  Total projects:  {}", format_count(data.total_projects));
    if let Some(ts) = data.data_start_date {
        println!("  Data since:      {}", format_date(ts));
    }

    if let Some(week) = &data.current_week {
        println!();
        println!("  Current period (usage)");
        println!("  {}", "-".repeat(48));
        println!("    Sessions:      {}", format_count(week.session_count));
        println!("    Tokens:        {}", format_count(week.total_tokens));
        println!(
            "    Files edited:  {}",
            format_count(week.total_files_edited)
        );
        println!("    Commits:       {}", format_count(week.commit_count));
    }

    if !data.top_projects.is_empty() {
        println!();
        println!("  Top projects");
        println!("  {}", "-".repeat(48));
        for p in data.top_projects.iter().take(5) {
            let name = if !p.display_name.is_empty() {
                p.display_name.as_str()
            } else if !p.name.is_empty() {
                p.name.as_str()
            } else {
                "(unnamed)"
            };
            println!(
                "    {:<30}  {} sessions",
                name,
                format_count(p.session_count)
            );
        }
    }

    if !data.top_skills.is_empty() {
        println!();
        println!("  Top skills");
        println!("  {}", "-".repeat(48));
        for s in data.top_skills.iter().take(5) {
            println!("    {:<30}  {}", s.name, format_count(s.count));
        }
    }

    println!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── format_count ───────────────────────────────────────────────────

    #[test]
    fn format_count_raw() {
        assert_eq!(format_count(0), "0");
        assert_eq!(format_count(999), "999");
    }

    #[test]
    fn format_count_thousands() {
        assert_eq!(format_count(1_000), "1.0K");
        assert_eq!(format_count(1_500), "1.5K");
    }

    #[test]
    fn format_count_millions() {
        assert_eq!(format_count(1_000_000), "1.0M");
        assert_eq!(format_count(2_500_000), "2.5M");
    }

    #[test]
    fn format_count_billions() {
        assert_eq!(format_count(1_000_000_000), "1.0B");
        assert_eq!(format_count(19_118_910_732), "19.1B");
    }

    // ── Real wire-format fixtures ──────────────────────────────────────
    //
    // These capture the actual JSON the server emits as of 2026-05-22.
    // Anchoring the tests to camelCase + nested structure prevents a
    // future schema change from silently breaking the CLI (since the
    // previous tests asserted only "no panic" on fake snake_case data).

    fn monitor_fixture() -> serde_json::Value {
        json!({
            "timestamp": 1779420000,
            "cpuPercent": 45.2,
            "memoryUsedBytes": 8_589_934_592_u64,
            "memoryTotalBytes": 17_179_869_184_u64,
            "diskUsedBytes": 250_000_000_000_u64,
            "diskTotalBytes": 1_000_000_000_000_u64,
            "topProcesses": [
                {"name": "claude-view", "cpuPercent": 0.2, "memoryBytes": 1_000_000_u64}
            ],
            "sessionResources": [
                {"sessionId": "abc"},
                {"sessionId": "def"}
            ]
        })
    }

    fn live_fixture() -> serde_json::Value {
        json!({
            "sessions": [
                {
                    "id": "abcdef1234567890",
                    "project": "-home-user-claude-view",
                    "projectDisplayName": "claude-view",
                    "status": "working",
                    "model": "claude-opus-4-7",
                    "cost": {"totalUsd": 3.0303}
                },
                {
                    "id": "short",
                    "projectDisplayName": "proj2",
                    "status": "paused",
                    "model": "claude-opus-4-7",
                    "cost": {"totalUsd": 0.05}
                }
            ],
            "recentlyClosed": [],
            "total": 2,
            "processCount": 0
        })
    }

    fn stats_fixture() -> serde_json::Value {
        json!({
            "totalSessions": 4873,
            "totalProjects": 65,
            "dataStartDate": 1768650027,
            "heatmap": [],
            "topSkills": [
                {"name": "browse", "count": 3500},
                {"name": "frontend-design", "count": 2300}
            ],
            "topCommands": [],
            "topMcpTools": [],
            "topAgents": [],
            "topProjects": [
                {"name": "claude-view", "displayName": "claude-view", "sessionCount": 1824},
                {"name": "another-project", "displayName": "another-project", "sessionCount": 847}
            ],
            "toolTotals": {},
            "longestSessions": [],
            "currentWeek": {
                "sessionCount": 4888,
                "totalTokens": 19_118_910_732_u64,
                "totalFilesEdited": 12_597,
                "commitCount": 1028
            },
            "meta": {
                "ranges": {"currentPeriod": {}, "heatmap": {}}
            }
        })
    }

    // ── Deserialization parity ────────────────────────────────────────

    #[test]
    fn monitor_view_parses_real_wire_format() {
        let view: MonitorView = serde_json::from_value(monitor_fixture()).unwrap();
        assert!((view.cpu_percent - 45.2).abs() < 0.01);
        assert_eq!(view.memory_used_bytes, 8_589_934_592);
        assert_eq!(view.disk_used_bytes, Some(250_000_000_000));
        assert_eq!(view.session_resources.len(), 2);
    }

    #[test]
    fn live_view_parses_real_wire_format() {
        let view: LiveListView = serde_json::from_value(live_fixture()).unwrap();
        assert_eq!(view.sessions.len(), 2);
        assert_eq!(view.sessions[0].id, "abcdef1234567890");
        assert_eq!(
            view.sessions[0].project_display_name.as_deref(),
            Some("claude-view")
        );
        assert_eq!(view.sessions[0].status.as_deref(), Some("working"));
        assert!((view.sessions[0].cost.as_ref().unwrap().total_usd - 3.0303).abs() < 0.001);
    }

    #[test]
    fn stats_view_parses_real_wire_format() {
        let view: StatsView = serde_json::from_value(stats_fixture()).unwrap();
        assert_eq!(view.total_sessions, 4873);
        assert_eq!(view.total_projects, 65);
        let week = view.current_week.expect("currentWeek required");
        assert_eq!(week.session_count, 4888);
        assert_eq!(week.total_tokens, 19_118_910_732);
        assert_eq!(view.top_projects.len(), 2);
        assert_eq!(view.top_skills.len(), 2);
    }

    // ── Snake_case (old/wrong shape) must fail to parse ────────────────
    //
    // Regression guard for the original bug: the v0.41.1 CLI silently
    // parsed snake_case test fixtures, masking the camelCase production
    // schema. If somebody re-introduces snake_case fixtures, these
    // negative tests catch it immediately.

    #[test]
    fn stats_view_rejects_snake_case_top_level() {
        let bad = json!({
            "total_sessions": 100,
            "total_projects": 5,
            "sessions_this_week": 10
        });
        assert!(serde_json::from_value::<StatsView>(bad).is_err());
    }

    #[test]
    fn monitor_view_rejects_snake_case() {
        let bad = json!({
            "cpu_percent": 10.0,
            "memory_used_bytes": 1u64,
            "memory_total_bytes": 2u64
        });
        assert!(serde_json::from_value::<MonitorView>(bad).is_err());
    }

    // ── Printers don't panic ───────────────────────────────────────────

    #[test]
    fn print_monitor_no_panic_full() {
        let view: MonitorView = serde_json::from_value(monitor_fixture()).unwrap();
        print_monitor(&view);
    }

    #[test]
    fn print_live_no_panic_full() {
        let view: LiveListView = serde_json::from_value(live_fixture()).unwrap();
        print_live(&view);
    }

    #[test]
    fn print_live_no_panic_empty() {
        let view = LiveListView {
            sessions: vec![],
            process_count: 0,
        };
        print_live(&view);
    }

    #[test]
    fn print_stats_no_panic_full() {
        let view: StatsView = serde_json::from_value(stats_fixture()).unwrap();
        print_stats(&view);
    }

    // The `*_fixture()` builders above are the wire-format contract: they
    // mirror the actual JSON the server emits (camelCase, nested
    // `currentWeek`, `cost.totalUsd`, `processCount`/`recentlyClosed`
    // wrapper for live sessions). Any future server-side rename that
    // breaks the contract surfaces here as a fixture-update PR.
}
