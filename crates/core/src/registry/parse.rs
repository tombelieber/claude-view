// crates/core/src/registry/parse.rs
//
// Deserialization types for installed_plugins.json and helper functions
// for reading plugin metadata from the filesystem.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use tracing::{debug, warn};

// ---------------------------------------------------------------------------
// Deserialization types for installed_plugins.json
// ---------------------------------------------------------------------------

// Every field is `#[serde(default)]` so a single malformed entry -- or a
// changed JSON type on any field -- can never fail the whole-file parse and
// silently wipe out ALL plugin-sourced skills/commands/agents/MCP tools from
// the registry. (Boundary normalizer must be truthful: external data structs
// reading ~/.claude/ files must default ALL fields.)
#[derive(Debug, Deserialize)]
pub(crate) struct InstalledPlugins {
    // Typed as `serde_json::Value` (not `u32`/`Option<u32>`) so that a schema
    // change -- e.g. the top-level `version` becoming a string `"2"` instead of
    // an int `2` -- cannot fail deserialization. The value is unused; we only
    // need the parse to succeed so `plugins` survives.
    #[allow(dead_code)]
    #[serde(default)]
    pub(crate) version: serde_json::Value,
    #[serde(default)]
    pub(crate) plugins: HashMap<String, Vec<PluginEntry>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PluginEntry {
    #[allow(dead_code)]
    #[serde(default)]
    pub(crate) scope: String,
    #[serde(rename = "installPath")]
    #[serde(default)]
    pub(crate) install_path: String,
    #[allow(dead_code)]
    #[serde(default)]
    pub(crate) version: String,
    #[serde(rename = "installedAt")]
    #[serde(default)]
    #[allow(dead_code)]
    pub(crate) installed_at: String,
}

/// Optional plugin.json at the root of a plugin's install path.
#[derive(Debug, Deserialize)]
pub(crate) struct PluginJson {
    #[serde(default)]
    pub(crate) name: Option<String>,
    #[serde(default)]
    pub(crate) description: Option<String>,
}

/// .mcp.json at the root of a plugin's install path.
/// Keys are MCP server names, values are server configs.
pub(crate) type McpJson = HashMap<String, serde_json::Value>;

// ---------------------------------------------------------------------------
// Filesystem readers
// ---------------------------------------------------------------------------

pub(crate) fn read_installed_plugins(path: &Path) -> Option<InstalledPlugins> {
    let data = match std::fs::read_to_string(path) {
        Ok(d) => d,
        Err(e) => {
            debug!(
                "Could not read installed_plugins.json at {}: {e}",
                path.display()
            );
            return None;
        }
    };
    match serde_json::from_str::<InstalledPlugins>(&data) {
        Ok(p) => Some(p),
        Err(e) => {
            warn!("Failed to parse installed_plugins.json: {e}");
            None
        }
    }
}

pub(crate) fn read_plugin_json(install_path: &Path) -> Option<PluginJson> {
    let path = install_path.join("plugin.json");
    let data = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&data).ok()
}

/// Extract plugin name from key like "superpowers@superpowers-marketplace" -> "superpowers"
pub(crate) fn extract_plugin_name(key: &str) -> String {
    key.split('@').next().unwrap_or(key).to_string()
}

/// Read the first non-empty, non-heading line from a markdown file as a description.
pub(crate) fn read_first_line_description(path: &Path) -> String {
    let data = match std::fs::read_to_string(path) {
        Ok(d) => d,
        Err(_) => return String::new(),
    };
    for line in data.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        // Return first meaningful line, truncated to 200 chars
        return if trimmed.len() > 200 {
            format!("{}...", &trimmed[..197])
        } else {
            trimmed.to_string()
        };
    }
    String::new()
}
