// crates/server/src/routes/plugins.rs
//! Plugin management API routes.
//!
//! - GET  /plugins        — Unified view of installed + available plugins
//! - POST /plugins/action — Mutations (install/update/uninstall/enable/disable)

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::{
    extract::Query,
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use ts_rs::TS;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use claude_view_core::registry::{InvocableInfo, InvocableKind};

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

/// A single invocable item within a plugin (skill, command, agent, or MCP tool).
#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct PluginItem {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub description: String,
    /// Full file content for the item (markdown for skills/commands/agents; pretty JSON for mcp_tool)
    pub content: String,
    pub invocation_count: i64,
    pub last_used_at: Option<i64>,
}

/// An installed plugin with its metadata, items, and usage stats.
#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub marketplace: String,
    pub scope: String,
    pub version: Option<String>,
    pub git_sha: Option<String>,
    pub enabled: bool,
    pub installed_at: String,
    pub last_updated: Option<String>,
    pub project_path: Option<String>,
    pub items: Vec<PluginItem>,
    pub skill_count: u32,
    pub command_count: u32,
    pub agent_count: u32,
    pub mcp_count: u32,
    pub total_invocations: i64,
    pub session_count: i64,
    pub last_used_at: Option<i64>,
    pub duplicate_marketplaces: Vec<String>,
    pub updatable: bool,
    pub errors: Vec<String>,
    /// True when the plugin's install directory exists on disk.
    /// False = truly orphaned (files deleted/moved).
    /// True + errors = CLI validation failure (catalog mismatch), but files are intact.
    pub source_exists: bool,
    /// Description from the marketplace listing (mirrors AvailablePlugin).
    pub description: Option<String>,
    /// Global install count from the marketplace listing (mirrors AvailablePlugin).
    pub install_count: Option<u64>,
}

/// A plugin available for installation (not yet installed).
#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct AvailablePlugin {
    pub plugin_id: String,
    pub name: String,
    pub description: String,
    pub marketplace_name: String,
    pub version: Option<String>,
    pub install_count: Option<u64>,
    pub already_installed: bool,
}

/// A user-created skill, command, or agent (not from any marketplace).
#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct UserItemInfo {
    pub name: String,
    pub kind: String, // "skill" | "command" | "agent"
    pub path: String, // relative display path, e.g. "prove-it/SKILL.md"
    pub total_invocations: i64,
    pub session_count: i64,
    pub last_used_at: Option<i64>,
}

/// Query parameters for filtering and sorting the plugins list.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginsQuery {
    pub scope: Option<String>,
    pub source: Option<String>,
    pub kind: Option<String>,
    pub search: Option<String>,
    pub sort: Option<String>,
}

/// Full response for the plugins endpoint.
#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct PluginsResponse {
    pub installed: Vec<PluginInfo>,
    pub available: Vec<AvailablePlugin>,
    pub total_installed: usize,
    pub total_available: usize,
    pub duplicate_count: usize,
    pub unused_count: usize,
    pub updatable_count: usize,
    pub marketplaces: Vec<MarketplaceInfo>,
    /// Non-empty when the CLI call failed — used by PluginHealthBanner.
    pub cli_error: Option<String>,
    pub orphan_count: usize,
    pub user_skills: Vec<UserItemInfo>,
    pub user_commands: Vec<UserItemInfo>,
    pub user_agents: Vec<UserItemInfo>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Split a plugin ID like "name@marketplace" into (name, marketplace).
///
/// Uses `rfind('@')` so names containing '@' are handled correctly.
/// If no '@' is found, returns the full string as name and empty marketplace.
pub fn parse_plugin_id(id: &str) -> (String, String) {
    match id.rfind('@') {
        Some(pos) => (id[..pos].to_string(), id[pos + 1..].to_string()),
        None => (id.to_string(), String::new()),
    }
}

/// Apply query filters to installed and available plugin lists.
///
/// Pure function for testability — no CLI or database access.
pub fn apply_filters(
    query: &PluginsQuery,
    installed: &mut Vec<PluginInfo>,
    available: &mut Vec<AvailablePlugin>,
) {
    // --- Search filter ---
    if let Some(ref search) = query.search {
        let needle = search.to_lowercase();
        installed.retain(|p| {
            p.name.to_lowercase().contains(&needle)
                || p.marketplace.to_lowercase().contains(&needle)
                || p.description
                    .as_deref()
                    .is_some_and(|d| d.to_lowercase().contains(&needle))
                || p.items
                    .iter()
                    .any(|i| i.name.to_lowercase().contains(&needle))
        });
        available.retain(|p| {
            p.name.to_lowercase().contains(&needle)
                || p.description.to_lowercase().contains(&needle)
                || p.marketplace_name.to_lowercase().contains(&needle)
        });
    }

    // --- Scope filter ---
    if let Some(ref scope) = query.scope {
        let scope_lower = scope.to_lowercase();
        if scope_lower == "available" {
            installed.clear();
        } else {
            // user or project scope — only show installed plugins matching that scope
            installed.retain(|p| p.scope.to_lowercase() == scope_lower);
            available.clear();
        }
    }

    // --- Source (marketplace) filter ---
    if let Some(ref source) = query.source {
        let source_lower = source.to_lowercase();
        installed.retain(|p| p.marketplace.to_lowercase() == source_lower);
        available.retain(|p| p.marketplace_name.to_lowercase() == source_lower);
    }

    // --- Kind filter ---
    if let Some(ref kind) = query.kind {
        let kind_lower = kind.to_lowercase();
        // "plugin" means show installed+available plugins (they ARE plugins);
        // any other kind filters installed to those containing items of that kind.
        if kind_lower != "plugin" {
            installed.retain(|p| p.items.iter().any(|i| i.kind.to_lowercase() == kind_lower));
        }
        // Available plugins don't have kind metadata — don't filter them by kind
    }

    // --- Sort: always by install count descending ---
    installed.sort_by(|a, b| {
        b.install_count
            .unwrap_or(0)
            .cmp(&a.install_count.unwrap_or(0))
    });
    available.sort_by(|a, b| {
        b.install_count
            .unwrap_or(0)
            .cmp(&a.install_count.unwrap_or(0))
    });
}

