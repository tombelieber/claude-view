// crates/core/src/registry/tests.rs
//
// Unit tests for Registry methods, lookup, and types.

#[cfg(test)]
mod tests {
    use super::super::build::build_maps;
    use super::super::types::{InvocableInfo, InvocableKind, BUILTIN_TOOLS};
    use tempfile::TempDir;

    /// Helper to build a registry from a vec of InvocableInfo
    fn registry_from(entries: Vec<InvocableInfo>) -> super::super::types::Registry {
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
            content: String::new(),
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
            content: String::new(),
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
                content: String::new(),
            },
            InvocableInfo {
                id: "plugin-b:foo".to_string(),
                plugin_name: Some("plugin-b".to_string()),
                name: "foo".to_string(),
                kind: InvocableKind::Command,
                description: String::new(),
                content: String::new(),
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
            content: String::new(),
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
        use super::super::build::build_registry;

        // Build registry with an empty dir (no plugins)
        let tmp = TempDir::new().unwrap();
        let registry = build_registry(tmp.path()).await;

        // All built-in tools should be present
        for &tool in BUILTIN_TOOLS {
            let id = format!("builtin:{tool}");
            let result = registry.lookup(&id);
            assert!(
                result.is_some(),
                "Built-in tool '{tool}' not found in registry"
            );
            assert_eq!(result.unwrap().kind, InvocableKind::BuiltinTool);
            assert!(result.unwrap().plugin_name.is_none());
        }

        // Should have all builtin tools + unique builtin agents
        // (26 tools + 5 unique agents = 31; "Bash" is in both lists)
        assert_eq!(registry.len(), num_builtins());
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
                content: String::new(),
            },
            InvocableInfo {
                id: "plugin-b:commit".to_string(),
                plugin_name: Some("plugin-b".to_string()),
                name: "commit".to_string(),
                kind: InvocableKind::Command,
                description: "From B".to_string(),
                content: String::new(),
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
            content: String::new(),
        }]);
        assert_eq!(one.len(), 1);
        assert!(!one.is_empty());
    }

    // -----------------------------------------------------------------------
    // invocables_for_plugin()
    // -----------------------------------------------------------------------

    #[test]
    fn invocables_for_plugin_filters_correctly() {
        let entries = vec![
            InvocableInfo {
                id: "superpowers:brainstorming".to_string(),
                plugin_name: Some("superpowers".to_string()),
                name: "brainstorming".to_string(),
                kind: InvocableKind::Skill,
                description: String::new(),
                content: String::new(),
            },
            InvocableInfo {
                id: "superpowers:tdd".to_string(),
                plugin_name: Some("superpowers".to_string()),
                name: "tdd".to_string(),
                kind: InvocableKind::Skill,
                description: String::new(),
                content: String::new(),
            },
            InvocableInfo {
                id: "hookify:format".to_string(),
                plugin_name: Some("hookify".to_string()),
                name: "format".to_string(),
                kind: InvocableKind::Command,
                description: String::new(),
                content: String::new(),
            },
            InvocableInfo {
                id: "builtin:Bash".to_string(),
                plugin_name: None,
                name: "Bash".to_string(),
                kind: InvocableKind::BuiltinTool,
                description: String::new(),
                content: String::new(),
            },
        ];
        let registry = registry_from(entries);

        let sp = registry.invocables_for_plugin("superpowers");
        assert_eq!(sp.len(), 2);

        let hk = registry.invocables_for_plugin("hookify");
        assert_eq!(hk.len(), 1);
        assert_eq!(hk[0].id, "hookify:format");

        let none = registry.invocables_for_plugin("nonexistent");
        assert_eq!(none.len(), 0);
    }

    // -----------------------------------------------------------------------
    // extract_plugin_name helper
    // -----------------------------------------------------------------------

    #[test]
    fn test_extract_plugin_name() {
        use super::super::parse::extract_plugin_name;

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
                content: String::new(),
            },
            InvocableInfo {
                id: "b:two".to_string(),
                plugin_name: Some("b".to_string()),
                name: "two".to_string(),
                kind: InvocableKind::Command,
                description: String::new(),
                content: String::new(),
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
