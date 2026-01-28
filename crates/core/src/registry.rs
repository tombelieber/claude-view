// crates/core/src/registry.rs
//
// Parse ~/.claude/plugins/installed_plugins.json, scan plugin directories,
// and build lookup maps for all invocables (skills, commands, agents, MCP tools, built-in tools).

use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InvocableKind {
    Skill,
    Command,
    Agent,
    McpTool,
    BuiltinTool,
}

impl std::fmt::Display for InvocableKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InvocableKind::Skill => write!(f, "skill"),
            InvocableKind::Command => write!(f, "command"),
            InvocableKind::Agent => write!(f, "agent"),
            InvocableKind::McpTool => write!(f, "mcp_tool"),
            InvocableKind::BuiltinTool => write!(f, "builtin_tool"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct InvocableInfo {
    /// Qualified ID: "superpowers:brainstorming", "mcp:playwright:browser_navigate", "builtin:Bash"
    pub id: String,
    /// Plugin name if from a plugin, None for built-ins
    pub plugin_name: Option<String>,
    /// Short name: "brainstorming", "Bash", "browser_navigate"
    pub name: String,
    /// Classification
    pub kind: InvocableKind,
    /// Human-readable description (may be empty)
    pub description: String,
}

#[derive(Clone)]
pub struct Registry {
    /// Qualified ID → InvocableInfo  (e.g. "superpowers:brainstorming")
    qualified: HashMap<String, InvocableInfo>,
    /// Bare name → possibly multiple matches (e.g. "brainstorming" → [info1, info2])
    bare: HashMap<String, Vec<InvocableInfo>>,
}

impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Registry")
            .field("qualified_count", &self.qualified.len())
            .field("bare_count", &self.bare.len())
            .finish()
    }
}

impl Registry {
    /// Look up an invocable by qualified or bare name.
    /// Tries qualified first ("superpowers:brainstorming"), then bare ("brainstorming").
    pub fn lookup(&self, name: &str) -> Option<&InvocableInfo> {
        self.qualified
            .get(name)
            .or_else(|| self.bare.get(name).and_then(|v| v.first()))
    }

    /// Look up an MCP tool by plugin name and tool name.
    /// Constructs "mcp:{plugin}:{tool}" and looks it up in the qualified map.
    pub fn lookup_mcp(&self, plugin: &str, tool: &str) -> Option<&InvocableInfo> {
        let id = format!("mcp:{plugin}:{tool}");
        self.qualified.get(&id)
    }

    /// Iterate over all registered invocables.
    pub fn all_invocables(&self) -> impl Iterator<Item = &InvocableInfo> {
        self.qualified.values()
    }

