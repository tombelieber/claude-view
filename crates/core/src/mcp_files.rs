//! Scanner for ~/.claude/plugins/cache/*/.mcp.json MCP server configurations.
//!
//! Walks plugin cache directories, collects all `.mcp.json` files, deduplicates
//! by server name (latest dir mtime wins). Also reads `mcp-needs-auth-cache.json`
//! to surface auth status.
//!
//! On-demand read, NO SQLite indexing — follows task_files.rs / memory_files.rs pattern.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use ts_rs::TS;

// ---------------------------------------------------------------------------
// On-disk schema (what Claude Code writes)
// ---------------------------------------------------------------------------

/// OAuth configuration for an MCP server.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpOAuthConfig {
    #[allow(dead_code)]
    client_id: Option<String>,
    #[allow(dead_code)]
    callback_port: Option<u16>, // excluded from response (internal)
}

/// A single MCP server entry in .mcp.json.
#[derive(Debug, Clone, Deserialize)]
struct McpServerEntry {
    #[serde(rename = "type")]
    server_type: Option<String>,
    url: Option<String>,
    oauth: Option<McpOAuthConfig>,
}

/// The .mcp.json file format.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpConfigFile {
    mcp_servers: Option<HashMap<String, McpServerEntry>>,
}

/// Auth cache entry from mcp-needs-auth-cache.json.
#[derive(Debug, Clone, Deserialize)]
struct AuthCacheEntry {
    #[allow(dead_code)]
    timestamp: Option<i64>,
}

// ---------------------------------------------------------------------------
// API response types
// ---------------------------------------------------------------------------

/// A deduplicated MCP server for API response.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct McpServer {
    /// Server name (e.g. "slack", "sentry", "notion").
    pub name: String,
    /// Transport type: "http", "stdio", or null if unknown.
    pub server_type: Option<String>,
    /// Server endpoint URL, if configured.
    pub url: Option<String>,
    /// Whether this server uses OAuth authentication.
    pub has_oauth: bool,
    /// Whether this server needs re-authentication (from auth cache).
    pub needs_reauth: bool,
}

/// Summary of all discovered MCP servers.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct McpServerIndex {
    /// Deduplicated list of MCP servers.
    pub servers: Vec<McpServer>,
    /// Total .mcp.json files found before dedup.
    pub raw_file_count: usize,
}

// ---------------------------------------------------------------------------
// Scanner implementation
// ---------------------------------------------------------------------------

/// Discover all MCP servers from plugin cache, deduplicated by name.
pub fn discover_mcp_servers() -> McpServerIndex {
    let Some(claude_dir) = dirs::home_dir().map(|h| h.join(".claude")) else {
        return McpServerIndex {
            servers: Vec::new(),
            raw_file_count: 0,
        };
    };

    let cache_dir = claude_dir.join("plugins").join("cache");
    let auth_needs = load_auth_cache(&claude_dir);

    let mut raw_count = 0usize;
    // name → (McpServer, dir_mtime)
    let mut dedup: HashMap<String, (McpServer, std::time::SystemTime)> = HashMap::new();

    if let Ok(entries) = std::fs::read_dir(&cache_dir) {
        for entry in entries.flatten() {
            let dir_path = entry.path();
            if !dir_path.is_dir() {
                continue;
            }
            // Walk subdirs (some plugins have versioned subdirs)
            collect_from_dir(&dir_path, &auth_needs, &mut dedup, &mut raw_count);
        }
    }

    let mut servers: Vec<McpServer> = dedup.into_values().map(|(s, _)| s).collect();
    servers.sort_by(|a, b| a.name.cmp(&b.name));

    McpServerIndex {
        servers,
        raw_file_count: raw_count,
    }
}

