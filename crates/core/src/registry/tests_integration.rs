// crates/core/src/registry/tests_integration.rs
//
// Integration tests for build_registry() with fake plugin structures
// and user-level scanner functions.

#[cfg(test)]
mod tests {
    use super::super::build::build_registry;
    use super::super::scanner::{scan_user_agents, scan_user_commands};
    use super::super::types::{InvocableKind, BUILTIN_TOOLS};
    use std::fs;
    use tempfile::TempDir;

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
        assert!(
            mcp.is_some(),
            "MCP tool 'mcp:test-plugin:my-server' not found"
        );
        assert_eq!(mcp.unwrap().kind, InvocableKind::McpTool);

        // Check bare name lookup
        let bare = registry.lookup("my-skill");
        assert!(bare.is_some(), "Bare name 'my-skill' not found");

        // Total: 1 skill + 1 command + 1 agent + 1 MCP + 31 builtins = 35
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
        assert_eq!(skill.unwrap().plugin_name.as_deref(), Some("fallback-test"));
    }

    // -----------------------------------------------------------------------
    // Empty plugins dir -> empty registry (no crash)
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
    // User-level custom skills
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_user_level_skills_registered() {
        let tmp = TempDir::new().unwrap();
        let claude_dir = tmp.path();

        // Create user-level skill: {claude_dir}/skills/prove-it/SKILL.md
        let skill_dir = claude_dir.join("skills/prove-it");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "# Prove It\nAudit proposed fixes.",
        )
        .unwrap();

        // Create another user-level skill
        let skill_dir2 = claude_dir.join("skills/shippable");
        fs::create_dir_all(&skill_dir2).unwrap();
        fs::write(
            skill_dir2.join("SKILL.md"),
            "# Shippable\nPost-implementation audit.",
        )
        .unwrap();

        let registry = build_registry(claude_dir).await;

        // User skills should be found by qualified name
        let prove_it = registry.lookup("user:prove-it");
        assert!(prove_it.is_some(), "User skill 'user:prove-it' not found");
        assert_eq!(prove_it.unwrap().kind, InvocableKind::Skill);
        assert_eq!(prove_it.unwrap().name, "prove-it");
        assert!(
            prove_it.unwrap().plugin_name.is_none(),
            "User skills should have no plugin_name"
        );
        assert_eq!(prove_it.unwrap().description, "Audit proposed fixes.");

        // User skills should also be found by bare name
        let bare = registry.lookup("prove-it");
        assert!(bare.is_some(), "Bare name 'prove-it' not found");
        assert_eq!(bare.unwrap().id, "user:prove-it");

        // Second skill should also be present
        let shippable = registry.lookup("user:shippable");
        assert!(shippable.is_some(), "User skill 'user:shippable' not found");

        // Total: 2 user skills + builtins
        assert_eq!(registry.len(), 2 + num_builtins());
    }

    #[tokio::test]
    async fn test_plugin_skills_take_precedence_over_user_skills() {
        let tmp = TempDir::new().unwrap();
        let claude_dir = tmp.path();

        // Create a plugin skill named "brainstorming"
        let install_path = claude_dir.join("plugins/cache/superpowers/1.0.0");
        fs::create_dir_all(&install_path).unwrap();
        fs::write(
            install_path.join("plugin.json"),
            r#"{"name": "superpowers", "description": "test"}"#,
        )
        .unwrap();
        let plugin_skill = install_path.join("brainstorming");
        fs::create_dir_all(&plugin_skill).unwrap();
        fs::write(
            plugin_skill.join("SKILL.md"),
            "# Brainstorming\nFrom plugin.",
        )
        .unwrap();

        let plugins_dir = claude_dir.join("plugins");
        fs::write(
            plugins_dir.join("installed_plugins.json"),
            serde_json::json!({
                "version": 2,
                "plugins": {
                    "superpowers@marketplace": [{
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

        // Also create a user-level skill with the SAME bare name "brainstorming"
        let user_skill = claude_dir.join("skills/brainstorming");
        fs::create_dir_all(&user_skill).unwrap();
        fs::write(user_skill.join("SKILL.md"), "# Brainstorming\nFrom user.").unwrap();

        let registry = build_registry(claude_dir).await;

        // Plugin skill should exist under its qualified name
        let plugin = registry.lookup("superpowers:brainstorming");
        assert!(plugin.is_some());
        assert_eq!(plugin.unwrap().description, "From plugin.");

        // User skill should exist under its qualified name (different ID)
        let user = registry.lookup("user:brainstorming");
        assert!(user.is_some());
        assert_eq!(user.unwrap().description, "From user.");

        // Bare name lookup should return plugin (registered first)
        let bare = registry.lookup("brainstorming");
        assert!(bare.is_some());
        assert_eq!(
            bare.unwrap().id,
            "superpowers:brainstorming",
            "Plugin skill should win bare-name lookup (registered first)"
        );
    }

    #[tokio::test]
    async fn test_no_user_skills_dir_no_crash() {
        let tmp = TempDir::new().unwrap();
        let claude_dir = tmp.path();
        // Don't create {claude_dir}/skills/ at all

        let registry = build_registry(claude_dir).await;

        // Should still work, just no user skills
        assert_eq!(registry.len(), num_builtins());
        assert!(registry.lookup("user:anything").is_none());
    }

    #[tokio::test]
    async fn test_fingerprint_deterministic() {
        let tmp = TempDir::new().unwrap();
        let claude_dir = tmp.path();

        let r1 = build_registry(claude_dir).await;
        let r2 = build_registry(claude_dir).await;

        assert_eq!(r1.fingerprint(), r2.fingerprint());
        assert!(!r1.fingerprint().is_empty());
    }

    #[tokio::test]
    async fn test_fingerprint_changes_with_new_skill() {
        let tmp = TempDir::new().unwrap();
        let claude_dir = tmp.path();

        let before = build_registry(claude_dir).await;

        // Add a user skill
        let skill_dir = claude_dir.join("skills/my-new-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: my-new-skill\ndescription: test\n---\nHello",
        )
        .unwrap();

        let after = build_registry(claude_dir).await;

        assert_ne!(before.fingerprint(), after.fingerprint());
    }

    // -----------------------------------------------------------------------
    // User-level custom commands
    // -----------------------------------------------------------------------

    #[test]
    fn test_scan_user_commands_reads_md_files() {
        let tmp = TempDir::new().unwrap();
        let cmds = tmp.path().join("commands");
        std::fs::create_dir_all(&cmds).unwrap();
        std::fs::write(cmds.join("wtf.md"), "# wtf\nsome content").unwrap();
        std::fs::write(cmds.join("notes.md"), "# notes").unwrap();

        let result = scan_user_commands(tmp.path());
        assert_eq!(result.len(), 2);
        let names: Vec<_> = result.iter().map(|i| i.name.as_str()).collect();
        assert!(names.contains(&"wtf"));
        assert!(names.contains(&"notes"));
        // Must have no plugin_name
        assert!(result.iter().all(|i| i.plugin_name.is_none()));
    }

    #[test]
    fn test_scan_user_agents_reads_md_files() {
        let tmp = TempDir::new().unwrap();
        let agents = tmp.path().join("agents");
        std::fs::create_dir_all(&agents).unwrap();
        std::fs::write(agents.join("full-codebase-docs-sync-scanner.md"), "# agent").unwrap();

        let result = scan_user_agents(tmp.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "full-codebase-docs-sync-scanner");
        assert!(result[0].plugin_name.is_none());
    }

    #[test]
    fn test_scan_user_commands_empty_when_dir_missing() {
        let tmp = TempDir::new().unwrap();
        let result = scan_user_commands(tmp.path());
        assert!(result.is_empty());
    }

    #[test]
    fn test_scan_user_agents_empty_when_dir_missing() {
        let tmp = TempDir::new().unwrap();
        let result = scan_user_agents(tmp.path());
        assert!(result.is_empty());
    }
}
