// crates/server/src/routes/plugins.rs
//! Plugin management API routes.
//!
//! - GET  /plugins        — Unified view of installed + available plugins
//! - POST /plugins/action — Mutations (install/update/uninstall/enable/disable)

use std::collections::{HashMap, HashSet};
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
    pub version: String,
    pub git_sha: Option<String>,
    pub enabled: bool,
    pub installed_at: String,
    pub last_updated: Option<String>,
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
    pub version: String,
    pub already_installed: bool,
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
    pub marketplaces: Vec<String>,
    /// Non-empty when the CLI call failed — used by PluginHealthBanner.
    pub cli_error: Option<String>,
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
        installed.retain(|p| p.items.iter().any(|i| i.kind.to_lowercase() == kind_lower));
        // Available plugins don't have kind metadata — don't filter them by kind
    }

    // --- Sort installed ---
    if let Some(ref sort) = query.sort {
        match sort.as_str() {
            "usage" => installed.sort_by(|a, b| b.total_invocations.cmp(&a.total_invocations)),
            "updated" => installed.sort_by(|a, b| {
                let a_ts = a.last_updated.as_deref().unwrap_or("");
                let b_ts = b.last_updated.as_deref().unwrap_or("");
                b_ts.cmp(a_ts)
            }),
            // "name" or default
            _ => installed.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
        }
    } else {
        // Default sort: by usage descending (most-used first)
        installed.sort_by(|a, b| b.total_invocations.cmp(&a.total_invocations));
    }

    // Available plugins are always sorted alphabetically
    available.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
}

// ---------------------------------------------------------------------------
// CLI JSON deserialization (private — matches `claude plugin list --json`)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CliInstalledPlugin {
    id: String,
    version: String,
    scope: String,
    enabled: bool,
    #[serde(default)]
    install_path: Option<String>,
    installed_at: String,
    #[serde(default)]
    last_updated: Option<String>,
    #[serde(default)]
    git_commit_sha: Option<String>,
    #[serde(default)]
    errors: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CliAvailablePlugin {
    plugin_id: String,
    name: String,
    description: String,
    marketplace_name: String,
    version: String,
}

/// Combined response from `claude plugin list --available --json`.
/// `#[serde(default)]` on both arrays as safety net for unknown CLI output shape.
#[derive(Debug, Deserialize)]
struct CliAvailableResponse {
    #[serde(default)]
    installed: Vec<CliInstalledPlugin>,
    #[serde(default)]
    available: Vec<CliAvailablePlugin>,
}

// ---------------------------------------------------------------------------
// CLI helper
// ---------------------------------------------------------------------------

/// Run a `claude plugin` subcommand and return stdout as String.
/// Strips ALL CLAUDE* env vars per CLAUDE.md hard rules.
async fn run_claude_plugin(args: &[&str]) -> Result<String, ApiError> {
    use std::process::Stdio;

    let cli_path = claude_view_core::resolved_cli_path().unwrap_or("claude");

    let mut cmd = Command::new(cli_path);
    cmd.arg("plugin");
    cmd.args(args);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

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
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ApiError::Internal(format!(
            "claude plugin {} failed: {stderr}",
            args.join(" ")
        )));
    }

    String::from_utf8(output.stdout)
        .map_err(|e| ApiError::Internal(format!("Invalid UTF-8 from CLI: {e}")))
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

