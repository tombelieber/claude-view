//! Reader for Claude Code settings files.
//!
//! Reads ~/.claude/settings.json + ~/.claude/settings.local.json,
//! merges local overrides into global, redacts sensitive env vars,
//! and splits hooks into user vs system (claude-view).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use ts_rs::TS;

/// Hook definition from settings.json.
#[derive(Debug, Clone, Serialize, Deserialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct HookDef {
    #[serde(rename = "type")]
    pub hook_type: String,
    pub command: String,
    #[serde(default)]
    pub r#async: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,
}

/// A hook matcher group from settings.json.
#[derive(Debug, Clone, Serialize, Deserialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct HookMatcher {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matcher: Option<String>,
    pub hooks: Vec<HookDef>,
}

/// Permission rules from settings.json.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct PermissionRules {
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
    #[serde(default)]
    pub ask: Vec<String>,
}

/// A user hook (non-claude-view) for display.
#[derive(Debug, Clone, Serialize, Deserialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct UserHook {
    /// Event name (e.g. "PreToolUse", "Stop")
    pub event: String,
    /// Matcher pattern (e.g. ".*", "Bash")
    pub matcher: Option<String>,
    /// The command
    pub command: String,
    /// Whether async
    pub is_async: bool,
}

/// Plugin entry for display.
#[derive(Debug, Clone, Serialize, Deserialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct PluginEntry {
    /// Plugin ID (e.g. "superpowers")
    pub id: String,
    /// Marketplace (e.g. "claude-plugins-official")
    pub marketplace: String,
    /// Whether enabled
    pub enabled: bool,
}

/// Custom marketplace entry.
#[derive(Debug, Clone, Serialize, Deserialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct CustomMarketplace {
    pub name: String,
    pub source_type: String,  // "github" or "directory"
    pub source_value: String, // repo slug or path
}

/// Merged, redacted Claude Code settings for display.
#[derive(Debug, Clone, Serialize, Deserialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ClaudeCodeSettings {
    /// Environment variables (sensitive values redacted)
    pub env: Vec<EnvVar>,
    /// Permission rules
    pub permissions: PermissionRules,
    /// User-defined hooks (non-claude-view)
    pub user_hooks: Vec<UserHook>,
    /// Count of claude-view system hooks
    pub system_hook_count: usize,
    /// System hooks (claude-view) — only included when expanded
    pub system_hooks: Vec<UserHook>,
    /// Enabled/disabled plugins
    pub plugins: Vec<PluginEntry>,
    /// Custom marketplaces
    pub custom_marketplaces: Vec<CustomMarketplace>,
    /// Misc settings
    pub voice_enabled: bool,
    pub skip_dangerous_prompt: bool,
    /// Status line command (if set)
    pub status_line: Option<String>,
    /// Default permission mode
    pub default_mode: Option<String>,
}

/// Environment variable entry (value may be redacted).
#[derive(Debug, Clone, Serialize, Deserialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct EnvVar {
    pub key: String,
    pub value: String,
    pub redacted: bool,
}

const CLAUDE_VIEW_HOOK_SENTINEL: &str = "claude-view-hook";

/// Patterns that indicate a sensitive env var key.
const SENSITIVE_PATTERNS: &[&str] = &["KEY", "SECRET", "TOKEN", "PASSWORD", "CREDENTIAL"];

fn is_sensitive_key(key: &str) -> bool {
    let upper = key.to_uppercase();
    SENSITIVE_PATTERNS
        .iter()
        .any(|pattern| upper.contains(pattern))
}

fn redact_value(key: &str, value: &str) -> EnvVar {
    if is_sensitive_key(key) {
        EnvVar {
            key: key.to_string(),
            value: "••••••••".to_string(),
            redacted: true,
        }
    } else {
        EnvVar {
            key: key.to_string(),
            value: value.to_string(),
            redacted: false,
        }
    }
}

fn is_system_hook(hook: &HookDef) -> bool {
    hook.command.contains(CLAUDE_VIEW_HOOK_SENTINEL)
}

/// Resolve the ~/.claude/ directory.
fn claude_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude"))
}