// ---------------------------------------------------------------------------
// CLI JSON deserialization (private — matches `claude plugin list --json`)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CliInstalledPlugin {
    id: String,
    #[serde(default)]
    version: Option<String>,
    scope: String,
    enabled: bool,
    installed_at: String,
    #[serde(default)]
    last_updated: Option<String>,
    #[serde(default)]
    git_commit_sha: Option<String>,
    #[serde(default)]
    project_path: Option<String>,
    #[serde(default)]
    errors: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CliAvailablePlugin {
    plugin_id: String,
    name: String,
    description: String,
    marketplace_name: String,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    install_count: Option<u64>,
}

/// Combined response from `claude plugin list --available --json`.
/// `pub(crate)` so `AppState` can hold `CachedUpstream<CliAvailableResponse>`.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CliAvailableResponse {
    #[serde(default)]
    pub(crate) installed: Vec<CliInstalledPlugin>,
    #[serde(default)]
    pub(crate) available: Vec<CliAvailablePlugin>,
}

// ---------------------------------------------------------------------------
// CLI helper
// ---------------------------------------------------------------------------

/// Strip ANSI escape sequences (color codes, cursor moves) from CLI output.
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if let Some(&next) = chars.peek() {
                if next == '[' {
                    chars.next();
                    // Consume CSI params until final byte (letter, ~, or @)
                    while let Some(&p) = chars.peek() {
                        chars.next();
                        if p.is_ascii_alphabetic() || p == '~' || p == '@' {
                            break;
                        }
                    }
                } else {
                    chars.next();
                    if next == '(' {
                        chars.next(); // charset designator
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Run a `claude plugin` subcommand and return stdout as String.
/// Strips ALL CLAUDE* env vars and ANSI codes per CLAUDE.md hard rules.
/// Optional `cwd` sets the working directory (needed for project-scoped uninstall).
pub(crate) async fn run_claude_plugin_in(
    args: &[&str],
    cwd: Option<&str>,
) -> Result<String, ApiError> {
    use std::process::Stdio;

    let cli_path = claude_view_core::resolved_cli_path().unwrap_or("claude");

    let mut cmd = Command::new(cli_path);
    cmd.arg("plugin");
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    // Suppress ANSI color codes in CLI output (https://no-color.org/)
    cmd.env("NO_COLOR", "1");

    // Strip ALL CLAUDE* + ANTHROPIC_API_KEY
    let vars_to_strip: Vec<String> = std::env::vars()
        .filter(|(k, _)| k.starts_with("CLAUDE") || k == "ANTHROPIC_API_KEY")
        .map(|(k, _)| k)
        .collect();
    for key in &vars_to_strip {
        cmd.env_remove(key);
    }

    let output = tokio::time::timeout(std::time::Duration::from_secs(30), cmd.output())
        .await
        .map_err(|_| ApiError::Internal("claude CLI timed out after 30s".into()))?
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ApiError::Internal(
                    "Claude CLI not found. Install: npm install -g @anthropic-ai/claude-code"
                        .into(),
                )
            } else {
                ApiError::Internal(format!("Failed to spawn claude CLI: {e}"))
            }
        })?;

    if !output.status.success() {
        let stderr = strip_ansi(&String::from_utf8_lossy(&output.stderr));
        return Err(ApiError::Internal(format!(
            "claude plugin {} failed: {stderr}",
            args.join(" ")
        )));
    }

    let stdout = String::from_utf8(output.stdout)
        .map_err(|e| ApiError::Internal(format!("Invalid UTF-8 from CLI: {e}")))?;
    Ok(strip_ansi(&stdout))
}

/// Convenience: run `claude plugin` in the default CWD.
async fn run_claude_plugin(args: &[&str]) -> Result<String, ApiError> {
    run_claude_plugin_in(args, None).await
}

// ---------------------------------------------------------------------------
// Shared CLI fetch + cache helpers
// ---------------------------------------------------------------------------

/// Bust the plugin CLI cache after a mutation so the next GET reflects changes.
pub(crate) async fn invalidate_plugin_cache(state: &AppState) {
    let _ = state
        .plugin_cli_cache
        .force_refresh(std::time::Duration::ZERO, fetch_plugin_cli_data)
        .await;
}

/// Fetch installed + available plugins from the CLI.
/// Signature matches `CachedUpstream::get_or_fetch` requirements.
async fn fetch_plugin_cli_data() -> Result<CliAvailableResponse, String> {
    let json = run_claude_plugin(&["list", "--available", "--json"])
        .await
        .map_err(|e| e.to_string())?;
    serde_json::from_str::<CliAvailableResponse>(&json)
        .map_err(|e| format!("Failed to parse plugin data: {e}"))
}

// ---------------------------------------------------------------------------
// Disk enrichment — description + install counts from local CLI cache files
// ---------------------------------------------------------------------------

/// Enrichment data read from the CLI's local cache files.
/// Keyed by plugin `id` (`"name@marketplace"`).
struct DiskEnrichment {
    /// Map of plugin_id → description (from `{installPath}/plugin.json`)
    descriptions: HashMap<String, String>,
    /// Map of plugin_id → unique install count (from `install-counts-cache.json`)
    install_counts: HashMap<String, u64>,
    /// Map of plugin_id → whether installPath exists on disk.
    /// The authoritative signal for "truly orphaned" vs "CLI validation error".
    source_exists: HashMap<String, bool>,
}

/// Minimal serde types for the two cache files we read.
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

/// Read description + install-count data from the CLI's local cache files.
/// Both files live under `~/.claude/plugins/` and are maintained by the CLI.
/// Returns an empty enrichment on any I/O or parse error (graceful degradation).
fn read_disk_enrichment() -> DiskEnrichment {
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

    // 1. Install counts from the CLI's server-side cache.
    let install_counts = read_install_counts(&plugins_dir);

    // 2. Descriptions + source-exists from per-plugin registrations.
    let (descriptions, source_exists) = read_plugin_descriptions_and_existence(&plugins_dir);

    DiskEnrichment {
        descriptions,
        install_counts,
        source_exists,
    }
}