/// GET /api/plugins — Unified view of installed + available plugins.
async fn list_plugins(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PluginsQuery>,
) -> ApiResult<Json<PluginsResponse>> {
    // 1. Get installed + available from CLI (non-fatal — empty on failure)
    let (cli_data, cli_error) = match run_claude_plugin(&["list", "--available", "--json"]).await {
        Ok(json) => match serde_json::from_str::<CliAvailableResponse>(&json) {
            Ok(data) => (data, None),
            Err(e) => {
                tracing::warn!("Failed to parse CLI JSON: {e}");
                (
                    CliAvailableResponse {
                        installed: vec![],
                        available: vec![],
                    },
                    Some(format!("Failed to parse plugin data: {e}")),
                )
            }
        },
        Err(e) => {
            tracing::warn!("CLI plugin list failed: {e}");
            (
                CliAvailableResponse {
                    installed: vec![],
                    available: vec![],
                },
                Some(e.to_string()),
            )
        }
    };

    // 2. Snapshot registry data (drop the RwLock guard before any .await)
    let registry_snapshot: HashMap<String, Vec<claude_view_core::registry::InvocableInfo>> = {
        let guard = state.registry.read().unwrap();
        if let Some(reg) = guard.as_ref() {
            let mut map: HashMap<String, Vec<claude_view_core::registry::InvocableInfo>> =
                HashMap::new();
            // Build a plugin_name → invocables map from the entire registry
            for inv in reg.all_invocables() {
                if let Some(ref pn) = inv.plugin_name {
                    map.entry(pn.clone()).or_default().push(inv.clone());
                }
            }
            map
        } else {
            HashMap::new()
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
                    invocation_count: inv_count,
                    last_used_at: last_used,
                });
            }
        }

        let total_invocations: i64 = items.iter().map(|i| i.invocation_count).sum();
        let last_used_at = items.iter().filter_map(|i| i.last_used_at).max();

        // Sort items by usage descending
        items.sort_by(|a, b| b.invocation_count.cmp(&a.invocation_count));

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
        .filter(|p| p.last_used_at.map_or(true, |t| t < thirty_days_ago))
        .count();
    let updatable_count = installed.iter().filter(|p| p.updatable).count();

    let mut all_marketplaces: Vec<String> = name_to_marketplaces
        .values()
        .flatten()
        .cloned()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    all_marketplaces.sort();

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
    /// For install: "user" | "project"
    #[serde(default)]
    pub scope: Option<String>,
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

const VALID_ACTIONS: &[&str] = &["install", "update", "uninstall", "enable", "disable"];