/// Read and parse a settings JSON file, returning the raw Value.
fn read_settings_file(path: &PathBuf) -> Option<serde_json::Value> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Read and merge Claude Code settings.
pub fn read_claude_code_settings() -> ClaudeCodeSettings {
    let claude = match claude_dir() {
        Some(d) => d,
        None => return empty_settings(),
    };

    let global_path = claude.join("settings.json");
    let local_path = claude.join("settings.local.json");

    let global = read_settings_file(&global_path)
        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
    let local = read_settings_file(&local_path);

    // Merge local overrides into global (shallow merge at top level)
    let merged = if let Some(local_val) = local {
        let mut base = global.clone();
        if let (Some(base_obj), Some(local_obj)) = (base.as_object_mut(), local_val.as_object()) {
            for (k, v) in local_obj {
                // For permissions, merge arrays
                if k == "permissions" {
                    if let (Some(base_perms), Some(local_perms)) = (
                        base_obj
                            .get_mut("permissions")
                            .and_then(|v| v.as_object_mut()),
                        v.as_object(),
                    ) {
                        for (pk, pv) in local_perms {
                            if let (Some(base_arr), Some(local_arr)) = (
                                base_perms.get_mut(pk).and_then(|v| v.as_array_mut()),
                                pv.as_array(),
                            ) {
                                for item in local_arr {
                                    if !base_arr.contains(item) {
                                        base_arr.push(item.clone());
                                    }
                                }
                            } else {
                                base_perms.insert(pk.clone(), pv.clone());
                            }
                        }
                    }
                } else {
                    base_obj.insert(k.clone(), v.clone());
                }
            }
        }
        base
    } else {
        global
    };

    parse_settings(&merged)
}