fn read_install_counts(plugins_dir: &Path) -> HashMap<String, u64> {
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
/// - descriptions: plugin_id → description string (from `.claude-plugin/plugin.json`)
/// - source_exists: plugin_id → whether the installPath directory exists on disk
///
/// `source_exists` is the authoritative signal for orphan status. A plugin whose
/// installPath is present is functional regardless of what the CLI marketplace
/// validator reports.
fn read_plugin_descriptions_and_existence(
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

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

/// GET /api/plugins — Unified view of installed + available plugins.
async fn list_plugins(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PluginsQuery>,
) -> ApiResult<Json<PluginsResponse>> {
    // 1. Get installed + available from cache (non-fatal — empty on failure)
    let (cli_data, cli_error) = match state
        .plugin_cli_cache
        .get_or_fetch(fetch_plugin_cli_data)
        .await
    {
        Ok((data, _ttl)) => (data, None),
        Err(e) => {
            tracing::warn!("Plugin CLI cache fetch failed: {e}");
            (
                CliAvailableResponse {
                    installed: vec![],
                    available: vec![],
                },
                Some(e),
            )
        }
    };

    // 2. Snapshot registry data — keyed map for plugin bucketing + flat list for user items
    let (registry_snapshot, user_invocables): (
        HashMap<String, Vec<InvocableInfo>>,
        Vec<InvocableInfo>,
    ) = {
        let guard = state.registry.read().unwrap();
        if let Some(reg) = guard.as_ref() {
            let mut map: HashMap<String, Vec<InvocableInfo>> = HashMap::new();
            let mut user_items: Vec<InvocableInfo> = Vec::new();
            for inv in reg.all_invocables() {
                match &inv.plugin_name {
                    Some(pn) => {
                        map.entry(pn.clone()).or_default().push(inv.clone());
                    }
                    None => {
                        user_items.push(inv.clone());
                    }
                }
            }
            (map, user_items)
        } else {
            (HashMap::new(), vec![])
        }
    };

    // 3. Get usage stats from DB
    let usage_map: HashMap<String, (i64, Option<i64>)> =
        if let Ok(invocables) = state.db.list_invocables_with_counts().await {
            invocables
                .into_iter()
                .map(|i| (i.id.clone(), (i.invocation_count, i.last_used_at)))
                .collect()
        } else {
            HashMap::new()
        };

    // 4. Build installed plugin list
    // Load disk-based enrichment first (description from plugin.json, install count
    // from install-counts-cache.json). Fall back to the CLI's --available list for
    // plugins that appear in the marketplace catalog (covers uninstalled plugins shown
    // in the available section).
    let disk = read_disk_enrichment();

    let available_by_name: HashMap<String, &CliAvailablePlugin> = cli_data
        .available
        .iter()
        .map(|p| (p.name.clone(), p))
        .collect();

    let mut installed: Vec<PluginInfo> = Vec::new();
    let mut name_to_marketplaces: HashMap<String, Vec<String>> = HashMap::new();
    let installed_names: HashSet<String> = cli_data
        .installed
        .iter()
        .map(|p| parse_plugin_id(&p.id).0)
        .collect();

    for cli_plugin in &cli_data.installed {
        let (name, marketplace) = parse_plugin_id(&cli_plugin.id);
        name_to_marketplaces
            .entry(name.clone())
            .or_default()
            .push(marketplace.clone());

        // Get invocables for this plugin from registry snapshot
        let mut items = Vec::new();
        let mut skill_count = 0u32;
        let mut command_count = 0u32;
        let mut agent_count = 0u32;
        let mut mcp_count = 0u32;

        if let Some(invocables) = registry_snapshot.get(&name) {
            for inv in invocables {
                let (inv_count, last_used) = usage_map.get(&inv.id).copied().unwrap_or((0, None));

                let kind_str = match inv.kind {
                    claude_view_core::registry::InvocableKind::Skill => {
                        skill_count += 1;
                        "skill"
                    }
                    claude_view_core::registry::InvocableKind::Command => {
                        command_count += 1;
                        "command"
                    }
                    claude_view_core::registry::InvocableKind::Agent => {
                        agent_count += 1;
                        "agent"
                    }
                    claude_view_core::registry::InvocableKind::McpTool => {
                        mcp_count += 1;
                        "mcp_tool"
                    }
                    claude_view_core::registry::InvocableKind::BuiltinTool => continue,
                };

                items.push(PluginItem {
                    id: inv.id.clone(),
                    name: inv.name.clone(),
                    kind: kind_str.to_string(),
                    description: inv.description.clone(),
                    content: inv.content.clone(),
                    invocation_count: inv_count,
                    last_used_at: last_used,
                });
            }
        }

        let total_invocations: i64 = items.iter().map(|i| i.invocation_count).sum();
        let last_used_at = items.iter().filter_map(|i| i.last_used_at).max();

        // Sort items by usage descending
        items.sort_by_key(|i| std::cmp::Reverse(i.invocation_count));

        // Resolve description: disk manifest > marketplace catalog entry > none
        let marketplace_entry = available_by_name.get(&name);
        let description = disk
            .descriptions
            .get(&cli_plugin.id)
            .cloned()
            .or_else(|| marketplace_entry.map(|e| e.description.clone()));

        // Resolve install count: disk cache > marketplace catalog entry > none
        let install_count = disk
            .install_counts
            .get(&cli_plugin.id)
            .copied()
            .or_else(|| marketplace_entry.and_then(|e| e.install_count));

        // Default to true when registry has no entry (conservative: assume files exist).
        let source_exists = disk
            .source_exists
            .get(&cli_plugin.id)
            .copied()
            .unwrap_or(true);

        installed.push(PluginInfo {
            id: cli_plugin.id.clone(),
            name: name.clone(),
            marketplace: marketplace.clone(),
            scope: cli_plugin.scope.clone(),
            version: cli_plugin.version.clone(),
            git_sha: cli_plugin.git_commit_sha.clone(),
            enabled: cli_plugin.enabled,
            installed_at: cli_plugin.installed_at.clone(),
            last_updated: cli_plugin.last_updated.clone(),
            project_path: cli_plugin.project_path.clone(),
            items,
            skill_count,
            command_count,
            agent_count,
            mcp_count,
            total_invocations,
            session_count: 0, // TODO: requires GROUP BY session query
            last_used_at,
            duplicate_marketplaces: vec![], // Filled below
            updatable: cli_plugin.git_commit_sha.is_some(),
            errors: cli_plugin.errors.clone(),
            source_exists,
            description,
            install_count,
        });
    }

    // 5. Detect duplicates
    for plugin in &mut installed {
        if let Some(markets) = name_to_marketplaces.get(&plugin.name) {
            plugin.duplicate_marketplaces = markets
                .iter()
                .filter(|m| **m != plugin.marketplace)
                .cloned()
                .collect();
        }
    }

    // Orphan count: only plugins whose source directory is genuinely missing.
    // Plugins with errors but intact source dirs are CLI validation failures, not orphans.
    let orphan_count = installed
        .iter()
        .filter(|p| !p.errors.is_empty() && !p.source_exists)
        .count();

    // User items from the flat user_invocables list, defaulting usage to zeros.
    let make_user_items = |kind: InvocableKind| -> Vec<UserItemInfo> {
        user_invocables
            .iter()
            .filter(|i| i.kind == kind)
            .map(|i| UserItemInfo {
                name: i.name.clone(),
                kind: i.kind.to_string(),
                path: match kind {
                    InvocableKind::Skill => format!("{}/SKILL.md", i.name),
                    InvocableKind::Command => format!("commands/{}.md", i.name),
                    InvocableKind::Agent => format!("agents/{}.md", i.name),
                    _ => i.name.clone(),
                },
                total_invocations: 0,
                session_count: 0,
                last_used_at: None,
            })
            .collect()
    };
    let mut user_skills = make_user_items(InvocableKind::Skill);
    let mut user_commands = make_user_items(InvocableKind::Command);
    let mut user_agents = make_user_items(InvocableKind::Agent);

    // Sort user items by usage descending (most-used first)
    user_skills.sort_by_key(|i| std::cmp::Reverse(i.total_invocations));
    user_commands.sort_by_key(|i| std::cmp::Reverse(i.total_invocations));
    user_agents.sort_by_key(|i| std::cmp::Reverse(i.total_invocations));

    // 6. Build available plugin list
    let available: Vec<AvailablePlugin> = cli_data
        .available
        .iter()
        .map(|p| AvailablePlugin {
            plugin_id: p.plugin_id.clone(),
            name: p.name.clone(),
            description: p.description.clone(),
            marketplace_name: p.marketplace_name.clone(),
            version: p.version.clone(),
            install_count: p.install_count,
            already_installed: installed_names.contains(&p.name),
        })
        .collect();

    // 7. Compute aggregates
    let now_epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let thirty_days_ago = now_epoch - (30 * 24 * 3600);

    let duplicate_count = installed
        .iter()
        .filter(|p| !p.duplicate_marketplaces.is_empty())
        .count();
    let unused_count = installed
        .iter()
        .filter(|p| p.last_used_at.is_none_or(|t| t < thirty_days_ago))
        .count();
    let updatable_count = installed.iter().filter(|p| p.updatable).count();

    // 7b. Build enriched marketplace list with repo + counts
    let marketplace_cli_data: HashMap<String, CliMarketplace> =
        match run_claude_plugin(&["marketplace", "list", "--json"]).await {
            Ok(json) => serde_json::from_str::<Vec<CliMarketplace>>(&json)
                .unwrap_or_default()
                .into_iter()
                .map(|m| (m.name.clone(), m))
                .collect(),
            Err(_) => HashMap::new(),
        };

    // Count installed/available per marketplace
    let mut installed_per_market: HashMap<String, usize> = HashMap::new();
    for p in &installed {
        *installed_per_market
            .entry(p.marketplace.clone())
            .or_default() += 1;
    }
    let mut available_per_market: HashMap<String, usize> = HashMap::new();
    for p in &available {
        *available_per_market
            .entry(p.marketplace_name.clone())
            .or_default() += 1;
    }

    let all_market_names: HashSet<String> =
        name_to_marketplaces.values().flatten().cloned().collect();
    let mut all_marketplaces: Vec<MarketplaceInfo> = all_market_names
        .into_iter()
        .map(|name| {
            let cli = marketplace_cli_data.get(&name);
            MarketplaceInfo {
                source: cli.map_or_else(|| "github".to_string(), |c| c.source.clone()),
                repo: cli.and_then(|c| c.repo.clone()),
                installed_count: *installed_per_market.get(&name).unwrap_or(&0),
                available_count: *available_per_market.get(&name).unwrap_or(&0),
                name,
            }
        })
        .collect();
    all_marketplaces.sort_by(|a, b| a.name.cmp(&b.name));

    // 8. Apply filters
    let mut filtered_installed = installed;
    let mut filtered_available = available;
    apply_filters(&params, &mut filtered_installed, &mut filtered_available);

    let total_installed = filtered_installed.len();
    let total_available = filtered_available.len();

    Ok(Json(PluginsResponse {
        installed: filtered_installed,
        available: filtered_available,
        total_installed,
        total_available,
        duplicate_count,
        unused_count,
        updatable_count,
        marketplaces: all_marketplaces,
        cli_error,
        orphan_count,
        user_skills,
        user_commands,
        user_agents,
    }))
}

// ---------------------------------------------------------------------------
// Mutation types + validation
// ---------------------------------------------------------------------------

/// Request body for POST /api/plugins/action.
#[derive(Debug, Deserialize, Serialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct PluginActionRequest {
    /// "install" | "update" | "uninstall" | "enable" | "disable"
    pub action: String,
    /// Plugin name or full ID (e.g. "superpowers" or "superpowers@marketplace")
    pub name: String,
    /// "user" | "project"
    #[serde(default)]
    pub scope: Option<String>,
    /// For project-scoped plugins: the project directory where it was installed.
    /// Required for uninstall of project-scoped plugins (CLI needs correct CWD).
    #[serde(default)]
    pub project_path: Option<String>,
}

/// Response for POST /api/plugins/action.
#[derive(Debug, Serialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct PluginActionResponse {
    pub success: bool,
    pub action: String,
    pub name: String,
    pub message: Option<String>,
}

