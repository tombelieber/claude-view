//! Auto-inject and clean up Live Monitor hooks in ~/.claude/settings.json.
//!
//! # Two kinds of hooks (CRITICAL distinction)
//!
//! Claude Code hooks come in two semantically different flavors:
//!
//! 1. **Observation hooks** — fire *alongside* the action. The hook's stdout
//!    is ignored. Safe to register as an async curl observer that always
//!    returns `|| true`. This is what Live Monitor wants for all of its
//!    events.
//!
//! 2. **Replacement hooks** — the hook REPLACES the default action. Claude
//!    Code reads the hook's stdout to determine the result. `WorktreeCreate`
//!    is the only replacement hook today: its stdout must be the path of
//!    the created worktree. Registering a replacement hook as an async
//!    observer (whose stdout is `{"ok":true}` from POST /api/live/hook)
//!    breaks the underlying feature silently. For `WorktreeCreate`, this
//!    means every `isolation: "worktree"` subagent call fails.
//!
//! # Three-layer defense against ever registering a replacement hook
//!
//! - **Layer 1 (compile-time taxonomy):** [`ALL_HOOKS`] is a single source of
//!   truth keyed by [`HookKind`]. [`observation_events()`] filters by kind;
//!   [`replacement_events()`] yields the exact complement. A mismatch is a
//!   type error, not a runtime bug. A const length canary pins the count so
//!   that taxonomy drift at least fails to compile.
//!
//! - **Layer 2 (runtime write-path invariant):** [`register()`] asserts
//!   (via `debug_assert!` in dev, `tracing::error!` + early return in prod)
//!   that no replacement event leaks into the observation set before any
//!   bytes hit disk. If the taxonomy is ever mis-edited the process refuses
//!   to write rather than corrupt settings.json.
//!
//! - **Layer 3 (runtime cleanup-path):** on every startup, [`remove_our_hooks()`]
//!   actively strips any stale claude-view entries (sentinel-based) and
//!   drops empty arrays. This heals settings.json from previous buggy
//!   versions (e.g. a build that wrongly registered `WorktreeCreate`) and
//!   from external re-adds. A warning is logged if any replacement hook is
//!   observed in the file — whether by us or by a third party.
//!
//! # Taxonomy source
//!
//! <https://code.claude.com/docs/en/hooks.md> — fetched 2026-04-16, 26 events.
//! If Claude Code adds a new event, update [`ALL_HOOKS`] and the snapshot in
//! `taxonomy_matches_official_docs_snapshot`.

use std::path::PathBuf;

// ── Sentinel ────────────────────────────────────────────────────────────────
//
// Every hook entry written by claude-view carries this marker in its
// `command` string, allowing us to distinguish our own entries from
// user-authored ones during cleanup.

const SENTINEL: &str = "# claude-view-hook";

// ── Taxonomy ────────────────────────────────────────────────────────────────
//
// Single source of truth for every Claude Code hook event we know about.
// Categorized by [`HookKind`] so the compile-time split between observation
// and replacement is enforced by the type system.

/// Semantic kind of a Claude Code hook event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HookKind {
    /// Fires alongside the action. Claude Code ignores the hook's stdout.
    /// Safe to register as an async curl observer.
    Observation,
    /// REPLACES the default action. Claude Code parses the hook's stdout as
    /// the result (e.g. worktree path). Registering as an observer breaks
    /// the feature silently. Must never appear in the observation set.
    Replacement,
}

/// One entry in the complete Claude Code hook taxonomy.
#[derive(Debug, Clone, Copy)]
struct HookEvent {
    name: &'static str,
    kind: HookKind,
}

