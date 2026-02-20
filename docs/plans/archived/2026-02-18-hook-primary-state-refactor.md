---
status: done
date: 2026-02-18
---

# Hook-Primary State Refactor

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the dual-source state system (hooks + JSONL merged by StateResolver) with a hook-only state machine where hooks are the sole authority for agent state, and JSONL is demoted to metadata-only enrichment.

**Architecture:** All 14 Claude Code hooks drive ALL state transitions via a simple FSM. JSONL file watching only extracts metadata (tokens, cost, model, context window, todos, git branch). The StateResolver, SessionStateClassifier, and all JSONL-based state derivation are deleted.

**Tech Stack:** Rust (Axum), React/TypeScript, Claude Code hooks API

---

## What Gets Deleted

| Component | File | Lines | Why |
|-----------|------|-------|-----|
| StateResolver | `crates/server/src/live/state_resolver.rs` | ~408 | No dual-source merge needed |
| SessionStateClassifier | `crates/server/src/live/classifier.rs` | ~349 | No JSONL-based pause classification |
| `derive_agent_state()` | `manager.rs:133-218` | ~85 | Hooks determine state, not JSONL |
| `pause_classification_to_agent_state()` | `manager.rs:114-131` | ~18 | Classifier gone |
| `derive_status()` | `state.rs:170-239` | ~70 | Status derived from agent_state group |
| `SignalSource` enum | `state.rs:38-44` | ~7 | Only one source: hooks |
| `confidence` field | `state.rs:20-21` | — | Hooks are always definitive |
| 3-phase lock pattern | `manager.rs` (process_detector) | ~200 | No state re-derivation |
| `recent_messages` ring buffer | `manager.rs` (accumulator) | — | Was for classifier context |

**Net reduction:** ~750+ lines deleted, simpler logic throughout.

---

## New State Machine

Every hook maps to exactly one state. No merging, no expiry, no confidence scores.

```
SessionStart(startup/resume/clear) ──→ NeedsYou / idle
SessionStart(compact) ──────────────→ Autonomous / thinking
UserPromptSubmit ───────────────────→ Autonomous / thinking
PreToolUse(AskUserQuestion) ────────→ NeedsYou / awaiting_input
PreToolUse(ExitPlanMode) ──────────→ NeedsYou / awaiting_approval
PreToolUse(other) ─────────────────→ Autonomous / acting  (+currentActivity from tool_input)
PostToolUse(any) ──────────────────→ Autonomous / thinking
PostToolUseFailure(interrupt) ─────→ NeedsYou / interrupted
PostToolUseFailure(error) ─────────→ NeedsYou / error
PermissionRequest ─────────────────→ NeedsYou / needs_permission  (+tool_name, +permission_suggestions)
Stop ──────────────────────────────→ NeedsYou / idle
Notification(permission_prompt) ───→ NeedsYou / needs_permission  (redundant with PermissionRequest, but harmless)
Notification(elicitation_dialog) ──→ NeedsYou / awaiting_input
Notification(idle_prompt) ─────────→ NeedsYou / idle
SubagentStart ─────────────────────→ Autonomous / delegating
SubagentStop ──────────────────────→ Autonomous / acting
TeammateIdle ──────────────────────→ (no state change — metadata: update sub-agent idle status)
TaskCompleted ─────────────────────→ NeedsYou / task_complete  (+task_subject)
PreCompact(manual) ────────────────→ Autonomous / thinking  "Compacting context..."
PreCompact(auto) ──────────────────→ Autonomous / thinking  "Auto-compacting..."
SessionEnd ────────────────────────→ NeedsYou / session_ended
```

**SessionStatus is derived, not computed:**
```rust
fn status_from_agent_state(state: &AgentState) -> SessionStatus {
    match state.state.as_str() {
        "session_ended" => SessionStatus::Done,
        _ => match state.group {
            AgentStateGroup::Autonomous => SessionStatus::Working,
            AgentStateGroup::NeedsYou => SessionStatus::Paused,
        }
    }
}
```

**PreToolUse gives instant activity labels with tool_input context:**
```rust
fn activity_from_pre_tool(tool_name: &str, tool_input: &Option<Value>) -> String {
    match tool_name {
        "Bash" => tool_input.as_ref()
            .and_then(|v| v.get("command")).and_then(|v| v.as_str())
            .map(|cmd| format!("Running: {}", &cmd[..cmd.len().min(60)]))
            .unwrap_or_else(|| "Running command".into()),
        "Read" => tool_input.as_ref()
            .and_then(|v| v.get("file_path")).and_then(|v| v.as_str())
            .map(|p| format!("Reading {}", short_path(p)))
            .unwrap_or_else(|| "Reading file".into()),
        "Edit" | "Write" => tool_input.as_ref()
            .and_then(|v| v.get("file_path")).and_then(|v| v.as_str())
            .map(|p| format!("Editing {}", short_path(p)))
            .unwrap_or_else(|| "Editing file".into()),
        "Grep" => "Searching code".into(),
        "Glob" => "Finding files".into(),
        "Task" => tool_input.as_ref()
            .and_then(|v| v.get("description")).and_then(|v| v.as_str())
            .map(|d| format!("Agent: {}", &d[..d.len().min(50)]))
            .unwrap_or_else(|| "Dispatching agent".into()),
        "WebFetch" => "Fetching web page".into(),
        "WebSearch" => "Searching web".into(),
        _ if tool_name.starts_with("mcp__") => format!("MCP: {}", tool_name.trim_start_matches("mcp__")),
        _ => format!("Using {}", tool_name),
    }
}
```

