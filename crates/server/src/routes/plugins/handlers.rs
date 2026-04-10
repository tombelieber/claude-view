//! Route handlers for the plugin management API.
//!
//! - GET  /plugins                    -- Unified view of installed + available plugins
//! - GET  /plugins/marketplaces       -- List configured marketplaces
//! - POST /plugins/marketplaces/action -- Marketplace mutations (add/remove/update)

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use axum::{extract::Query, extract::State, Json};
use claude_view_core::registry::{InvocableInfo, InvocableKind};

use super::cli::{
    fetch_plugin_cli_data, invalidate_plugin_cache, run_claude_plugin, CliAvailableResponse,
    CliMarketplace,
};
use super::enrichment::read_disk_enrichment;
use super::filters::{apply_filters, parse_plugin_id};
use super::types::*;
use super::validation::{
    get_marketplace_lock, validate_marketplace_source, validate_plugin_name, validate_scope,
};

// ---------------------------------------------------------------------------
// GET /api/plugins
// ---------------------------------------------------------------------------

/// GET /api/plugins -- Unified view of installed + available plugins.
#[utoipa::path(get, path = "/api/plugins", tag = "plugins",
    responses(
        (status = 200, description = "Unified plugin list", body = PluginsResponse),
    )
)]
pub async fn list_plugins(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PluginsQuery>,
) -> ApiResult<Json<PluginsResponse>> {
    // 1. Get installed + available from cache (non-fatal -- empty on failure)
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

    // 2. Snapshot registry data -- keyed map for plugin bucketing + flat list for user items
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
    let disk = read_disk_enrichment();

    let available_by_name: HashMap<String, &_> = cli_data
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
        items.sort_by(|a, b| b.invocation_count.cmp(&a.invocation_count));

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
    user_skills.sort_by(|a, b| b.total_invocations.cmp(&a.total_invocations));
    user_commands.sort_by(|a, b| b.total_invocations.cmp(&a.total_invocations));
    user_agents.sort_by(|a, b| b.total_invocations.cmp(&a.total_invocations));

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
// GET /api/plugins/marketplaces
// ---------------------------------------------------------------------------

/// GET /api/plugins/marketplaces
#[utoipa::path(get, path = "/api/plugins/marketplaces", tag = "plugins",
    responses(
        (status = 200, description = "Configured marketplaces", body = Vec<MarketplaceInfo>),
    )
)]
pub async fn list_marketplaces(
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

// ---------------------------------------------------------------------------
// POST /api/plugins/marketplaces/action
// ---------------------------------------------------------------------------

/// POST /api/plugins/marketplaces/action
#[utoipa::path(post, path = "/api/plugins/marketplaces/action", tag = "plugins",
    request_body = MarketplaceActionRequest,
    responses(
        (status = 200, description = "Marketplace action result", body = PluginActionResponse),
        (status = 400, description = "Invalid action or missing required fields"),
        (status = 409, description = "A mutation is already in progress"),
        (status = 500, description = "Internal error"),
    )
)]
pub async fn marketplace_action(
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
            let name = req.name.as_deref().ok_or_else(|| {
                ApiError::BadRequest(
                    "Use POST /api/plugins/marketplaces/refresh-all for bulk updates".into(),
                )
            })?;
            validate_plugin_name(name)?;

            let _guard = get_marketplace_lock()
                .try_lock()
                .map_err(|_| ApiError::Conflict("A mutation is already in progress.".into()))?;

            let result = match run_claude_plugin(&["marketplace", "update", name]).await {
                Ok(stdout) => Ok(Json(PluginActionResponse {
                    success: true,
                    action: "update".into(),
                    name: name.to_string(),
                    message: if stdout.trim().is_empty() {
                        None
                    } else {
                        Some(stdout.trim().to_string())
                    },
                })),
                Err(e) => Ok(Json(PluginActionResponse {
                    success: false,
                    action: "update".into(),
                    name: name.to_string(),
                    message: Some(e.to_string()),
                })),
            };
            invalidate_plugin_cache(&state).await;
            result
        }
        _ => unreachable!(),
    }
}
