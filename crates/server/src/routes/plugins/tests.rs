#[cfg(test)]
mod tests {
    use crate::routes::plugins::cli::*;
    use crate::routes::plugins::enrichment::*;
    use crate::routes::plugins::filters::*;
    use crate::routes::plugins::types::*;
    use crate::routes::plugins::validation::*;

    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use claude_view_db::Database;
    use tower::ServiceExt;

    /// Helper: build a minimal router with just the plugins route.
    fn build_app(db: Database) -> axum::Router {
        let state = crate::state::AppState::new(db);
        axum::Router::new()
            .nest("/api", crate::routes::plugins::router())
            .nest("/api", crate::routes::plugin_ops::router())
            .with_state(state)
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
        assert!(json["totalInstalled"].is_number());
        assert!(json["totalAvailable"].is_number());
        assert!(json["duplicateCount"].is_number());
        assert!(json["unusedCount"].is_number());
        assert!(json["updatableCount"].is_number());
        assert!(json["marketplaces"].is_array());
    }

    #[tokio::test]
    async fn test_plugins_response_includes_user_sections() {
        let db = Database::new_in_memory().await.expect("in-memory DB");
        let app = build_app(db);
        let (status, body) = get_response(app, "/api/plugins").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        // New fields must exist (even if empty arrays/zero)
        assert!(json["userSkills"].is_array(), "missing userSkills");
        assert!(json["userCommands"].is_array(), "missing userCommands");
        assert!(json["userAgents"].is_array(), "missing userAgents");
        assert!(json["orphanCount"].is_number(), "missing orphanCount");
    }

    #[tokio::test]
    async fn test_user_item_path_format_matches_mockup() {
        // Verify that the path field uses kind-aware formatting:
        // - skills: "prove-it/SKILL.md" (name/SKILL.md)
        // - commands: "commands/wtf.md" (commands/name.md)
        // - agents: "agents/scanner.md" (agents/name.md)
        let db = Database::new_in_memory().await.expect("in-memory DB");
        let app = build_app(db);
        let (status, body) = get_response(app, "/api/plugins").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();

        // If there are user skills, verify path format
        if let Some(skills) = json["userSkills"].as_array() {
            for skill in skills {
                let path = skill["path"].as_str().unwrap();
                assert!(
                    path.ends_with("/SKILL.md"),
                    "skill path '{path}' should end with /SKILL.md"
                );
            }
        }
        if let Some(commands) = json["userCommands"].as_array() {
            for cmd in commands {
                let path = cmd["path"].as_str().unwrap();
                assert!(
                    path.starts_with("commands/"),
                    "command path '{path}' should start with commands/"
                );
                assert!(
                    path.ends_with(".md"),
                    "command path '{path}' should end with .md"
                );
            }
        }
        if let Some(agents) = json["userAgents"].as_array() {
            for agent in agents {
                let path = agent["path"].as_str().unwrap();
                assert!(
                    path.starts_with("agents/"),
                    "agent path '{path}' should start with agents/"
                );
                assert!(
                    path.ends_with(".md"),
                    "agent path '{path}' should end with .md"
                );
            }
        }
    }

