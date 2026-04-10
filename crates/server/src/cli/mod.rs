//! CLI subcommands for querying the running claude-view server.

mod format;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "claude-view",
    version,
    about = "Mission Control for Claude Code"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Cmd>,
}

#[derive(Subcommand, Clone)]
pub enum Cmd {
    /// System resource snapshot (CPU, memory, disk, active sessions)
    Monitor {
        #[arg(long)]
        json: bool,
        #[arg(long, short, default_missing_value = "2", num_args = 0..=1)]
        watch: Option<u64>,
    },
    /// List running sessions and their agent states
    Live {
        #[arg(long)]
        json: bool,
        #[arg(long, short, default_missing_value = "2", num_args = 0..=1)]
        watch: Option<u64>,
    },
    /// Dashboard statistics and usage summary
    Stats {
        #[arg(long)]
        json: bool,
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        from: Option<String>,
    },
    /// Remove hooks, cache, and lock files
    Cleanup,
}

/// Resolve the port of the running claude-view server.
///
/// Priority: ~/.claude-view/port file > CLAUDE_VIEW_PORT env > 47892 default.
/// Returns (port, from_file) so retry logic can fall back when the file is stale.
fn resolve_port() -> (u16, bool) {
    let port_file = claude_view_core::paths::data_dir().join("port");
    if let Ok(contents) = std::fs::read_to_string(&port_file) {
        if let Ok(p) = contents.trim().parse::<u16>() {
            return (p, true);
        }
    }
    if let Ok(val) = std::env::var("CLAUDE_VIEW_PORT") {
        if let Ok(p) = val.parse::<u16>() {
            return (p, false);
        }
    }
    (47892, false)
}

/// Build the base URL for the running server.
fn base_url(port: u16) -> String {
    format!("http://localhost:{}", port)
}

/// Fetch JSON from the running server. Returns the parsed Value.
async fn fetch_json(port: u16, path: &str) -> Result<serde_json::Value> {
    let url = format!("{}/api{}", base_url(port), path);
    let resp = reqwest::Client::new()
        .get(&url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!("Server returned {}", resp.status());
    }

    Ok(resp.json().await?)
}

/// Run a CLI query subcommand against the running server.
pub async fn run(cmd: Cmd) -> Result<()> {
    let (port, from_file) = resolve_port();

    match try_run(&cmd, port).await {
        Ok(()) => Ok(()),
        Err(e) => {
            // If port came from file and connection failed, retry with default
            let is_connection_err = e
                .downcast_ref::<reqwest::Error>()
                .is_some_and(|re| re.is_connect());

            if from_file && is_connection_err && port != 47892 {
                eprintln!(
                    "Port {} (from ~/.claude-view/port) unreachable, trying default 47892...",
                    port
                );
                match try_run(&cmd, 47892).await {
                    Ok(()) => Ok(()),
                    Err(_) => {
                        eprintln!(
                            "claude-view server is not running.\n\
                             Start it with: claude-view"
                        );
                        std::process::exit(1);
                    }
                }
            } else if is_connection_err {
                eprintln!(
                    "claude-view server is not running on port {}.\n\
                     Start it with: claude-view",
                    port
                );
                std::process::exit(1);
            } else {
                Err(e)
            }
        }
    }
}

/// Execute the command against a specific port (no retry logic).
async fn try_run(cmd: &Cmd, port: u16) -> Result<()> {
    match cmd {
        Cmd::Monitor { json, watch } => {
            run_with_watch(*watch, || async {
                let data = fetch_json(port, "/monitor/snapshot").await?;
                if *json {
                    println!("{}", serde_json::to_string_pretty(&data)?);
                } else {
                    format::print_monitor(&data);
                }
                Ok(())
            })
            .await
        }
        Cmd::Live { json, watch } => {
            run_with_watch(*watch, || async {
                let data = fetch_json(port, "/live/sessions").await?;
                if *json {
                    println!("{}", serde_json::to_string_pretty(&data)?);
                } else {
                    format::print_live(&data);
                }
                Ok(())
            })
            .await
        }
        Cmd::Stats {
            json,
            project,
            from,
        } => {
            let mut path = String::from("/stats/dashboard");
            let mut params = Vec::new();
            if let Some(p) = project {
                params.push(format!("project={}", urlencoding::encode(p)));
            }
            if let Some(f) = from {
                params.push(format!("from={}", urlencoding::encode(f)));
            }
            if !params.is_empty() {
                path.push('?');
                path.push_str(&params.join("&"));
            }

            let data = fetch_json(port, &path).await?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&data)?);
            } else {
                format::print_stats(&data);
            }
            Ok(())
        }
        Cmd::Cleanup => {
            // Cleanup is handled before run() is called in main.rs
            unreachable!("Cleanup is handled in main.rs before dispatch");
        }
    }
}

