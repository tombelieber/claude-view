#[cfg(test)]
mod tests {
    use crate::jsonl_index::build_team_jsonl_index;
    use crate::jsonl_reconstruct::{
        reconstruct_inbox_from_jsonl, reconstruct_team_and_inbox_from_jsonl,
        reconstruct_team_from_jsonl,
    };
    use crate::parser::classify_message;
    use crate::snapshot::snapshot_team;
    use crate::store::TeamsStore;
    use crate::types::{InboxMessageType, TeamJSONLRef};
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    fn make_test_team(dir: &Path) {
        let team_dir = dir.join("teams").join("test-team");
        fs::create_dir_all(team_dir.join("inboxes")).unwrap();

        let config = serde_json::json!({
            "name": "test-team",
            "description": "Test team for unit tests",
            "createdAt": 1772568545480_i64,
            "leadAgentId": "team-lead@test-team",
            "leadSessionId": "dbd14eb6-b165-4089-ba51-4995e8640d5f",
            "members": [
                {
                    "agentId": "team-lead@test-team",
                    "name": "team-lead",
                    "agentType": "team-lead",
                    "model": "claude-opus-4-6",
                    "joinedAt": 1772568545480_i64,
                    "tmuxPaneId": "",
                    "cwd": "/tmp/test",
                    "subscriptions": []
                },
                {
                    "agentId": "researcher@test-team",
                    "name": "researcher",
                    "agentType": "Explore",
                    "model": "haiku",
                    "prompt": "Research the codebase",
                    "color": "blue",
                    "planModeRequired": false,
                    "joinedAt": 1772568557410_i64,
                    "tmuxPaneId": "in-process",
                    "cwd": "/tmp/test",
                    "subscriptions": [],
                    "backendType": "in-process"
                }
            ]
        });
        fs::write(
            team_dir.join("config.json"),
            serde_json::to_string_pretty(&config).unwrap(),
        )
        .unwrap();

        let inbox = serde_json::json!([
            {
                "from": "researcher",
                "text": "# Research Report\n\nFound 3 call sites.",
                "timestamp": "2026-03-03T20:10:42.127Z",
                "read": true,
                "color": "blue"
            },
            {
                "from": "researcher",
                "text": "{\"type\":\"idle_notification\",\"from\":\"researcher\",\"timestamp\":\"2026-03-03T20:10:42.127Z\",\"idleReason\":\"available\"}",
                "timestamp": "2026-03-03T20:10:43.000Z",
                "read": true
            }
        ]);
        fs::write(
            team_dir.join("inboxes").join("team-lead.json"),
            serde_json::to_string_pretty(&inbox).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn test_load_teams() {
        let tmp = TempDir::new().unwrap();
        make_test_team(tmp.path());

        let store = TeamsStore::load(tmp.path());
        assert_eq!(store.teams.len(), 1);
        assert!(store.teams.contains_key("test-team"));

        let team = &store.teams["test-team"];
        assert_eq!(team.members.len(), 2);
        assert_eq!(team.lead_session_id, "dbd14eb6-b165-4089-ba51-4995e8640d5f");
    }

    #[test]
    fn test_inbox_parsing() {
        let tmp = TempDir::new().unwrap();
        make_test_team(tmp.path());

        let store = TeamsStore::load(tmp.path());
        let inbox = &store.inboxes["test-team"];
        assert_eq!(inbox.len(), 2);

        // First message is plain text
        assert!(matches!(inbox[0].message_type, InboxMessageType::PlainText));
        assert_eq!(inbox[0].from, "researcher");

        // Second message is idle notification
        assert!(matches!(
            inbox[1].message_type,
            InboxMessageType::IdleNotification
        ));
    }

    #[test]
    fn test_summaries() {
        let tmp = TempDir::new().unwrap();
        make_test_team(tmp.path());

        let store = TeamsStore::load(tmp.path());
        let summaries = store.summaries();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].member_count, 2);
        assert_eq!(summaries[0].message_count, 2);
        assert_eq!(summaries[0].models, vec!["claude-opus-4-6", "haiku"]);
    }

