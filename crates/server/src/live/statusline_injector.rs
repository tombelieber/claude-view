//! Inject a claude-view statusline command into ~/.claude/settings.json.
//!
//! Claude Code supports a single `statusLine` command that receives rich
//! per-turn JSON (context_window_size, used_percentage, cost, model, etc.)
//! via stdin. This is the authoritative source for context window data —
//! it knows the real max (200K vs 1M) from turn 1, without any guessing.
//!
//! Since `statusLine` is a single slot (not an array like hooks), we wrap
//! the user's existing command rather than replacing it:
//!   - Read the user's current statusLine command (if any), save as "original"
//!   - Write a wrapper script to ~/.cache/claude-view/statusline-wrapper.sh
//!   - Set statusLine.command to the wrapper path
//!   - On cleanup: restore the original statusLine (or remove ours)
//!
//! The wrapper pipes stdin to both the user's original command (for their
//! terminal bar output) AND fires a background curl to our server.

use std::path::PathBuf;

const SENTINEL: &str = "# claude-view-statusline";
const WRAPPER_SCRIPT_NAME: &str = "statusline-wrapper.sh";

fn settings_path() -> Option<PathBuf> {
    Some(dirs::home_dir()?.join(".claude").join("settings.json"))
}

fn wrapper_script_path() -> Option<PathBuf> {
    Some(
        dirs::cache_dir()?
            .join("claude-view")
            .join(WRAPPER_SCRIPT_NAME),
    )
}

/// Build the wrapper shell script content.
///
/// The script:
/// 1. Reads stdin once into $input
/// 2. Extracts session_id from the JSON (needed for server correlation)
/// 3. POSTs the full JSON to our server in background (fire-and-forget)
/// 4. If the user had an original command, pipes $input to it and forwards stdout/exit code
/// 5. If no original command, exits silently (no terminal output)
fn build_wrapper_script(port: u16, original_command: Option<&str>) -> String {
    let post_block = format!(
        r#"input=$(cat)
session_id=$(printf '%s' "$input" | jq -r '.session_id // empty' 2>/dev/null)
[ -n "$session_id" ] && printf '%s' "$input" | \
  curl -s -X POST "http://localhost:{port}/api/live/statusline" \
    -H 'Content-Type: application/json' \
    --data-binary @- \
    > /dev/null 2>&1 & {sentinel}"#,
        port = port,
        sentinel = SENTINEL,
    );

    match original_command {
        Some(cmd) if !cmd.trim().is_empty() => {
            // Escape single quotes in the original command for sh -c '...' context.
            let escaped = cmd.replace('\'', "'\\''");
            format!(
                "#!/bin/sh\n{post_block}\nprintf '%s' \"$input\" | sh -c '{escaped}'\n",
                post_block = post_block,
                escaped = escaped,
            )
        }
        _ => {
            // No original command — just POST, no terminal output
            format!("#!/bin/sh\n{post_block}\n")
        }
    }
}

