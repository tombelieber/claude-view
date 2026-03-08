// crates/server/src/routes/plugins.rs
//! Plugin management API routes.
//!
//! - GET /plugins — Unified view of installed + available plugins

use std::sync::Arc;

use axum::{extract::Query, extract::State, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::ApiResult;
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
        // Default sort: by name
        installed.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    }

    // Available plugins are always sorted alphabetically
    available.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

/// GET /api/plugins — Unified view of installed + available plugins.
///
/// Placeholder handler that returns an empty response. The real implementation
/// will be wired in Task 2 (plugin data assembly).
async fn list_plugins(
    State(_state): State<Arc<AppState>>,
    Query(_query): Query<PluginsQuery>,
) -> ApiResult<Json<PluginsResponse>> {
    Ok(Json(PluginsResponse {
        installed: vec![],
        available: vec![],
        total_installed: 0,
        total_available: 0,
        duplicate_count: 0,
        unused_count: 0,
        updatable_count: 0,
        marketplaces: vec![],
    }))
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/plugins", get(list_plugins))
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
}