pub(crate) const VALID_ACTIONS: &[&str] = &["install", "update", "uninstall", "enable", "disable"];

/// Reject CLI flag injection — only [a-zA-Z0-9._@-] allowed, must not start with `-`.
pub(crate) fn validate_plugin_name(name: &str) -> Result<(), ApiError> {
    if name.is_empty()
        || name.len() > 128
        || name.starts_with('-')
        || name
            .chars()
            .any(|c| !c.is_alphanumeric() && c != '-' && c != '_' && c != '.' && c != '@')
    {
        return Err(ApiError::BadRequest(format!(
            "Invalid plugin name: {name}. Must start with alphanumeric and contain only alphanumeric, hyphens, underscores, dots, and @."
        )));
    }
    Ok(())
}

pub(crate) fn validate_scope(scope: &Option<String>) -> Result<(), ApiError> {
    if let Some(s) = scope {
        if s != "user" && s != "project" {
            return Err(ApiError::BadRequest(format!(
                "Invalid scope: {s}. Must be 'user' or 'project'."
            )));
        }
    }
    Ok(())
}

// Marketplace-only mutation lock (plugin mutations go through the op queue).
static MARKETPLACE_LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();

fn get_marketplace_lock() -> &'static tokio::sync::Mutex<()> {
    MARKETPLACE_LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