/// Recursively collect .mcp.json files from a directory tree.
fn collect_from_dir(
    dir: &Path,
    auth_needs: &HashMap<String, bool>,
    dedup: &mut HashMap<String, (McpServer, std::time::SystemTime)>,
    raw_count: &mut usize,
) {
    // Check for .mcp.json in this directory
    let mcp_path = dir.join(".mcp.json");
    if mcp_path.is_file() {
        if let Some(servers) = parse_mcp_file(&mcp_path) {
            *raw_count += 1;
            let dir_mtime = dir
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH);

            for (name, entry) in servers {
                let existing_newer = dedup
                    .get(&name)
                    .map(|(_, t)| *t >= dir_mtime)
                    .unwrap_or(false);

                if !existing_newer {
                    let needs_reauth = auth_needs
                        .keys()
                        .any(|k| k.to_lowercase().contains(&name.to_lowercase()));

                    dedup.insert(
                        name.clone(),
                        (
                            McpServer {
                                name,
                                server_type: entry.server_type,
                                url: entry.url,
                                has_oauth: entry.oauth.is_some(),
                                needs_reauth,
                            },
                            dir_mtime,
                        ),
                    );
                }
            }
        }
    }

    // Recurse into subdirectories (for versioned plugin dirs)
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_from_dir(&path, auth_needs, dedup, raw_count);
            }
        }
    }
}

/// Parse a single .mcp.json file.
fn parse_mcp_file(path: &Path) -> Option<HashMap<String, McpServerEntry>> {
    let contents = std::fs::read_to_string(path).ok()?;
    let config: McpConfigFile = serde_json::from_str(&contents).ok()?;
    config.mcp_servers
}

/// Load the auth-needs-cache to determine which servers need re-auth.
fn load_auth_cache(claude_dir: &Path) -> HashMap<String, bool> {
    let path = claude_dir.join("mcp-needs-auth-cache.json");
    let Ok(contents) = std::fs::read_to_string(&path) else {
        return HashMap::new();
    };
    let Ok(cache) = serde_json::from_str::<HashMap<String, AuthCacheEntry>>(&contents) else {
        return HashMap::new();
    };
    cache.into_keys().map(|k| (k, true)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_parse_mcp_file_basic() {
        let tmp = tempfile::tempdir().unwrap();
        let mcp_path = tmp.path().join(".mcp.json");
        fs::write(
            &mcp_path,
            r#"{
                "mcpServers": {
                    "slack": {
                        "type": "http",
                        "url": "https://mcp.slack.com/mcp",
                        "oauth": { "clientId": "abc123", "callbackPort": 3118 }
                    },
                    "sentry": {
                        "type": "http",
                        "url": "https://mcp.sentry.dev/mcp"
                    }
                }
            }"#,
        )
        .unwrap();

        let servers = parse_mcp_file(&mcp_path).unwrap();
        assert_eq!(servers.len(), 2);
        assert!(servers.contains_key("slack"));
        assert!(servers["slack"].oauth.is_some());
        assert!(servers["sentry"].oauth.is_none());
    }

    #[test]
    fn test_parse_mcp_file_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let mcp_path = tmp.path().join(".mcp.json");
        fs::write(&mcp_path, r#"{"mcpServers": {}}"#).unwrap();

        let servers = parse_mcp_file(&mcp_path).unwrap();
        assert!(servers.is_empty());
    }

    #[test]
    fn test_parse_mcp_file_invalid_json() {
        let tmp = tempfile::tempdir().unwrap();
        let mcp_path = tmp.path().join(".mcp.json");
        fs::write(&mcp_path, "not json").unwrap();

        assert!(parse_mcp_file(&mcp_path).is_none());
    }

    #[test]
    fn test_dedup_by_name_latest_mtime_wins() {
        let tmp = tempfile::tempdir().unwrap();
        let cache_dir = tmp.path().join("plugins").join("cache");

        // Older dir
        let old_dir = cache_dir.join("old-plugin");
        fs::create_dir_all(&old_dir).unwrap();
        fs::write(
            old_dir.join(".mcp.json"),
            r#"{"mcpServers": {"slack": {"type": "http", "url": "https://old.slack.com"}}}"#,
        )
        .unwrap();

        // Brief pause so mtimes differ
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Newer dir
        let new_dir = cache_dir.join("new-plugin");
        fs::create_dir_all(&new_dir).unwrap();
        fs::write(
            new_dir.join(".mcp.json"),
            r#"{"mcpServers": {"slack": {"type": "http", "url": "https://new.slack.com"}}}"#,
        )
        .unwrap();

        let auth_needs = HashMap::new();
        let mut dedup = HashMap::new();
        let mut raw_count = 0;

        collect_from_dir(&cache_dir, &auth_needs, &mut dedup, &mut raw_count);

        assert_eq!(raw_count, 2);
        assert_eq!(dedup.len(), 1);
        let (server, _) = &dedup["slack"];
        assert_eq!(server.url.as_deref(), Some("https://new.slack.com"));
    }
}