    /// Number of registered invocables.
    pub fn len(&self) -> usize {
        self.qualified.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.qualified.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Built-in tools
// ---------------------------------------------------------------------------

pub const BUILTIN_TOOLS: &[&str] = &[
    "Bash",
    "Read",
    "Write",
    "Edit",
    "Glob",
    "Grep",
    "Task",
    "TaskCreate",
    "TaskUpdate",
    "TaskList",
    "TaskGet",
    "TaskOutput",
    "TaskStop",
    "WebFetch",
    "WebSearch",
    "AskUserQuestion",
    "EnterPlanMode",
    "ExitPlanMode",
    "NotebookEdit",
    "ToolSearch",
];

// ---------------------------------------------------------------------------
// Deserialization types for installed_plugins.json
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct InstalledPlugins {
    #[allow(dead_code)]
    version: u32,
    plugins: HashMap<String, Vec<PluginEntry>>,
}

#[derive(Debug, Deserialize)]
struct PluginEntry {
    #[allow(dead_code)]
    scope: String,
    #[serde(rename = "installPath")]
    install_path: String,
    #[allow(dead_code)]
    version: String,
    #[serde(rename = "installedAt")]
    #[allow(dead_code)]
    installed_at: String,
}

/// Optional plugin.json at the root of a plugin's install path.
#[derive(Debug, Deserialize)]
struct PluginJson {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

/// .mcp.json at the root of a plugin's install path.
/// Keys are MCP server names, values are server configs.
type McpJson = HashMap<String, serde_json::Value>;

// ---------------------------------------------------------------------------
// build_registry()
// ---------------------------------------------------------------------------

/// Build the invocable registry by scanning the Claude plugins directory.
///
/// Reads `{claude_dir}/plugins/installed_plugins.json` and scans each plugin's
/// install path for skills, commands, agents, and MCP tool definitions.
/// Also registers all known built-in tools.
///
/// All filesystem operations are graceful — missing files/dirs are logged and skipped.
pub async fn build_registry(claude_dir: &Path) -> Registry {
    let mut entries: Vec<InvocableInfo> = Vec::new();

    // 1. Read installed_plugins.json
    let plugins_path = claude_dir.join("plugins/installed_plugins.json");
    if let Some(installed) = read_installed_plugins(&plugins_path) {
        // 2. Scan each plugin
        for (plugin_key, versions) in &installed.plugins {
            // Use the first (most recent) entry — installed_plugins.json only lists the active version
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
                let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

                // Scan for skills: direct subdirectories containing SKILL.md
                let skills = scan_skills(&install_path, display_name, plugin_description);
                for s in skills {
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
                    entries.push(a);
                }

                // Read .mcp.json for MCP tools
                let mcp_tools = scan_mcp_json(&install_path, display_name);
                entries.extend(mcp_tools);
            }
        }
    }

    // 3. Register built-in tools
    for &tool_name in BUILTIN_TOOLS {
        entries.push(InvocableInfo {
            id: format!("builtin:{tool_name}"),
            plugin_name: None,
            name: tool_name.to_string(),
            kind: InvocableKind::BuiltinTool,
            description: String::new(),
        });
    }

    // 4. Register built-in agents (Task subagent types that classify_tool_use
    //    maps to "builtin:{name}" — must exist in invocables for FK constraint)
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
        });
    }

    // 5. Build lookup maps
    build_maps(entries)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn read_installed_plugins(path: &Path) -> Option<InstalledPlugins> {
    let data = match std::fs::read_to_string(path) {
        Ok(d) => d,
        Err(e) => {
            debug!("Could not read installed_plugins.json at {}: {e}", path.display());
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

fn read_plugin_json(install_path: &Path) -> Option<PluginJson> {
    let path = install_path.join("plugin.json");
    let data = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&data).ok()
}

/// Extract plugin name from key like "superpowers@superpowers-marketplace" → "superpowers"
fn extract_plugin_name(key: &str) -> String {
    key.split('@').next().unwrap_or(key).to_string()
}

/// Scan for skills as direct subdirectories containing SKILL.md.
/// Falls back to `skills/*/SKILL.md` nested layout if flat layout finds nothing.
fn scan_skills(install_path: &Path, plugin_name: &str, _plugin_desc: &str) -> Vec<InvocableInfo> {
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
                    results.push(InvocableInfo {
                        id: format!("{plugin_name}:{skill_name}"),
                        plugin_name: Some(plugin_name.to_string()),
                        name: skill_name,
                        kind: InvocableKind::Skill,
                        description,
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
                        results.push(InvocableInfo {
                            id: format!("{plugin_name}:{skill_name}"),
                            plugin_name: Some(plugin_name.to_string()),
                            name: skill_name,
                            kind: InvocableKind::Skill,
                            description,
                        });
                    }
                }
            }
        }
    }

    results
}

/// Scan a directory for .md files and register each as the given kind.
fn scan_md_dir(dir: &Path, plugin_name: &str, kind: InvocableKind) -> Vec<InvocableInfo> {
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
            results.push(InvocableInfo {
                id: format!("{plugin_name}:{name}"),
                plugin_name: Some(plugin_name.to_string()),
                name,
                kind,
                description,
            });
        }
    }

    results
}