// ---------------------------------------------------------------------------
// Marketplace types + endpoints
// ---------------------------------------------------------------------------

/// A configured marketplace.
#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceInfo {
    pub name: String,
    pub source: String,
    pub repo: Option<String>,
    pub installed_count: usize,
    pub available_count: usize,
}

/// Request body for POST /api/plugins/marketplaces/action.
#[derive(Debug, Deserialize, Serialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceActionRequest {
    /// "add" | "remove" | "update"
    pub action: String,
    /// For add: GitHub repo URL or owner/repo
    #[serde(default)]
    pub source: Option<String>,
    /// For remove/update: marketplace name
    #[serde(default)]
    pub name: Option<String>,
    /// For add: "user" | "project"
    #[serde(default)]
    pub scope: Option<String>,
}

/// Validate marketplace source — must be "owner/repo" short form.
fn validate_marketplace_source(source: &str) -> Result<String, ApiError> {
    let short = source
        .trim_start_matches("https://github.com/")
        .trim_start_matches("http://github.com/")
        .trim_end_matches('/')
        .trim_end_matches(".git");

    let parts: Vec<&str> = short.split('/').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err(ApiError::BadRequest(format!(
            "Invalid marketplace source: {source}. Use 'owner/repo' format."
        )));
    }

    for part in &parts {
        if part
            .chars()
            .any(|c| !c.is_alphanumeric() && c != '-' && c != '_' && c != '.')
        {
            return Err(ApiError::BadRequest(format!(
                "Invalid characters in marketplace source: {source}."
            )));
        }
    }

    if short.len() > 256 {
        return Err(ApiError::BadRequest("Marketplace source too long.".into()));
    }

    Ok(short.to_string())
}

/// CLI JSON shape for `claude plugin marketplace list --json`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CliMarketplace {
    name: String,
    #[serde(default)]
    source: String,
    #[serde(default)]
    repo: Option<String>,
}

