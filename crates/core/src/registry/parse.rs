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

#[derive(Debug, Deserialize)]
pub(crate) struct InstalledPlugins {
    #[allow(dead_code)]
    pub(crate) version: u32,
    pub(crate) plugins: HashMap<String, Vec<PluginEntry>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PluginEntry {
    #[allow(dead_code)]
    pub(crate) scope: String,
    #[serde(rename = "installPath")]
    pub(crate) install_path: String,
    #[allow(dead_code)]
    pub(crate) version: String,
    #[serde(rename = "installedAt")]
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
