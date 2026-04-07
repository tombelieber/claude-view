// crates/core/src/registry/scanner.rs
//
// Filesystem scanners for plugin skills, commands, agents, MCP tools,
// and user-level custom invocables.

use std::path::Path;
use tracing::warn;

use super::parse::{read_first_line_description, read_plugin_json, McpJson};
use super::types::{InvocableInfo, InvocableKind};

// ---------------------------------------------------------------------------
// Plugin scanners
// ---------------------------------------------------------------------------

/// Scan for skills as direct subdirectories containing SKILL.md.
/// Falls back to `skills/*/SKILL.md` nested layout if flat layout finds nothing.
pub(crate) fn scan_skills(
    install_path: &Path,
    plugin_name: &str,
    _plugin_desc: &str,
) -> Vec<InvocableInfo> {
    let mut results = Vec::new();

    // Try flat layout first: {installPath}/*/SKILL.md
    if let Ok(entries) = std::fs::read_dir(install_path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                let skill_md = entry_path.join("SKILL.md");
                if skill_md.exists() {
                    let skill_name = entry_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let description = read_first_line_description(&skill_md);
                    let content = std::fs::read_to_string(&skill_md).unwrap_or_default();
                    results.push(InvocableInfo {
                        id: format!("{plugin_name}:{skill_name}"),
                        plugin_name: Some(plugin_name.to_string()),
                        name: skill_name,
                        kind: InvocableKind::Skill,
                        description,
                        content,
                    });
                }
            }
        }
    }

    // Fall back to nested layout: {installPath}/skills/*/SKILL.md
    if results.is_empty() {
        let skills_dir = install_path.join("skills");
        if let Ok(entries) = std::fs::read_dir(&skills_dir) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if entry_path.is_dir() {
                    let skill_md = entry_path.join("SKILL.md");
                    if skill_md.exists() {
                        let skill_name = entry_path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown")
                            .to_string();
                        let description = read_first_line_description(&skill_md);
                        let content = std::fs::read_to_string(&skill_md).unwrap_or_default();
                        results.push(InvocableInfo {
                            id: format!("{plugin_name}:{skill_name}"),
                            plugin_name: Some(plugin_name.to_string()),
                            name: skill_name,
                            kind: InvocableKind::Skill,
                            description,
                            content,
                        });
                    }
                }
            }
        }
    }

    results
}

/// Scan a directory for .md files and register each as the given kind.
pub(crate) fn scan_md_dir(
    dir: &Path,
    plugin_name: &str,
    kind: InvocableKind,
) -> Vec<InvocableInfo> {
    let mut results = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return results, // dir doesn't exist, that's fine
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("md") {
            let name = path
                .file_stem()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            let description = read_first_line_description(&path);
            let content = std::fs::read_to_string(&path).unwrap_or_default();
            results.push(InvocableInfo {
                id: format!("{plugin_name}:{name}"),
                plugin_name: Some(plugin_name.to_string()),
                name,
                kind,
                description,
                content,
            });
        }
    }

    results
}

/// Read .mcp.json and register each key as an MCP tool.
pub(crate) fn scan_mcp_json(install_path: &Path, plugin_name: &str) -> Vec<InvocableInfo> {
    let mcp_path = install_path.join(".mcp.json");
    let data = match std::fs::read_to_string(&mcp_path) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    let mcp: McpJson = match serde_json::from_str(&data) {
        Ok(m) => m,
        Err(e) => {
            warn!("Failed to parse .mcp.json at {}: {e}", mcp_path.display());
            return Vec::new();
        }
    };

    mcp.iter()
        .map(|(server_name, server_config)| InvocableInfo {
            id: format!("mcp:{plugin_name}:{server_name}"),
            plugin_name: Some(plugin_name.to_string()),
            name: server_name.clone(),
            kind: InvocableKind::McpTool,
            description: String::new(),
            content: serde_json::to_string_pretty(server_config).unwrap_or_default(),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// User-level scanners
// ---------------------------------------------------------------------------

/// Scan user-level custom skills at `{claude_dir}/skills/*/SKILL.md`.
/// These are skills created by the user directly, not installed via plugins.
pub(crate) fn scan_user_skills(claude_dir: &Path) -> Vec<InvocableInfo> {
    let skills_dir = claude_dir.join("skills");
    let entries = match std::fs::read_dir(&skills_dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(), // dir doesn't exist, that's fine
    };

    let mut results = Vec::new();
    for entry in entries.flatten() {
        let entry_path = entry.path();
        if entry_path.is_dir() {
            let skill_md = entry_path.join("SKILL.md");
            if skill_md.exists() {
                let skill_name = entry_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                let description = read_first_line_description(&skill_md);
                let content = std::fs::read_to_string(&skill_md).unwrap_or_default();
                results.push(InvocableInfo {
                    id: format!("user:{skill_name}"),
                    plugin_name: None,
                    name: skill_name,
                    kind: InvocableKind::Skill,
                    description,
                    content,
                });
            }
        }
    }

    results
}

/// Scan user-level custom commands at `{claude_dir}/commands/*.md`.
pub(crate) fn scan_user_commands(claude_dir: &Path) -> Vec<InvocableInfo> {
    let dir = claude_dir.join("commands");
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };
    let mut results = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let name = match path.file_stem().and_then(|s| s.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        let description = read_first_line_description(&path);
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        results.push(InvocableInfo {
            id: format!("user:command:{name}"),
            plugin_name: None,
            name,
            kind: InvocableKind::Command,
            description,
            content,
        });
    }
    results
}

/// Scan user-level custom agents at `{claude_dir}/agents/*.md`.
pub(crate) fn scan_user_agents(claude_dir: &Path) -> Vec<InvocableInfo> {
    let dir = claude_dir.join("agents");
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };
    let mut results = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let name = match path.file_stem().and_then(|s| s.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        let description = read_first_line_description(&path);
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        results.push(InvocableInfo {
            id: format!("user:agent:{name}"),
            plugin_name: None,
            name,
            kind: InvocableKind::Agent,
            description,
            content,
        });
    }
    results
}