/// Complete taxonomy of Claude Code hook events.
///
/// Source: <https://code.claude.com/docs/en/hooks.md> (snapshot 2026-04-16).
///
/// To add a new event:
/// 1. Determine its kind from docs (observation vs replacement)
/// 2. Add it to this list with the correct kind
/// 3. Update the snapshot in `taxonomy_matches_official_docs_snapshot`
/// 4. Update [`EXPECTED_TAXONOMY_LEN`] below
const ALL_HOOKS: &[HookEvent] = &[
    // ── Session lifecycle ──
    HookEvent {
        name: "SessionStart",
        kind: HookKind::Observation,
    }, // sync — see is_sync_event
    HookEvent {
        name: "SessionEnd",
        kind: HookKind::Observation,
    },
    // ── User input ──
    HookEvent {
        name: "UserPromptSubmit",
        kind: HookKind::Observation,
    },
    // ── Tool events ──
    HookEvent {
        name: "PreToolUse",
        kind: HookKind::Observation,
    },
    HookEvent {
        name: "PostToolUse",
        kind: HookKind::Observation,
    },
    HookEvent {
        name: "PostToolUseFailure",
        kind: HookKind::Observation,
    },
    HookEvent {
        name: "PermissionRequest",
        kind: HookKind::Observation,
    },
    HookEvent {
        name: "PermissionDenied",
        kind: HookKind::Observation,
    },
    // ── Agent turn end ──
    HookEvent {
        name: "Stop",
        kind: HookKind::Observation,
    },
    HookEvent {
        name: "StopFailure",
        kind: HookKind::Observation,
    },
    // ── Notifications ──
    HookEvent {
        name: "Notification",
        kind: HookKind::Observation,
    },
    // ── Sub-entities ──
    HookEvent {
        name: "SubagentStart",
        kind: HookKind::Observation,
    },
    HookEvent {
        name: "SubagentStop",
        kind: HookKind::Observation,
    },
    HookEvent {
        name: "TeammateIdle",
        kind: HookKind::Observation,
    },
    HookEvent {
        name: "TaskCreated",
        kind: HookKind::Observation,
    },
    HookEvent {
        name: "TaskCompleted",
        kind: HookKind::Observation,
    },
    // ── Context management ──
    HookEvent {
        name: "PreCompact",
        kind: HookKind::Observation,
    },
    HookEvent {
        name: "PostCompact",
        kind: HookKind::Observation,
    },
    // ── Configuration / environment ──
    HookEvent {
        name: "InstructionsLoaded",
        kind: HookKind::Observation,
    },
    HookEvent {
        name: "ConfigChange",
        kind: HookKind::Observation,
    },
    HookEvent {
        name: "CwdChanged",
        kind: HookKind::Observation,
    },
    HookEvent {
        name: "FileChanged",
        kind: HookKind::Observation,
    },
    // ── Worktree ──
    HookEvent {
        name: "WorktreeCreate",
        kind: HookKind::Replacement,
    }, // ⚠️ DO NOT OBSERVE
    HookEvent {
        name: "WorktreeRemove",
        kind: HookKind::Observation,
    },
    // ── MCP Elicitation ──
    HookEvent {
        name: "Elicitation",
        kind: HookKind::Observation,
    },
    HookEvent {
        name: "ElicitationResult",
        kind: HookKind::Observation,
    },
];

/// Count canary — if Claude Code changes the event list, compilation fails
/// until a contributor updates the taxonomy and this constant in lock-step.
///
/// `#[allow(dead_code)]`: used at type level in `_TAXONOMY_LEN_CHECK` below
/// and in tests; Rust's `dead_code` lint only tracks runtime usage and
/// therefore flags this false positive.
#[allow(dead_code)]
const EXPECTED_TAXONOMY_LEN: usize = 26;
const _TAXONOMY_LEN_CHECK: [(); EXPECTED_TAXONOMY_LEN] = [(); ALL_HOOKS.len()];

/// Iterator over events we register as observers.
fn observation_events() -> impl Iterator<Item = &'static str> {
    ALL_HOOKS
        .iter()
        .filter(|h| matches!(h.kind, HookKind::Observation))
        .map(|h| h.name)
}

/// Iterator over events that must NEVER be registered as observers.
fn replacement_events() -> impl Iterator<Item = &'static str> {
    ALL_HOOKS
        .iter()
        .filter(|h| matches!(h.kind, HookKind::Replacement))
        .map(|h| h.name)
}

/// SessionStart is the only sync hook — it may inject `additionalContext`
/// before the session begins, so Claude Code blocks startup on it. Every
/// other hook runs async so the server can ack in the background.
fn is_sync_event(event: &str) -> bool {
    event == "SessionStart"
}