/// Reject CLI flag injection — only [a-zA-Z0-9._@-] allowed, must not start with `-`.
fn validate_plugin_name(name: &str) -> Result<(), ApiError> {
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

fn validate_scope(scope: &Option<String>) -> Result<(), ApiError> {
    if let Some(s) = scope {
        if s != "user" && s != "project" {
            return Err(ApiError::BadRequest(format!(
                "Invalid scope: {s}. Must be 'user' or 'project'."
            )));
        }
    }
    Ok(())
}

// Single-mutation-at-a-time lock (shared across plugin + marketplace mutations).
static MUTATION_LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();

fn get_mutation_lock() -> &'static tokio::sync::Mutex<()> {
    MUTATION_LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

// ---------------------------------------------------------------------------
// Mutation handler
// ---------------------------------------------------------------------------

/// POST /api/plugins/action — Run a plugin mutation via `claude plugin <action>`.
async fn plugin_action(
    Json(req): Json<PluginActionRequest>,
) -> ApiResult<Json<PluginActionResponse>> {
    // Validate inputs before acquiring the lock
    validate_plugin_name(&req.name)?;
    validate_scope(&req.scope)?;

    if !VALID_ACTIONS.contains(&req.action.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "Invalid action: {}. Must be one of: {}",
            req.action,
            VALID_ACTIONS.join(", ")
        )));
    }

    // try_lock — return 409 if another mutation is running
    let _guard = get_mutation_lock().try_lock().map_err(|_| {
        ApiError::Conflict("A plugin mutation is already in progress. Try again shortly.".into())
    })?;

    // Build CLI args
    let mut args: Vec<&str> = vec![&req.action, &req.name];
    let scope_str;
    if let Some(ref scope) = req.scope {
        scope_str = scope.clone();
        args.push("--scope");
        args.push(&scope_str);
    }

    let output = run_claude_plugin(&args).await;

    match output {
        Ok(stdout) => Ok(Json(PluginActionResponse {
            success: true,
            action: req.action,
            name: req.name,
            message: if stdout.trim().is_empty() {
                None
            } else {
                Some(stdout.trim().to_string())
            },
        })),
        Err(e) => {
            // Return a structured error response instead of 500
            tracing::warn!("Plugin action {} {} failed: {e}", req.action, req.name);
            Ok(Json(PluginActionResponse {
                success: false,
                action: req.action,
                name: req.name,
                message: Some(e.to_string()),
            }))
        }
    }
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/plugins", get(list_plugins))
        .route("/plugins/action", post(plugin_action))
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
        axum::Router::new().nest("/api", router()).with_state(state)
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
        assert_eq!(json["totalInstalled"], 0);
        assert_eq!(json["totalAvailable"], 0);
        assert_eq!(json["duplicateCount"], 0);
        assert_eq!(json["unusedCount"], 0);
        assert_eq!(json["updatableCount"], 0);
        assert!(json["marketplaces"].is_array());
        // CLI is unavailable in test → cliError should be present
        assert!(json["cliError"].is_string());
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
                version: "1.0.0".to_string(),
                git_sha: None,
                enabled: true,
                installed_at: "2026-01-01T00:00:00Z".to_string(),
                last_updated: None,
                items: vec![PluginItem {
                    id: "superpowers:brainstorming".to_string(),
                    name: "brainstorming".to_string(),
                    kind: "skill".to_string(),
                    description: "Explore ideas".to_string(),
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
            },
            PluginInfo {
                id: "hookify@marketplace".to_string(),
                name: "hookify".to_string(),
                marketplace: "marketplace".to_string(),
                scope: "project".to_string(),
                version: "2.0.0".to_string(),
                git_sha: None,
                enabled: true,
                installed_at: "2026-02-01T00:00:00Z".to_string(),
                last_updated: None,
                items: vec![PluginItem {
                    id: "hookify:format".to_string(),
                    name: "format".to_string(),
                    kind: "command".to_string(),
                    description: "Format code".to_string(),
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
            },
        ];

        let mut available = vec![AvailablePlugin {
            plugin_id: "other-plugin".to_string(),
            name: "other-plugin".to_string(),
            description: "Does other things".to_string(),
            marketplace_name: "marketplace".to_string(),
            version: "1.0.0".to_string(),
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
                version: "1.0.0".to_string(),
                git_sha: None,
                enabled: true,
                installed_at: "2026-01-01T00:00:00Z".to_string(),
                last_updated: None,
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
            },
            PluginInfo {
                id: "b@m".to_string(),
                name: "b".to_string(),
                marketplace: "m".to_string(),
                scope: "project".to_string(),
                version: "1.0.0".to_string(),
                git_sha: None,
                enabled: true,
                installed_at: "2026-01-01T00:00:00Z".to_string(),
                last_updated: None,
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
            },
        ];

        let mut available = vec![AvailablePlugin {
            plugin_id: "c".to_string(),
            name: "c".to_string(),
            description: "Available".to_string(),
            marketplace_name: "m".to_string(),
            version: "1.0.0".to_string(),
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
    fn test_apply_filters_sort_by_usage() {
        let mut installed = vec![
            PluginInfo {
                id: "low@m".to_string(),
                name: "low-usage".to_string(),
                marketplace: "m".to_string(),
                scope: "user".to_string(),
                version: "1.0.0".to_string(),
                git_sha: None,
                enabled: true,
                installed_at: "2026-01-01T00:00:00Z".to_string(),
                last_updated: None,
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
            },
            PluginInfo {
                id: "high@m".to_string(),
                name: "high-usage".to_string(),
                marketplace: "m".to_string(),
                scope: "user".to_string(),
                version: "1.0.0".to_string(),
                git_sha: None,
                enabled: true,
                installed_at: "2026-01-01T00:00:00Z".to_string(),
                last_updated: None,
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
            },
        ];

        let mut available = vec![];

        let query = PluginsQuery {
            sort: Some("usage".to_string()),
            ..Default::default()
        };
        apply_filters(&query, &mut installed, &mut available);

        // Highest usage first
        assert_eq!(installed[0].name, "high-usage");
        assert_eq!(installed[1].name, "low-usage");
    }

    #[tokio::test]
    async fn test_plugin_action_rejects_invalid_name() {
        let db = Database::new_in_memory().await.expect("in-memory DB");
        let app = build_app(db);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/plugins/action")
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"action":"install","name":"--force"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_plugin_action_rejects_invalid_action() {
        let db = Database::new_in_memory().await.expect("in-memory DB");
        let app = build_app(db);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/plugins/action")
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"action":"rm_rf","name":"test"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_plugin_action_rejects_invalid_scope() {
        let db = Database::new_in_memory().await.expect("in-memory DB");
        let app = build_app(db);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/plugins/action")
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
}
