//! Terminal table formatters for CLI subcommand output.

use serde_json::Value;

/// Format a count with K/M suffixes for compact display.
pub fn format_count(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

/// Print system monitor snapshot as a formatted table.
pub fn print_monitor(data: &Value) {
    println!("System Monitor");
    println!("{}", "-".repeat(50));

    let cpu = data.get("cpu_percent").and_then(|v| v.as_f64());
    let mem_used = data.get("memory_used_bytes").and_then(|v| v.as_u64());
    let mem_total = data.get("memory_total_bytes").and_then(|v| v.as_u64());
    let disk_percent = data.get("disk_usage_percent").and_then(|v| v.as_f64());
    let process_count = data.get("process_count").and_then(|v| v.as_u64());

    if let Some(cpu) = cpu {
        let warn = if cpu > 80.0 { " !" } else { "" };
        println!("  CPU:        {:.1}%{}", cpu, warn);
    }

    if let (Some(used), Some(total)) = (mem_used, mem_total) {
        let used_gb = used as f64 / 1_073_741_824.0;
        let total_gb = total as f64 / 1_073_741_824.0;
        let pct = if total > 0 {
            (used as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        let warn = if pct > 80.0 { " !" } else { "" };
        println!(
            "  Memory:     {:.1} / {:.1} GB ({:.0}%){}",
            used_gb, total_gb, pct, warn
        );
    }

    if let Some(disk) = disk_percent {
        let warn = if disk > 80.0 { " !" } else { "" };
        println!("  Disk:       {:.1}%{}", disk, warn);
    }

    if let Some(count) = process_count {
        println!("  Processes:  {}", count);
    }

    // Active sessions count if present
    if let Some(sessions) = data.get("active_sessions").and_then(|v| v.as_u64()) {
        println!("  Sessions:   {}", sessions);
    }

    println!();
}

/// Print live sessions as a formatted table.
pub fn print_live(data: &Value) {
    let sessions = match data.as_array() {
        Some(arr) => arr,
        None => {
            println!("No live sessions.");
            return;
        }
    };

    if sessions.is_empty() {
        println!("No live sessions.");
        return;
    }

    println!(
        "{:<10} {:<20} {:<12} {:<12} {:>8}",
        "ID", "PROJECT", "STATE", "MODEL", "COST"
    );
    println!("{}", "-".repeat(66));

    let mut total_cost = 0.0_f64;

    for s in sessions {
        let id = s.get("session_id").and_then(|v| v.as_str()).unwrap_or("?");
        let id_short = if id.len() > 8 { &id[..8] } else { id };

        let project = s
            .get("project_name")
            .or_else(|| s.get("project_display_name"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let project_trunc = if project.len() > 18 {
            format!("{}...", &project[..15])
        } else {
            project.to_string()
        };

        let state = s
            .get("state")
            .or_else(|| s.get("status"))
            .and_then(|v| v.as_str())
            .unwrap_or("?");

        let model = s.get("model").and_then(|v| v.as_str()).unwrap_or("");

        let cost = s
            .get("cost_usd")
            .or_else(|| s.get("total_cost_usd"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        total_cost += cost;

        println!(
            "{:<10} {:<20} {:<12} {:<12} {:>7.2}",
            id_short, project_trunc, state, model, cost
        );
    }

    println!("{}", "-".repeat(66));
    println!(
        "{} session(s)  total cost: ${:.2}",
        sessions.len(),
        total_cost
    );
    println!();
}

/// Print dashboard statistics as a formatted summary.
pub fn print_stats(data: &Value) {
    println!("Dashboard Statistics");
    println!("{}", "-".repeat(50));

    if let Some(sessions) = data.get("total_sessions").and_then(|v| v.as_u64()) {
        println!("  Sessions:     {}", format_count(sessions));
    }

    if let Some(projects) = data.get("total_projects").and_then(|v| v.as_u64()) {
        println!("  Projects:     {}", format_count(projects));
    }

    if let Some(week) = data.get("sessions_this_week").and_then(|v| v.as_u64()) {
        println!("  This week:    {}", format_count(week));
    }

    // Token usage
    if let Some(input) = data.get("total_input_tokens").and_then(|v| v.as_u64()) {
        println!("  Input tokens: {}", format_count(input));
    }
    if let Some(output) = data.get("total_output_tokens").and_then(|v| v.as_u64()) {
        println!("  Output tokens:{}", format_count(output));
    }

    // Cache hit ratio
    if let Some(ratio) = data.get("cache_hit_ratio").and_then(|v| v.as_f64()) {
        println!("  Cache hit:    {:.1}%", ratio * 100.0);
    }

    // Total cost
    if let Some(cost) = data.get("total_cost_usd").and_then(|v| v.as_f64()) {
        println!("  Total cost:   ${:.2}", cost);
    }

    // Top project
    if let Some(top) = data.get("top_project").and_then(|v| v.as_str()) {
        println!("  Top project:  {}", top);
    }

    println!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // --- format_count ---

    #[test]
    fn format_count_raw() {
        assert_eq!(format_count(0), "0");
        assert_eq!(format_count(999), "999");
    }

    #[test]
    fn format_count_thousands() {
        assert_eq!(format_count(1_000), "1.0K");
        assert_eq!(format_count(1_500), "1.5K");
        assert_eq!(format_count(999_999), "1000.0K");
    }

    #[test]
    fn format_count_millions() {
        assert_eq!(format_count(1_000_000), "1.0M");
        assert_eq!(format_count(2_500_000), "2.5M");
        assert_eq!(format_count(100_000_000), "100.0M");
    }

    // --- print_monitor ---

    #[test]
    fn print_monitor_no_panic_full_data() {
        let data = json!({
            "cpu_percent": 45.2,
            "memory_used_bytes": 8_589_934_592_u64,
            "memory_total_bytes": 17_179_869_184_u64,
            "disk_usage_percent": 72.1,
            "process_count": 312,
            "active_sessions": 3
        });
        print_monitor(&data); // should not panic
    }

    #[test]
    fn print_monitor_no_panic_empty() {
        print_monitor(&json!({}));
    }

    #[test]
    fn print_monitor_warns_high_cpu() {
        // Just verify no panic with high values
        let data = json!({
            "cpu_percent": 95.0,
            "memory_used_bytes": 15_000_000_000_u64,
            "memory_total_bytes": 16_000_000_000_u64,
            "disk_usage_percent": 90.0,
        });
        print_monitor(&data);
    }

    // --- print_live ---

    #[test]
    fn print_live_no_panic_with_sessions() {
        let data = json!([
            {
                "session_id": "abcdef1234567890",
                "project_name": "my-very-long-project-name-here",
                "state": "running",
                "model": "opus",
                "cost_usd": 1.23
            },
            {
                "session_id": "short",
                "project_display_name": "proj2",
                "status": "idle",
                "total_cost_usd": 0.05
            }
        ]);
        print_live(&data);
    }

    #[test]
    fn print_live_no_panic_empty_array() {
        print_live(&json!([]));
    }

    #[test]
    fn print_live_no_panic_not_array() {
        print_live(&json!({"error": "bad"}));
    }

    // --- print_stats ---

    #[test]
    fn print_stats_no_panic_full_data() {
        let data = json!({
            "total_sessions": 1463,
            "total_projects": 12,
            "sessions_this_week": 87,
            "total_input_tokens": 5_200_000_u64,
            "total_output_tokens": 1_800_000_u64,
            "cache_hit_ratio": 0.423,
            "total_cost_usd": 142.56,
            "top_project": "claude-view"
        });
        print_stats(&data);
    }

    #[test]
    fn print_stats_no_panic_empty() {
        print_stats(&json!({}));
    }
}