/// Read .mcp.json and register each key as an MCP tool.
fn scan_mcp_json(install_path: &Path, plugin_name: &str) -> Vec<InvocableInfo> {
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

    mcp.keys()
        .map(|server_name| InvocableInfo {
            id: format!("mcp:{plugin_name}:{server_name}"),
            plugin_name: Some(plugin_name.to_string()),
            name: server_name.clone(),
            kind: InvocableKind::McpTool,
            description: String::new(),
        })
        .collect()
}

/// Read the first non-empty, non-heading line from a markdown file as a description.
fn read_first_line_description(path: &Path) -> String {
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

/// Build the qualified and bare lookup maps from a list of InvocableInfo entries.
fn build_maps(entries: Vec<InvocableInfo>) -> Registry {
    let mut qualified = HashMap::with_capacity(entries.len());
    let mut bare: HashMap<String, Vec<InvocableInfo>> = HashMap::new();

    for info in entries {
        // Insert into qualified map (last write wins if duplicate ID)
        if qualified.contains_key(&info.id) {
            warn!("Duplicate invocable ID: {}", info.id);
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Helper to build a registry from a vec of InvocableInfo
    fn registry_from(entries: Vec<InvocableInfo>) -> Registry {
        build_maps(entries)
    }

    /// Count unique builtins (tools + agents, deduplicating "Bash" which appears in both).
    fn num_builtins() -> usize {
        let mut ids: std::collections::HashSet<String> = std::collections::HashSet::new();
        for &t in BUILTIN_TOOLS {
            ids.insert(format!("builtin:{t}"));
        }
        for &a in crate::invocation::BUILTIN_AGENT_NAMES {
            ids.insert(format!("builtin:{a}"));
        }
        ids.len()
    }

    // -----------------------------------------------------------------------
    // Registry::lookup() tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_lookup_qualified_name() {
        let registry = registry_from(vec![InvocableInfo {
            id: "superpowers:brainstorming".to_string(),
            plugin_name: Some("superpowers".to_string()),
            name: "brainstorming".to_string(),
            kind: InvocableKind::Skill,
            description: "Explore ideas".to_string(),
        }]);

        let result = registry.lookup("superpowers:brainstorming");
        assert!(result.is_some());
        assert_eq!(result.unwrap().id, "superpowers:brainstorming");
    }

    #[test]
    fn test_lookup_bare_name() {
        let registry = registry_from(vec![InvocableInfo {
            id: "commit-commands:commit".to_string(),
            plugin_name: Some("commit-commands".to_string()),
            name: "commit".to_string(),
            kind: InvocableKind::Command,
            description: "Create a git commit".to_string(),
        }]);

        let result = registry.lookup("commit");
        assert!(result.is_some());
        assert_eq!(result.unwrap().id, "commit-commands:commit");
    }

    #[test]
    fn test_lookup_not_found() {
        let registry = registry_from(vec![]);
        assert!(registry.lookup("nonexistent").is_none());
    }

    #[test]
    fn test_lookup_qualified_takes_precedence_over_bare() {
        let registry = registry_from(vec![
            InvocableInfo {
                id: "plugin-a:foo".to_string(),
                plugin_name: Some("plugin-a".to_string()),
                name: "foo".to_string(),
                kind: InvocableKind::Skill,
                description: String::new(),
            },
            InvocableInfo {
                id: "plugin-b:foo".to_string(),
                plugin_name: Some("plugin-b".to_string()),
                name: "foo".to_string(),
                kind: InvocableKind::Command,
                description: String::new(),
            },
        ]);

        // Qualified lookup returns exact match
        let result = registry.lookup("plugin-b:foo");
        assert!(result.is_some());
        assert_eq!(result.unwrap().kind, InvocableKind::Command);

        // Bare lookup returns first match
        let result = registry.lookup("foo");
        assert!(result.is_some());
        // Should return one of them (first inserted)
        assert_eq!(result.unwrap().name, "foo");
    }

    // -----------------------------------------------------------------------
    // Registry::lookup_mcp() tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_lookup_mcp_found() {
        let registry = registry_from(vec![InvocableInfo {
            id: "mcp:playwright:browser_navigate".to_string(),
            plugin_name: Some("playwright".to_string()),
            name: "browser_navigate".to_string(),
            kind: InvocableKind::McpTool,
            description: String::new(),
        }]);

        let result = registry.lookup_mcp("playwright", "browser_navigate");
        assert!(result.is_some());
        assert_eq!(result.unwrap().id, "mcp:playwright:browser_navigate");
    }

    #[test]
    fn test_lookup_mcp_not_found() {
        let registry = registry_from(vec![]);
        assert!(registry.lookup_mcp("nonexistent", "tool").is_none());
    }

    // -----------------------------------------------------------------------
    // Built-in tools tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_builtin_tools_all_registered() {
        // Build registry with an empty dir (no plugins)
        let tmp = TempDir::new().unwrap();
        let registry = build_registry(tmp.path()).await;

        // All built-in tools should be present
        for &tool in BUILTIN_TOOLS {
            let id = format!("builtin:{tool}");
            let result = registry.lookup(&id);
            assert!(result.is_some(), "Built-in tool '{tool}' not found in registry");
            assert_eq!(result.unwrap().kind, InvocableKind::BuiltinTool);
            assert!(result.unwrap().plugin_name.is_none());
        }

        // Should have all builtin tools + unique builtin agents
        // (20 tools + 5 unique agents = 25; "Bash" is in both lists)
        assert_eq!(registry.len(), num_builtins());
    }

    // -----------------------------------------------------------------------
    // build_registry() with fake plugin structure
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_build_registry_with_fake_plugin() {
        let tmp = TempDir::new().unwrap();
        let claude_dir = tmp.path();

        // Create plugin install path
        let install_path = claude_dir.join("plugins/cache/test-plugin/1.0.0");
        fs::create_dir_all(&install_path).unwrap();

        // Create plugin.json
        fs::write(
            install_path.join("plugin.json"),
            r#"{"name": "test-plugin", "description": "A test plugin"}"#,
        )
        .unwrap();

        // Create a skill: {installPath}/my-skill/SKILL.md
        let skill_dir = install_path.join("my-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "# My Skill\nThis skill does something useful.",
        )
        .unwrap();

        // Create commands dir with a command
        let commands_dir = install_path.join("commands");
        fs::create_dir_all(&commands_dir).unwrap();
        fs::write(commands_dir.join("deploy.md"), "# Deploy\nDeploy the app.").unwrap();

        // Create agents dir with an agent
        let agents_dir = install_path.join("agents");
        fs::create_dir_all(&agents_dir).unwrap();
        fs::write(agents_dir.join("reviewer.md"), "# Reviewer\nReview code.").unwrap();

        // Create .mcp.json
        fs::write(
            install_path.join(".mcp.json"),
            r#"{"my-server": {"command": "npx", "args": ["my-server"]}}"#,
        )
        .unwrap();

        // Create installed_plugins.json
        let plugins_dir = claude_dir.join("plugins");
        // plugins_dir already exists from install_path creation
        fs::write(
            plugins_dir.join("installed_plugins.json"),
            serde_json::json!({
                "version": 2,
                "plugins": {
                    "test-plugin@test-marketplace": [{
                        "scope": "user",
                        "installPath": install_path.to_str().unwrap(),
                        "version": "1.0.0",
                        "installedAt": "2026-01-01T00:00:00Z"
                    }]
                }
            })
            .to_string(),
        )
        .unwrap();

        let registry = build_registry(claude_dir).await;

        // Check skill
        let skill = registry.lookup("test-plugin:my-skill");
        assert!(skill.is_some(), "Skill 'test-plugin:my-skill' not found");
        assert_eq!(skill.unwrap().kind, InvocableKind::Skill);
        assert_eq!(
            skill.unwrap().description,
            "This skill does something useful."
        );

        // Check command
        let cmd = registry.lookup("test-plugin:deploy");
        assert!(cmd.is_some(), "Command 'test-plugin:deploy' not found");
        assert_eq!(cmd.unwrap().kind, InvocableKind::Command);

        // Check agent
        let agent = registry.lookup("test-plugin:reviewer");
        assert!(agent.is_some(), "Agent 'test-plugin:reviewer' not found");
        assert_eq!(agent.unwrap().kind, InvocableKind::Agent);

        // Check MCP tool
        let mcp = registry.lookup_mcp("test-plugin", "my-server");
        assert!(mcp.is_some(), "MCP tool 'mcp:test-plugin:my-server' not found");
        assert_eq!(mcp.unwrap().kind, InvocableKind::McpTool);

        // Check bare name lookup
        let bare = registry.lookup("my-skill");
        assert!(bare.is_some(), "Bare name 'my-skill' not found");

        // Total: 1 skill + 1 command + 1 agent + 1 MCP + 25 builtins = 29
        assert_eq!(registry.len(), 4 + num_builtins());
    }

    // -----------------------------------------------------------------------
    // Missing plugin.json falls back to key name
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_missing_plugin_json_uses_key_name() {
        let tmp = TempDir::new().unwrap();
        let claude_dir = tmp.path();

        // Create plugin install path WITHOUT plugin.json
        let install_path = claude_dir.join("plugins/cache/fallback-test/1.0.0");
        fs::create_dir_all(&install_path).unwrap();

        // Create a skill
        let skill_dir = install_path.join("my-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "# Skill\nA skill.").unwrap();

        // Create installed_plugins.json (note: key is "fallback-test@marketplace")
        let plugins_dir = claude_dir.join("plugins");
        fs::write(
            plugins_dir.join("installed_plugins.json"),
            serde_json::json!({
                "version": 2,
                "plugins": {
                    "fallback-test@marketplace": [{
                        "scope": "user",
                        "installPath": install_path.to_str().unwrap(),
                        "version": "1.0.0",
                        "installedAt": "2026-01-01T00:00:00Z"
                    }]
                }
            })
            .to_string(),
        )
        .unwrap();

        let registry = build_registry(claude_dir).await;

        // Should use "fallback-test" (from key, split on @) as plugin name
        let skill = registry.lookup("fallback-test:my-skill");
        assert!(
            skill.is_some(),
            "Should find skill using key-derived plugin name"
        );
        assert_eq!(
            skill.unwrap().plugin_name.as_deref(),
            Some("fallback-test")
        );
    }

    // -----------------------------------------------------------------------
    // Empty plugins dir → empty registry (no crash)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_empty_plugins_dir_no_crash() {
        let tmp = TempDir::new().unwrap();
        let claude_dir = tmp.path();

        // Create plugins dir but no installed_plugins.json
        fs::create_dir_all(claude_dir.join("plugins")).unwrap();

        let registry = build_registry(claude_dir).await;

        // Should have only builtins (tools + unique agents)
        assert_eq!(registry.len(), num_builtins());
        assert!(!registry.is_empty());
    }

    #[tokio::test]
    async fn test_completely_missing_claude_dir() {
        let tmp = TempDir::new().unwrap();
        let nonexistent = tmp.path().join("does-not-exist");

        let registry = build_registry(&nonexistent).await;

        // Should still have builtins, no crash
        assert_eq!(registry.len(), num_builtins());
    }

    // -----------------------------------------------------------------------
    // Nested skills fallback
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_nested_skills_fallback() {
        let tmp = TempDir::new().unwrap();
        let claude_dir = tmp.path();

        let install_path = claude_dir.join("plugins/cache/nested-plugin/1.0.0");
        fs::create_dir_all(&install_path).unwrap();

        // Create nested layout: skills/my-skill/SKILL.md
        let skill_dir = install_path.join("skills/nested-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "# Nested\nNested skill.").unwrap();

        let plugins_dir = claude_dir.join("plugins");
        fs::write(
            plugins_dir.join("installed_plugins.json"),
            serde_json::json!({
                "version": 2,
                "plugins": {
                    "nested-plugin@marketplace": [{
                        "scope": "user",
                        "installPath": install_path.to_str().unwrap(),
                        "version": "1.0.0",
                        "installedAt": "2026-01-01T00:00:00Z"
                    }]
                }
            })
            .to_string(),
        )
        .unwrap();

        let registry = build_registry(claude_dir).await;

        let skill = registry.lookup("nested-plugin:nested-skill");
        assert!(skill.is_some(), "Should find nested skill via fallback");
    }

    // -----------------------------------------------------------------------
    // Bare name ambiguity
    // -----------------------------------------------------------------------

    #[test]
    fn test_bare_name_ambiguous_returns_first() {
        let registry = registry_from(vec![
            InvocableInfo {
                id: "plugin-a:commit".to_string(),
                plugin_name: Some("plugin-a".to_string()),
                name: "commit".to_string(),
                kind: InvocableKind::Command,
                description: "From A".to_string(),
            },
            InvocableInfo {
                id: "plugin-b:commit".to_string(),
                plugin_name: Some("plugin-b".to_string()),
                name: "commit".to_string(),
                kind: InvocableKind::Command,
                description: "From B".to_string(),
            },
        ]);

        // Bare lookup should return *some* match (first one inserted)
        let result = registry.lookup("commit");
        assert!(result.is_some());
        assert_eq!(result.unwrap().name, "commit");

        // Qualified lookups should be distinct
        let a = registry.lookup("plugin-a:commit");
        let b = registry.lookup("plugin-b:commit");
        assert!(a.is_some());
        assert!(b.is_some());
        assert_eq!(a.unwrap().description, "From A");
        assert_eq!(b.unwrap().description, "From B");
    }

    // -----------------------------------------------------------------------
    // len() and is_empty()
    // -----------------------------------------------------------------------

    #[test]
    fn test_len_and_is_empty() {
        let empty = registry_from(vec![]);
        assert_eq!(empty.len(), 0);
        assert!(empty.is_empty());

        let one = registry_from(vec![InvocableInfo {
            id: "builtin:Bash".to_string(),
            plugin_name: None,
            name: "Bash".to_string(),
            kind: InvocableKind::BuiltinTool,
            description: String::new(),
        }]);
        assert_eq!(one.len(), 1);
        assert!(!one.is_empty());
    }

    // -----------------------------------------------------------------------
    // extract_plugin_name helper
    // -----------------------------------------------------------------------

    #[test]
    fn test_extract_plugin_name() {
        assert_eq!(
            extract_plugin_name("superpowers@superpowers-marketplace"),
            "superpowers"
        );
        assert_eq!(
            extract_plugin_name("commit-commands@marketplace"),
            "commit-commands"
        );
        assert_eq!(extract_plugin_name("no-at-sign"), "no-at-sign");
        assert_eq!(extract_plugin_name(""), "");
    }

    // -----------------------------------------------------------------------
    // all_invocables iterator
    // -----------------------------------------------------------------------

    #[test]
    fn test_all_invocables() {
        let registry = registry_from(vec![
            InvocableInfo {
                id: "a:one".to_string(),
                plugin_name: Some("a".to_string()),
                name: "one".to_string(),
                kind: InvocableKind::Skill,
                description: String::new(),
            },
            InvocableInfo {
                id: "b:two".to_string(),
                plugin_name: Some("b".to_string()),
                name: "two".to_string(),
                kind: InvocableKind::Command,
                description: String::new(),
            },
        ]);

        let all: Vec<_> = registry.all_invocables().collect();
        assert_eq!(all.len(), 2);
    }

    // -----------------------------------------------------------------------
    // InvocableKind Display
    // -----------------------------------------------------------------------

    #[test]
    fn test_invocable_kind_display() {
        assert_eq!(InvocableKind::Skill.to_string(), "skill");
        assert_eq!(InvocableKind::Command.to_string(), "command");
        assert_eq!(InvocableKind::Agent.to_string(), "agent");
        assert_eq!(InvocableKind::McpTool.to_string(), "mcp_tool");
        assert_eq!(InvocableKind::BuiltinTool.to_string(), "builtin_tool");
    }
}
