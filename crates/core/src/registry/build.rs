// crates/core/src/registry/build.rs
//
// build_registry() orchestrator: reads installed_plugins.json, scans plugin
// directories, registers builtins, and assembles the final Registry.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tracing::debug;

use super::parse::{extract_plugin_name, read_installed_plugins, read_plugin_json};
use super::scanner::{
    scan_mcp_json, scan_md_dir, scan_skills, scan_user_agents, scan_user_commands, scan_user_skills,
};
use super::types::{InvocableInfo, InvocableKind, Registry, BUILTIN_TOOLS};

// ---------------------------------------------------------------------------
// build_registry()
// ---------------------------------------------------------------------------

/// Build the invocable registry by scanning the Claude plugins directory.
///
/// Reads `{claude_dir}/plugins/installed_plugins.json` and scans each plugin's
/// install path for skills, commands, agents, and MCP tool definitions.
/// Also registers all known built-in tools.
///
/// All filesystem operations are graceful -- missing files/dirs are logged and skipped.
pub async fn build_registry(claude_dir: &Path) -> Registry {
    let mut entries: Vec<InvocableInfo> = Vec::new();
    // Track IDs globally to avoid duplicates when the same plugin is installed at multiple scopes
    let mut global_seen_ids: HashSet<String> = HashSet::new();

    // 1. Read installed_plugins.json
    let plugins_path = claude_dir.join("plugins/installed_plugins.json");
    if let Some(installed) = read_installed_plugins(&plugins_path) {
        // 2. Scan each plugin
        for (plugin_key, versions) in &installed.plugins {
            // Use the first (most recent) entry -- installed_plugins.json only lists the active version
            if let Some(entry) = versions.first() {
                let install_path = PathBuf::from(&entry.install_path);
                let plugin_name = extract_plugin_name(plugin_key);

                // Read plugin.json for metadata
                let plugin_meta = read_plugin_json(&install_path);
                let display_name = plugin_meta
                    .as_ref()
                    .and_then(|p| p.name.as_deref())
                    .unwrap_or(&plugin_name);
                let plugin_description = plugin_meta
                    .as_ref()
                    .and_then(|p| p.description.as_deref())
                    .unwrap_or("");

                // Track IDs we've seen for this plugin to avoid duplicates
                // (some plugins register the same name as both skill and command)
                let mut seen_ids: HashSet<String> = HashSet::new();

                // Scan for skills: direct subdirectories containing SKILL.md
                let skills = scan_skills(&install_path, display_name, plugin_description);
                for s in skills {
                    if !global_seen_ids.insert(s.id.clone()) {
                        debug!("Skipping duplicate invocable (multi-scope): {}", s.id);
                        continue;
                    }
                    seen_ids.insert(s.id.clone());
                    entries.push(s);
                }

                // Scan for commands: commands/*.md
                let commands = scan_md_dir(
                    &install_path.join("commands"),
                    display_name,
                    InvocableKind::Command,
                );
                for c in commands {
                    if !seen_ids.insert(c.id.clone()) {
                        debug!("Skipping duplicate invocable from commands: {}", c.id);
                        continue;
                    }
                    if !global_seen_ids.insert(c.id.clone()) {
                        debug!("Skipping duplicate invocable (multi-scope): {}", c.id);
                        continue;
                    }
                    entries.push(c);
                }

                // Scan for agents: agents/*.md
                let agents = scan_md_dir(
                    &install_path.join("agents"),
                    display_name,
                    InvocableKind::Agent,
                );
                for a in agents {
                    if !seen_ids.insert(a.id.clone()) {
                        debug!("Skipping duplicate invocable from agents: {}", a.id);
                        continue;
                    }
                    if !global_seen_ids.insert(a.id.clone()) {
                        debug!("Skipping duplicate invocable (multi-scope): {}", a.id);
                        continue;
                    }
                    entries.push(a);
                }

                // Read .mcp.json for MCP tools
                let mcp_tools = scan_mcp_json(&install_path, display_name);
                for t in mcp_tools {
                    if !global_seen_ids.insert(t.id.clone()) {
                        debug!("Skipping duplicate MCP tool (multi-scope): {}", t.id);
                        continue;
                    }
                    entries.push(t);
                }
            }
        }
    }

    // 2a. Scan user-level custom skills: {claude_dir}/skills/*/SKILL.md
    let user_skills = scan_user_skills(claude_dir);
    for s in user_skills {
        if !global_seen_ids.insert(s.id.clone()) {
            debug!("Skipping duplicate user skill: {}", s.id);
            continue;
        }
        entries.push(s);
    }

    let user_commands = scan_user_commands(claude_dir);
    for c in user_commands {
        if !global_seen_ids.insert(c.id.clone()) {
            debug!("Skipping duplicate user command: {}", c.id);
            continue;
        }
        entries.push(c);
    }

    let user_agents = scan_user_agents(claude_dir);
    for a in user_agents {
        if !global_seen_ids.insert(a.id.clone()) {
            debug!("Skipping duplicate user agent: {}", a.id);
            continue;
        }
        entries.push(a);
    }

    // 3. Register built-in tools
    for &tool_name in BUILTIN_TOOLS {
        entries.push(InvocableInfo {
            id: format!("builtin:{tool_name}"),
            plugin_name: None,
            name: tool_name.to_string(),
            kind: InvocableKind::BuiltinTool,
            description: String::new(),
            content: String::new(),
        });
    }

    // 4. Register built-in agents (Task subagent types that classify_tool_use
    //    maps to "builtin:{name}" -- must exist in invocables for FK constraint)
    for &agent_name in crate::invocation::BUILTIN_AGENT_NAMES {
        let id = format!("builtin:{agent_name}");
        // Skip if already registered (e.g. "Bash" is both a tool and an agent)
        if entries.iter().any(|e| e.id == id) {
            continue;
        }
        entries.push(InvocableInfo {
            id,
            plugin_name: None,
            name: agent_name.to_string(),
            kind: InvocableKind::BuiltinTool,
            description: String::new(),
            content: String::new(),
        });
    }

    // 5. Build lookup maps
    build_maps(entries)
}

/// Build the qualified and bare lookup maps from a list of InvocableInfo entries.
pub(crate) fn build_maps(entries: Vec<InvocableInfo>) -> Registry {
    let mut qualified = HashMap::with_capacity(entries.len());
    let mut bare: HashMap<String, Vec<InvocableInfo>> = HashMap::new();

    for info in entries {
        // Insert into qualified map (last write wins if duplicate ID)
        if qualified.contains_key(&info.id) {
            debug!("Duplicate invocable ID in build_maps: {}", info.id);
        }
        qualified.insert(info.id.clone(), info.clone());

        // Insert into bare name map
        bare.entry(info.name.clone()).or_default().push(info);
    }

    debug!(
        "Registry built: {} qualified entries, {} bare names",
        qualified.len(),
        bare.len()
    );

    Registry { qualified, bare }
}
