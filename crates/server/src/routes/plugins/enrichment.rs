//! Disk-based enrichment: descriptions and install counts from CLI cache files.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

// ---------------------------------------------------------------------------
// Enrichment data container
// ---------------------------------------------------------------------------

/// Enrichment data read from the CLI's local cache files.
/// Keyed by plugin `id` (`"name@marketplace"`).
pub(crate) struct DiskEnrichment {
    /// Map of plugin_id -> description (from `{installPath}/plugin.json`)
    pub descriptions: HashMap<String, String>,
    /// Map of plugin_id -> unique install count (from `install-counts-cache.json`)
    pub install_counts: HashMap<String, u64>,
    /// Map of plugin_id -> whether installPath exists on disk.
    /// The authoritative signal for "truly orphaned" vs "CLI validation error".
    pub source_exists: HashMap<String, bool>,
}

// ---------------------------------------------------------------------------
// Serde types for cache files
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct InstallCountsCache {
    counts: Vec<InstallCountEntry>,
}

#[derive(Deserialize)]
struct InstallCountEntry {
    plugin: String,
    unique_installs: u64,
}

#[derive(Deserialize)]
struct InstalledPluginsRegistry {
    plugins: HashMap<String, Vec<InstalledPluginEntry>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstalledPluginEntry {
    install_path: String,
}

#[derive(Deserialize)]
struct PluginManifest {
    #[serde(default)]
    description: Option<String>,
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Read description + install-count data from the CLI's local cache files.
/// Both files live under `~/.claude/plugins/` and are maintained by the CLI.
/// Returns an empty enrichment on any I/O or parse error (graceful degradation).
pub(crate) fn read_disk_enrichment() -> DiskEnrichment {
    let plugins_dir = match dirs::home_dir() {
        Some(h) => h.join(".claude").join("plugins"),
        None => {
            return DiskEnrichment {
                descriptions: HashMap::new(),
                install_counts: HashMap::new(),
                source_exists: HashMap::new(),
            }
        }
    };

    let install_counts = read_install_counts(&plugins_dir);
    let (descriptions, source_exists) = read_plugin_descriptions_and_existence(&plugins_dir);

    DiskEnrichment {
        descriptions,
        install_counts,
        source_exists,
    }
}

// ---------------------------------------------------------------------------
// Internal readers
// ---------------------------------------------------------------------------

pub(crate) fn read_install_counts(plugins_dir: &Path) -> HashMap<String, u64> {
    let path = plugins_dir.join("install-counts-cache.json");
    let data = match std::fs::read_to_string(&path) {
        Ok(d) => d,
        Err(_) => return HashMap::new(),
    };
    match serde_json::from_str::<InstallCountsCache>(&data) {
        Ok(cache) => cache
            .counts
            .into_iter()
            .map(|e| (e.plugin, e.unique_installs))
            .collect(),
        Err(_) => HashMap::new(),
    }
}

/// Reads `installed_plugins.json` once and returns two maps:
/// - descriptions: plugin_id -> description string (from `.claude-plugin/plugin.json`)
/// - source_exists: plugin_id -> whether the installPath directory exists on disk
///
/// `source_exists` is the authoritative signal for orphan status. A plugin whose
/// installPath is present is functional regardless of what the CLI marketplace
/// validator reports.
pub(crate) fn read_plugin_descriptions_and_existence(
    plugins_dir: &Path,
) -> (HashMap<String, String>, HashMap<String, bool>) {
    let registry_path = plugins_dir.join("installed_plugins.json");
    let data = match std::fs::read_to_string(&registry_path) {
        Ok(d) => d,
        Err(_) => return (HashMap::new(), HashMap::new()),
    };
    let registry = match serde_json::from_str::<InstalledPluginsRegistry>(&data) {
        Ok(r) => r,
        Err(_) => return (HashMap::new(), HashMap::new()),
    };

    let mut descriptions = HashMap::new();
    let mut source_exists = HashMap::new();

    for (plugin_id, entries) in &registry.plugins {
        // Use the first entry's installPath (duplicates have separate ids).
        if let Some(entry) = entries.first() {
            let install_dir = PathBuf::from(&entry.install_path);

            // Ground truth: does the directory exist?
            source_exists.insert(plugin_id.clone(), install_dir.exists());

            // Description from `.claude-plugin/plugin.json` (new layout) or
            // legacy `plugin.json` at the root.
            let manifest_path = if install_dir
                .join(".claude-plugin")
                .join("plugin.json")
                .exists()
            {
                install_dir.join(".claude-plugin").join("plugin.json")
            } else {
                install_dir.join("plugin.json")
            };

            if let Ok(manifest_data) = std::fs::read_to_string(&manifest_path) {
                if let Ok(manifest) = serde_json::from_str::<PluginManifest>(&manifest_data) {
                    if let Some(desc) = manifest.description {
                        if !desc.is_empty() {
                            descriptions.insert(plugin_id.clone(), desc);
                        }
                    }
                }
            }
        }
    }

    (descriptions, source_exists)
}
