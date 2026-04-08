// crates/core/src/registry/types.rs
//
// Public types: InvocableKind, InvocableInfo, Registry, and BUILTIN_TOOLS constant.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// InvocableKind
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

// ---------------------------------------------------------------------------
// InvocableInfo
// ---------------------------------------------------------------------------

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
    /// Full file content (markdown for skills/commands/agents; pretty JSON for mcp_tool; empty for builtins)
    pub content: String,
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct Registry {
    /// Qualified ID -> InvocableInfo  (e.g. "superpowers:brainstorming")
    pub(crate) qualified: HashMap<String, InvocableInfo>,
    /// Bare name -> possibly multiple matches (e.g. "brainstorming" -> [info1, info2])
    pub(crate) bare: HashMap<String, Vec<InvocableInfo>>,
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

    /// Get all invocables belonging to a specific plugin.
    pub fn invocables_for_plugin(&self, plugin_name: &str) -> Vec<&InvocableInfo> {
        self.qualified
            .values()
            .filter(|info| info.plugin_name.as_deref() == Some(plugin_name))
            .collect()
    }

    /// Compute a stable fingerprint of the registry contents.
    ///
    /// Returns a hex-encoded hash of all sorted qualified IDs.
    /// Used to detect when the registry changes (new plugins installed,
    /// user skills added/removed) so the indexer can auto-reindex.
    pub fn fingerprint(&self) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut ids: Vec<&str> = self.qualified.keys().map(|s| s.as_str()).collect();
        ids.sort_unstable();

        let mut hasher = DefaultHasher::new();
        ids.len().hash(&mut hasher);
        for id in &ids {
            id.hash(&mut hasher);
        }
        format!("{:016x}", hasher.finish())
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
    "Agent",
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
    // Added: tools present in real JSONL data but previously missing
    "TodoWrite",
    "SendMessage",
    "TeamCreate",
    "TeamDelete",
    "CronCreate",
];