// ── Path resolution ─────────────────────────────────────────────────────────

fn settings_path() -> Option<PathBuf> {
    Some(dirs::home_dir()?.join(".claude").join("settings.json"))
}

// ── Handler / matcher-group builders ────────────────────────────────────────
//
// Claude Code hooks use a matcher-based nested format:
//   hooks[event] → [matcher_group] → hooks → [handler]
// Omitting `matcher` matches all occurrences of the event.

fn make_hook_handler(port: u16, event: &str) -> serde_json::Value {
    let command = format!(
        "curl -s -X POST http://localhost:{port}/api/live/hook \
         -H 'Content-Type: application/json' \
         -H 'X-Claude-PID: '$PPID \
         --data-binary @- 2>/dev/null || true {SENTINEL}"
    );
    let mut handler = serde_json::json!({
        "type": "command",
        "command": command,
        "statusMessage": "Live Monitor",
    });
    if !is_sync_event(event) {
        handler["async"] = serde_json::json!(true);
    }
    handler
}

fn make_matcher_group(port: u16, event: &str) -> serde_json::Value {
    serde_json::json!({
        "hooks": [make_hook_handler(port, event)],
    })
}

// ── Sentinel management ─────────────────────────────────────────────────────
//
// These helpers preserve user-authored hooks while cleaning out everything
// that carries our SENTINEL marker.

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

/// Strip claude-view entries from a single event's matcher-group array.
/// Handles both the new matcher-group format and the old flat format
/// (legacy cleanup from previous versions).
///
/// Returns `true` if the array is now empty and the parent key should be
/// dropped (prevents stale `"PermissionDenied": []` style garbage).
fn strip_sentinel_entries(arr: &mut Vec<serde_json::Value>) -> bool {
    arr.retain(|entry| {
        // New format: matcher group with nested `hooks` array
        if entry.get("hooks").is_some() {
            return !matcher_group_has_sentinel(entry);
        }
        // Old flat format: handler directly in event array
        entry
            .get("command")
            .and_then(|c| c.as_str())
            .map(|c| !c.contains(SENTINEL))
            .unwrap_or(true)
    });
    arr.is_empty()
}

/// Remove all Live Monitor entries and drop any now-empty arrays.
///
/// Empty-array drops are important: without this, every time a hook event
/// leaves the registered set (e.g. a previous release included an event we
/// later withdrew) the key lingers as `"Event": []` forever. Claude Code
/// treats it as a zero-handler no-op, but formatters and diffs see noise.
fn remove_our_hooks(hooks: &mut serde_json::Map<String, serde_json::Value>) {
    let mut keys_to_drop: Vec<String> = Vec::new();
    for (event, entries) in hooks.iter_mut() {
        if let Some(arr) = entries.as_array_mut() {
            if strip_sentinel_entries(arr) {
                keys_to_drop.push(event.clone());
            }
        } else if entries.as_array().map(|a| a.is_empty()).unwrap_or(false) {
            // Non-mutable empty-array case (unreachable here but defensive)
            keys_to_drop.push(event.clone());
        }
    }
    // Also drop keys that arrived already-empty (e.g. `"PermissionDenied": []`
    // left over from a formatter collapse before we ever saw the file).
    for (event, entries) in hooks.iter() {
        if let Some(arr) = entries.as_array() {
            if arr.is_empty() && !keys_to_drop.contains(event) {
                keys_to_drop.push(event.clone());
            }
        }
    }
    for key in keys_to_drop {
        hooks.remove(&key);
    }
}

// ── Public registration API ─────────────────────────────────────────────────