fn parse_settings(value: &serde_json::Value) -> ClaudeCodeSettings {
    let obj = match value.as_object() {
        Some(o) => o,
        None => return empty_settings(),
    };

    // Environment variables
    let env = obj
        .get("env")
        .and_then(|v| v.as_object())
        .map(|env_obj| {
            let mut vars: Vec<EnvVar> = env_obj
                .iter()
                .map(|(k, v)| redact_value(k, v.as_str().unwrap_or("")))
                .collect();
            vars.sort_by(|a, b| a.key.cmp(&b.key));
            vars
        })
        .unwrap_or_default();

    // Permissions
    let permissions = obj
        .get("permissions")
        .and_then(|v| serde_json::from_value::<PermissionRules>(v.clone()).ok())
        .unwrap_or_default();

    // Hooks — split into user vs system
    let mut user_hooks = Vec::new();
    let mut system_hooks = Vec::new();
    let mut system_hook_count = 0;

    if let Some(hooks_obj) = obj.get("hooks").and_then(|v| v.as_object()) {
        for (event_name, matchers_val) in hooks_obj {
            if let Some(matchers) = matchers_val.as_array() {
                for matcher_val in matchers {
                    if let Ok(matcher) = serde_json::from_value::<HookMatcher>(matcher_val.clone())
                    {
                        for hook in &matcher.hooks {
                            let entry = UserHook {
                                event: event_name.clone(),
                                matcher: matcher.matcher.clone(),
                                command: hook.command.clone(),
                                is_async: hook.r#async,
                            };

                            if is_system_hook(hook) {
                                system_hook_count += 1;
                                system_hooks.push(entry);
                            } else {
                                user_hooks.push(entry);
                            }
                        }
                    }
                }
            }
        }
    }

    // Plugins
    let mut plugins: Vec<PluginEntry> = Vec::new();
    if let Some(plugins_obj) = obj.get("enabledPlugins").and_then(|v| v.as_object()) {
        for (full_id, enabled_val) in plugins_obj {
            let enabled = enabled_val.as_bool().unwrap_or(false);
            // Split "superpowers@claude-plugins-official" into id + marketplace
            let parts: Vec<&str> = full_id.splitn(2, '@').collect();
            let (id, marketplace) = if parts.len() == 2 {
                (parts[0].to_string(), parts[1].to_string())
            } else {
                (full_id.clone(), "unknown".to_string())
            };
            plugins.push(PluginEntry {
                id,
                marketplace,
                enabled,
            });
        }
    }
    plugins.sort_by(|a, b| {
        // Enabled first, then alphabetical
        b.enabled.cmp(&a.enabled).then_with(|| a.id.cmp(&b.id))
    });

    // Custom marketplaces
    let mut custom_marketplaces = Vec::new();
    if let Some(mkts) = obj
        .get("extraKnownMarketplaces")
        .and_then(|v| v.as_object())
    {
        for (name, source_obj) in mkts {
            if let Some(source) = source_obj.get("source").and_then(|s| s.as_object()) {
                let source_type = source
                    .get("source")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let source_value = source
                    .get("repo")
                    .or_else(|| source.get("path"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                custom_marketplaces.push(CustomMarketplace {
                    name: name.clone(),
                    source_type,
                    source_value,
                });
            }
        }
    }
    custom_marketplaces.sort_by(|a, b| a.name.cmp(&b.name));

    // Misc settings
    let voice_enabled = obj
        .get("voiceEnabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let skip_dangerous_prompt = obj
        .get("skipDangerousModePermissionPrompt")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Status line — redact claude-view internal backup
    let status_line = obj.get("statusLine").and_then(|v| {
        if let Some(cmd) = v.get("command").and_then(|c| c.as_str()) {
            Some(cmd.to_string())
        } else {
            v.as_str().map(|s| s.to_string())
        }
    });

    let default_mode = obj
        .get("permissions")
        .and_then(|p| p.get("defaultMode"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    ClaudeCodeSettings {
        env,
        permissions,
        user_hooks,
        system_hook_count,
        system_hooks,
        plugins,
        custom_marketplaces,
        voice_enabled,
        skip_dangerous_prompt,
        status_line,
        default_mode,
    }
}

fn empty_settings() -> ClaudeCodeSettings {
    ClaudeCodeSettings {
        env: Vec::new(),
        permissions: PermissionRules::default(),
        user_hooks: Vec::new(),
        system_hook_count: 0,
        system_hooks: Vec::new(),
        plugins: Vec::new(),
        custom_marketplaces: Vec::new(),
        voice_enabled: false,
        skip_dangerous_prompt: false,
        status_line: None,
        default_mode: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_sensitive_key() {
        assert!(is_sensitive_key("ANTHROPIC_API_KEY"));
        assert!(is_sensitive_key("SECRET_TOKEN"));
        assert!(is_sensitive_key("DB_PASSWORD"));
        assert!(is_sensitive_key("credential_store"));
        assert!(!is_sensitive_key("API_TIMEOUT_MS"));
        assert!(!is_sensitive_key("CLAUDE_CODE_EFFORT_LEVEL"));
    }

    #[test]
    fn test_redact_value() {
        let ev = redact_value("ANTHROPIC_API_KEY", "sk-abc123");
        assert_eq!(ev.value, "••••••••");
        assert!(ev.redacted);

        let ev = redact_value("API_TIMEOUT_MS", "3000000");
        assert_eq!(ev.value, "3000000");
        assert!(!ev.redacted);
    }

    #[test]
    fn test_is_system_hook() {
        let hook = HookDef {
            hook_type: "command".to_string(),
            command: "curl http://localhost:47892/api/live/hook # claude-view-hook".to_string(),
            r#async: true,
            status_message: None,
        };
        assert!(is_system_hook(&hook));

        let hook = HookDef {
            hook_type: "command".to_string(),
            command: "node count_tokens.js".to_string(),
            r#async: false,
            status_message: None,
        };
        assert!(!is_system_hook(&hook));
    }

    #[test]
    fn test_parse_settings_basic() {
        let json = serde_json::json!({
            "env": {
                "API_TIMEOUT_MS": "3000000",
                "ANTHROPIC_API_KEY": "sk-secret"
            },
            "permissions": {
                "allow": ["Bash(npm:*)"],
                "deny": [],
                "ask": []
            },
            "voiceEnabled": true,
            "skipDangerousModePermissionPrompt": false,
            "enabledPlugins": {
                "superpowers@claude-plugins-official": true,
                "posthog@claude-plugins-official": false
            }
        });

        let settings = parse_settings(&json);
        assert_eq!(settings.env.len(), 2);

        // Check redaction
        let api_key = settings
            .env
            .iter()
            .find(|e| e.key == "ANTHROPIC_API_KEY")
            .unwrap();
        assert_eq!(api_key.value, "••••••••");
        assert!(api_key.redacted);

        let timeout = settings
            .env
            .iter()
            .find(|e| e.key == "API_TIMEOUT_MS")
            .unwrap();
        assert_eq!(timeout.value, "3000000");
        assert!(!timeout.redacted);

        assert_eq!(settings.permissions.allow, vec!["Bash(npm:*)"]);
        assert!(settings.voice_enabled);
        assert!(!settings.skip_dangerous_prompt);

        assert_eq!(settings.plugins.len(), 2);
        // Enabled first
        assert!(settings.plugins[0].enabled);
    }

    #[test]
    fn test_parse_settings_hooks_split() {
        let json = serde_json::json!({
            "hooks": {
                "Stop": [
                    {
                        "matcher": ".*",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "node count_tokens.js",
                                "async": false
                            }
                        ]
                    }
                ],
                "PreToolUse": [
                    {
                        "hooks": [
                            {
                                "type": "command",
                                "command": "curl http://localhost:47892/api/live/hook # claude-view-hook",
                                "async": true
                            }
                        ]
                    }
                ]
            }
        });

        let settings = parse_settings(&json);
        assert_eq!(settings.user_hooks.len(), 1);
        assert_eq!(settings.user_hooks[0].event, "Stop");
        assert_eq!(settings.user_hooks[0].command, "node count_tokens.js");

        assert_eq!(settings.system_hook_count, 1);
        assert_eq!(settings.system_hooks.len(), 1);
    }
}
