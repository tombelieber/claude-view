//! Plugin ID parsing and query filter logic.

use super::types::{AvailablePlugin, PluginInfo, PluginsQuery};

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
/// Pure function for testability -- no CLI or database access.
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
            // user or project scope -- only show installed plugins matching that scope
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
        // Available plugins don't have kind metadata -- don't filter them by kind
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