    #[test]
    fn test_parse_plugin_id() {
        // Normal case: name@marketplace
        let (name, marketplace) = parse_plugin_id("superpowers@superpowers-marketplace");
        assert_eq!(name, "superpowers");
        assert_eq!(marketplace, "superpowers-marketplace");

        // No @ sign -- full string is name, empty marketplace
        let (name, marketplace) = parse_plugin_id("standalone");
        assert_eq!(name, "standalone");
        assert_eq!(marketplace, "");

        // Multiple @ signs -- split on LAST one
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
                version: Some("1.0.0".to_string()),
                git_sha: None,
                enabled: true,
                installed_at: "2026-01-01T00:00:00Z".to_string(),
                last_updated: None,
                project_path: None,
                items: vec![PluginItem {
                    id: "superpowers:brainstorming".to_string(),
                    name: "brainstorming".to_string(),
                    kind: "skill".to_string(),
                    description: "Explore ideas".to_string(),
                    content: String::new(),
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
                source_exists: true,
                description: None,
                install_count: None,
            },
            PluginInfo {
                id: "hookify@marketplace".to_string(),
                name: "hookify".to_string(),
                marketplace: "marketplace".to_string(),
                scope: "project".to_string(),
                version: Some("2.0.0".to_string()),
                git_sha: None,
                enabled: true,
                installed_at: "2026-02-01T00:00:00Z".to_string(),
                last_updated: None,
                project_path: None,
                items: vec![PluginItem {
                    id: "hookify:format".to_string(),
                    name: "format".to_string(),
                    kind: "command".to_string(),
                    description: "Format code".to_string(),
                    content: String::new(),
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
                source_exists: true,
                description: None,
                install_count: None,
            },
        ];

        let mut available = vec![AvailablePlugin {
            plugin_id: "other-plugin".to_string(),
            name: "other-plugin".to_string(),
            description: "Does other things".to_string(),
            marketplace_name: "marketplace".to_string(),
            version: Some("1.0.0".to_string()),
            install_count: None,
            already_installed: false,
        }];

        // Search for "super" -- should match superpowers, not hookify
        let query = PluginsQuery {
            search: Some("super".to_string()),
            ..Default::default()
        };
        apply_filters(&query, &mut installed, &mut available);

        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].name, "superpowers");

        // Available should also be filtered -- "other-plugin" doesn't match "super"
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
                version: Some("1.0.0".to_string()),
                git_sha: None,
                enabled: true,
                installed_at: "2026-01-01T00:00:00Z".to_string(),
                last_updated: None,
                project_path: None,
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
                source_exists: true,
                description: None,
                install_count: None,
            },
            PluginInfo {
                id: "b@m".to_string(),
                name: "b".to_string(),
                marketplace: "m".to_string(),
                scope: "project".to_string(),
                version: Some("1.0.0".to_string()),
                git_sha: None,
                enabled: true,
                installed_at: "2026-01-01T00:00:00Z".to_string(),
                last_updated: None,
                project_path: None,
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
                source_exists: true,
                description: None,
                install_count: None,
            },
        ];

        let mut available = vec![AvailablePlugin {
            plugin_id: "c".to_string(),
            name: "c".to_string(),
            description: "Available".to_string(),
            marketplace_name: "m".to_string(),
            version: Some("1.0.0".to_string()),
            install_count: None,
            already_installed: false,
        }];

        // Filter by scope "user" -- should keep only user-scoped installed, clear available
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
    fn test_apply_filters_sort_by_install_count() {
        let mut installed = vec![
            PluginInfo {
                id: "low@m".to_string(),
                name: "low-usage".to_string(),
                marketplace: "m".to_string(),
                scope: "user".to_string(),
                version: Some("1.0.0".to_string()),
                git_sha: None,
                enabled: true,
                installed_at: "2026-01-01T00:00:00Z".to_string(),
                last_updated: None,
                project_path: None,
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
                source_exists: true,
                description: None,
                install_count: Some(50),
            },
            PluginInfo {
                id: "high@m".to_string(),
                name: "high-installs".to_string(),
                marketplace: "m".to_string(),
                scope: "user".to_string(),
                version: Some("1.0.0".to_string()),
                git_sha: None,
                enabled: true,
                installed_at: "2026-01-01T00:00:00Z".to_string(),
                last_updated: None,
                project_path: None,
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
                source_exists: true,
                description: None,
                install_count: Some(5000),
            },
        ];

        let mut available = vec![];

        apply_filters(&PluginsQuery::default(), &mut installed, &mut available);

        // Higher install_count comes first
        assert_eq!(installed[0].name, "high-installs");
        assert_eq!(installed[1].name, "low-usage");
    }

    #[tokio::test]
    async fn test_plugin_ops_rejects_invalid_name() {
        let db = Database::new_in_memory().await.expect("in-memory DB");
        let app = build_app(db);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/plugins/ops")
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"action":"install","name":"--force"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_plugin_ops_rejects_invalid_action() {
        let db = Database::new_in_memory().await.expect("in-memory DB");
        let app = build_app(db);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/plugins/ops")
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"action":"rm_rf","name":"test"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_plugin_ops_rejects_invalid_scope() {
        let db = Database::new_in_memory().await.expect("in-memory DB");
        let app = build_app(db);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/plugins/ops")
                    .header("Content-Type", "application/json")
                    .body(Body::from(
                        r#"{"action":"install","name":"test","scope":"global"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validate_plugin_name() {
        // Valid names
        assert!(validate_plugin_name("superpowers").is_ok());
        assert!(validate_plugin_name("my-plugin").is_ok());
        assert!(validate_plugin_name("my_plugin.v2").is_ok());
        assert!(validate_plugin_name("plugin@marketplace").is_ok());

        // Invalid names -- CLI flag injection attempts
        assert!(validate_plugin_name("--force").is_err());
        assert!(validate_plugin_name("-rf").is_err());
        assert!(validate_plugin_name("foo;rm -rf /").is_err());
        assert!(validate_plugin_name("").is_err());
        assert!(validate_plugin_name(&"a".repeat(129)).is_err());
    }

    #[test]
    fn test_validate_marketplace_source() {
        // Valid sources
        assert_eq!(
            validate_marketplace_source("owner/repo").unwrap(),
            "owner/repo"
        );
        assert_eq!(
            validate_marketplace_source("https://github.com/owner/repo").unwrap(),
            "owner/repo"
        );
        assert_eq!(
            validate_marketplace_source("https://github.com/owner/repo.git").unwrap(),
            "owner/repo"
        );
        assert_eq!(
            validate_marketplace_source("https://github.com/owner/repo/").unwrap(),
            "owner/repo"
        );

        // Invalid sources
        assert!(validate_marketplace_source("just-a-name").is_err());
        assert!(validate_marketplace_source("a/b/c").is_err());
        assert!(validate_marketplace_source("/repo").is_err());
        assert!(validate_marketplace_source("owner/").is_err());
        assert!(validate_marketplace_source("owner/repo;evil").is_err());
    }

    #[tokio::test]
    async fn test_marketplace_action_rejects_add_without_source() {
        let db = Database::new_in_memory().await.expect("in-memory DB");
        let app = build_app(db);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/plugins/marketplaces/action")
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"action":"add"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_marketplace_action_rejects_invalid_action() {
        let db = Database::new_in_memory().await.expect("in-memory DB");
        let app = build_app(db);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/plugins/marketplaces/action")
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"action":"destroy"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    // ---------------------------------------------------------------------------
    // Disk enrichment regression tests
    // ---------------------------------------------------------------------------

    #[test]
    fn test_disk_enrichment_populates_description_from_plugin_json() {
        let dir = tempfile::tempdir().expect("tempdir");
        let plugins_dir = dir.path();

        // Write a plugin.json manifest for "my-plugin@my-marketplace"
        let install_path = plugins_dir
            .join("cache")
            .join("my-marketplace")
            .join("my-plugin")
            .join("1.0.0");
        std::fs::create_dir_all(&install_path).unwrap();
        std::fs::write(
            install_path.join("plugin.json"),
            r#"{"name":"my-plugin","description":"A great plugin description"}"#,
        )
        .unwrap();

        // Write installed_plugins.json pointing to that installPath
        let registry = serde_json::json!({
            "version": 2,
            "plugins": {
                "my-plugin@my-marketplace": [{
                    "scope": "user",
                    "installPath": install_path.to_str().unwrap(),
                    "version": "1.0.0",
                    "installedAt": "2026-01-01T00:00:00.000Z",
                    "lastUpdated": "2026-01-01T00:00:00.000Z"
                }]
            }
        });
        std::fs::write(
            plugins_dir.join("installed_plugins.json"),
            registry.to_string(),
        )
        .unwrap();

        let (descriptions, source_exists) =
            read_plugin_descriptions_and_existence(&plugins_dir.to_path_buf());

        assert_eq!(
            descriptions
                .get("my-plugin@my-marketplace")
                .map(String::as_str),
            Some("A great plugin description"),
            "description must be populated from plugin.json on disk"
        );
        assert_eq!(
            source_exists.get("my-plugin@my-marketplace"),
            Some(&true),
            "installPath exists -> source_exists=true"
        );
    }

    #[test]
    fn test_disk_enrichment_populates_install_count_from_cache() {
        let dir = tempfile::tempdir().expect("tempdir");
        let plugins_dir = dir.path();

        let cache = serde_json::json!({
            "version": 1,
            "fetchedAt": "2026-03-01T00:00:00.000Z",
            "counts": [
                {"plugin": "my-plugin@my-marketplace", "unique_installs": 42000},
                {"plugin": "other@other", "unique_installs": 1}
            ]
        });
        std::fs::write(
            plugins_dir.join("install-counts-cache.json"),
            cache.to_string(),
        )
        .unwrap();

        let counts = read_install_counts(&plugins_dir.to_path_buf());

        assert_eq!(
            counts.get("my-plugin@my-marketplace").copied(),
            Some(42000u64),
            "install count must be populated from install-counts-cache.json"
        );
    }

    #[test]
    fn test_disk_enrichment_returns_empty_when_files_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        // No files written -- all helpers must return empty maps, not panic
        let plugins_dir = dir.path().to_path_buf();
        let (descriptions, source_exists) = read_plugin_descriptions_and_existence(&plugins_dir);
        let install_counts = read_install_counts(&plugins_dir);
        assert!(descriptions.is_empty());
        assert!(install_counts.is_empty());
        assert!(source_exists.is_empty());
    }

    /// Regression: source_exists is derived from the filesystem, not from CLI errors.
    /// A plugin with CLI errors but an intact installPath must have source_exists=true.
    /// A plugin with a deleted installPath must have source_exists=false.
    #[test]
    fn test_source_exists_reflects_filesystem_not_cli_errors() {
        let dir = tempfile::tempdir().unwrap();
        let plugins_dir = dir.path();

        // Plugin A: directory exists (local user-created plugin like "wtf")
        let plugin_a_path = plugins_dir.join("wtf");
        std::fs::create_dir_all(&plugin_a_path).unwrap();

        // Plugin B: directory does NOT exist (truly orphaned)
        let plugin_b_path = plugins_dir.join("gone");
        // intentionally not created

        let registry_json = serde_json::json!({
            "version": 2,
            "plugins": {
                "wtf@local": [{ "installPath": plugin_a_path.to_str().unwrap() }],
                "gone@some-marketplace": [{ "installPath": plugin_b_path.to_str().unwrap() }],
            }
        });
        std::fs::write(
            plugins_dir.join("installed_plugins.json"),
            registry_json.to_string(),
        )
        .unwrap();

        let (_, source_exists) = read_plugin_descriptions_and_existence(plugins_dir);

        assert_eq!(
            source_exists.get("wtf@local"),
            Some(&true),
            "wtf dir exists -> source_exists=true"
        );
        assert_eq!(
            source_exists.get("gone@some-marketplace"),
            Some(&false),
            "gone dir missing -> source_exists=false"
        );
    }

    // -----------------------------------------------------------------------
    // strip_ansi -- regression tests for large output integrity
    // -----------------------------------------------------------------------

    #[test]
    fn test_strip_ansi_preserves_large_json() {
        // Regression: strip_ansi must not truncate or corrupt large strings.
        // The CLI output can exceed 64KB of valid JSON.
        let large_json = serde_json::json!({
            "plugins": (0..500).map(|i| {
                serde_json::json!({
                    "id": format!("plugin-{i}@marketplace"),
                    "name": format!("plugin-{i}"),
                    "url": format!("https://github.com/org/plugin-{i}.git"),
                    "description": format!("Plugin {i} does something useful for developers"),
                })
            }).collect::<Vec<_>>(),
        });
        let json_str = serde_json::to_string_pretty(&large_json).unwrap();
        assert!(
            json_str.len() > 65_536,
            "test data must exceed 64KB pipe buffer; got {}",
            json_str.len()
        );

        let result = strip_ansi(&json_str);
        assert_eq!(result, json_str, "strip_ansi should not alter clean JSON");
        // Verify JSON still parses after strip_ansi
        serde_json::from_str::<serde_json::Value>(&result)
            .expect("strip_ansi output must remain valid JSON");
    }

    #[test]
    fn test_strip_ansi_removes_codes_from_large_output() {
        // Ensure ANSI codes are stripped without data loss in large payloads
        let payload = "a".repeat(70_000);
        let with_ansi = format!("\x1b[31m{payload}\x1b[0m");
        let result = strip_ansi(&with_ansi);
        assert_eq!(result.len(), 70_000);
        assert_eq!(result, payload);
    }

    #[test]
    fn test_strip_ansi_handles_empty_and_no_ansi() {
        assert_eq!(strip_ansi(""), "");
        assert_eq!(strip_ansi("hello world"), "hello world");
        assert_eq!(strip_ansi("{\"key\": \"value\"}"), "{\"key\": \"value\"}");
    }

    // -----------------------------------------------------------------------
    // Tempfile stdout redirect -- regression for 64KB pipe truncation
    // -----------------------------------------------------------------------

    /// Regression: Node.js CLI stdout truncates at 64KB (macOS pipe buffer)
    /// when using piped stdout. This test verifies the tempfile redirect
    /// pattern captures output beyond 64KB without data loss.
    #[tokio::test]
    async fn test_tempfile_stdout_captures_beyond_64kb() {
        use std::process::Stdio;

        // Write a script that outputs >64KB to stdout (simulates Node.js CLI)
        let script = tempfile::NamedTempFile::with_suffix(".sh").unwrap();
        let payload_size = 80_000; // well beyond 64KB
        std::fs::write(
            script.path(),
            format!("#!/bin/sh\nprintf '%0{}d' 0", payload_size),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(script.path(), std::fs::Permissions::from_mode(0o755))
                .unwrap();
        }

        // Method 1: piped stdout (may truncate for Node.js CLIs)
        let piped_output = tokio::process::Command::new(script.path())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .await
            .unwrap();

        // Method 2: tempfile stdout (our fix)
        let stdout_file = tempfile::NamedTempFile::new().unwrap();
        let stdout_fd: Stdio = stdout_file.as_file().try_clone().unwrap().into();

        let mut child = tokio::process::Command::new(script.path())
            .stdout(stdout_fd)
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        child.wait().await.unwrap();

        let tempfile_output = tokio::fs::read_to_string(stdout_file.path()).await.unwrap();

        // Both should capture the full output for a shell script
        // (shell scripts don't have Node's async stdout issue)
        assert_eq!(
            piped_output.stdout.len(),
            payload_size,
            "piped output should be complete for shell scripts"
        );
        assert_eq!(
            tempfile_output.len(),
            payload_size,
            "tempfile output must capture full payload"
        );
    }

    /// Verify that tokio Command::output() overrides stdout to piped,
    /// defeating file redirect. This is why we use spawn() instead.
    #[tokio::test]
    async fn test_output_overrides_stdout_fd() {
        use std::process::Stdio;

        let stdout_file = tempfile::NamedTempFile::new().unwrap();
        let stdout_fd: Stdio = stdout_file.as_file().try_clone().unwrap().into();

        // output() silently overrides stdout to piped
        let output = tokio::process::Command::new("echo")
            .arg("hello")
            .stdout(stdout_fd)
            .output()
            .await
            .unwrap();

        let file_content = tokio::fs::read_to_string(stdout_file.path()).await.unwrap();

        // output() captured it via pipe, file is empty
        assert!(
            !output.stdout.is_empty(),
            "output() should capture stdout via pipe"
        );
        assert!(
            file_content.is_empty(),
            "file should be empty because output() overrode the fd"
        );
    }

    /// Verify that spawn() preserves file fd redirect.
    #[tokio::test]
    async fn test_spawn_preserves_stdout_fd() {
        use std::process::Stdio;

        let stdout_file = tempfile::NamedTempFile::new().unwrap();
        let stdout_fd: Stdio = stdout_file.as_file().try_clone().unwrap().into();

        // spawn() preserves our stdout redirect
        let mut child = tokio::process::Command::new("echo")
            .arg("hello")
            .stdout(stdout_fd)
            .spawn()
            .unwrap();
        child.wait().await.unwrap();

        let file_content = tokio::fs::read_to_string(stdout_file.path()).await.unwrap();

        assert_eq!(
            file_content.trim(),
            "hello",
            "spawn() must preserve file fd, capturing output to the file"
        );
    }

    /// Integration: run_claude_plugin_in returns valid JSON for `list --available`
    /// which is the command that exceeds 64KB and triggered the original bug.
    #[tokio::test]
    async fn test_run_claude_plugin_returns_complete_json() {
        // Skip if CLI not available (CI environments)
        if claude_view_core::resolved_cli_path().is_none() {
            eprintln!("Skipping: claude CLI not found");
            return;
        }

        let result = run_claude_plugin(&["list", "--available", "--json"]).await;
        match result {
            Ok(json_str) => {
                assert!(
                    json_str.len() > 65_536,
                    "output should exceed 64KB pipe buffer; got {} bytes",
                    json_str.len()
                );

                let parsed: serde_json::Value = serde_json::from_str(&json_str).expect(
                    "run_claude_plugin must return valid JSON (regression: 64KB pipe truncation)",
                );
                assert!(
                    parsed["installed"].is_array(),
                    "parsed JSON must have 'installed' array"
                );
                assert!(
                    parsed["available"].is_array(),
                    "parsed JSON must have 'available' array"
                );
            }
            Err(e) => {
                let err_str = format!("{e:?}");
                assert!(
                    !err_str.contains("EOF while parsing"),
                    "regression: stdout truncation must not cause JSON parse errors: {err_str}"
                );
                // Other errors (CLI not installed, auth) are OK to skip
                eprintln!("Skipping: CLI error (not truncation): {err_str}");
            }
        }
    }
}
