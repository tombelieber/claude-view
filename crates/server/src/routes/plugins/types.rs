//! Public and internal types for the plugin management API.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

/// A single invocable item within a plugin (skill, command, agent, or MCP tool).
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
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
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
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
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
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
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
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
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
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
    /// Non-empty when the CLI call failed -- used by PluginHealthBanner.
    pub cli_error: Option<String>,
    pub orphan_count: usize,
    pub user_skills: Vec<UserItemInfo>,
    pub user_commands: Vec<UserItemInfo>,
    pub user_agents: Vec<UserItemInfo>,
}

/// A configured marketplace.
#[derive(Debug, Clone, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceInfo {
    pub name: String,
    pub source: String,
    pub repo: Option<String>,
    pub installed_count: usize,
    pub available_count: usize,
}

// ---------------------------------------------------------------------------
// Mutation request/response types
// ---------------------------------------------------------------------------

/// Request body for POST /api/plugins/action.
#[derive(Debug, Deserialize, Serialize, TS, utoipa::ToSchema)]
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
#[derive(Debug, Serialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct PluginActionResponse {
    pub success: bool,
    pub action: String,
    pub name: String,
    pub message: Option<String>,
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