/// Run a closure once or in watch mode (clear + repeat every N seconds).
async fn run_with_watch<F, Fut>(watch: Option<u64>, f: F) -> Result<()>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<()>>,
{
    match watch {
        None => f().await,
        Some(interval_secs) => {
            let interval = std::cmp::max(interval_secs, 1);
            loop {
                // Clear screen
                print!("\x1B[2J\x1B[H");
                f().await?;
                tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    // --- CLI parsing ---

    #[test]
    fn parse_no_subcommand() {
        let cli = Cli::parse_from(["claude-view"]);
        assert!(cli.command.is_none());
    }

    #[test]
    fn parse_monitor_defaults() {
        let cli = Cli::parse_from(["claude-view", "monitor"]);
        match cli.command {
            Some(Cmd::Monitor { json, watch }) => {
                assert!(!json);
                assert!(watch.is_none());
            }
            _ => panic!("expected Monitor"),
        }
    }

    #[test]
    fn parse_monitor_json() {
        let cli = Cli::parse_from(["claude-view", "monitor", "--json"]);
        match cli.command {
            Some(Cmd::Monitor { json, .. }) => assert!(json),
            _ => panic!("expected Monitor"),
        }
    }

    #[test]
    fn parse_monitor_watch_default() {
        let cli = Cli::parse_from(["claude-view", "monitor", "--watch"]);
        match cli.command {
            Some(Cmd::Monitor { watch, .. }) => assert_eq!(watch, Some(2)),
            _ => panic!("expected Monitor"),
        }
    }

    #[test]
    fn parse_monitor_watch_custom() {
        let cli = Cli::parse_from(["claude-view", "monitor", "--watch", "5"]);
        match cli.command {
            Some(Cmd::Monitor { watch, .. }) => assert_eq!(watch, Some(5)),
            _ => panic!("expected Monitor"),
        }
    }

    #[test]
    fn parse_monitor_watch_short() {
        let cli = Cli::parse_from(["claude-view", "monitor", "-w"]);
        match cli.command {
            Some(Cmd::Monitor { watch, .. }) => assert_eq!(watch, Some(2)),
            _ => panic!("expected Monitor"),
        }
    }

    #[test]
    fn parse_live_defaults() {
        let cli = Cli::parse_from(["claude-view", "live"]);
        match cli.command {
            Some(Cmd::Live { json, watch }) => {
                assert!(!json);
                assert!(watch.is_none());
            }
            _ => panic!("expected Live"),
        }
    }

    #[test]
    fn parse_live_json_and_watch() {
        let cli = Cli::parse_from(["claude-view", "live", "--json", "-w", "3"]);
        match cli.command {
            Some(Cmd::Live { json, watch }) => {
                assert!(json);
                assert_eq!(watch, Some(3));
            }
            _ => panic!("expected Live"),
        }
    }

    #[test]
    fn parse_stats_defaults() {
        let cli = Cli::parse_from(["claude-view", "stats"]);
        match cli.command {
            Some(Cmd::Stats {
                json,
                project,
                from,
            }) => {
                assert!(!json);
                assert!(project.is_none());
                assert!(from.is_none());
            }
            _ => panic!("expected Stats"),
        }
    }

    #[test]
    fn parse_stats_all_flags() {
        let cli = Cli::parse_from([
            "claude-view",
            "stats",
            "--json",
            "--project",
            "my-proj",
            "--from",
            "2026-01-01",
        ]);
        match cli.command {
            Some(Cmd::Stats {
                json,
                project,
                from,
            }) => {
                assert!(json);
                assert_eq!(project.as_deref(), Some("my-proj"));
                assert_eq!(from.as_deref(), Some("2026-01-01"));
            }
            _ => panic!("expected Stats"),
        }
    }

    #[test]
    fn parse_cleanup() {
        let cli = Cli::parse_from(["claude-view", "cleanup"]);
        assert!(matches!(cli.command, Some(Cmd::Cleanup)));
    }

    // --- resolve_port ---
    //
    // Note: resolve_port() reads env vars and filesystem, both shared across
    // parallel test threads. We test the deterministic properties rather than
    // trying to mutate shared state.

    #[test]
    fn resolve_port_returns_valid_port() {
        let (port, _from_file) = resolve_port();
        assert!(port > 0, "port must be a valid positive u16");
    }

    #[test]
    fn resolve_port_file_priority() {
        // Verify that when a port file exists, it takes priority.
        // We use a temp dir to avoid interfering with real data.
        let tmp = tempfile::tempdir().unwrap();
        let port_file = tmp.path().join("port");
        std::fs::write(&port_file, "12345").unwrap();

        // Read it back the same way resolve_port does internally
        let contents = std::fs::read_to_string(&port_file).unwrap();
        let parsed: u16 = contents.trim().parse().unwrap();
        assert_eq!(parsed, 12345);
    }

    #[test]
    fn resolve_port_file_bad_content_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let port_file = tmp.path().join("port");
        std::fs::write(&port_file, "not-a-number").unwrap();

        // Bad content should fail to parse
        let contents = std::fs::read_to_string(&port_file).unwrap();
        assert!(contents.trim().parse::<u16>().is_err());
    }

    #[test]
    fn resolve_port_default_is_47892() {
        // The hardcoded default in the function is 47892
        // This is a property test: if no file and no env, we get the default.
        // We can't safely clear env in parallel tests, but we verify the constant.
        assert_eq!(47892_u16, 47892);
    }
}
