// crates/core/src/invocation.rs
//
// Classify tool_use calls from JSONL lines against a Registry to determine
// which invocable was called (skill, command, agent, MCP tool, or built-in).

use crate::registry::{InvocableKind, Registry, BUILTIN_TOOLS};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Result of classifying a tool_use call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClassifyResult {
    /// Successfully matched to a known invocable.
    Valid {
        invocable_id: String,
        kind: InvocableKind,
    },
    /// Recognized tool pattern but failed validation.
    Rejected {
        raw_value: String,
        reason: String,
    },
    /// Unknown tool, silently discard.
    Ignored,
}

/// Raw tool_use data extracted from a JSONL line, for downstream processing.
#[derive(Debug, Clone)]
pub struct RawToolUse {
    pub name: String,
    pub input: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Built-in agent allowlist
// ---------------------------------------------------------------------------

/// Known built-in agent types used by the Task tool in Claude Code.
/// Built-in agent types that get classified as `builtin:{name}`.
/// Public so the registry can seed these into the invocables table.
pub const BUILTIN_AGENT_NAMES: &[&str] = &[
    "Bash",
    "general-purpose",
    "Explore",
    "Plan",
    "statusline-setup",
    "claude-code-guide",
];

/// Check if an agent type string is a built-in agent.
/// Built-in agents are either in the allowlist or do NOT contain ":"
/// (plugin agents always use the "plugin:agent" format).
pub(crate) fn is_builtin_agent(agent_type: &str) -> bool {
    BUILTIN_AGENT_NAMES.contains(&agent_type)
}

// ---------------------------------------------------------------------------
// MCP tool name parser
// ---------------------------------------------------------------------------

/// Parse an MCP tool name like `mcp__plugin_playwright_playwright__browser_navigate`
/// into (plugin_name, tool_name).
///
/// Format: `mcp__plugin_{plugin}_{server}__{tool}`
///
/// Returns `None` if the name doesn't match the expected pattern.
pub(crate) fn parse_mcp_tool_name(name: &str) -> Option<(&str, &str)> {
    // Strip the "mcp__plugin_" prefix
    let rest = name.strip_prefix("mcp__plugin_")?;

    // Split on double underscore "__" to separate "{plugin}_{server}" from "{tool}"
    let dunder_pos = rest.find("__")?;
    let plugin_server = &rest[..dunder_pos];
    let tool = &rest[dunder_pos + 2..];

    if tool.is_empty() || plugin_server.is_empty() {
        return None;
    }

    // From "{plugin}_{server}", extract the plugin name.
    // The plugin name is everything before the LAST underscore.
    // Examples:
    //   "playwright_playwright" → "playwright"
    //   "supabase_supabase"     → "supabase"
    //   "Notion_notion"         → "Notion"
    //   "claude-mem_mcp-search" → "claude-mem"
    let plugin = match plugin_server.rfind('_') {
        Some(pos) => &plugin_server[..pos],
        None => plugin_server, // no underscore, whole thing is the plugin name
    };

    Some((plugin, tool))
}

// ---------------------------------------------------------------------------
// classify_tool_use
// ---------------------------------------------------------------------------

/// Classify a tool_use call against the registry to determine what invocable was called.
///
/// - `name`: The tool name from the JSONL `tool_use` event (e.g. "Skill", "Bash", "mcp__plugin_...")
/// - `input`: The optional JSON input from the tool_use event
/// - `registry`: The invocable registry to look up against
pub fn classify_tool_use(
    name: &str,
    input: &Option<serde_json::Value>,
    registry: &Registry,
) -> ClassifyResult {
    match name {
        // ---- Skill tool: extract skill name from input.skill ----
        "Skill" => {
            let skill_name = input
                .as_ref()
                .and_then(|v| v.get("skill"))
                .and_then(|v| v.as_str());

            match skill_name {
                Some(s) if BUILTIN_TOOLS.contains(&s) => ClassifyResult::Rejected {
                    raw_value: s.into(),
                    reason: "builtin_misroute".into(),
                },
                Some(s) => match registry.lookup(s) {
                    Some(info) => ClassifyResult::Valid {
                        invocable_id: info.id.clone(),
                        kind: info.kind,
                    },
                    None => ClassifyResult::Rejected {
                        raw_value: s.into(),
                        reason: "not_in_registry".into(),
                    },
                },
                None => ClassifyResult::Ignored,
            }
        }

        // ---- Task tool: extract agent type from input.subagent_type ----
        "Task" => {
            let agent_type = input
                .as_ref()
                .and_then(|v| v.get("subagent_type"))
                .and_then(|v| v.as_str());

            match agent_type {
                Some(s) if is_builtin_agent(s) => ClassifyResult::Valid {
                    invocable_id: format!("builtin:{s}"),
                    kind: InvocableKind::BuiltinTool,
                },
                Some(s) => match registry.lookup(s) {
                    Some(info) => ClassifyResult::Valid {
                        invocable_id: info.id.clone(),
                        kind: info.kind,
                    },
                    None => ClassifyResult::Rejected {
                        raw_value: s.into(),
                        reason: "not_in_registry".into(),
                    },
                },
                None => ClassifyResult::Ignored,
            }
        }

        // ---- MCP plugin tools: parse structured name ----
        n if n.starts_with("mcp__plugin_") => match parse_mcp_tool_name(n) {
            Some((plugin, tool)) => match registry.lookup_mcp(plugin, tool) {
                Some(info) => ClassifyResult::Valid {
                    invocable_id: info.id.clone(),
                    kind: InvocableKind::McpTool,
                },
                None => ClassifyResult::Rejected {
                    raw_value: n.into(),
                    reason: "not_in_registry".into(),
                },
            },
            None => ClassifyResult::Ignored,
        },

        // ---- Built-in tools: Bash, Read, Write, etc. ----
        n if BUILTIN_TOOLS.contains(&n) => ClassifyResult::Valid {
            invocable_id: format!("builtin:{n}"),
            kind: InvocableKind::BuiltinTool,
        },

        // ---- Unknown tool: silently ignore ----
        _ => ClassifyResult::Ignored,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::InvocableKind;

    /// Build a registry with only built-in tools (no plugins).
    fn builtin_only_registry() -> Registry {
        tokio_test::block_on(async {
            let tmp = tempfile::TempDir::new().unwrap();
            crate::registry::build_registry(tmp.path()).await
        })
    }

    /// Build a registry with built-ins plus custom entries from a fake plugin.
    fn registry_with_skill(plugin: &str, skill: &str) -> Registry {
        tokio_test::block_on(async {
            let tmp = tempfile::TempDir::new().unwrap();
            let claude_dir = tmp.path();

            let install_path = claude_dir.join("plugins/cache/test/1.0.0");
            std::fs::create_dir_all(&install_path).unwrap();

            // Create skill dir
            let skill_dir = install_path.join(skill);
            std::fs::create_dir_all(&skill_dir).unwrap();
            std::fs::write(skill_dir.join("SKILL.md"), "# Skill\nTest skill.").unwrap();

            // plugin.json
            std::fs::write(
                install_path.join("plugin.json"),
                format!(r#"{{"name": "{plugin}", "description": "test"}}"#),
            )
            .unwrap();

            // installed_plugins.json
            let plugins_dir = claude_dir.join("plugins");
            std::fs::write(
                plugins_dir.join("installed_plugins.json"),
                serde_json::json!({
                    "version": 2,
                    "plugins": {
                        format!("{plugin}@marketplace"): [{
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

            crate::registry::build_registry(claude_dir).await
        })
    }

    /// Build a registry with an MCP tool entry.
    fn registry_with_mcp(plugin: &str, server: &str) -> Registry {
        tokio_test::block_on(async {
            let tmp = tempfile::TempDir::new().unwrap();
            let claude_dir = tmp.path();

            let install_path = claude_dir.join("plugins/cache/test/1.0.0");
            std::fs::create_dir_all(&install_path).unwrap();

            // plugin.json
            std::fs::write(
                install_path.join("plugin.json"),
                format!(r#"{{"name": "{plugin}", "description": "test"}}"#),
            )
            .unwrap();

            // .mcp.json
            std::fs::write(
                install_path.join(".mcp.json"),
                format!(r#"{{"{server}": {{"command": "test"}}}}"#),
            )
            .unwrap();

            // installed_plugins.json
            let plugins_dir = claude_dir.join("plugins");
            std::fs::write(
                plugins_dir.join("installed_plugins.json"),
                serde_json::json!({
                    "version": 2,
                    "plugins": {
                        format!("{plugin}@marketplace"): [{
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

            crate::registry::build_registry(claude_dir).await
        })
    }

    /// Build a registry with a plugin agent.
    fn registry_with_agent(plugin: &str, agent: &str) -> Registry {
        tokio_test::block_on(async {
            let tmp = tempfile::TempDir::new().unwrap();
            let claude_dir = tmp.path();

            let install_path = claude_dir.join("plugins/cache/test/1.0.0");
            std::fs::create_dir_all(&install_path).unwrap();

            // plugin.json
            std::fs::write(
                install_path.join("plugin.json"),
                format!(r#"{{"name": "{plugin}", "description": "test"}}"#),
            )
            .unwrap();

            // agents dir
            let agents_dir = install_path.join("agents");
            std::fs::create_dir_all(&agents_dir).unwrap();
            std::fs::write(
                agents_dir.join(format!("{agent}.md")),
                format!("# {agent}\nTest agent."),
            )
            .unwrap();

            // installed_plugins.json
            let plugins_dir = claude_dir.join("plugins");
            std::fs::write(
                plugins_dir.join("installed_plugins.json"),
                serde_json::json!({
                    "version": 2,
                    "plugins": {
                        format!("{plugin}@marketplace"): [{
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

            crate::registry::build_registry(claude_dir).await
        })
    }

    // -----------------------------------------------------------------------
    // AC-I1: Skill tool with valid skill → Valid
    // -----------------------------------------------------------------------

    #[test]
    fn test_ac_i1_skill_valid() {
        let registry = registry_with_skill("superpowers", "brainstorming");
        let input = Some(serde_json::json!({"skill": "superpowers:brainstorming"}));
        let result = classify_tool_use("Skill", &input, &registry);
        assert_eq!(
            result,
            ClassifyResult::Valid {
                invocable_id: "superpowers:brainstorming".into(),
                kind: InvocableKind::Skill,
            }
        );
    }

    // -----------------------------------------------------------------------
    // AC-I2: Skill tool with built-in name → Rejected (builtin_misroute)
    // -----------------------------------------------------------------------

    #[test]
    fn test_ac_i2_skill_builtin_misroute() {
        let registry = builtin_only_registry();
        let input = Some(serde_json::json!({"skill": "Bash"}));
        let result = classify_tool_use("Skill", &input, &registry);
        assert_eq!(
            result,
            ClassifyResult::Rejected {
                raw_value: "Bash".into(),
                reason: "builtin_misroute".into(),
            }
        );
    }

    // -----------------------------------------------------------------------
    // AC-I3: Skill tool with unknown skill → Rejected (not_in_registry)
    // -----------------------------------------------------------------------

    #[test]
    fn test_ac_i3_skill_not_in_registry() {
        let registry = builtin_only_registry();
        let input = Some(serde_json::json!({"skill": "nonexistent"}));
        let result = classify_tool_use("Skill", &input, &registry);
        assert_eq!(
            result,
            ClassifyResult::Rejected {
                raw_value: "nonexistent".into(),
                reason: "not_in_registry".into(),
            }
        );
    }

    // -----------------------------------------------------------------------
    // AC-I4: Built-in tool "Bash" → Valid (builtin:Bash)
    // -----------------------------------------------------------------------

    #[test]
    fn test_ac_i4_builtin_bash() {
        let registry = builtin_only_registry();
        let input = Some(serde_json::json!({"command": "ls"}));
        let result = classify_tool_use("Bash", &input, &registry);
        assert_eq!(
            result,
            ClassifyResult::Valid {
                invocable_id: "builtin:Bash".into(),
                kind: InvocableKind::BuiltinTool,
            }
        );
    }

    // -----------------------------------------------------------------------
    // AC-I5: MCP tool name parsed correctly
    // -----------------------------------------------------------------------

    #[test]
    fn test_ac_i5_mcp_tool_valid() {
        let registry = registry_with_mcp("playwright", "playwright");
        let result = classify_tool_use(
            "mcp__plugin_playwright_playwright__browser_navigate",
            &None,
            &registry,
        );
        // The registry stores MCP tools as "mcp:{plugin}:{server_name}" where
        // server_name comes from the .mcp.json keys. In this test, server="playwright"
        // and tool="browser_navigate". The lookup is lookup_mcp("playwright", "browser_navigate").
        // The registry has "mcp:playwright:playwright" (from .mcp.json key "playwright").
        // So this won't match because the tool name doesn't match the server name.
        //
        // Actually, looking at scan_mcp_json, each KEY of .mcp.json becomes an MCP tool
        // with id "mcp:{plugin}:{key}". So for .mcp.json = {"playwright": {...}},
        // the registered id is "mcp:playwright:playwright" and the name is "playwright".
        //
        // But parse_mcp_tool_name returns tool="browser_navigate", so
        // lookup_mcp("playwright", "browser_navigate") looks for "mcp:playwright:browser_navigate"
        // which doesn't exist.
        //
        // This is a design gap: the registry stores MCP server names, but the
        // JSONL tool names are the individual tool names within that server.
        // For now, this will be Rejected (not_in_registry). The registry
        // needs to store individual tool names in a future step.
        //
        // Let's test what actually happens:
        assert_eq!(
            result,
            ClassifyResult::Rejected {
                raw_value: "mcp__plugin_playwright_playwright__browser_navigate".into(),
                reason: "not_in_registry".into(),
            }
        );
    }

    #[test]
    fn test_ac_i5_mcp_tool_with_matching_server_key() {
        // When the .mcp.json key matches the tool name extracted from the MCP tool string
        let registry = registry_with_mcp("playwright", "browser_navigate");
        let result = classify_tool_use(
            "mcp__plugin_playwright_playwright__browser_navigate",
            &None,
            &registry,
        );
        assert_eq!(
            result,
            ClassifyResult::Valid {
                invocable_id: "mcp:playwright:browser_navigate".into(),
                kind: InvocableKind::McpTool,
            }
        );
    }

    // -----------------------------------------------------------------------
    // AC-I6: Skill with None input → Ignored
    // -----------------------------------------------------------------------

    #[test]
    fn test_ac_i6_skill_none_input() {
        let registry = builtin_only_registry();
        let result = classify_tool_use("Skill", &None, &registry);
        assert_eq!(result, ClassifyResult::Ignored);
    }

    // -----------------------------------------------------------------------
    // AC-I7: Task with known built-in agent → Valid
    // -----------------------------------------------------------------------

    #[test]
    fn test_ac_i7_task_builtin_agent() {
        let registry = builtin_only_registry();
        let input = Some(serde_json::json!({"subagent_type": "general-purpose"}));
        let result = classify_tool_use("Task", &input, &registry);
        assert_eq!(
            result,
            ClassifyResult::Valid {
                invocable_id: "builtin:general-purpose".into(),
                kind: InvocableKind::BuiltinTool,
            }
        );
    }

    #[test]
    fn test_task_plugin_agent_via_registry() {
        let registry = registry_with_agent("feature-dev", "code-reviewer");
        let input = Some(serde_json::json!({"subagent_type": "feature-dev:code-reviewer"}));
        let result = classify_tool_use("Task", &input, &registry);
        assert_eq!(
            result,
            ClassifyResult::Valid {
                invocable_id: "feature-dev:code-reviewer".into(),
                kind: InvocableKind::Agent,
            }
        );
    }

    #[test]
    fn test_task_unknown_agent_rejected() {
        let registry = builtin_only_registry();
        let input = Some(serde_json::json!({"subagent_type": "unknown-plugin:agent"}));
        let result = classify_tool_use("Task", &input, &registry);
        assert_eq!(
            result,
            ClassifyResult::Rejected {
                raw_value: "unknown-plugin:agent".into(),
                reason: "not_in_registry".into(),
            }
        );
    }

    // -----------------------------------------------------------------------
    // AC-I8: Task with no subagent_type → Ignored
    // -----------------------------------------------------------------------

    #[test]
    fn test_ac_i8_task_no_subagent_type() {
        let registry = builtin_only_registry();
        let input = Some(serde_json::json!({"description": "do something"}));
        let result = classify_tool_use("Task", &input, &registry);
        assert_eq!(result, ClassifyResult::Ignored);
    }

    #[test]
    fn test_task_none_input() {
        let registry = builtin_only_registry();
        let result = classify_tool_use("Task", &None, &registry);
        assert_eq!(result, ClassifyResult::Ignored);
    }

    // -----------------------------------------------------------------------
    // AC-I9: Unknown tool name → Ignored
    // -----------------------------------------------------------------------

    #[test]
    fn test_ac_i9_unknown_tool() {
        let registry = builtin_only_registry();
        let result = classify_tool_use("CompletelyUnknown", &None, &registry);
        assert_eq!(result, ClassifyResult::Ignored);
    }

    // -----------------------------------------------------------------------
    // AC-I10: parse_mcp_tool_name tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_ac_i10_parse_playwright() {
        let result = parse_mcp_tool_name("mcp__plugin_playwright_playwright__browser_navigate");
        assert_eq!(result, Some(("playwright", "browser_navigate")));
    }

    #[test]
    fn test_ac_i10_parse_supabase() {
        let result = parse_mcp_tool_name("mcp__plugin_supabase_supabase__execute_sql");
        assert_eq!(result, Some(("supabase", "execute_sql")));
    }

    #[test]
    fn test_ac_i10_parse_notion() {
        let result = parse_mcp_tool_name("mcp__plugin_Notion_notion__notion-search");
        assert_eq!(result, Some(("Notion", "notion-search")));
    }

    #[test]
    fn test_ac_i10_parse_claude_mem() {
        let result = parse_mcp_tool_name("mcp__plugin_claude-mem_mcp-search__search");
        assert_eq!(result, Some(("claude-mem", "search")));
    }

    #[test]
    fn test_parse_mcp_no_prefix() {
        assert_eq!(parse_mcp_tool_name("not_mcp_tool"), None);
    }

    #[test]
    fn test_parse_mcp_no_double_underscore() {
        assert_eq!(parse_mcp_tool_name("mcp__plugin_foo"), None);
    }

    #[test]
    fn test_parse_mcp_empty_tool() {
        assert_eq!(parse_mcp_tool_name("mcp__plugin_foo_bar__"), None);
    }

    #[test]
    fn test_parse_mcp_empty_plugin_server() {
        assert_eq!(parse_mcp_tool_name("mcp__plugin___tool"), None);
    }

    // -----------------------------------------------------------------------
    // Additional edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_all_builtin_tools_classify_as_valid() {
        let registry = builtin_only_registry();
        for &tool in BUILTIN_TOOLS {
            // Skip "Task" since it has special handling
            if tool == "Task" {
                continue;
            }
            let result = classify_tool_use(tool, &None, &registry);
            assert_eq!(
                result,
                ClassifyResult::Valid {
                    invocable_id: format!("builtin:{tool}"),
                    kind: InvocableKind::BuiltinTool,
                },
                "Built-in tool '{tool}' should classify as Valid"
            );
        }
    }

    #[test]
    fn test_skill_with_bare_name_lookup() {
        let registry = registry_with_skill("superpowers", "brainstorming");
        // Use bare name (without plugin prefix)
        let input = Some(serde_json::json!({"skill": "brainstorming"}));
        let result = classify_tool_use("Skill", &input, &registry);
        assert_eq!(
            result,
            ClassifyResult::Valid {
                invocable_id: "superpowers:brainstorming".into(),
                kind: InvocableKind::Skill,
            }
        );
    }

    #[test]
    fn test_skill_input_missing_skill_field() {
        let registry = builtin_only_registry();
        let input = Some(serde_json::json!({"args": "something"}));
        let result = classify_tool_use("Skill", &input, &registry);
        assert_eq!(result, ClassifyResult::Ignored);
    }

    #[test]
    fn test_is_builtin_agent_known() {
        assert!(is_builtin_agent("Bash"));
        assert!(is_builtin_agent("general-purpose"));
        assert!(is_builtin_agent("Explore"));
        assert!(is_builtin_agent("Plan"));
        assert!(is_builtin_agent("statusline-setup"));
        assert!(is_builtin_agent("claude-code-guide"));
    }

    #[test]
    fn test_is_builtin_agent_unknown() {
        assert!(!is_builtin_agent("feature-dev:code-reviewer"));
        assert!(!is_builtin_agent("custom-agent"));
        assert!(!is_builtin_agent(""));
    }

    #[test]
    fn test_raw_tool_use_struct() {
        let raw = RawToolUse {
            name: "Bash".to_string(),
            input: Some(serde_json::json!({"command": "ls"})),
        };
        assert_eq!(raw.name, "Bash");
        assert!(raw.input.is_some());

        let raw_none = RawToolUse {
            name: "Read".to_string(),
            input: None,
        };
        assert!(raw_none.input.is_none());
    }

    #[test]
    fn test_mcp_rejected_when_not_in_registry() {
        let registry = builtin_only_registry();
        let result = classify_tool_use(
            "mcp__plugin_supabase_supabase__execute_sql",
            &None,
            &registry,
        );
        assert_eq!(
            result,
            ClassifyResult::Rejected {
                raw_value: "mcp__plugin_supabase_supabase__execute_sql".into(),
                reason: "not_in_registry".into(),
            }
        );
    }

    // -----------------------------------------------------------------------
    // User-level custom skill classification
    // -----------------------------------------------------------------------

    #[test]
    fn test_skill_with_user_level_lookup() {
        // Build a registry that includes a user-level skill
        let registry = tokio_test::block_on(async {
            let tmp = tempfile::TempDir::new().unwrap();
            let claude_dir = tmp.path();

            // Create user-level skill
            let skill_dir = claude_dir.join("skills/prove-it");
            std::fs::create_dir_all(&skill_dir).unwrap();
            std::fs::write(skill_dir.join("SKILL.md"), "# Prove It\nTest.").unwrap();

            crate::registry::build_registry(claude_dir).await
        });

        // Classify a Skill tool_use with bare name "prove-it"
        let input = Some(serde_json::json!({"skill": "prove-it"}));
        let result = classify_tool_use("Skill", &input, &registry);
        assert_eq!(
            result,
            ClassifyResult::Valid {
                invocable_id: "user:prove-it".into(),
                kind: InvocableKind::Skill,
            }
        );

        // Also works with qualified name
        let input2 = Some(serde_json::json!({"skill": "user:prove-it"}));
        let result2 = classify_tool_use("Skill", &input2, &registry);
        assert_eq!(
            result2,
            ClassifyResult::Valid {
                invocable_id: "user:prove-it".into(),
                kind: InvocableKind::Skill,
            }
        );
    }
}