    #[test]
    fn test_skips_dir_without_config() {
        let tmp = TempDir::new().unwrap();
        let broken_dir = tmp.path().join("teams").join("no-config");
        fs::create_dir_all(broken_dir.join("inboxes")).unwrap();

        let store = TeamsStore::load(tmp.path());
        assert_eq!(store.teams.len(), 0);
    }

    #[test]
    fn test_parses_members_missing_agent_type() {
        // Regression: Claude Code's newer team format omits agentType on spawned
        // members. Our parser must not reject the entire team because of it.
        let tmp = TempDir::new().unwrap();
        let team_dir = tmp.path().join("teams").join("bench-team");
        fs::create_dir_all(team_dir.join("inboxes")).unwrap();

        let config = serde_json::json!({
            "name": "bench-team",
            "description": "Benchmark team",
            "createdAt": 1775511338926_i64,
            "leadAgentId": "team-lead@bench-team",
            "leadSessionId": "6da88ea5-b2b5-4388-a92d-f75664ae95ca",
            "members": [
                {
                    "agentId": "team-lead@bench-team",
                    "name": "team-lead",
                    "agentType": "team-lead",
                    "model": "claude-opus-4-6",
                    "joinedAt": 1775511338926_i64,
                    "tmuxPaneId": "",
                    "cwd": "/tmp",
                    "subscriptions": []
                },
                {
                    // No agentType field -- this is the new format
                    "agentId": "ws-agent@bench-team",
                    "name": "ws-agent",
                    "model": "sonnet",
                    "prompt": "Design WebSocket benchmark",
                    "color": "yellow",
                    "planModeRequired": false,
                    "joinedAt": 1775511377975_i64,
                    "tmuxPaneId": "%2",
                    "cwd": "/tmp",
                    "subscriptions": [],
                    "backendType": "tmux"
                }
            ]
        });
        fs::write(
            team_dir.join("config.json"),
            serde_json::to_string_pretty(&config).unwrap(),
        )
        .unwrap();

        let store = TeamsStore::load(tmp.path());
        assert_eq!(
            store.teams.len(),
            1,
            "team must be parsed despite missing agentType"
        );
        let team = &store.teams["bench-team"];
        assert_eq!(team.members.len(), 2);
        // Missing agentType should default to "general-purpose"
        assert_eq!(team.members[1].agent_type, "general-purpose");
    }

    #[test]
    fn test_live_reload_picks_up_new_team() {
        // Teams created AFTER TeamsStore::load() must be visible on next query.
        // This is the root cause of /teams page showing "No teams found" for
        // teams created during the current server session.
        let tmp = TempDir::new().unwrap();

        // Initial load -- no teams yet
        let store = TeamsStore::load(tmp.path());
        assert_eq!(store.summaries().len(), 0);

        // Team created after initial load (simulates Claude Code /team command)
        make_test_team(tmp.path());

        // Must find the new team WITHOUT restarting
        assert_eq!(
            store.summaries().len(),
            1,
            "TeamsStore should pick up teams created after initial load"
        );
        assert!(
            store.get("test-team").is_some(),
            "Team detail should be available for newly created team"
        );
    }

    #[test]
    fn test_get_prefers_filesystem_over_jsonl() {
        let tmp = TempDir::new().unwrap();

        // Create filesystem team
        make_test_team(tmp.path());

        // Also create JSONL with different description for same team name
        let projects_dir = tmp.path().join("projects").join("test-project");
        fs::create_dir_all(&projects_dir).unwrap();
        let jsonl_path = projects_dir.join("sess-jsonl.jsonl");
        let lines = vec![
            r#"{"type":"assistant","sessionId":"sess-jsonl","teamName":"test-team","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_1","name":"TeamCreate","input":{"team_name":"test-team","description":"JSONL version"}}]},"timestamp":"2026-03-11T10:00:00.000Z"}"#,
        ];
        fs::write(&jsonl_path, lines.join("\n") + "\n").unwrap();

        let store = TeamsStore::load(tmp.path());

        // Should return filesystem version (original description), NOT JSONL version
        let detail = store.get("test-team").unwrap();
        assert_eq!(detail.description, "Test team for unit tests");
    }