---

## Task 1: Register All 14 Claude Code Hooks

**Files:**
- Modify: `crates/server/src/live/hook_registrar.rs:19-28`

**Step 1: Replace HOOK_EVENTS with all 14 events**

```rust
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
```

SessionStart remains the only sync hook. All others are async.

**Step 2: Verify build**

Run: `cargo check -p claude-view-server`

No test needed — hook registration is an idempotent side-effect (writes to `~/.claude/settings.json`). Existing integration tests cover the format.

**Step 3: Commit**

```bash
git add crates/server/src/live/hook_registrar.rs
git commit -m "feat(hooks): register all 14 Claude Code hooks"
```

---

## Task 2: Simplify AgentState Types

**Files:**
- Modify: `crates/server/src/live/state.rs`

**Step 1: Write tests for the new `status_from_agent_state` helper**

Add to the existing `#[cfg(test)] mod tests` in `state.rs`:

```rust
#[test]
fn test_status_from_autonomous_acting() {
    let state = AgentState {
        group: AgentStateGroup::Autonomous,
        state: "acting".into(),
        label: "Working".into(),
        context: None,
    };
    assert_eq!(status_from_agent_state(&state), SessionStatus::Working);
}

#[test]
fn test_status_from_needs_you_idle() {
    let state = AgentState {
        group: AgentStateGroup::NeedsYou,
        state: "idle".into(),
        label: "Idle".into(),
        context: None,
    };
    assert_eq!(status_from_agent_state(&state), SessionStatus::Paused);
}

#[test]
fn test_status_from_session_ended() {
    let state = AgentState {
        group: AgentStateGroup::NeedsYou,
        state: "session_ended".into(),
        label: "Ended".into(),
        context: None,
    };
    assert_eq!(status_from_agent_state(&state), SessionStatus::Done);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p claude-view-server -- live::state::tests::test_status_from`
