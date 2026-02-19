//! Auto-inject and clean up Live Monitor hooks in ~/.claude/settings.json.
//!
//! Claude Code hooks use a matcher-based nested format:
//!   hooks[event] → [matcher_group] → hooks → [handler]
//!
//! Each event array contains matcher group objects:
//!   { "matcher": "<regex>", "hooks": [{ "type": "command", ... }] }
//!
//! Omitting `matcher` matches all occurrences of the event.
//! Files with JSON errors are skipped entirely by Claude Code, so we
//! must validate our output carefully and write atomically.

use std::path::PathBuf;

const SENTINEL: &str = "# claude-view-hook";

/// Hook events to register. SessionStart is sync (no "async" field),
/// all others are async ("async": true).
const HOOK_EVENTS: &[&str] = &[
    "SessionStart",        // sync — blocks startup until server acknowledges
    "UserPromptSubmit",    // async
    "PreToolUse",          // async — NEW: real-time tool activity
    "PostToolUse",         // async — NEW: tool completion tracking
    "PostToolUseFailure",  // async
    "PermissionRequest",   // async — NEW: richer permission data (tool_name, suggestions)
    "Stop",                // async
    "Notification",        // async
    "SubagentStart",       // async
    "SubagentStop",        // async
    "TeammateIdle",        // async — NEW: sub-agent idle tracking
    "TaskCompleted",       // async — NEW: task completion events
    "PreCompact",          // async — NEW: context compaction indicator
    "SessionEnd",          // async
];

fn settings_path() -> PathBuf {
    dirs::home_dir()
        .expect("home dir exists")
        .join(".claude")
        .join("settings.json")
}

/// Build a hook handler (the inner object inside a matcher group's `hooks` array).
fn make_hook_handler(port: u16, event: &str) -> serde_json::Value {
    let command = format!(
        "curl -s -X POST http://localhost:{}/api/live/hook \
         -H 'Content-Type: application/json' \
         -H 'X-Claude-PID: '$PPID \
         --data-binary @- 2>/dev/null || true {}",
        port, SENTINEL
    );
    let mut handler = serde_json::json!({
        "type": "command",
        "command": command,
        "statusMessage": "Live Monitor"
    });
    // SessionStart is sync (default). All others are async.
    if event != "SessionStart" {
        handler["async"] = serde_json::json!(true);
    }
    handler
}

/// Build a matcher group that wraps our hook handler.
/// Omits `matcher` so it fires on all occurrences of the event.
fn make_matcher_group(port: u16, event: &str) -> serde_json::Value {
    let handler = make_hook_handler(port, event);
    serde_json::json!({
        "hooks": [handler]
    })
}