/// Register all observation hooks in ~/.claude/settings.json and clean up
/// any stale entries from previous versions.
///
/// Invariants enforced:
/// 1. Only [`HookKind::Observation`] events are ever written (compile-time
///    via [`observation_events()`] + runtime write-path assert).
/// 2. Any stale claude-view entries for replacement events are stripped.
/// 3. Empty arrays are removed (no `"Event": []` lingering).
/// 4. Atomic write with pre-flight JSON validation — never leaves a corrupt
///    settings.json on disk.
/// 5. Idempotent — calling multiple times yields the same result.
pub fn register(port: u16) {
    let Some(path) = settings_path() else {
        tracing::error!("could not determine home directory");
        return;
    };

    // ── Layer 2a: runtime invariant on the taxonomy itself ──
    // If ALL_HOOKS is mis-edited so the same name is both observation and
    // replacement, bail out loudly rather than corrupt settings.json.
    for rep in replacement_events() {
        if observation_events().any(|o| o == rep) {
            tracing::error!(
                event = rep,
                "FATAL: hook taxonomy is corrupted — {} is both observation \
                 and replacement. Refusing to register any hooks.",
                rep
            );
            return;
        }
    }

    // Read existing or create minimal settings
    let mut settings: serde_json::Value = if path.exists() {
        match std::fs::read_to_string(&path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_else(|e| {
                tracing::warn!(error = %e, path = %path.display(), "settings.json invalid JSON — resetting to empty");
                serde_json::json!({})
            }),
            Err(e) => {
                tracing::warn!(error = %e, path = %path.display(), "failed to read settings.json, treating as empty");
                serde_json::json!({})
            }
        }
    } else {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        serde_json::json!({})
    };

    if settings.get("hooks").is_none() {
        settings["hooks"] = serde_json::json!({});
    }

    let Some(hooks) = settings["hooks"].as_object_mut() else {
        tracing::error!("settings.json has unexpected structure — cannot register hooks");
        return;
    };

    // ── Layer 3: cleanup (remove our stale entries + empty arrays) ──
    remove_our_hooks(hooks);

    // ── Defensive observability: warn if a replacement hook remains in the
    //    file (not ours — e.g. user's legitimate custom replacement, or an
    //    unknown third party). We do NOT touch non-sentinel entries.
    for event in replacement_events() {
        if let Some(entries) = hooks.get(event) {
            let non_empty = entries.as_array().map(|a| !a.is_empty()).unwrap_or(false);
            if non_empty {
                tracing::warn!(
                    event = event,
                    "{} hook is present in settings.json but is a REPLACEMENT hook. \
                     Live Monitor will NOT observe it. If this entry was authored by \
                     another tool, make sure it prints the expected result on stdout; \
                     otherwise Claude Code features backed by this hook will fail.",
                    event
                );
            }
        }
    }

    // ── Layer 2b: runtime write-path invariant ──
    // Before writing a single byte, assert we are about to write ONLY
    // observation events. This is a last line of defense — if it ever
    // trips, a bug has bypassed the compile-time split.
    let obs: Vec<&'static str> = observation_events().collect();
    for o in &obs {
        if replacement_events().any(|r| r == *o) {
            tracing::error!(
                event = o,
                "FATAL: observation_events() yielded a replacement hook. \
                 Refusing to write settings.json."
            );
            return;
        }
    }

    // Append our matcher-group for each observation event.
    for event in &obs {
        let matcher_group = make_matcher_group(port, event);
        let arr = hooks
            .entry((*event).to_string())
            .or_insert_with(|| serde_json::json!([]));
        if let Some(arr) = arr.as_array_mut() {
            arr.push(matcher_group);
        }
    }

    // Validate JSON is well-formed (Claude Code skips files with errors
    // entirely — not just the invalid settings).
    let Ok(content) = serde_json::to_string_pretty(&settings) else {
        tracing::error!("failed to serialize settings.json");
        return;
    };
    if serde_json::from_str::<serde_json::Value>(&content).is_err() {
        tracing::error!("generated invalid JSON for settings.json — aborting hook registration");
        return;
    }

    // Atomic write (temp file + rename)
    let tmp_path = path.with_extension("json.tmp");
    if std::fs::write(&tmp_path, &content).is_ok() {
        if let Err(e) = std::fs::rename(&tmp_path, &path) {
            tracing::error!(error = %e, "failed to rename settings.json");
            return;
        }
        tracing::info!(count = obs.len(), port, "Registered Live Monitor hooks");
    } else {
        tracing::warn!("failed to write hooks to {:?}", path);
    }
}