/// Inject our statusline wrapper into ~/.claude/settings.json.
///
/// Saves the user's current statusLine command (if any) as
/// `_claude_view_original_statusline` so it can be restored on cleanup.
/// Writes the wrapper script to ~/.cache/claude-view/statusline-wrapper.sh.
/// Called at server startup alongside hook_registrar::register().
pub fn register(port: u16) {
    let Some(settings_path) = settings_path() else {
        tracing::warn!("statusline_injector: could not determine home directory");
        return;
    };
    let Some(wrapper_path) = wrapper_script_path() else {
        tracing::warn!("statusline_injector: could not determine cache directory");
        return;
    };

    // Read existing settings
    let mut settings: serde_json::Value = if settings_path.exists() {
        let content = match std::fs::read_to_string(&settings_path) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(error = %e, "statusline_injector: failed to read settings.json");
                return;
            }
        };
        serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    // If we already injected our wrapper (sentinel present), skip re-injection
    // to avoid overwriting on rapid restarts.
    if let Some(cmd) = settings
        .get("statusLine")
        .and_then(|s| s.get("command"))
        .and_then(|c| c.as_str())
    {
        if cmd.contains(SENTINEL) || cmd.contains(WRAPPER_SCRIPT_NAME) {
            tracing::debug!("statusline_injector: wrapper already injected, skipping");
            return;
        }
    }

    // Capture the user's existing statusLine command (if any)
    let original_command = settings
        .get("statusLine")
        .and_then(|s| s.get("command"))
        .and_then(|c| c.as_str())
        .map(|s| s.to_string());

    // Save original so cleanup can restore it
    if let Some(ref orig) = original_command {
        settings["_claude_view_original_statusline"] = serde_json::json!(orig);
    } else {
        // Explicitly record that there was no original, so cleanup removes statusLine entirely
        settings["_claude_view_original_statusline"] = serde_json::Value::Null;
    }

    // Write the wrapper script
    let script_content = build_wrapper_script(port, original_command.as_deref());

    if let Some(parent) = wrapper_path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            tracing::warn!(error = %e, path = %parent.display(), "statusline_injector: failed to create wrapper directory");
        }
    }

    if let Err(e) = std::fs::write(&wrapper_path, &script_content) {
        tracing::warn!(error = %e, "statusline_injector: failed to write wrapper script");
        return;
    }

    // Make it executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&wrapper_path, std::fs::Permissions::from_mode(0o755));
    }

    // Point statusLine at our wrapper
    settings["statusLine"] = serde_json::json!({
        "type": "command",
        "command": wrapper_path.display().to_string(),
    });

    // Validate + write atomically
    let Ok(content) = serde_json::to_string_pretty(&settings) else {
        tracing::error!("statusline_injector: failed to serialize settings.json");
        return;
    };

    let tmp_path = settings_path.with_extension("json.tmp");
    if std::fs::write(&tmp_path, &content).is_ok() {
        if let Err(e) = std::fs::rename(&tmp_path, &settings_path) {
            tracing::error!(error = %e, "statusline_injector: failed to rename settings.json");
        } else {
            tracing::info!(
                port = port,
                wrapper = %wrapper_path.display(),
                original = ?original_command,
                "Registered statusline wrapper"
            );
        }
    }
}