    #[test]
    fn test_inbox_fallback_from_jsonl() {
        let tmp = TempDir::new().unwrap();
        let projects_dir = tmp.path().join("projects").join("test-project");
        fs::create_dir_all(&projects_dir).unwrap();

        let jsonl_path = projects_dir.join("sess-inbox.jsonl");
        let lines = vec![
            r#"{"type":"assistant","sessionId":"sess-inbox","teamName":"inbox-only","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_1","name":"TeamCreate","input":{"team_name":"inbox-only","description":"Test"}}]},"timestamp":"2026-03-11T10:00:00.000Z"}"#,
            r#"{"type":"assistant","sessionId":"sess-inbox","teamName":"inbox-only","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_2","name":"SendMessage","input":{"type":"message","recipient":"worker","content":"Hello worker"}}]},"timestamp":"2026-03-11T10:01:00.000Z"}"#,
        ];
        fs::write(&jsonl_path, lines.join("\n") + "\n").unwrap();

        let store = TeamsStore::load(tmp.path());
        let inbox = store.inbox("inbox-only");
        assert!(inbox.is_some());
        assert_eq!(inbox.unwrap().len(), 1);
    }

    #[test]
    fn test_summaries_includes_jsonl_only_teams() {
        let tmp = TempDir::new().unwrap();

        // Create one filesystem team
        make_test_team(tmp.path());

        // Create one JSONL-only team
        let projects_dir = tmp.path().join("projects").join("test-project");
        fs::create_dir_all(&projects_dir).unwrap();
        let jsonl_path = projects_dir.join("sess-summary.jsonl");
        let lines = vec![
            r#"{"type":"assistant","sessionId":"sess-summary","teamName":"jsonl-team","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_1","name":"TeamCreate","input":{"team_name":"jsonl-team","description":"JSONL-only team"}}]},"timestamp":"2026-03-11T10:00:00.000Z"}"#,
            r#"{"type":"assistant","sessionId":"sess-summary","teamName":"jsonl-team","message":{"model":"haiku","role":"assistant","content":[{"type":"tool_use","id":"toolu_2","name":"Agent","input":{"name":"agent-a","team_name":"jsonl-team","prompt":"Work"}}]},"timestamp":"2026-03-11T10:00:01.000Z"}"#,
        ];
        fs::write(&jsonl_path, lines.join("\n") + "\n").unwrap();

        let store = TeamsStore::load(tmp.path());
        let summaries = store.summaries();

        assert_eq!(summaries.len(), 2);
        let names: Vec<_> = summaries.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"test-team"));
        assert!(names.contains(&"jsonl-team"));
    }

    #[test]
    fn test_teams_store_with_jsonl_index() {
        let tmp = TempDir::new().unwrap();
        let projects_dir = tmp.path().join("projects").join("test-project");
        fs::create_dir_all(&projects_dir).unwrap();

        let jsonl_path = projects_dir.join("sess-fallback.jsonl");
        let lines = vec![
            r#"{"type":"assistant","sessionId":"sess-fallback","teamName":"ghost-team","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_1","name":"TeamCreate","input":{"team_name":"ghost-team","description":"A team that no longer exists on disk"}}]},"timestamp":"2026-03-11T10:00:00.000Z"}"#,
            r#"{"type":"assistant","sessionId":"sess-fallback","teamName":"ghost-team","message":{"model":"haiku","role":"assistant","content":[{"type":"tool_use","id":"toolu_2","name":"Agent","input":{"name":"worker","team_name":"ghost-team","prompt":"Do stuff","subagent_type":"general-purpose"}}]},"timestamp":"2026-03-11T10:00:01.000Z"}"#,
        ];
        fs::write(&jsonl_path, lines.join("\n") + "\n").unwrap();

        // Load with JSONL index -- no teams/ directory exists
        let store = TeamsStore::load(tmp.path());

        let detail = store.get("ghost-team");
        assert!(detail.is_some(), "Should reconstruct ghost-team from JSONL");
        let detail = detail.unwrap();
        assert_eq!(detail.name, "ghost-team");
        assert_eq!(detail.description, "A team that no longer exists on disk");
        assert_eq!(detail.members.len(), 1);
        assert_eq!(detail.members[0].name, "worker");
    }

    #[test]
    fn test_reconstruct_inbox_from_jsonl() {
        let tmp = TempDir::new().unwrap();
        let jsonl_path = tmp.path().join("sess-789.jsonl");

        let lines = vec![
            r#"{"type":"assistant","sessionId":"sess-789","teamName":"inbox-team","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_1","name":"SendMessage","input":{"type":"message","recipient":"analyst","summary":"Data ready","content":"Here is the analysis data."}}]},"timestamp":"2026-03-11T10:05:00.000Z"}"#,
            r#"{"type":"assistant","sessionId":"sess-789","teamName":"inbox-team","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_2","name":"SendMessage","input":{"type":"shutdown_request","recipient":"analyst","content":"All done."}}]},"timestamp":"2026-03-11T10:10:00.000Z"}"#,
        ];
        fs::write(&jsonl_path, lines.join("\n") + "\n").unwrap();

        let refs = vec![TeamJSONLRef {
            session_id: "sess-789".to_string(),
            jsonl_path,
        }];

        let inbox = reconstruct_inbox_from_jsonl("inbox-team", &refs);
        assert_eq!(inbox.len(), 2);
        assert_eq!(inbox[0].from, "team-lead");
        assert!(inbox[0].text.contains("analysis data"));
        assert!(matches!(inbox[0].message_type, InboxMessageType::PlainText));
        assert!(matches!(
            inbox[1].message_type,
            InboxMessageType::ShutdownRequest
        ));
        assert!(inbox[0].timestamp < inbox[1].timestamp);
    }

    #[test]
    fn test_reconstruct_inbox_empty_when_no_send_messages() {
        let tmp = TempDir::new().unwrap();
        let jsonl_path = tmp.path().join("sess-empty.jsonl");

        let lines = vec![
            r#"{"type":"assistant","sessionId":"sess-empty","teamName":"no-msg-team","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_1","name":"TeamCreate","input":{"team_name":"no-msg-team","description":"Test"}}]},"timestamp":"2026-03-11T10:00:00.000Z"}"#,
        ];
        fs::write(&jsonl_path, lines.join("\n") + "\n").unwrap();

        let refs = vec![TeamJSONLRef {
            session_id: "sess-empty".to_string(),
            jsonl_path,
        }];

        let inbox = reconstruct_inbox_from_jsonl("no-msg-team", &refs);
        assert!(inbox.is_empty());
    }

    #[test]
    fn test_reconstruct_team_from_jsonl() {
        let tmp = TempDir::new().unwrap();
        let jsonl_path = tmp.path().join("sess-123.jsonl");

        let lines = vec![
            r#"{"type":"assistant","sessionId":"sess-123","teamName":"demo-team","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_1","name":"TeamCreate","input":{"team_name":"demo-team","description":"Demo research team"}}]},"timestamp":"2026-03-11T10:00:00.000Z"}"#,
            r#"{"type":"assistant","sessionId":"sess-123","teamName":"demo-team","message":{"model":"claude-sonnet-4-6","role":"assistant","content":[{"type":"tool_use","id":"toolu_2","name":"Agent","input":{"name":"researcher","team_name":"demo-team","prompt":"Research the topic","subagent_type":"Explore"}}]},"timestamp":"2026-03-11T10:00:01.000Z"}"#,
            r#"{"type":"assistant","sessionId":"sess-123","teamName":"demo-team","message":{"model":"haiku","role":"assistant","content":[{"type":"tool_use","id":"toolu_3","name":"Agent","input":{"name":"writer","team_name":"demo-team","prompt":"Write the report","subagent_type":"code-writer"}}]},"timestamp":"2026-03-11T10:00:02.000Z"}"#,
        ];
        fs::write(&jsonl_path, lines.join("\n") + "\n").unwrap();

        let refs = vec![TeamJSONLRef {
            session_id: "sess-123".to_string(),
            jsonl_path: jsonl_path.clone(),
        }];

        let detail = reconstruct_team_from_jsonl("demo-team", &refs);
        assert!(detail.is_some(), "Should reconstruct team");
        let detail = detail.unwrap();
        assert_eq!(detail.name, "demo-team");
        assert_eq!(detail.description, "Demo research team");
        assert_eq!(detail.lead_session_id, "sess-123");
        assert_eq!(detail.members.len(), 2);
        assert_eq!(detail.members[0].name, "researcher");
        assert_eq!(detail.members[0].agent_type, "Explore");
        assert_eq!(detail.members[1].name, "writer");
        assert!(!detail.members[0].color.is_empty());
        assert!(!detail.members[1].color.is_empty());
        assert_ne!(detail.members[0].color, detail.members[1].color);
    }

    #[test]
    fn test_reconstruct_ignores_non_team_agent_spawns() {
        let tmp = TempDir::new().unwrap();
        let jsonl_path = tmp.path().join("sess-456.jsonl");

        let lines = vec![
            r#"{"type":"assistant","sessionId":"sess-456","teamName":"my-team","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_1","name":"TeamCreate","input":{"team_name":"my-team","description":"Test"}}]},"timestamp":"2026-03-11T10:00:00.000Z"}"#,
            r#"{"type":"assistant","sessionId":"sess-456","teamName":"my-team","message":{"model":"opus","role":"assistant","content":[{"type":"tool_use","id":"toolu_2","name":"Agent","input":{"name":"member-a","team_name":"my-team","prompt":"Do work"}}]},"timestamp":"2026-03-11T10:00:01.000Z"}"#,
            r#"{"type":"assistant","sessionId":"sess-456","teamName":"my-team","message":{"model":"haiku","role":"assistant","content":[{"type":"tool_use","id":"toolu_3","name":"Agent","input":{"name":"helper","prompt":"Quick task"}}]},"timestamp":"2026-03-11T10:00:02.000Z"}"#,
        ];
        fs::write(&jsonl_path, lines.join("\n") + "\n").unwrap();

        let refs = vec![TeamJSONLRef {
            session_id: "sess-456".to_string(),
            jsonl_path,
        }];

        let detail = reconstruct_team_from_jsonl("my-team", &refs).unwrap();
        assert_eq!(
            detail.members.len(),
            1,
            "Only team member spawn should be included"
        );
        assert_eq!(detail.members[0].name, "member-a");
    }

    #[test]
    fn test_build_team_jsonl_index_finds_teams() {
        let tmp = TempDir::new().unwrap();
        let projects_dir = tmp.path().join("projects").join("test-project");
        std::fs::create_dir_all(&projects_dir).unwrap();

        let jsonl_path = projects_dir.join("sess-abc.jsonl");
        let lines = vec![
            r#"{"type":"user","sessionId":"sess-abc","message":{"role":"user","content":"hi"},"timestamp":"2026-03-11T10:00:00Z"}"#,
            r#"{"type":"assistant","sessionId":"sess-abc","teamName":"demo-team","message":{"role":"assistant","content":[]},"timestamp":"2026-03-11T10:00:01Z"}"#,
        ];
        std::fs::write(&jsonl_path, lines.join("\n") + "\n").unwrap();

        let index = build_team_jsonl_index(tmp.path());
        assert!(
            index.contains_key("demo-team"),
            "Should find demo-team in index"
        );
        assert_eq!(index["demo-team"].len(), 1);
        assert_eq!(index["demo-team"][0].session_id, "sess-abc");
        assert_eq!(index["demo-team"][0].jsonl_path, jsonl_path);
    }

    #[test]
    fn test_build_team_jsonl_index_multiple_teams_one_session() {
        let tmp = TempDir::new().unwrap();
        let projects_dir = tmp.path().join("projects").join("test-project");
        std::fs::create_dir_all(&projects_dir).unwrap();

        let jsonl_path = projects_dir.join("sess-multi.jsonl");
        let lines = vec![
            r#"{"type":"assistant","sessionId":"sess-multi","teamName":"team-a","message":{"role":"assistant","content":[]},"timestamp":"2026-03-11T10:00:01Z"}"#,
            r#"{"type":"assistant","sessionId":"sess-multi","teamName":"team-b","message":{"role":"assistant","content":[]},"timestamp":"2026-03-11T10:00:02Z"}"#,
        ];
        std::fs::write(&jsonl_path, lines.join("\n") + "\n").unwrap();

        let index = build_team_jsonl_index(tmp.path());
        assert!(index.contains_key("team-a"));
        assert!(index.contains_key("team-b"));
    }

    #[test]
    fn test_build_team_jsonl_index_ignores_non_jsonl_files() {
        let tmp = TempDir::new().unwrap();
        let projects_dir = tmp.path().join("projects").join("test-project");
        std::fs::create_dir_all(&projects_dir).unwrap();

        std::fs::write(
            projects_dir.join("sess-abc.meta.json"),
            r#"{"teamName":"ghost-team"}"#,
        )
        .unwrap();

        let index = build_team_jsonl_index(tmp.path());
        assert!(index.is_empty());
    }

    #[test]
    fn test_team_jsonl_ref_creation() {
        let r = TeamJSONLRef {
            session_id: "abc-123".to_string(),
            jsonl_path: std::path::PathBuf::from("/tmp/test.jsonl"),
        };
        assert_eq!(r.session_id, "abc-123");
    }

    #[test]
    fn test_classify_message() {
        assert!(matches!(
            classify_message("plain text"),
            InboxMessageType::PlainText
        ));
        assert!(matches!(
            classify_message(r#"{"type":"idle_notification","from":"x"}"#),
            InboxMessageType::IdleNotification
        ));
        assert!(matches!(
            classify_message(r#"{"type":"task_assignment","taskId":"1"}"#),
            InboxMessageType::TaskAssignment
        ));
        assert!(matches!(
            classify_message(r#"{"type":"shutdown_request","requestId":"1"}"#),
            InboxMessageType::ShutdownRequest
        ));
        assert!(matches!(
            classify_message(r#"{"type":"shutdown_approved","requestId":"1"}"#),
            InboxMessageType::ShutdownApproved
        ));
    }

    /// Regression: TeamCreate assistant messages in real Claude Code JSONL do NOT
    /// carry a top-level "teamName" field -- the team name only appears inside
    /// message.content[].input.team_name. reconstruct_team_from_jsonl must still
    /// find the team without requiring a top-level teamName on that line.
    #[test]
    fn test_reconstruct_team_from_jsonl_without_toplevel_teamname() {
        let tmp = TempDir::new().unwrap();
        let jsonl_path = tmp.path().join("sess-real.jsonl");

        // Real-world shape: TeamCreate line has NO top-level teamName.
        // Subsequent Agent lines DO have teamName (real Claude Code behaviour).
        let lines = vec![
            // TeamCreate -- no teamName at top level (matches real JSONL structure)
            r#"{"type":"assistant","sessionId":"sess-real","message":{"model":"claude-sonnet-4-6","role":"assistant","content":[{"type":"tool_use","id":"toolu_abc","name":"TeamCreate","input":{"team_name":"real-team","description":"A real world team"},"caller":{"type":"direct"}}]},"timestamp":"2026-03-11T10:00:00.000Z"}"#,
            // Agent spawn -- has teamName (subsequent messages after team creation)
            r#"{"type":"assistant","sessionId":"sess-real","teamName":"real-team","message":{"model":"claude-sonnet-4-6","role":"assistant","content":[{"type":"tool_use","id":"toolu_def","name":"Agent","input":{"name":"worker","team_name":"real-team","prompt":"Do work","subagent_type":"general-purpose"}}]},"timestamp":"2026-03-11T10:00:01.000Z"}"#,
        ];
        std::fs::write(&jsonl_path, lines.join("\n") + "\n").unwrap();

        let refs = vec![TeamJSONLRef {
            session_id: "sess-real".to_string(),
            jsonl_path: jsonl_path.clone(),
        }];

        let result = reconstruct_team_from_jsonl("real-team", &refs);
        assert!(
            result.is_some(),
            "Should reconstruct team even when TeamCreate line has no top-level teamName"
        );
        let team = result.unwrap();
        assert_eq!(team.name, "real-team");
        assert_eq!(team.description, "A real world team");
        assert_eq!(team.members.len(), 1, "Should find Agent spawn member");
        assert_eq!(team.members[0].name, "worker");
    }

    /// Regression: reconstruct_team_and_inbox_from_jsonl (used in summaries())
    /// must also work when TeamCreate line has no top-level teamName.
    #[test]
    fn test_reconstruct_combined_without_toplevel_teamname() {
        let tmp = TempDir::new().unwrap();
        let jsonl_path = tmp.path().join("sess-combined.jsonl");

        let lines = vec![
            // TeamCreate -- no teamName at top level
            r#"{"type":"assistant","sessionId":"sess-combined","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_1","name":"TeamCreate","input":{"team_name":"combo-team","description":"Combined test team"}}]},"timestamp":"2026-03-11T10:00:00.000Z"}"#,
            // SendMessage -- has teamName
            r#"{"type":"assistant","sessionId":"sess-combined","teamName":"combo-team","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_2","name":"SendMessage","input":{"type":"message","recipient":"worker","content":"Go!"}}]},"timestamp":"2026-03-11T10:01:00.000Z"}"#,
        ];
        std::fs::write(&jsonl_path, lines.join("\n") + "\n").unwrap();

        let refs = vec![TeamJSONLRef {
            session_id: "sess-combined".to_string(),
            jsonl_path: jsonl_path.clone(),
        }];

        let result = reconstruct_team_and_inbox_from_jsonl("combo-team", &refs);
        assert!(
            result.is_some(),
            "summaries() path must find team without top-level teamName on TeamCreate line"
        );
        let (team, inbox) = result.unwrap();
        assert_eq!(team.name, "combo-team");
        assert_eq!(team.description, "Combined test team");
        assert_eq!(inbox.len(), 1);
    }

    // ====================================================================
    // Team Snapshot + Backup Fallback Tests
    // ====================================================================

    #[test]
    fn test_snapshot_team_copies_files() {
        let claude_dir = TempDir::new().unwrap();
        let cv_dir = TempDir::new().unwrap();
        make_test_team(claude_dir.path());

        snapshot_team("test-team", "session-123", claude_dir.path(), cv_dir.path()).unwrap();

        let dst_config = cv_dir
            .path()
            .join("session-123/teams/test-team/config.json");
        let dst_inbox = cv_dir
            .path()
            .join("session-123/teams/test-team/inboxes/team-lead.json");
        assert!(dst_config.exists(), "config.json must be copied");
        assert!(dst_inbox.exists(), "inbox file must be copied");

        // Content must match
        let src_content =
            fs::read_to_string(claude_dir.path().join("teams/test-team/config.json")).unwrap();
        let dst_content = fs::read_to_string(&dst_config).unwrap();
        assert_eq!(src_content, dst_content);
    }

    #[test]
    fn test_snapshot_team_noop_when_source_missing() {
        let claude_dir = TempDir::new().unwrap();
        let cv_dir = TempDir::new().unwrap();

        // No team dir exists -- should return Ok(()) without creating anything
        snapshot_team(
            "nonexistent",
            "session-456",
            claude_dir.path(),
            cv_dir.path(),
        )
        .unwrap();
        assert!(!cv_dir.path().join("teams/nonexistent").exists());
    }

    #[test]
    fn test_get_falls_back_to_backup() {
        let claude_dir = TempDir::new().unwrap();
        let cv_dir = TempDir::new().unwrap();

        // Write team fixture under {cv_dir}/{session_id}/ (new layout)
        let session_dir = cv_dir.path().join("session-abc");
        fs::create_dir_all(&session_dir).unwrap();
        make_test_team(&session_dir);

        let store = TeamsStore::load_with_backup(claude_dir.path(), cv_dir.path());
        let team = store.get("test-team");
        assert!(team.is_some(), "Must find team from backup dir");
        assert_eq!(team.unwrap().name, "test-team");

        let inbox = store.inbox("test-team");
        assert!(inbox.is_some(), "Must find inbox from backup dir");
        assert!(!inbox.unwrap().is_empty());
    }

    #[test]
    fn test_primary_wins_over_backup() {
        let claude_dir = TempDir::new().unwrap();
        let cv_dir = TempDir::new().unwrap();

        // Primary: "v2" description
        make_test_team(claude_dir.path());
        let primary_config = claude_dir.path().join("teams/test-team/config.json");
        let mut config: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&primary_config).unwrap()).unwrap();
        config["description"] = serde_json::json!("v2 primary");
        fs::write(
            &primary_config,
            serde_json::to_string_pretty(&config).unwrap(),
        )
        .unwrap();

        // Backup: "v1" description (under session_id subdir)
        let session_dir = cv_dir.path().join("session-old");
        fs::create_dir_all(&session_dir).unwrap();
        make_test_team(&session_dir);

        let store = TeamsStore::load_with_backup(claude_dir.path(), cv_dir.path());
        let team = store.get("test-team").unwrap();
        assert_eq!(
            team.description, "v2 primary",
            "Primary must win over backup"
        );
    }

    #[test]
    fn test_augments_members_from_inbox_senders() {
        let tmp = TempDir::new().unwrap();
        let team_dir = tmp.path().join("teams").join("debate-team");
        fs::create_dir_all(team_dir.join("inboxes")).unwrap();

        // config.json has only team-lead + judge
        let config = serde_json::json!({
            "name": "debate-team",
            "description": "AI debate",
            "createdAt": 1772568545480_i64,
            "leadSessionId": "lead-session-id",
            "members": [
                { "agentId": "tl", "name": "team-lead", "agentType": "team-lead", "model": "haiku", "cwd": "/tmp" },
                { "agentId": "dj", "name": "debate-judge", "agentType": "general-purpose", "model": "opus", "color": "purple", "cwd": "/tmp" }
            ]
        });
        fs::write(
            team_dir.join("config.json"),
            serde_json::to_string(&config).unwrap(),
        )
        .unwrap();

        // Inbox has messages from 3 additional agents not in config
        let inbox = serde_json::json!([
            { "from": "advocate", "text": "Collaboration is key", "timestamp": "2026-04-07T01:00:00Z", "read": true, "color": "green" },
            { "from": "champion", "text": "Competition drives innovation", "timestamp": "2026-04-07T01:01:00Z", "read": true, "color": "red" },
            { "from": "pragmatist", "text": "Both have merit", "timestamp": "2026-04-07T01:02:00Z", "read": true },
            { "from": "debate-judge", "text": "Good points all", "timestamp": "2026-04-07T01:03:00Z", "read": true, "color": "purple" }
        ]);
        fs::write(
            team_dir.join("inboxes").join("team-lead.json"),
            serde_json::to_string(&inbox).unwrap(),
        )
        .unwrap();

        let store = TeamsStore::load(tmp.path());
        let team = &store.teams["debate-team"];

        // Should have 5 members: 2 from config + 3 from inbox
        assert_eq!(team.members.len(), 5, "Expected 2 config + 3 inbox members");

        let names: Vec<&str> = team.members.iter().map(|m| m.name.as_str()).collect();
        assert!(
            names.contains(&"advocate"),
            "advocate should be augmented from inbox"
        );
        assert!(
            names.contains(&"champion"),
            "champion should be augmented from inbox"
        );
        assert!(
            names.contains(&"pragmatist"),
            "pragmatist should be augmented from inbox"
        );

        // Augmented members should pick up color from their first inbox message
        let advocate = team.members.iter().find(|m| m.name == "advocate").unwrap();
        assert_eq!(advocate.color, "green", "color from inbox message");

        // pragmatist had no color in message -- gets deterministic fallback
        let pragmatist = team
            .members
            .iter()
            .find(|m| m.name == "pragmatist")
            .unwrap();
        assert!(
            !pragmatist.color.is_empty(),
            "should get deterministic color"
        );

        // debate-judge should NOT be duplicated
        assert_eq!(
            team.members
                .iter()
                .filter(|m| m.name == "debate-judge")
                .count(),
            1,
            "config member should not be duplicated"
        );
    }

    #[test]
    fn test_snapshot_team_overwrites_stale_backup() {
        let claude_dir = TempDir::new().unwrap();
        let cv_dir = TempDir::new().unwrap();

        // Create stale backup with original test team
        make_test_team(cv_dir.path());

        // Create primary with updated config
        make_test_team(claude_dir.path());
        let primary_config = claude_dir.path().join("teams/test-team/config.json");
        let mut config: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&primary_config).unwrap()).unwrap();
        config["description"] = serde_json::json!("updated after snapshot");
        fs::write(
            &primary_config,
            serde_json::to_string_pretty(&config).unwrap(),
        )
        .unwrap();

        // Snapshot overwrites backup
        snapshot_team("test-team", "session-123", claude_dir.path(), cv_dir.path()).unwrap();

        let backup_content = fs::read_to_string(
            cv_dir
                .path()
                .join("session-123/teams/test-team/config.json"),
        )
        .unwrap();
        let backup_config: serde_json::Value = serde_json::from_str(&backup_content).unwrap();
        assert_eq!(
            backup_config["description"].as_str().unwrap(),
            "updated after snapshot",
            "Backup must reflect latest primary data",
        );
    }
}