Expected: FAIL (function doesn't exist yet, struct fields wrong)

**Step 3: Simplify AgentState and add status_from_agent_state**

Replace the AgentState struct and remove SignalSource:

```rust
/// The universal agent state — driven by hooks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentState {
    /// Which UI group: NeedsYou or Autonomous
    pub group: AgentStateGroup,
    /// Sub-state within group (open string — new states added freely)
    pub state: String,
    /// Human-readable label for the UI
    pub label: String,
    /// Optional context (tool input, error details, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<serde_json::Value>,
}

// DELETE the SignalSource enum entirely
// DELETE the Delivered variant from AgentStateGroup (or keep with #[allow(dead_code)])

/// Derive SessionStatus from AgentState. No heuristics — purely structural.
pub fn status_from_agent_state(agent_state: &AgentState) -> SessionStatus {
    match agent_state.state.as_str() {
        "session_ended" => SessionStatus::Done,
        _ => match agent_state.group {
            AgentStateGroup::Autonomous => SessionStatus::Working,
            AgentStateGroup::NeedsYou | AgentStateGroup::Delivered => SessionStatus::Paused,
        }
    }
}
```

**Step 4: Fix all compilation errors from removed fields**

Every `AgentState { ... confidence, source, ... }` construction site must drop those fields. Use `cargo check` iteratively. Key files:
- `routes/hooks.rs` — `resolve_state_from_hook()` (all match arms)
- `manager.rs` — `SessionAccumulator::new()`, `derive_agent_state()`, `pause_classification_to_agent_state()`

**For this step, only fix the compilation errors. Do NOT refactor logic yet.** The goal is type-level clean compile. Logic refactoring happens in Tasks 3-5.

**Step 5: Run tests**

Run: `cargo test -p claude-view-server -- live::state`
Expected: PASS (including new status_from tests)

**Step 6: Delete `derive_status()` and its tests**

Remove the entire `derive_status()` function (lines 170-239) and all `test_status_*` tests that tested it. These are replaced by `status_from_agent_state()`.

Keep `derive_activity()` and its tests — they're still useful as a fallback for JSONL-discovered sessions.

**Step 7: Run tests**

Run: `cargo test -p claude-view-server`
Expected: PASS

**Step 8: Commit**

```bash
git add crates/server/src/live/state.rs crates/server/src/routes/hooks.rs crates/server/src/live/manager.rs
git commit -m "refactor(state): simplify AgentState, remove confidence/source/derive_status"
```

---

## Task 3: Rewrite Hook Handler as Sole State Authority

**Files:**
- Modify: `crates/server/src/routes/hooks.rs`

This is the core of the refactor. The hook handler becomes the **only** place that mutates `session.agent_state`.

**Step 1: Write tests for new hook state mappings**

Add to `hooks.rs` tests:

```rust
// --- PreToolUse ---

#[test]
fn test_pre_tool_use_bash_returns_acting() {
    let mut payload = minimal_payload("PreToolUse");
    payload.tool_name = Some("Bash".into());
    payload.tool_input = Some(serde_json::json!({"command": "git status"}));
    let state = resolve_state_from_hook(&payload);
    assert_eq!(state.state, "acting");
    assert!(matches!(state.group, AgentStateGroup::Autonomous));
}

#[test]
fn test_pre_tool_use_ask_user_returns_awaiting_input() {
    let mut payload = minimal_payload("PreToolUse");
    payload.tool_name = Some("AskUserQuestion".into());
    let state = resolve_state_from_hook(&payload);
    assert_eq!(state.state, "awaiting_input");
    assert!(matches!(state.group, AgentStateGroup::NeedsYou));
}

#[test]
fn test_pre_tool_use_exit_plan_returns_awaiting_approval() {
    let mut payload = minimal_payload("PreToolUse");
    payload.tool_name = Some("ExitPlanMode".into());
    let state = resolve_state_from_hook(&payload);
    assert_eq!(state.state, "awaiting_approval");
    assert!(matches!(state.group, AgentStateGroup::NeedsYou));
}

#[test]
fn test_pre_tool_use_read_has_activity_label() {
    let mut payload = minimal_payload("PreToolUse");
    payload.tool_name = Some("Read".into());
    payload.tool_input = Some(serde_json::json!({"file_path": "/src/lib.rs"}));
    let state = resolve_state_from_hook(&payload);
    assert!(state.label.contains("lib.rs"));
}

// --- PostToolUse ---

#[test]
fn test_post_tool_use_returns_thinking() {
    let mut payload = minimal_payload("PostToolUse");
    payload.tool_name = Some("Bash".into());
    let state = resolve_state_from_hook(&payload);
    assert_eq!(state.state, "thinking");
    assert!(matches!(state.group, AgentStateGroup::Autonomous));
}

// --- PermissionRequest ---

#[test]
fn test_permission_request_returns_needs_permission() {
    let mut payload = minimal_payload("PermissionRequest");
    payload.tool_name = Some("Bash".into());
    let state = resolve_state_from_hook(&payload);
    assert_eq!(state.state, "needs_permission");
    assert!(matches!(state.group, AgentStateGroup::NeedsYou));
    assert!(state.label.contains("Bash"));
}

// --- TaskCompleted ---

#[test]
fn test_task_completed_returns_task_complete() {
    let mut payload = minimal_payload("TaskCompleted");
    payload.task_subject = Some("Fix login bug".into());
    let state = resolve_state_from_hook(&payload);
    assert_eq!(state.state, "task_complete");
    assert!(matches!(state.group, AgentStateGroup::NeedsYou));
    assert!(state.label.contains("Fix login bug"));
}

// --- PreCompact ---

#[test]
fn test_pre_compact_returns_thinking() {
    let mut payload = minimal_payload("PreCompact");
    payload.source = Some("auto".into());
    let state = resolve_state_from_hook(&payload);
    assert_eq!(state.state, "thinking");
    assert!(matches!(state.group, AgentStateGroup::Autonomous));
    assert!(state.label.contains("compact") || state.label.contains("Compact"));
}

// --- TeammateIdle ---

#[test]
fn test_teammate_idle_returns_delegating() {
    let mut payload = minimal_payload("TeammateIdle");
    payload.teammate_name = Some("researcher".into());
    let state = resolve_state_from_hook(&payload);
    // TeammateIdle doesn't change the parent session's core state —
    // the session is still delegating (running subagents)
    assert_eq!(state.state, "delegating");
    assert!(matches!(state.group, AgentStateGroup::Autonomous));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p claude-view-server -- routes::hooks::tests`
Expected: Some FAIL (PreToolUse tests, since current PostToolUse mapping handles AskUserQuestion differently)

**Step 3: Rewrite resolve_state_from_hook with PreToolUse**

Replace the function. Key changes:
- `"PreToolUse"` arm with tool-specific state mapping (AskUserQuestion → awaiting_input, ExitPlanMode → awaiting_approval, others → acting)
- `"PostToolUse"` arm simplified to always return thinking (Claude is between tools)
- Activity labels from `tool_input` for PreToolUse
- Remove all `confidence` and `source` fields

```rust
fn resolve_state_from_hook(payload: &HookPayload) -> AgentState {
    match payload.hook_event_name.as_str() {
        "SessionStart" => {
            if payload.source.as_deref() == Some("compact") {
                AgentState {
                    group: AgentStateGroup::Autonomous,
                    state: "thinking".into(),
                    label: "Compacting context...".into(),
                    context: None,
                }
            } else {
                AgentState {
                    group: AgentStateGroup::NeedsYou,
                    state: "idle".into(),
                    label: "Waiting for first prompt".into(),
                    context: None,
                }
            }
        }
        "UserPromptSubmit" => AgentState {
            group: AgentStateGroup::Autonomous,
            state: "thinking".into(),
            label: "Processing prompt...".into(),
            context: None,
        },
        "PreToolUse" => {
            let tool_name = payload.tool_name.as_deref().unwrap_or("unknown");
            match tool_name {
                "AskUserQuestion" => AgentState {
                    group: AgentStateGroup::NeedsYou,
                    state: "awaiting_input".into(),
                    label: "Asked you a question".into(),
                    context: payload.tool_input.clone(),
                },
                "ExitPlanMode" => AgentState {
                    group: AgentStateGroup::NeedsYou,
                    state: "awaiting_approval".into(),
                    label: "Plan ready for review".into(),
                    context: None,
                },
                "EnterPlanMode" => AgentState {
                    group: AgentStateGroup::Autonomous,
                    state: "thinking".into(),
                    label: "Entering plan mode...".into(),
                    context: None,
                },
                _ => AgentState {
                    group: AgentStateGroup::Autonomous,
                    state: "acting".into(),
                    label: activity_from_pre_tool(tool_name, &payload.tool_input),
                    context: None,
                },
            }
        }
        "PostToolUse" => AgentState {
            group: AgentStateGroup::Autonomous,
            state: "thinking".into(),
            label: "Thinking...".into(),
            context: None,
        },
        "PostToolUseFailure" => {
            if payload.is_interrupt.unwrap_or(false) {
                AgentState {
                    group: AgentStateGroup::NeedsYou,
                    state: "interrupted".into(),
                    label: format!("You interrupted {}", payload.tool_name.as_deref().unwrap_or("tool")),
                    context: None,
                }
            } else {
                AgentState {
                    group: AgentStateGroup::NeedsYou,
                    state: "error".into(),
                    label: format!("Failed: {}", payload.tool_name.as_deref().unwrap_or("tool")),
                    context: payload.error.as_ref().map(|e| serde_json::json!({"error": e})),
                }
            }
        }
        "Stop" => AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "idle".into(),
            label: "Waiting for your next prompt".into(),
            context: None,
        },
        "Notification" => match payload.notification_type.as_deref() {
            Some("permission_prompt") => AgentState {
                group: AgentStateGroup::NeedsYou,
                state: "needs_permission".into(),
                label: "Needs permission".into(),
                context: None,
            },
            Some("idle_prompt") => AgentState {
                group: AgentStateGroup::NeedsYou,
                state: "idle".into(),
                label: "Session idle".into(),
                context: None,
            },
            Some("elicitation_dialog") => AgentState {
                group: AgentStateGroup::NeedsYou,
                state: "awaiting_input".into(),
                label: payload.message.as_deref()
                    .map(|m| m.chars().take(100).collect::<String>())
                    .unwrap_or_else(|| "Awaiting input".into()),
                context: None,
            },
            _ => AgentState {
                group: AgentStateGroup::NeedsYou,
                state: "awaiting_input".into(),
                label: "Notification".into(),
                context: None,
            },
        },
        "SubagentStart" => AgentState {
            group: AgentStateGroup::Autonomous,
            state: "delegating".into(),
            label: format!("Running {} agent", payload.agent_type.as_deref().unwrap_or("sub")),
            context: None,
        },
        "SubagentStop" => AgentState {
            group: AgentStateGroup::Autonomous,
            state: "acting".into(),
            label: format!("{} agent finished", payload.agent_type.as_deref().unwrap_or("Sub")),
            context: None,
        },
        "SessionEnd" => AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "session_ended".into(),
            label: "Session closed".into(),
            context: None,
        },
        "PermissionRequest" => {
            let tool = payload.tool_name.as_deref().unwrap_or("tool");
            AgentState {
                group: AgentStateGroup::NeedsYou,
                state: "needs_permission".into(),
                label: format!("Needs permission: {}", tool),
                context: payload.tool_input.clone(), // includes permission_suggestions
            }
        }
        "TaskCompleted" => AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "task_complete".into(),
            label: payload.task_subject.clone().unwrap_or_else(|| "Task completed".into()),
            context: None,
        },
        "TeammateIdle" => AgentState {
            group: AgentStateGroup::Autonomous,
            state: "delegating".into(),
            label: format!("Teammate {} idle", payload.teammate_name.as_deref().unwrap_or("unknown")),
            context: None,
        },
        "PreCompact" => {
            let trigger = payload.source.as_deref().unwrap_or("auto"); // "trigger" field mapped to source
            AgentState {
                group: AgentStateGroup::Autonomous,
                state: "thinking".into(),
                label: if trigger == "manual" {
                    "Compacting context...".into()
                } else {
                    "Auto-compacting context...".into()
                },
                context: None,
            }
        }
        _ => AgentState {
            group: AgentStateGroup::Autonomous,
            state: "acting".into(),
            label: format!("Event: {}", payload.hook_event_name),
            context: None,
        },
    }
}
```

**Step 4: Add activity_from_pre_tool helper**

```rust
/// Derive a rich activity label from PreToolUse hook data.
fn activity_from_pre_tool(tool_name: &str, tool_input: &Option<serde_json::Value>) -> String {
    let input = tool_input.as_ref();
    match tool_name {
        "Bash" => input
            .and_then(|v| v.get("command")).and_then(|v| v.as_str())
            .map(|cmd| {
                let truncated: String = cmd.chars().take(60).collect();
                format!("Running: {}", truncated)
            })
            .unwrap_or_else(|| "Running command".into()),
        "Read" => input
            .and_then(|v| v.get("file_path")).and_then(|v| v.as_str())
            .map(|p| format!("Reading {}", short_path(p)))
            .unwrap_or_else(|| "Reading file".into()),
        "Edit" | "Write" => input
            .and_then(|v| v.get("file_path")).and_then(|v| v.as_str())
            .map(|p| format!("Editing {}", short_path(p)))
            .unwrap_or_else(|| "Editing file".into()),
        "Grep" => input
            .and_then(|v| v.get("pattern")).and_then(|v| v.as_str())
            .map(|pat| {
                let truncated: String = pat.chars().take(40).collect();
                format!("Searching: {}", truncated)
            })
            .unwrap_or_else(|| "Searching code".into()),
        "Glob" => "Finding files".into(),
        "Task" => input
            .and_then(|v| v.get("description")).and_then(|v| v.as_str())
            .map(|d| {
                let truncated: String = d.chars().take(50).collect();
                format!("Agent: {}", truncated)
            })
            .unwrap_or_else(|| "Dispatching agent".into()),
        "WebFetch" => "Fetching web page".into(),
        "WebSearch" => input
            .and_then(|v| v.get("query")).and_then(|v| v.as_str())
            .map(|q| {
                let truncated: String = q.chars().take(40).collect();
                format!("Searching: {}", truncated)
            })
            .unwrap_or_else(|| "Searching web".into()),
        _ if tool_name.starts_with("mcp__") => {
            let short = tool_name.trim_start_matches("mcp__");
            format!("MCP: {}", short)
        }
        _ => format!("Using {}", tool_name),
    }
}

/// Extract the last path component for display.
fn short_path(path: &str) -> &str {
    std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
}
```

**Step 5: Update handle_hook to remove StateResolver calls**

Remove these lines from `handle_hook`:
- `state.state_resolver.update_from_hook(...)` (line 68)
- `state.state_resolver.clear_hook_state(...)` in UserPromptSubmit (line 136)
- `state_clone.state_resolver.clear_hook_state(...)` in SessionEnd cleanup (line 175)

The hook handler now directly mutates `session.agent_state` and `session.status` (derived via `status_from_agent_state`). It already does this — just remove the StateResolver calls.

Add the new hook events to the match in `handle_hook`:

```rust
"PreToolUse" | "PostToolUse" | "PermissionRequest" | "PreCompact" => {
    let mut sessions = state.live_sessions.write().await;
    if let Some(session) = sessions.get_mut(&payload.session_id) {
        session.agent_state = agent_state.clone();
        session.status = status_from_agent_state(&agent_state);
        // PreToolUse: update current_activity with rich label from tool_input
        if payload.hook_event_name == "PreToolUse" {
            session.current_activity = agent_state.label.clone();
        }
        session.last_activity_at = now;
        let _ = state.live_tx.send(SessionEvent::SessionUpdated {
            session: session.clone(),
        });
    }
}
"TeammateIdle" => {
    // Informational: update sub-agent status but don't change parent session state
    let mut sessions = state.live_sessions.write().await;
    if let Some(session) = sessions.get_mut(&payload.session_id) {
        if let Some(teammate) = &payload.teammate_name {
            // Mark matching sub-agent as idle in the sub_agents list
            for agent in &mut session.sub_agents {
                if agent.name.as_deref() == Some(teammate) {
                    agent.status = claude_view_core::subagent::SubAgentStatus::Completed;
                }
            }
        }
        // Keep parent state as delegating (still in agent team mode)
        session.agent_state = agent_state.clone();
        let _ = state.live_tx.send(SessionEvent::SessionUpdated {
            session: session.clone(),
        });
    }
}
"TaskCompleted" => {
    let mut sessions = state.live_sessions.write().await;
    if let Some(session) = sessions.get_mut(&payload.session_id) {
        session.agent_state = agent_state.clone();
        session.status = status_from_agent_state(&agent_state);
        // Update progress_items: mark the completed task
        if let Some(task_id) = &payload.task_id {
            for item in &mut session.progress_items {
                if item.id.as_deref() == Some(task_id) {
                    item.status = claude_view_core::progress::ProgressStatus::Completed;
                }
            }
        }
        let _ = state.live_tx.send(SessionEvent::SessionUpdated {
            session: session.clone(),
        });
    }
}
```

Also update the existing match arms to derive status from agent_state:
- SessionStart: `status: status_from_agent_state(&agent_state)` instead of manual is_compact check
- UserPromptSubmit: `session.status = status_from_agent_state(&agent_state)` instead of `SessionStatus::Working`
- Generic arm: add `session.status = status_from_agent_state(&agent_state)`

**Step 6: Run tests**

Run: `cargo test -p claude-view-server -- routes::hooks`
Expected: PASS

**Step 7: Commit**

```bash
git add crates/server/src/routes/hooks.rs
git commit -m "feat(hooks): PreToolUse/PostToolUse handling, remove StateResolver from hook handler"
```

---

## Task 4: Strip State Derivation from JSONL Processing

**Files:**
- Modify: `crates/server/src/live/manager.rs`

This is the largest single task. The JSONL processing pipeline (`process_jsonl_update`) must stop deriving agent state and only extract metadata.

**Step 1: Remove state-related imports and fields from SessionAccumulator**

Remove from `SessionAccumulator`:
- `agent_state` field (hooks own state now)
- `recent_messages` field (was for classifier context)
- `last_status` field (was for transition detection)

Remove imports:
- `super::classifier::*`
- `super::state_resolver::StateResolver`
- `SignalSource` from state imports

Keep in `SessionAccumulator`:
- `offset`, `tokens`, `context_window_tokens`, `model`
- `user_turn_count`, `first_user_message`, `last_user_message`
- `git_branch`, `started_at`, `last_line`, `completed_at`
- `current_turn_started_at`, `last_turn_task_seconds`
- `sub_agents`, `todo_items`, `task_items`

**Step 2: Remove state derivation from process_jsonl_update**

In `process_jsonl_update()`, remove:
- The `evidence_time` capture
- The call to `derive_agent_state()`
- The call to `clear_hook_state_if_before()`
- The call to `state_resolver.update_from_jsonl()`
- The call to `state_resolver.resolve()`
- The `handle_transitions()` call for agent state transitions

Keep in `process_jsonl_update()`:
- Token accumulation (input, output, cache_read, cache_creation)
- Context window fill tracking
- Model ID tracking
- User message tracking (first = title, latest = last_user_message)
- Git branch detection
- Sub-agent spawn/completion/progress tracking
- Todo/Task progress tracking
- Started_at, last_activity_at timestamps
- Cost calculation
- Current turn timing (current_turn_started_at tracking)

**After removing state derivation, the JSONL handler updates the session map like this:**

```rust
// In process_jsonl_update, after accumulating metadata:
let mut sessions = self.sessions.write().await;
if let Some(session) = sessions.get_mut(&session_id) {
    // ONLY update metadata fields, NEVER touch agent_state or status
    session.tokens = acc.tokens.clone();
    session.context_window_tokens = acc.context_window_tokens;
    session.model = acc.model.clone();
    session.cost = calculate_live_cost(&acc.tokens, &self.pricing, acc.model.as_deref());
    session.cache_status = derive_cache_status(&acc.tokens);
    session.git_branch = acc.git_branch.clone();
    session.sub_agents = acc.sub_agents.clone();
    session.progress_items = [acc.todo_items.clone(), acc.task_items.clone()].concat();
    session.last_activity_at = last_activity_at;
    // Title/message: only update if hook hasn't set them already
    if session.title.is_empty() && !acc.first_user_message.is_empty() {
        session.title = acc.first_user_message.clone();
    }
    if !acc.last_user_message.is_empty() {
        session.last_user_message = acc.last_user_message.clone();
    }
    // Broadcast the metadata update
    let _ = self.tx.send(SessionEvent::SessionUpdated { session: session.clone() });
} else {
    // Session not yet created by hook (server restart recovery).
    // Create with fallback state — next hook event will correct it.
    let fallback_state = AgentState {
        group: AgentStateGroup::Autonomous,
        state: "unknown".into(),
        label: "Connecting...".into(),
        context: None,
    };
    let session = LiveSession {
        id: session_id.clone(),
        // ... populate from accumulator metadata ...
        status: SessionStatus::Working, // assume active until hook says otherwise
        agent_state: fallback_state,
        // ... rest of fields from accumulator ...
    };
    sessions.insert(session_id.clone(), session.clone());
    let _ = self.tx.send(SessionEvent::SessionDiscovered { session });
}
```

**Step 3: Remove classifier and state_resolver from LiveSessionManager**

Remove from `LiveSessionManager` struct:
- `classifier: Arc<SessionStateClassifier>` field
- `state_resolver: StateResolver` field

Update `LiveSessionManager::start()` signature:
- Remove `state_resolver: StateResolver` parameter
- Remove `classifier` creation

```rust
// BEFORE
pub fn start(pricing: HashMap<String, ModelPricing>, state_resolver: StateResolver)
    -> (Arc<Self>, LiveSessionMap, broadcast::Sender<SessionEvent>)

// AFTER
pub fn start(pricing: HashMap<String, ModelPricing>)
    -> (Arc<Self>, LiveSessionMap, broadcast::Sender<SessionEvent>)
```

**Step 4: Simplify handle_transitions**

Keep only the task-time tracking parts:
- current_turn_started_at tracking (set by hooks via UserPromptSubmit, read here for elapsed time)
- last_turn_task_seconds computation
- Sub-agent orphan cleanup on session removal

Remove:
- Status transition detection (last_status tracking)
- Agent state changes on transitions

**Step 5: Verify build**

Run: `cargo check -p claude-view-server`
Fix any compilation errors iteratively.

**Step 6: Run tests**

Run: `cargo test -p claude-view-server`
Expected: PASS (some old tests may need updating/removal if they tested JSONL→state derivation)

**Step 7: Commit**

```bash
git add crates/server/src/live/manager.rs
git commit -m "refactor(manager): strip state derivation from JSONL processing, metadata-only"
```

---

## Task 5: Simplify Process Detector to Crash-Only

**Files:**
- Modify: `crates/server/src/live/manager.rs` (spawn_process_detector section)

**Step 1: Rewrite spawn_process_detector**

The process detector no longer derives state. It does exactly two things:
1. Update `session.pid` for each session (so the UI can show process status)
2. Mark dead sessions: if no process found AND no hook event in 5 minutes → session_ended

```rust
fn spawn_process_detector(
    sessions: LiveSessionMap,
    tx: broadcast::Sender<SessionEvent>,
    processes: Arc<RwLock<HashMap<PathBuf, ClaudeProcess>>>,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5)); // slowed from 2s
        loop {
            interval.tick().await;

            // Detect running Claude processes
            let detected = detect_claude_processes().await;
            *processes.write().await = detected.clone();

            let now_secs = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            let mut sessions = sessions.write().await;
            let mut to_remove = Vec::new();

            for (id, session) in sessions.iter_mut() {
                // Update PID from process table
                let process = detected.values().find(|p| {
                    // Match by session file path or working directory
                    session.project_path == p.cwd.to_string_lossy()
                });
                session.pid = process.map(|p| p.pid);

                // Crash detection: no process + stale for 5 minutes → dead
                let stale_seconds = (now_secs - session.last_activity_at).max(0) as u64;
                if session.pid.is_none()
                    && stale_seconds > 300
                    && session.status != SessionStatus::Done
                {
                    session.agent_state = AgentState {
                        group: AgentStateGroup::NeedsYou,
                        state: "session_ended".into(),
                        label: "Session ended (no process)".into(),
                        context: None,
                    };
                    session.status = SessionStatus::Done;
                    let _ = tx.send(SessionEvent::SessionUpdated {
                        session: session.clone(),
                    });
                    to_remove.push(id.clone());
                }
            }

            // Schedule removal for dead sessions
            for id in to_remove {
                sessions.remove(&id);
                let _ = tx.send(SessionEvent::SessionCompleted { session_id: id });
            }
        }
    });
}
```

This replaces ~500 lines with ~40 lines. No 3-phase lock pattern needed.

**Step 2: Verify build**

Run: `cargo check -p claude-view-server`

**Step 3: Run tests**

Run: `cargo test -p claude-view-server`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/server/src/live/manager.rs
git commit -m "refactor(detector): simplify process detector to crash-only, remove state derivation"
```

---

## Task 6: Delete StateResolver + Classifier, Clean Wiring

**Files:**
- Delete: `crates/server/src/live/state_resolver.rs`
- Delete: `crates/server/src/live/classifier.rs`
- Modify: `crates/server/src/live/mod.rs`
- Modify: `crates/server/src/state.rs`
- Modify: `crates/server/src/lib.rs`

**Step 1: Delete the files**

```bash
rm crates/server/src/live/state_resolver.rs
rm crates/server/src/live/classifier.rs
```

**Step 2: Remove from mod.rs**

```rust
// Remove these lines:
pub mod classifier;
pub mod state_resolver;
```

**Step 3: Remove StateResolver from AppState**

In `crates/server/src/state.rs`, remove:
- `use crate::live::state_resolver::StateResolver;` import
- `pub state_resolver: StateResolver` field
- All `StateResolver::new()` calls in constructors (`new`, `new_with_indexing`, `new_with_indexing_and_registry`)

**Step 4: Remove StateResolver from create_app_full**

In `crates/server/src/lib.rs`:
- Remove `use live::state_resolver::StateResolver;`
- Remove `let resolver = StateResolver::new();`
- Change `LiveSessionManager::start(pricing.clone(), resolver.clone())` to `LiveSessionManager::start(pricing.clone())`
- Remove `state_resolver: resolver` from AppState construction

**Step 5: Remove StateResolver from test helpers**

In `crates/server/src/lib.rs` (`create_app_with_git_sync`):
- Remove `state_resolver: StateResolver::new()`

**Step 6: Verify build**

Run: `cargo check -p claude-view-server`
Fix any remaining references.

**Step 7: Run all tests**

Run: `cargo test -p claude-view-server`
Expected: PASS

**Step 8: Commit**

```bash
git add -A
git commit -m "refactor: delete StateResolver + Classifier (757 lines removed)"
```

---

## Task 7: Update Frontend Types + Verify UI

**Files:**
- Modify: `src/components/live/types.ts`
- Modify: `src/components/live/use-live-sessions.ts` (if needed)
- Modify: `src/components/live/SessionCard.tsx` (if needed)

**Step 1: Simplify AgentState type**

In `types.ts`, remove `confidence`, `source`, and `SignalSource`:

```typescript
// BEFORE
type SignalSource = 'hook' | 'jsonl' | 'fallback'
interface AgentState {
  group: AgentStateGroup
  state: string
  label: string
  confidence: number
  source: SignalSource
  context?: Record<string, unknown>
}

// AFTER
interface AgentState {
  group: AgentStateGroup
  state: string
  label: string
  context?: Record<string, unknown>
}
```

**Step 2: Verify no frontend code uses confidence or source**

Run: `grep -rn 'confidence\|\.source\|SignalSource' src/components/live/`

If any component uses these fields (e.g., showing confidence badges or "hook"/"jsonl" source indicators), remove those UI elements.

**Step 3: Verify build**

Run: `bun run build` (or `npx tsc --noEmit` for type-checking only)
Expected: PASS

**Step 4: Manual verification**

Start the dev server (`bun run dev`) with a Claude Code session running and verify:
1. Session cards appear in correct Kanban columns
2. State transitions are instant (hook-driven, no 2-30s delay)
3. PreToolUse shows rich activity labels ("Running: git status", "Reading lib.rs")
4. Cost, tokens, model, context window still update from JSONL
5. Todo/task progress still shows

**Step 5: Commit**

```bash
git add src/components/live/types.ts
git commit -m "refactor(ui): simplify AgentState types, remove confidence/source"
```

---

## Task 8: Update Architecture Documentation

**Files:**
- Modify: `docs/architecture/live-monitor.md`

Update the architecture doc to reflect the new hook-primary design:
- Section 2 (Signal Sources): Hooks = state, JSONL = metadata only
- Section 3 (Hook System): Add PreToolUse + PostToolUse
- Section 4 (LiveSessionManager): Remove StateResolver references, simplify process detector description
- Section 5 (State Model): Remove StateResolver section, simplify to FSM
- Section 11 (Key Invariants): Rewrite for hook-primary

**Step 1: Update doc**

This is a documentation-only task. Rewrite to match the new reality.

**Step 2: Commit**

```bash
git add docs/architecture/live-monitor.md
git commit -m "docs: update live-monitor architecture for hook-primary design"
```

---

## HookPayload Field Mapping for New Events

The `HookPayload` struct in `hooks.rs` already has fields for most new hooks. Field mapping for the 4 new events:

| Hook Event | Payload Field | Maps to HookPayload field |
|-----------|--------------|--------------------------|
| **PreToolUse** | `tool_name` | `tool_name` (already exists) |
| | `tool_input` | `tool_input` (already exists) |
| | `tool_use_id` | `tool_use_id` (already exists) |
| **PostToolUse** | `tool_name`, `tool_input`, `tool_response`, `tool_use_id` | All already exist |
| **PermissionRequest** | `tool_name`, `tool_input`, `permission_suggestions` | Need to add `permission_suggestions: Option<Value>` |
| **TeammateIdle** | `teammate_name`, `team_name` | Already exist |
| **TaskCompleted** | `task_id`, `task_subject`, `task_description`, `teammate_name`, `team_name` | Already exist |
| **PreCompact** | `trigger`, `custom_instructions` | Map `trigger` → add `trigger: Option<String>` field, or reuse `source` |

Add these new fields to `HookPayload`:
```rust
pub permission_suggestions: Option<serde_json::Value>, // PermissionRequest
pub trigger: Option<String>,                           // PreCompact: "manual" | "auto"
pub custom_instructions: Option<String>,               // PreCompact: user's /compact instructions
```

---

## Summary

| Task | Description | Key Files | Est. Size |
|------|-------------|-----------|-----------|
| 1 | Register all 14 Claude Code hooks | `hook_registrar.rs` | Small |
| 2 | Simplify AgentState types | `state.rs`, fix callers | Medium |
| 3 | Rewrite hook handler as sole state authority (all 14 hooks) | `hooks.rs` | Medium |
| 4 | Strip state derivation from JSONL | `manager.rs` | Large |
| 5 | Simplify process detector to crash-only | `manager.rs` | Medium |
| 6 | Delete StateResolver + Classifier, clean wiring | `state_resolver.rs`, `classifier.rs`, `state.rs`, `lib.rs`, `mod.rs` | Medium |
| 7 | Update frontend types | `types.ts` | Small |
| 8 | Update architecture docs | `live-monitor.md` | Small |

**Execution order matters:** Tasks 1-3 are additive/safe. Task 4-5 are the big refactor. Task 6 is cleanup (can only happen after 4-5). Task 7-8 are polish.