/// Restore the user's original statusLine in ~/.claude/settings.json.
/// Called at server shutdown alongside hook_registrar::cleanup().
pub fn cleanup() {
    let Some(settings_path) = settings_path() else {
        return;
    };
    let Some(wrapper_path) = wrapper_script_path() else {
        return;
    };

    if !settings_path.exists() {
        return;
    }

    let content = match std::fs::read_to_string(&settings_path) {
        Ok(c) => c,
        Err(_) => return,
    };
    let mut settings: serde_json::Value = match serde_json::from_str(&content) {
        Ok(s) => s,
        Err(_) => return,
    };

    // Only act if our wrapper is currently set
    let is_our_wrapper = settings
        .get("statusLine")
        .and_then(|s| s.get("command"))
        .and_then(|c| c.as_str())
        .map(|cmd| cmd.contains(SENTINEL) || cmd.contains(WRAPPER_SCRIPT_NAME))
        .unwrap_or(false);

    if !is_our_wrapper {
        return;
    }

    // Restore original
    let original = settings
        .get("_claude_view_original_statusline")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    match original {
        serde_json::Value::String(cmd) => {
            settings["statusLine"] = serde_json::json!({
                "type": "command",
                "command": cmd,
            });
        }
        _ => {
            // No original — remove statusLine entirely
            if let Some(obj) = settings.as_object_mut() {
                obj.remove("statusLine");
            }
        }
    }

    // Remove our saved key
    if let Some(obj) = settings.as_object_mut() {
        obj.remove("_claude_view_original_statusline");
    }

    let tmp_path = settings_path.with_extension("json.tmp");
    if let Ok(serialized) = serde_json::to_string_pretty(&settings) {
        if std::fs::write(&tmp_path, &serialized).is_ok() {
            if let Err(e) = std::fs::rename(&tmp_path, &settings_path) {
                tracing::error!(error = %e, "statusline_injector: failed to restore settings.json");
            } else {
                tracing::info!("Restored original statusLine on shutdown");
            }
        }
    }

    // Remove wrapper script
    let _ = std::fs::remove_file(&wrapper_path);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrapper_script_with_original_command() {
        let script = build_wrapper_script(47892, Some("bash ~/.claude/statusline-command.sh"));
        assert!(script.starts_with("#!/bin/sh\n"));
        assert!(script.contains(SENTINEL));
        assert!(script.contains("http://localhost:47892/api/live/statusline"));
        // Command must be wrapped in sh -c '...' for shell safety
        assert!(script.contains("sh -c 'bash ~/.claude/statusline-command.sh'"));
        // Must read stdin once and reuse $input — not read stdin twice
        assert!(script.contains("input=$(cat)"));
        let cat_count = script.matches("$(cat)").count();
        assert_eq!(cat_count, 1, "stdin must be read exactly once");
    }

    #[test]
    fn test_wrapper_script_without_original_command() {
        let script = build_wrapper_script(47892, None);
        assert!(script.starts_with("#!/bin/sh\n"));
        assert!(script.contains(SENTINEL));
        assert!(script.contains("http://localhost:47892/api/live/statusline"));
        // No forwarding to original command
        assert!(!script.contains("| bash"));
    }

    #[test]
    fn test_wrapper_script_empty_original_is_treated_as_none() {
        let script_none = build_wrapper_script(47892, None);
        let script_empty = build_wrapper_script(47892, Some(""));
        // Both should produce the same output (no forwarding)
        assert_eq!(script_none, script_empty);
    }

    #[test]
    fn shell_injection_single_quotes_escaped() {
        let script = build_wrapper_script(47892, Some("echo 'hello world'"));
        // Single quotes in the original command must be escaped for sh -c '...' context
        assert!(script.contains("sh -c '"), "must use sh -c wrapper");
        assert!(
            script.contains(r"'\''"),
            "single quotes must be escaped as '\\''"
        );
        // Verify the command is NOT interpolated raw (without sh -c wrapper)
        assert!(
            !script.contains("| echo 'hello world'\n"),
            "command must be inside sh -c, not raw-interpolated after pipe"
        );
        // Verify the full sh -c invocation contains the escaped command
        assert!(
            script.contains("sh -c 'echo '\\''hello world'\\'''"),
            "full escaped command must appear inside sh -c wrapper"
        );
    }

    #[test]
    fn shell_injection_semicolon_neutralized() {
        let script = build_wrapper_script(47892, Some("foo; rm -rf ~"));
        // Inside sh -c '...', semicolons are literal, not command separators
        assert!(script.contains("sh -c '"));
        // The semicolon MUST be inside the single-quoted sh -c argument,
        // NOT as a bare command separator outside the wrapper.
        let sh_c_idx = script.find("sh -c '").expect("must have sh -c wrapper");
        let after_sh_c = &script[sh_c_idx..];
        assert!(
            after_sh_c.contains("foo; rm -rf ~"),
            "dangerous payload must be INSIDE sh -c single-quoted argument, got: {after_sh_c}"
        );
        let before_sh_c = &script[..sh_c_idx];
        assert!(
            !before_sh_c.contains("foo; rm -rf ~"),
            "dangerous payload must NOT appear before sh -c wrapper"
        );
    }

    #[test]
    fn shell_injection_backtick_neutralized() {
        let script = build_wrapper_script(47892, Some("echo `whoami`"));
        // Inside sh -c '...', backticks are literal
        assert!(script.contains("sh -c '"));
    }
}