/// GET /api/plugins/marketplaces
async fn list_marketplaces(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<Vec<MarketplaceInfo>>> {
    let json = run_claude_plugin(&["marketplace", "list", "--json"]).await;

    match json {
        Ok(data) => {
            let markets: Vec<CliMarketplace> = serde_json::from_str(&data).unwrap_or_default();

            // Use cached plugin data for per-marketplace counts
            let (mut installed_per_market, mut available_per_market) = (
                HashMap::<String, usize>::new(),
                HashMap::<String, usize>::new(),
            );

            if let Ok((cli_data, _)) = state
                .plugin_cli_cache
                .get_or_fetch(fetch_plugin_cli_data)
                .await
            {
                for p in &cli_data.installed {
                    let (_, marketplace) = parse_plugin_id(&p.id);
                    *installed_per_market.entry(marketplace).or_default() += 1;
                }
                for p in &cli_data.available {
                    *available_per_market
                        .entry(p.marketplace_name.clone())
                        .or_default() += 1;
                }
            }

            let mut result: Vec<MarketplaceInfo> = markets
                .into_iter()
                .map(|m| {
                    let installed_count = installed_per_market.get(&m.name).copied().unwrap_or(0);
                    let available_count = available_per_market.get(&m.name).copied().unwrap_or(0);
                    MarketplaceInfo {
                        repo: m.repo,
                        installed_count,
                        available_count,
                        name: m.name,
                        source: m.source,
                    }
                })
                .collect();
            result.sort_by(|a, b| a.name.cmp(&b.name));

            Ok(Json(result))
        }
        Err(e) => {
            tracing::warn!("Marketplace list failed: {e}");
            Ok(Json(vec![]))
        }
    }
}

/// POST /api/plugins/marketplaces/action
async fn marketplace_action(
    State(state): State<Arc<AppState>>,
    Json(req): Json<MarketplaceActionRequest>,
) -> ApiResult<Json<PluginActionResponse>> {
    let valid_actions = ["add", "remove", "update"];
    if !valid_actions.contains(&req.action.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "Invalid marketplace action: {}. Must be one of: {}",
            req.action,
            valid_actions.join(", ")
        )));
    }

    // Validate inputs per action
    match req.action.as_str() {
        "add" => {
            let source = req.source.as_deref().ok_or_else(|| {
                ApiError::BadRequest("'source' is required for add action.".into())
            })?;
            let validated = validate_marketplace_source(source)?;
            validate_scope(&req.scope)?;

            let _guard = get_marketplace_lock()
                .try_lock()
                .map_err(|_| ApiError::Conflict("A mutation is already in progress.".into()))?;

            let mut args = vec!["marketplace", "add", &validated];
            let scope_str;
            if let Some(ref scope) = req.scope {
                scope_str = scope.clone();
                args.push("--scope");
                args.push(&scope_str);
            }

            let result = match run_claude_plugin(&args).await {
                Ok(stdout) => Ok(Json(PluginActionResponse {
                    success: true,
                    action: "add".into(),
                    name: validated,
                    message: if stdout.trim().is_empty() {
                        None
                    } else {
                        Some(stdout.trim().to_string())
                    },
                })),
                Err(e) => Ok(Json(PluginActionResponse {
                    success: false,
                    action: "add".into(),
                    name: validated,
                    message: Some(e.to_string()),
                })),
            };
            invalidate_plugin_cache(&state).await;
            result
        }
        "remove" => {
            let name = req.name.as_deref().ok_or_else(|| {
                ApiError::BadRequest("'name' is required for remove action.".into())
            })?;
            validate_plugin_name(name)?;

            let _guard = get_marketplace_lock()
                .try_lock()
                .map_err(|_| ApiError::Conflict("A mutation is already in progress.".into()))?;

            let result = match run_claude_plugin(&["marketplace", "remove", name]).await {
                Ok(_) => Ok(Json(PluginActionResponse {
                    success: true,
                    action: "remove".into(),
                    name: name.to_string(),
                    message: None,
                })),
                Err(e) => Ok(Json(PluginActionResponse {
                    success: false,
                    action: "remove".into(),
                    name: name.to_string(),
                    message: Some(e.to_string()),
                })),
            };
            invalidate_plugin_cache(&state).await;
            result
        }
        "update" => {
            let _guard = get_marketplace_lock()
                .try_lock()
                .map_err(|_| ApiError::Conflict("A mutation is already in progress.".into()))?;

            let args = if let Some(ref name) = req.name {
                validate_plugin_name(name)?;
                vec!["marketplace", "update", name.as_str()]
            } else {
                vec!["marketplace", "update"]
            };

            let result = match run_claude_plugin(&args).await {
                Ok(stdout) => Ok(Json(PluginActionResponse {
                    success: true,
                    action: "update".into(),
                    name: req.name.unwrap_or_default(),
                    message: if stdout.trim().is_empty() {
                        None
                    } else {
                        Some(stdout.trim().to_string())
                    },
                })),
                Err(e) => Ok(Json(PluginActionResponse {
                    success: false,
                    action: "update".into(),
                    name: req.name.unwrap_or_default(),
                    message: Some(e.to_string()),
                })),
            };
            invalidate_plugin_cache(&state).await;
            result
        }
        _ => unreachable!(),
    }
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/plugins", get(list_plugins))
        .route("/plugins/marketplaces", get(list_marketplaces))
        .route("/plugins/marketplaces/action", post(marketplace_action))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use claude_view_db::Database;
    use tower::ServiceExt;

    /// Helper: build a minimal router with just the plugins route.
    fn build_app(db: Database) -> axum::Router {
        let state = crate::state::AppState::new(db);
        axum::Router::new()
            .nest("/api", router())
            .nest("/api", crate::routes::plugin_ops::router())
            .with_state(state)
    }

    /// Helper: make a GET request and return status + body string.
    async fn get_response(app: axum::Router, uri: &str) -> (StatusCode, String) {
        let response = app
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    #[tokio::test]
    async fn test_plugins_endpoint_returns_ok() {
        let db = Database::new_in_memory().await.expect("in-memory DB");
        let app = build_app(db);
        let (status, body) = get_response(app, "/api/plugins").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(json["installed"].is_array());
        assert!(json["available"].is_array());
        assert!(json["totalInstalled"].is_number());
        assert!(json["totalAvailable"].is_number());
        assert!(json["duplicateCount"].is_number());
        assert!(json["unusedCount"].is_number());
        assert!(json["updatableCount"].is_number());
        assert!(json["marketplaces"].is_array());
    }

    #[tokio::test]
    async fn test_plugins_response_includes_user_sections() {
        let db = Database::new_in_memory().await.expect("in-memory DB");
        let app = build_app(db);
        let (status, body) = get_response(app, "/api/plugins").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        // New fields must exist (even if empty arrays/zero)
        assert!(json["userSkills"].is_array(), "missing userSkills");
        assert!(json["userCommands"].is_array(), "missing userCommands");
        assert!(json["userAgents"].is_array(), "missing userAgents");
        assert!(json["orphanCount"].is_number(), "missing orphanCount");
    }

    #[tokio::test]
    async fn test_user_item_path_format_matches_mockup() {
        // Verify that the path field uses kind-aware formatting:
        // - skills: "prove-it/SKILL.md" (name/SKILL.md)
        // - commands: "commands/wtf.md" (commands/name.md)
        // - agents: "agents/scanner.md" (agents/name.md)
        let db = Database::new_in_memory().await.expect("in-memory DB");
        let app = build_app(db);
        let (status, body) = get_response(app, "/api/plugins").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        // If there are user skills, verify path format
        if let Some(skills) = json["userSkills"].as_array() {
            for skill in skills {
                let path = skill["path"].as_str().unwrap();
                assert!(
                    path.ends_with("/SKILL.md"),
                    "skill path '{path}' should end with /SKILL.md"
                );
            }
        }
        if let Some(commands) = json["userCommands"].as_array() {
            for cmd in commands {
                let path = cmd["path"].as_str().unwrap();
                assert!(
                    path.starts_with("commands/"),
                    "command path '{path}' should start with commands/"
                );
                assert!(
                    path.ends_with(".md"),
                    "command path '{path}' should end with .md"
                );
            }
        }
        if let Some(agents) = json["userAgents"].as_array() {
            for agent in agents {
                let path = agent["path"].as_str().unwrap();
                assert!(
                    path.starts_with("agents/"),
                    "agent path '{path}' should start with agents/"
                );
                assert!(
                    path.ends_with(".md"),
                    "agent path '{path}' should end with .md"
                );
            }
        }
    }

    #[test]
    fn test_parse_plugin_id() {
        // Normal case: name@marketplace
        let (name, marketplace) = parse_plugin_id("superpowers@superpowers-marketplace");
        assert_eq!(name, "superpowers");
        assert_eq!(marketplace, "superpowers-marketplace");

        // No @ sign — full string is name, empty marketplace
        let (name, marketplace) = parse_plugin_id("standalone");
        assert_eq!(name, "standalone");
        assert_eq!(marketplace, "");

        // Multiple @ signs — split on LAST one
        let (name, marketplace) = parse_plugin_id("user@domain@registry");
        assert_eq!(name, "user@domain");
        assert_eq!(marketplace, "registry");

        // Empty string
        let (name, marketplace) = parse_plugin_id("");
        assert_eq!(name, "");
        assert_eq!(marketplace, "");
    }

    #[test]
    fn test_apply_filters_search() {
        let mut installed = vec![
            PluginInfo {
                id: "superpowers@marketplace".to_string(),
                name: "superpowers".to_string(),
                marketplace: "marketplace".to_string(),
                scope: "user".to_string(),
                version: Some("1.0.0".to_string()),
                git_sha: None,
                enabled: true,
                installed_at: "2026-01-01T00:00:00Z".to_string(),
                last_updated: None,
                project_path: None,
                items: vec![PluginItem {
                    id: "superpowers:brainstorming".to_string(),
                    name: "brainstorming".to_string(),
                    kind: "skill".to_string(),
                    description: "Explore ideas".to_string(),
                    content: String::new(),
                    invocation_count: 5,
                    last_used_at: Some(1000),
                }],
                skill_count: 1,
                command_count: 0,
                agent_count: 0,
                mcp_count: 0,
                total_invocations: 5,
                session_count: 3,
                last_used_at: Some(1000),
                duplicate_marketplaces: vec![],
                updatable: false,
                errors: vec![],
                source_exists: true,
                description: None,
                install_count: None,
            },
            PluginInfo {
                id: "hookify@marketplace".to_string(),
                name: "hookify".to_string(),
                marketplace: "marketplace".to_string(),
                scope: "project".to_string(),
                version: Some("2.0.0".to_string()),
                git_sha: None,
                enabled: true,
                installed_at: "2026-02-01T00:00:00Z".to_string(),
                last_updated: None,
                project_path: None,
                items: vec![PluginItem {
                    id: "hookify:format".to_string(),
                    name: "format".to_string(),
                    kind: "command".to_string(),
                    description: "Format code".to_string(),
                    content: String::new(),
                    invocation_count: 0,
                    last_used_at: None,
                }],
                skill_count: 0,
                command_count: 1,
                agent_count: 0,
                mcp_count: 0,
                total_invocations: 0,
                session_count: 0,
                last_used_at: None,
                duplicate_marketplaces: vec![],
                updatable: false,
                errors: vec![],
                source_exists: true,
                description: None,
                install_count: None,
            },
        ];

        let mut available = vec![AvailablePlugin {
            plugin_id: "other-plugin".to_string(),
            name: "other-plugin".to_string(),
            description: "Does other things".to_string(),
            marketplace_name: "marketplace".to_string(),
            version: Some("1.0.0".to_string()),
            install_count: None,
            already_installed: false,
        }];

        // Search for "super" — should match superpowers, not hookify
        let query = PluginsQuery {
            search: Some("super".to_string()),
            ..Default::default()
        };
        apply_filters(&query, &mut installed, &mut available);

        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].name, "superpowers");

        // Available should also be filtered — "other-plugin" doesn't match "super"
        assert_eq!(available.len(), 0);
    }

    #[test]
    fn test_apply_filters_scope() {
        let mut installed = vec![
            PluginInfo {
                id: "a@m".to_string(),
                name: "a".to_string(),
                marketplace: "m".to_string(),
                scope: "user".to_string(),
                version: Some("1.0.0".to_string()),
                git_sha: None,
                enabled: true,
                installed_at: "2026-01-01T00:00:00Z".to_string(),
                last_updated: None,
                project_path: None,
                items: vec![],
                skill_count: 0,
                command_count: 0,
                agent_count: 0,
                mcp_count: 0,
                total_invocations: 0,
                session_count: 0,
                last_used_at: None,
                duplicate_marketplaces: vec![],
                updatable: false,
                errors: vec![],
                source_exists: true,
                description: None,
                install_count: None,
            },
            PluginInfo {
                id: "b@m".to_string(),
                name: "b".to_string(),
                marketplace: "m".to_string(),
                scope: "project".to_string(),
                version: Some("1.0.0".to_string()),
                git_sha: None,
                enabled: true,
                installed_at: "2026-01-01T00:00:00Z".to_string(),
                last_updated: None,
                project_path: None,
                items: vec![],
                skill_count: 0,
                command_count: 0,
                agent_count: 0,
                mcp_count: 0,
                total_invocations: 0,
                session_count: 0,
                last_used_at: None,
                duplicate_marketplaces: vec![],
                updatable: false,
                errors: vec![],
                source_exists: true,
                description: None,
                install_count: None,
            },
        ];

        let mut available = vec![AvailablePlugin {
            plugin_id: "c".to_string(),
            name: "c".to_string(),
            description: "Available".to_string(),
            marketplace_name: "m".to_string(),
            version: Some("1.0.0".to_string()),
            install_count: None,
            already_installed: false,
        }];

        // Filter by scope "user" — should keep only user-scoped installed, clear available
        let query = PluginsQuery {
            scope: Some("user".to_string()),
            ..Default::default()
        };
        apply_filters(&query, &mut installed, &mut available);

        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].name, "a");
        assert_eq!(available.len(), 0);
    }

    #[test]
    fn test_apply_filters_sort_by_install_count() {
        let mut installed = vec![
            PluginInfo {
                id: "low@m".to_string(),
                name: "low-usage".to_string(),
                marketplace: "m".to_string(),
                scope: "user".to_string(),
                version: Some("1.0.0".to_string()),
                git_sha: None,
                enabled: true,
                installed_at: "2026-01-01T00:00:00Z".to_string(),
                last_updated: None,
                project_path: None,
                items: vec![],
                skill_count: 0,
                command_count: 0,
                agent_count: 0,
                mcp_count: 0,
                total_invocations: 2,
                session_count: 1,
                last_used_at: None,
                duplicate_marketplaces: vec![],
                updatable: false,
                errors: vec![],
                source_exists: true,
                description: None,
                install_count: Some(50),
            },
            PluginInfo {
                id: "high@m".to_string(),
                name: "high-installs".to_string(),
                marketplace: "m".to_string(),
                scope: "user".to_string(),
                version: Some("1.0.0".to_string()),
                git_sha: None,
                enabled: true,
                installed_at: "2026-01-01T00:00:00Z".to_string(),
                last_updated: None,
                project_path: None,
                items: vec![],
                skill_count: 0,
                command_count: 0,
                agent_count: 0,
                mcp_count: 0,
                total_invocations: 100,
                session_count: 50,
                last_used_at: None,
                duplicate_marketplaces: vec![],
                updatable: false,
                errors: vec![],
                source_exists: true,
                description: None,
                install_count: Some(5000),
            },
        ];

        let mut available = vec![];

        apply_filters(&PluginsQuery::default(), &mut installed, &mut available);

        // Higher install_count comes first
        assert_eq!(installed[0].name, "high-installs");
        assert_eq!(installed[1].name, "low-usage");
    }

    #[tokio::test]
    async fn test_plugin_ops_rejects_invalid_name() {
        let db = Database::new_in_memory().await.expect("in-memory DB");
        let app = build_app(db);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/plugins/ops")
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"action":"install","name":"--force"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_plugin_ops_rejects_invalid_action() {
        let db = Database::new_in_memory().await.expect("in-memory DB");
        let app = build_app(db);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/plugins/ops")
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"action":"rm_rf","name":"test"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_plugin_ops_rejects_invalid_scope() {
        let db = Database::new_in_memory().await.expect("in-memory DB");
        let app = build_app(db);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/plugins/ops")
                    .header("Content-Type", "application/json")
                    .body(Body::from(
                        r#"{"action":"install","name":"test","scope":"global"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validate_plugin_name() {
        // Valid names
        assert!(validate_plugin_name("superpowers").is_ok());
        assert!(validate_plugin_name("my-plugin").is_ok());
        assert!(validate_plugin_name("my_plugin.v2").is_ok());
        assert!(validate_plugin_name("plugin@marketplace").is_ok());

        // Invalid names — CLI flag injection attempts
        assert!(validate_plugin_name("--force").is_err());
        assert!(validate_plugin_name("-rf").is_err());
        assert!(validate_plugin_name("foo;rm -rf /").is_err());
        assert!(validate_plugin_name("").is_err());
        assert!(validate_plugin_name(&"a".repeat(129)).is_err());
    }

    #[test]
    fn test_validate_marketplace_source() {
        // Valid sources
        assert_eq!(
            validate_marketplace_source("owner/repo").unwrap(),
            "owner/repo"
        );
        assert_eq!(
            validate_marketplace_source("https://github.com/owner/repo").unwrap(),
            "owner/repo"
        );
        assert_eq!(
            validate_marketplace_source("https://github.com/owner/repo.git").unwrap(),
            "owner/repo"
        );
        assert_eq!(
            validate_marketplace_source("https://github.com/owner/repo/").unwrap(),
            "owner/repo"
        );

        // Invalid sources
        assert!(validate_marketplace_source("just-a-name").is_err());
        assert!(validate_marketplace_source("a/b/c").is_err());
        assert!(validate_marketplace_source("/repo").is_err());
        assert!(validate_marketplace_source("owner/").is_err());
        assert!(validate_marketplace_source("owner/repo;evil").is_err());
    }

    #[tokio::test]
    async fn test_marketplace_action_rejects_add_without_source() {
        let db = Database::new_in_memory().await.expect("in-memory DB");
        let app = build_app(db);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/plugins/marketplaces/action")
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"action":"add"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_marketplace_action_rejects_invalid_action() {
        let db = Database::new_in_memory().await.expect("in-memory DB");
        let app = build_app(db);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/plugins/marketplaces/action")
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"action":"destroy"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    // ---------------------------------------------------------------------------
    // Disk enrichment regression tests
    // ---------------------------------------------------------------------------

    #[test]
    fn test_disk_enrichment_populates_description_from_plugin_json() {
        let dir = tempfile::tempdir().expect("tempdir");
        let plugins_dir = dir.path();

        // Write a plugin.json manifest for "my-plugin@my-marketplace"
        let install_path = plugins_dir
            .join("cache")
            .join("my-marketplace")
            .join("my-plugin")
            .join("1.0.0");
        std::fs::create_dir_all(&install_path).unwrap();
        std::fs::write(
            install_path.join("plugin.json"),
            r#"{"name":"my-plugin","description":"A great plugin description"}"#,
        )
        .unwrap();

        // Write installed_plugins.json pointing to that installPath
        let registry = serde_json::json!({
            "version": 2,
            "plugins": {
                "my-plugin@my-marketplace": [{
                    "scope": "user",
                    "installPath": install_path.to_str().unwrap(),
                    "version": "1.0.0",
                    "installedAt": "2026-01-01T00:00:00.000Z",
                    "lastUpdated": "2026-01-01T00:00:00.000Z"
                }]
            }
        });
        std::fs::write(
            plugins_dir.join("installed_plugins.json"),
            registry.to_string(),
        )
        .unwrap();

        let (descriptions, source_exists) =
            read_plugin_descriptions_and_existence(&plugins_dir.to_path_buf());

        assert_eq!(
            descriptions
                .get("my-plugin@my-marketplace")
                .map(String::as_str),
            Some("A great plugin description"),
            "description must be populated from plugin.json on disk"
        );
        assert_eq!(
            source_exists.get("my-plugin@my-marketplace"),
            Some(&true),
            "installPath exists → source_exists=true"
        );
    }

    #[test]
    fn test_disk_enrichment_populates_install_count_from_cache() {
        let dir = tempfile::tempdir().expect("tempdir");
        let plugins_dir = dir.path();

        let cache = serde_json::json!({
            "version": 1,
            "fetchedAt": "2026-03-01T00:00:00.000Z",
            "counts": [
                {"plugin": "my-plugin@my-marketplace", "unique_installs": 42000},
                {"plugin": "other@other", "unique_installs": 1}
            ]
        });
        std::fs::write(
            plugins_dir.join("install-counts-cache.json"),
            cache.to_string(),
        )
        .unwrap();

        let counts = read_install_counts(&plugins_dir.to_path_buf());

        assert_eq!(
            counts.get("my-plugin@my-marketplace").copied(),
            Some(42000u64),
            "install count must be populated from install-counts-cache.json"
        );
    }

    #[test]
    fn test_disk_enrichment_returns_empty_when_files_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        // No files written — all helpers must return empty maps, not panic
        let plugins_dir = dir.path().to_path_buf();
        let (descriptions, source_exists) = read_plugin_descriptions_and_existence(&plugins_dir);
        let install_counts = read_install_counts(&plugins_dir);
        assert!(descriptions.is_empty());
        assert!(install_counts.is_empty());
        assert!(source_exists.is_empty());
    }

    /// Regression: source_exists is derived from the filesystem, not from CLI errors.
    /// A plugin with CLI errors but an intact installPath must have source_exists=true.
    /// A plugin with a deleted installPath must have source_exists=false.
    #[test]
    fn test_source_exists_reflects_filesystem_not_cli_errors() {
        let dir = tempfile::tempdir().unwrap();
        let plugins_dir = dir.path();

        // Plugin A: directory exists (local user-created plugin like "wtf")
        let plugin_a_path = plugins_dir.join("wtf");
        std::fs::create_dir_all(&plugin_a_path).unwrap();

        // Plugin B: directory does NOT exist (truly orphaned)
        let plugin_b_path = plugins_dir.join("gone");
        // intentionally not created

        let registry_json = serde_json::json!({
            "version": 2,
            "plugins": {
                "wtf@local": [{ "installPath": plugin_a_path.to_str().unwrap() }],
                "gone@some-marketplace": [{ "installPath": plugin_b_path.to_str().unwrap() }],
            }
        });
        std::fs::write(
            plugins_dir.join("installed_plugins.json"),
            registry_json.to_string(),
        )
        .unwrap();

        let (_, source_exists) = read_plugin_descriptions_and_existence(plugins_dir);

        assert_eq!(
            source_exists.get("wtf@local"),
            Some(&true),
            "wtf dir exists → source_exists=true"
        );
        assert_eq!(
            source_exists.get("gone@some-marketplace"),
            Some(&false),
            "gone dir missing → source_exists=false"
        );
    }
}