/// Check if a matcher group contains any Live Monitor hook handlers.
fn matcher_group_has_sentinel(group: &serde_json::Value) -> bool {
    group
        .get("hooks")
        .and_then(|h| h.as_array())
        .map(|handlers| {
            handlers.iter().any(|handler| {
                handler
                    .get("command")
                    .and_then(|c| c.as_str())
                    .map(|c| c.contains(SENTINEL))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

/// Remove Live Monitor hooks from all event arrays.
/// Handles both the new matcher-group format and the old flat format
/// (for cleaning up stale hooks from previous versions).
fn remove_our_hooks(hooks: &mut serde_json::Map<String, serde_json::Value>) {
    for (_event, entries) in hooks.iter_mut() {
        if let Some(arr) = entries.as_array_mut() {
            arr.retain(|entry| {
                // New format: matcher group with nested `hooks` array
                if entry.get("hooks").is_some() {
                    return !matcher_group_has_sentinel(entry);
                }
                // Old flat format: handler directly in event array (legacy cleanup)
                entry
                    .get("command")
                    .and_then(|c| c.as_str())
                    .map(|c| !c.contains(SENTINEL))
                    .unwrap_or(true)
            });
        }
    }
}

/// Register 14 hooks in ~/.claude/settings.json.
/// Removes any previous Live Monitor hooks first (idempotent).
/// Called from create_app_full() on server startup.
pub fn register(port: u16) {
    let path = settings_path();

    // Read existing or create minimal settings
    let mut settings: serde_json::Value = if path.exists() {
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        // Ensure parent dir exists
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        serde_json::json!({})
    };

    // Ensure hooks object exists
    if settings.get("hooks").is_none() {
        settings["hooks"] = serde_json::json!({});
    }

    // Remove previous Live Monitor hooks (both old flat format and new matcher format)
    if let Some(hooks) = settings["hooks"].as_object_mut() {
        remove_our_hooks(hooks);
    }

    // Append our 14 hooks in the new matcher-group format
    let hooks = settings["hooks"].as_object_mut().expect("hooks is object");
    for event in HOOK_EVENTS {
        let matcher_group = make_matcher_group(port, event);
        let arr = hooks
            .entry(event.to_string())
            .or_insert_with(|| serde_json::json!([]));
        if let Some(arr) = arr.as_array_mut() {
            arr.push(matcher_group);
        }
    }

    // Validate the JSON is well-formed before writing (Claude Code skips
    // files with errors entirely, not just the invalid settings).
    let content = serde_json::to_string_pretty(&settings).expect("serialize settings");
    if serde_json::from_str::<serde_json::Value>(&content).is_err() {
        tracing::error!("Generated invalid JSON for settings.json — aborting hook registration");
        return;
    }

    // Write atomically (temp file + rename)
    let tmp_path = path.with_extension("json.tmp");
    if std::fs::write(&tmp_path, &content).is_ok() {
        let _ = std::fs::rename(&tmp_path, &path);
        tracing::info!("Registered {} Live Monitor hooks on port {}", HOOK_EVENTS.len(), port);
    } else {
        tracing::warn!("Failed to write hooks to {:?}", path);
    }
}

/// Remove Live Monitor hooks from ~/.claude/settings.json.
/// Called on graceful server shutdown.
pub fn cleanup(port: u16) {
    let _ = port; // port param reserved for future multi-instance support
    let path = settings_path();
    if !path.exists() {
        return;
    }

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return,
    };
    let mut settings: serde_json::Value = match serde_json::from_str(&content) {
        Ok(s) => s,
        Err(_) => return,
    };

    if let Some(hooks) = settings.get_mut("hooks").and_then(|h| h.as_object_mut()) {
        remove_our_hooks(hooks);
    }

    let tmp_path = path.with_extension("json.tmp");
    let serialized = serde_json::to_string_pretty(&settings).expect("serialize settings");
    if std::fs::write(&tmp_path, &serialized).is_ok() {
        let _ = std::fs::rename(&tmp_path, &path);
        tracing::info!("Cleaned up Live Monitor hooks");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_matcher_group_format() {
        let group = make_matcher_group(47892, "SessionStart");

        // Must have a `hooks` array (matcher-group format)
        let hooks_arr = group.get("hooks").unwrap().as_array().unwrap();
        assert_eq!(hooks_arr.len(), 1);

        // Must NOT have a top-level `command` (old flat format)
        assert!(group.get("command").is_none());
        assert!(group.get("type").is_none());

        // Inner handler must have the right fields
        let handler = &hooks_arr[0];
        assert_eq!(handler["type"], "command");
        assert!(handler["command"].as_str().unwrap().contains(SENTINEL));
        assert_eq!(handler["statusMessage"], "Live Monitor");

        // SessionStart is sync — no "async" field
        assert!(handler.get("async").is_none());
    }

    #[test]
    fn test_make_matcher_group_async() {
        let group = make_matcher_group(47892, "UserPromptSubmit");
        let handler = &group["hooks"][0];
        assert_eq!(handler["async"], true);
    }

    #[test]
    fn test_matcher_group_has_sentinel() {
        let our_group = make_matcher_group(47892, "Stop");
        assert!(matcher_group_has_sentinel(&our_group));

        let other_group = serde_json::json!({
            "matcher": "Bash",
            "hooks": [{ "type": "command", "command": "echo hello" }]
        });
        assert!(!matcher_group_has_sentinel(&other_group));
    }

    #[test]
    fn test_remove_our_hooks_new_format() {
        let mut hooks = serde_json::Map::new();
        hooks.insert(
            "Stop".into(),
            serde_json::json!([
                { "hooks": [{ "type": "command", "command": "echo user-hook" }] },
                make_matcher_group(47892, "Stop"),
            ]),
        );

        remove_our_hooks(&mut hooks);

        let arr = hooks["Stop"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert!(!matcher_group_has_sentinel(&arr[0]));
    }

    #[test]
    fn test_hook_command_includes_ppid_header() {
        let group = make_matcher_group(47892, "SessionStart");
        let handler = &group["hooks"][0];
        let command = handler["command"].as_str().unwrap();
        assert!(
            command.contains("X-Claude-PID"),
            "Hook command must include X-Claude-PID header, got: {}",
            command
        );
        // $PPID must NOT be inside single quotes (shell must expand it).
        // The format is: -H 'X-Claude-PID: '$PPID
        // where 'X-Claude-PID: ' is quoted and $PPID is unquoted for expansion.
        assert!(
            command.contains("'$PPID"),
            "PPID must be outside quotes for shell expansion, got: {}",
            command
        );
    }

    #[test]
    fn test_remove_our_hooks_old_flat_format() {
        // Simulate old-format hooks left over from a previous version
        let mut hooks = serde_json::Map::new();
        hooks.insert(
            "Stop".into(),
            serde_json::json!([
                { "type": "command", "command": "echo user-hook" },
                {
                    "type": "command",
                    "command": format!("curl ... 2>/dev/null || true {}", SENTINEL),
                    "async": true
                },
            ]),
        );

        remove_our_hooks(&mut hooks);

        let arr = hooks["Stop"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["command"], "echo user-hook");
    }
}
