//! Debug lifecycle logger — writes events to ~/.claude-view/debug/pid-lifecycle.jsonl
//!
//! Only active in debug builds. Compiles to no-ops in release.

/// Append a lifecycle event to the shared JSONL debug log.
///
/// No-op in release builds. Silently ignores write failures.
#[allow(unused_variables)]
pub fn log_lifecycle(
    event: &str,
    pid: Option<u32>,
    session_id: Option<&str>,
    tmux_name: Option<&str>,
    detail: &str,
) {
    #[cfg(debug_assertions)]
    {
        use std::io::Write;

        let now = chrono::Utc::now();
        let ts = now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let record = serde_json::json!({
            "source": "rust-server",
            "event": event,
            "ts": ts,
            "epochMs": now.timestamp_millis(),
            "pid": pid,
            "sessionId": session_id,
            "tmuxName": tmux_name,
            "detail": detail,
        });

        let Some(home) = dirs::home_dir() else {
            return;
        };
        let log_dir = home.join(".claude-view/debug");
        let _ = std::fs::create_dir_all(&log_dir);
        let log_path = log_dir.join("pid-lifecycle.jsonl");

        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            let _ = writeln!(f, "{}", record);
        }

        let text_path = log_dir.join("pid-lifecycle.log");
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&text_path)
        {
            let _ = writeln!(
                f,
                "[{}] ⚙ SERVER {} | pid={} sessionId={} tmux={} | {}",
                ts,
                event,
                pid.map(|p| p.to_string()).unwrap_or_else(|| "?".into()),
                session_id.unwrap_or("?"),
                tmux_name.unwrap_or("?"),
                detail,
            );
        }
    }
}