/// Remove Live Monitor hooks from ~/.claude/settings.json.
/// Called on graceful server shutdown and by the `cleanup` subcommand.
/// Returns a list of what was cleaned up for user feedback.
pub fn cleanup(_port: u16) -> Vec<String> {
    let mut removed = Vec::new();
    let Some(path) = settings_path() else {
        tracing::error!("could not determine home directory");
        return removed;
    };

    if path.exists() {
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return removed,
        };
        let mut settings: serde_json::Value = match serde_json::from_str(&content) {
            Ok(s) => s,
            Err(_) => return removed,
        };

        let had_hooks = settings
            .get("hooks")
            .and_then(|h| h.as_object())
            .map(|hooks| {
                hooks.values().any(|entries| {
                    entries
                        .as_array()
                        .map(|arr| arr.iter().any(matcher_group_has_sentinel))
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);

        if had_hooks {
            if let Some(hooks) = settings.get_mut("hooks").and_then(|h| h.as_object_mut()) {
                remove_our_hooks(hooks);
            }

            let tmp_path = path.with_extension("json.tmp");
            let Ok(serialized) = serde_json::to_string_pretty(&settings) else {
                tracing::error!("failed to serialize settings.json");
                return removed;
            };
            if std::fs::write(&tmp_path, &serialized).is_ok() {
                if let Err(e) = std::fs::rename(&tmp_path, &path) {
                    tracing::error!(error = %e, "failed to rename settings.json");
                }
                removed.push("Removed hooks from ~/.claude/settings.json".to_string());
                tracing::info!("Cleaned up Live Monitor hooks");
            }
        }
    }

    // Clean up atomic-write temp file if left behind by a crash
    let tmp_path = path.with_extension("json.tmp");
    if tmp_path.exists() {
        match std::fs::remove_file(&tmp_path) {
            Ok(()) => removed.push("Removed ~/.claude/settings.json.tmp".to_string()),
            Err(e) => removed.push(format!("Failed to remove settings.json.tmp: {}", e)),
        }
    }

    removed
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // ── Taxonomy tests ──

    #[test]
    fn taxonomy_has_no_duplicates() {
        let mut seen = HashSet::new();
        for h in ALL_HOOKS {
            assert!(
                seen.insert(h.name),
                "duplicate event in ALL_HOOKS: {}",
                h.name
            );
        }
    }

    #[test]
    fn taxonomy_length_canary() {
        assert_eq!(
            ALL_HOOKS.len(),
            EXPECTED_TAXONOMY_LEN,
            "ALL_HOOKS length drifted from EXPECTED_TAXONOMY_LEN — update both in lock-step"
        );
    }

    /// If this test fails: the official Claude Code docs have added or
    /// removed an event. Fetch https://code.claude.com/docs/en/hooks.md,
    /// update [`ALL_HOOKS`] with the new taxonomy, then update this snapshot
    /// and [`EXPECTED_TAXONOMY_LEN`] to match.
    #[test]
    fn taxonomy_matches_official_docs_snapshot() {
        const OFFICIAL_26: &[&str] = &[
            "SessionStart",
            "UserPromptSubmit",
            "PreToolUse",
            "PermissionRequest",
            "PermissionDenied",
            "PostToolUse",
            "PostToolUseFailure",
            "Notification",
            "SubagentStart",
            "SubagentStop",
            "TaskCreated",
            "TaskCompleted",
            "Stop",
            "StopFailure",
            "TeammateIdle",
            "InstructionsLoaded",
            "ConfigChange",
            "CwdChanged",
            "FileChanged",
            "WorktreeCreate",
            "WorktreeRemove",
            "PreCompact",
            "PostCompact",
            "Elicitation",
            "ElicitationResult",
            "SessionEnd",
        ];
        let taxonomy: HashSet<&str> = ALL_HOOKS.iter().map(|h| h.name).collect();
        let official: HashSet<&str> = OFFICIAL_26.iter().copied().collect();

        let missing: Vec<_> = official.difference(&taxonomy).collect();
        let extra: Vec<_> = taxonomy.difference(&official).collect();

        assert!(
            missing.is_empty() && extra.is_empty(),
            "taxonomy drift from official docs:\n  missing from ALL_HOOKS: {missing:?}\n  extra in ALL_HOOKS: {extra:?}"
        );
    }

    #[test]
    fn worktree_create_is_replacement_not_observation() {
        let entry = ALL_HOOKS
            .iter()
            .find(|h| h.name == "WorktreeCreate")
            .expect("WorktreeCreate must exist in taxonomy");
        assert_eq!(
            entry.kind,
            HookKind::Replacement,
            "WorktreeCreate MUST be a replacement hook"
        );
        assert!(
            !observation_events().any(|e| e == "WorktreeCreate"),
            "WorktreeCreate leaked into observation_events() — would break all \
             `isolation: \"worktree\"` subagent calls"
        );
        assert!(
            replacement_events().any(|e| e == "WorktreeCreate"),
            "WorktreeCreate missing from replacement_events()"
        );
    }

    #[test]
    fn worktree_remove_is_observation() {
        // WorktreeRemove fires alongside (not replacing) the removal, so
        // observing it is safe and correct.
        assert!(observation_events().any(|e| e == "WorktreeRemove"));
    }

    #[test]
    fn permission_denied_is_registered() {
        // Regression: used to be missing from HOOK_EVENTS, leaving a stale
        // `"PermissionDenied": []` in settings.json.
        assert!(observation_events().any(|e| e == "PermissionDenied"));
    }

    #[test]
    fn observation_and_replacement_sets_are_disjoint() {
        let obs: HashSet<&str> = observation_events().collect();
        let rep: HashSet<&str> = replacement_events().collect();
        let overlap: Vec<_> = obs.intersection(&rep).collect();
        assert!(
            overlap.is_empty(),
            "observation/replacement overlap: {overlap:?}"
        );
    }

    #[test]
    fn observation_union_replacement_equals_taxonomy() {
        let obs: HashSet<&str> = observation_events().collect();
        let rep: HashSet<&str> = replacement_events().collect();
        let union: HashSet<&str> = obs.union(&rep).copied().collect();
        let taxonomy: HashSet<&str> = ALL_HOOKS.iter().map(|h| h.name).collect();
        assert_eq!(union, taxonomy, "obs ∪ rep must equal the full taxonomy");
    }

    // ── Handler/matcher-group builder tests ──

    #[test]
    fn make_matcher_group_uses_nested_format() {
        let group = make_matcher_group(47892, "SessionStart");
        let hooks_arr = group.get("hooks").unwrap().as_array().unwrap();
        assert_eq!(hooks_arr.len(), 1);
        // Must NOT be the old flat format
        assert!(group.get("command").is_none());
        assert!(group.get("type").is_none());
        let handler = &hooks_arr[0];
        assert_eq!(handler["type"], "command");
        assert!(handler["command"].as_str().unwrap().contains(SENTINEL));
        assert_eq!(handler["statusMessage"], "Live Monitor");
    }

    #[test]
    fn session_start_is_sync_no_async_flag() {
        let group = make_matcher_group(47892, "SessionStart");
        let handler = &group["hooks"][0];
        assert!(
            handler.get("async").is_none(),
            "SessionStart must be sync (no async flag) to allow additionalContext injection"
        );
    }

    #[test]
    fn non_session_start_is_async() {
        for event in observation_events().filter(|e| !is_sync_event(e)) {
            let group = make_matcher_group(47892, event);
            assert_eq!(group["hooks"][0]["async"], true, "{event} must be async");
        }
    }

    #[test]
    fn hook_command_includes_ppid_header() {
        let group = make_matcher_group(47892, "PreToolUse");
        let handler = &group["hooks"][0];
        let command = handler["command"].as_str().unwrap();
        assert!(
            command.contains("X-Claude-PID"),
            "missing X-Claude-PID header"
        );
        // $PPID must be outside quotes for shell expansion
        assert!(command.contains("'$PPID"), "PPID not shell-expanded");
    }

    // ── Sentinel management tests ──

    #[test]
    fn matcher_group_has_sentinel_detects_ours() {
        let ours = make_matcher_group(47892, "Stop");
        assert!(matcher_group_has_sentinel(&ours));
    }

    #[test]
    fn matcher_group_has_sentinel_ignores_user_entries() {
        let user = serde_json::json!({
            "matcher": "Bash",
            "hooks": [{ "type": "command", "command": "echo hello" }],
        });
        assert!(!matcher_group_has_sentinel(&user));
    }

    #[test]
    fn strip_sentinel_entries_preserves_user_entries() {
        let mut arr = vec![
            serde_json::json!({ "hooks": [{ "type": "command", "command": "echo user" }] }),
            make_matcher_group(47892, "Stop"),
        ];
        let empty = strip_sentinel_entries(&mut arr);
        assert!(!empty);
        assert_eq!(arr.len(), 1);
        assert!(!matcher_group_has_sentinel(&arr[0]));
    }

    #[test]
    fn strip_sentinel_entries_handles_old_flat_format() {
        let mut arr = vec![
            serde_json::json!({ "type": "command", "command": "echo user-hook" }),
            serde_json::json!({
                "type": "command",
                "command": format!("curl ... 2>/dev/null || true {}", SENTINEL),
                "async": true,
            }),
        ];
        let empty = strip_sentinel_entries(&mut arr);
        assert!(!empty);
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["command"], "echo user-hook");
    }

    #[test]
    fn strip_sentinel_entries_reports_empty_when_only_ours() {
        let mut arr = vec![make_matcher_group(47892, "Stop")];
        let empty = strip_sentinel_entries(&mut arr);
        assert!(empty, "array should be empty after removing only-our entry");
        assert_eq!(arr.len(), 0);
    }

    // ── remove_our_hooks tests ──

    #[test]
    fn remove_our_hooks_drops_stale_worktree_create() {
        // Simulate the exact bug we are fixing: a stale claude-view
        // WorktreeCreate registered by a previous buggy version.
        let mut hooks = serde_json::Map::new();
        hooks.insert(
            "WorktreeCreate".into(),
            serde_json::json!([{
                "hooks": [{
                    "type": "command",
                    "command": format!("curl -s ... || true {}", SENTINEL),
                    "async": true,
                }]
            }]),
        );
        remove_our_hooks(&mut hooks);
        assert!(
            !hooks.contains_key("WorktreeCreate"),
            "stale WorktreeCreate entry must be fully removed, not left as []"
        );
    }

    #[test]
    fn remove_our_hooks_preserves_user_worktree_create() {
        // User may have a legitimate custom WorktreeCreate replacement.
        // We must NOT touch it.
        let user_entry = serde_json::json!({
            "hooks": [{
                "type": "command",
                "command": "/usr/local/bin/my-custom-worktree-creator",
            }]
        });
        let mut hooks = serde_json::Map::new();
        hooks.insert(
            "WorktreeCreate".into(),
            serde_json::json!([user_entry.clone()]),
        );
        remove_our_hooks(&mut hooks);
        let arr = hooks.get("WorktreeCreate").unwrap().as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0], user_entry);
    }

    #[test]
    fn remove_our_hooks_removes_only_our_entries_when_mixed() {
        let user_entry = serde_json::json!({
            "hooks": [{ "type": "command", "command": "/usr/local/bin/custom" }]
        });
        let mut hooks = serde_json::Map::new();
        hooks.insert(
            "WorktreeCreate".into(),
            serde_json::json!([
                user_entry.clone(),
                {
                    "hooks": [{
                        "type": "command",
                        "command": format!("curl ... {}", SENTINEL),
                        "async": true,
                    }]
                }
            ]),
        );
        remove_our_hooks(&mut hooks);
        let arr = hooks.get("WorktreeCreate").unwrap().as_array().unwrap();
        assert_eq!(arr.len(), 1, "user entry must survive");
        assert_eq!(arr[0], user_entry);
    }

    #[test]
    fn remove_our_hooks_drops_preexisting_empty_arrays() {
        // Regression: `"PermissionDenied": []` left over from a formatter or
        // older version must be removed (otherwise it lingers forever).
        let mut hooks = serde_json::Map::new();
        hooks.insert("PermissionDenied".into(), serde_json::json!([]));
        remove_our_hooks(&mut hooks);
        assert!(
            !hooks.contains_key("PermissionDenied"),
            "empty array must be dropped"
        );
    }

    #[test]
    fn remove_our_hooks_preserves_coexisting_user_stop_hook() {
        // User's own Stop hook (e.g. count_tokens.js) must survive a round
        // of cleanup even when our Stop entry is also present.
        let user_stop = serde_json::json!({
            "hooks": [{ "type": "command", "command": "/Users/me/count_tokens.js" }],
            "matcher": ".*",
        });
        let mut hooks = serde_json::Map::new();
        hooks.insert(
            "Stop".into(),
            serde_json::json!([user_stop.clone(), make_matcher_group(47892, "Stop")]),
        );
        remove_our_hooks(&mut hooks);
        let arr = hooks.get("Stop").unwrap().as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0], user_stop);
    }

    // ── Round-trip integration tests ──

    #[test]
    fn register_would_write_exactly_one_entry_per_observation_event() {
        // Mimic the core of register() to verify the final settings object
        // contains one claude-view entry for each observation event and none
        // for any replacement event.
        let mut hooks = serde_json::Map::new();
        remove_our_hooks(&mut hooks);
        for event in observation_events() {
            let matcher_group = make_matcher_group(47892, event);
            let arr = hooks
                .entry(event.to_string())
                .or_insert_with(|| serde_json::json!([]));
            if let Some(arr) = arr.as_array_mut() {
                arr.push(matcher_group);
            }
        }
        // Every observation event present
        for event in observation_events() {
            assert!(hooks.contains_key(event), "{event} missing after register");
            let arr = hooks[event].as_array().unwrap();
            assert_eq!(arr.len(), 1);
            assert!(matcher_group_has_sentinel(&arr[0]));
        }
        // No replacement event present
        for event in replacement_events() {
            assert!(
                !hooks.contains_key(event),
                "{event} (replacement) must NEVER be in the registered hooks"
            );
        }
    }

    #[test]
    fn register_is_idempotent_across_stale_entries() {
        // Start with a polluted settings.json: stale WorktreeCreate, empty
        // PermissionDenied, and a user's Stop hook. After a cleanup +
        // re-register cycle, user's Stop hook survives and all our entries
        // are exactly what observation_events() prescribes.
        let user_stop = serde_json::json!({
            "hooks": [{ "type": "command", "command": "/Users/me/count.js" }],
        });
        let mut hooks = serde_json::Map::new();
        hooks.insert("PermissionDenied".into(), serde_json::json!([]));
        hooks.insert(
            "WorktreeCreate".into(),
            serde_json::json!([{
                "hooks": [{
                    "type": "command",
                    "command": format!("curl ... {}", SENTINEL),
                    "async": true,
                }]
            }]),
        );
        hooks.insert(
            "Stop".into(),
            serde_json::json!([user_stop.clone(), make_matcher_group(47892, "Stop")]),
        );

        // Cleanup
        remove_our_hooks(&mut hooks);
        // Re-register (inline, same as register())
        for event in observation_events() {
            let matcher_group = make_matcher_group(47892, event);
            let arr = hooks
                .entry(event.to_string())
                .or_insert_with(|| serde_json::json!([]));
            if let Some(arr) = arr.as_array_mut() {
                arr.push(matcher_group);
            }
        }

        // User's Stop hook survived alongside ours
        let stop_arr = hooks["Stop"].as_array().unwrap();
        assert_eq!(stop_arr.len(), 2);
        assert_eq!(stop_arr[0], user_stop);
        assert!(matcher_group_has_sentinel(&stop_arr[1]));

        // WorktreeCreate is entirely gone
        assert!(!hooks.contains_key("WorktreeCreate"));

        // PermissionDenied is now properly registered
        let pd_arr = hooks["PermissionDenied"].as_array().unwrap();
        assert_eq!(pd_arr.len(), 1);
        assert!(matcher_group_has_sentinel(&pd_arr[0]));
    }
}
