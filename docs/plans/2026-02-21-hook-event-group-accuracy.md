# Hook Event Group Accuracy Fix

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make hook event log `group` field reflect the session's actual applied state, not the theoretical resolved state — eliminating visual false positives for `TaskCompleted`, `SubagentStop`, and `TeammateIdle` events.

**Architecture:** Currently `resolve_state_from_hook()` runs first and produces a single `AgentState`, which is used both for session state updates (in some match arms) and for building the hook event log entry. For metadata-only match arms (`TaskCompleted`, `SubagentStop`, `TeammateIdle`), the resolved state is never applied to `session.agent_state`, but the hook event still records the resolved group — creating a discrepancy. The fix moves hook event construction to AFTER the match arms, reading the session's actual `agent_state.group` for the hook event's `group` field while preserving the resolved `label` for display.

**Tech Stack:** Rust (Axum), no new dependencies.

**Root cause evidence:** Session `927cd7c2` shows 6 `TaskCompleted` events logged as `needs_you` in the hook event log, while the session's actual `agent_state` remained `autonomous` throughout (the `TaskCompleted` match arm at hooks.rs:398-419 never touches `session.agent_state`). This causes amber `needs_you` highlighting on events where the agent was still working autonomously.

---

### Task 1: Write failing integration test for TaskCompleted hook event group

**Files:**
- Modify: `crates/server/src/routes/hooks.rs` (add test at bottom of `mod tests`)

**Step 1: Write the failing test**

Add an integration test that sends a `TaskCompleted` hook to a session already in `autonomous` state and verifies the recorded hook event has `group = "autonomous"`, not `"needs_you"`.

```rust
#[tokio::test]
async fn test_task_completed_hook_event_records_actual_session_group() {
    // Setup: create AppState with a session in autonomous state
    let db = claude_view_db::Database::new_in_memory().await.unwrap();
    let state = crate::state::AppState::new(db);

    // Pre-populate a session in autonomous state (simulating a working session)
    {
        let mut sessions = state.live_sessions.write().await;
        sessions.insert(
            "test-session".to_string(),
            crate::live::state::LiveSession {
                id: "test-session".to_string(),
                project: String::new(),
                project_display_name: "test".to_string(),
                project_path: "/tmp/test".to_string(),
                file_path: "/tmp/test.jsonl".to_string(),
                status: crate::live::state::SessionStatus::Working,
                agent_state: AgentState {
                    group: AgentStateGroup::Autonomous,
                    state: "acting".into(),
                    label: "Using TaskUpdate".into(),
                    context: None,
                },
                git_branch: None,
                pid: None,
                title: "Test session".into(),
                last_user_message: String::new(),
                current_activity: "Using TaskUpdate".into(),
                turn_count: 5,
                started_at: Some(1000),
                last_activity_at: 1000,
                model: None,
                tokens: claude_view_core::pricing::TokenUsage::default(),
                context_window_tokens: 0,
                cost: claude_view_core::pricing::CostBreakdown::default(),
                cache_status: claude_view_core::pricing::CacheStatus::Unknown,
                current_turn_started_at: None,
                last_turn_task_seconds: None,
                sub_agents: Vec::new(),
                progress_items: Vec::new(),
                last_cache_hit_at: None,
                hook_events: Vec::new(),
            },
        );
    }

    // Send a TaskCompleted hook
    let app = crate::api_routes(state.clone());
    let body = serde_json::json!({
        "session_id": "test-session",
        "hook_event_name": "TaskCompleted",
        "task_id": "task-1",
        "task_subject": "Fix login bug"
    });
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/live/hook")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);

    // Verify: hook event should record "autonomous" (actual session group),
    // NOT "needs_you" (what resolve_state_from_hook returns for TaskCompleted)
    let sessions = state.live_sessions.read().await;
    let session = sessions.get("test-session").unwrap();
    assert_eq!(session.hook_events.len(), 1);
    let event = &session.hook_events[0];
    assert_eq!(event.event_name, "TaskCompleted");
    assert_eq!(event.label, "Fix login bug"); // label still from resolved state
    assert_eq!(
        event.group, "autonomous",
        "TaskCompleted hook event should record the session's actual group (autonomous), \
         not the resolved group (needs_you)"
    );
    // Also verify session.agent_state was NOT changed
    assert!(matches!(
        session.agent_state.group,
        AgentStateGroup::Autonomous
    ));
}
```

**Step 2: Run the test to verify it fails**

Run: `cargo test -p claude-view-server test_task_completed_hook_event_records_actual_session_group -- --nocapture`

Expected: FAIL — `assertion ... left: "needs_you", right: "autonomous"` because the hook event currently uses the resolved group.

**Step 3: Commit the failing test**

```
git add crates/server/src/routes/hooks.rs
git commit -m "test: failing test for TaskCompleted hook event recording wrong group"
```

---

### Task 2: Write failing tests for SubagentStop and TeammateIdle

**Files:**
- Modify: `crates/server/src/routes/hooks.rs` (add tests)

**Step 1: Write two more failing tests**

These follow the same pattern — session is autonomous, metadata-only hook arrives, hook event should still say autonomous.

```rust
#[tokio::test]
async fn test_subagent_stop_hook_event_records_actual_session_group() {
    let db = claude_view_db::Database::new_in_memory().await.unwrap();
    let state = crate::state::AppState::new(db);

    // Pre-populate session in autonomous/delegating state
    {
        let mut sessions = state.live_sessions.write().await;
        sessions.insert(
            "test-session".to_string(),
            make_autonomous_session("test-session"),
        );
    }

    let app = crate::api_routes(state.clone());
    let body = serde_json::json!({
        "session_id": "test-session",
        "hook_event_name": "SubagentStop",
        "agent_type": "code-explorer",
        "agent_id": "agent-1"
    });
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/live/hook")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);

    let sessions = state.live_sessions.read().await;
    let session = sessions.get("test-session").unwrap();
    assert_eq!(session.hook_events.len(), 1);
    assert_eq!(
        session.hook_events[0].group, "autonomous",
        "SubagentStop hook event should record session's actual group"
    );
}

#[tokio::test]
async fn test_teammate_idle_hook_event_records_actual_session_group() {
    let db = claude_view_db::Database::new_in_memory().await.unwrap();
    let state = crate::state::AppState::new(db);

    {
        let mut sessions = state.live_sessions.write().await;
        sessions.insert(
            "test-session".to_string(),
            make_autonomous_session("test-session"),
        );
    }

    let app = crate::api_routes(state.clone());
    let body = serde_json::json!({
        "session_id": "test-session",
        "hook_event_name": "TeammateIdle",
        "teammate_name": "researcher"
    });
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/live/hook")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);

    let sessions = state.live_sessions.read().await;
    let session = sessions.get("test-session").unwrap();
    assert_eq!(session.hook_events.len(), 1);
    assert_eq!(
        session.hook_events[0].group, "autonomous",
        "TeammateIdle hook event should record session's actual group"
    );
}
```

Also add the shared test helper `make_autonomous_session` alongside the existing `minimal_payload` helper:

```rust
/// Create a LiveSession in autonomous/acting state for integration tests.
fn make_autonomous_session(id: &str) -> crate::live::state::LiveSession {
    crate::live::state::LiveSession {
        id: id.to_string(),
        project: String::new(),
        project_display_name: "test".to_string(),
        project_path: "/tmp/test".to_string(),
        file_path: "/tmp/test.jsonl".to_string(),
        status: crate::live::state::SessionStatus::Working,
        agent_state: AgentState {
            group: AgentStateGroup::Autonomous,
            state: "acting".into(),
            label: "Working".into(),
            context: None,
        },
        git_branch: None,
        pid: None,
        title: "Test session".into(),
        last_user_message: String::new(),
        current_activity: "Working".into(),
        turn_count: 5,
        started_at: Some(1000),
        last_activity_at: 1000,
        model: None,
        tokens: claude_view_core::pricing::TokenUsage::default(),
        context_window_tokens: 0,
        cost: claude_view_core::pricing::CostBreakdown::default(),
        cache_status: claude_view_core::pricing::CacheStatus::Unknown,
        current_turn_started_at: None,
        last_turn_task_seconds: None,
        sub_agents: Vec::new(),
        progress_items: Vec::new(),
        last_cache_hit_at: None,
        hook_events: Vec::new(),
    }
}
```

**Step 2: Run to verify they fail**

Run: `cargo test -p claude-view-server test_subagent_stop_hook_event test_teammate_idle_hook_event -- --nocapture`

Expected: Both FAIL with group assertion mismatch.

**Step 3: Commit**

```
git add crates/server/src/routes/hooks.rs
git commit -m "test: failing tests for SubagentStop/TeammateIdle hook event groups"
```

---

### Task 3: Write positive test — normal events record correct group

**Files:**
- Modify: `crates/server/src/routes/hooks.rs` (add test)

**Step 1: Write a test confirming state-changing events still record the resolved group**

This guards against regressions — make sure `PreToolUse/AskUserQuestion` (which DOES transition to `needs_you`) still records `needs_you` in the hook event.

```rust
#[tokio::test]
async fn test_state_changing_event_hook_event_records_new_group() {
    let db = claude_view_db::Database::new_in_memory().await.unwrap();
    let state = crate::state::AppState::new(db);

    // Session starts autonomous
    {
        let mut sessions = state.live_sessions.write().await;
        sessions.insert(
            "test-session".to_string(),
            make_autonomous_session("test-session"),
        );
    }

    // Send PreToolUse/AskUserQuestion — this SHOULD transition to needs_you
    let app = crate::api_routes(state.clone());
    let body = serde_json::json!({
        "session_id": "test-session",
        "hook_event_name": "PreToolUse",
        "tool_name": "AskUserQuestion",
        "tool_input": {"question": "Which approach?"}
    });
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/live/hook")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);

    let sessions = state.live_sessions.read().await;
    let session = sessions.get("test-session").unwrap();
    assert_eq!(session.hook_events.len(), 1);
    assert_eq!(
        session.hook_events[0].group, "needs_you",
        "AskUserQuestion hook event should record needs_you (state was applied)"
    );
    // Verify session.agent_state was updated too
    assert!(matches!(
        session.agent_state.group,
        AgentStateGroup::NeedsYou
    ));
}
```

**Step 2: Run to verify it passes (already correct behavior)**

Run: `cargo test -p claude-view-server test_state_changing_event_hook_event -- --nocapture`

Expected: PASS — the catch-all arm applies the resolved state, so group matches.

**Step 3: Commit**

```
git add crates/server/src/routes/hooks.rs
git commit -m "test: positive test confirming state-changing events record correct group"
```

---

### Task 4: Implement the fix — move hook event construction after match arms

**Files:**
- Modify: `crates/server/src/routes/hooks.rs`

This is the actual fix. The change is mechanical: remove early hook event construction and build it inside the existing appending block where we already have access to the session's actual state.

**Step 1: Remove early `group_str` and `hook_event` construction (lines 105-123)**

Delete these lines:

```rust
// DELETE: lines 105-109
let group_str = match &agent_state.group {
    AgentStateGroup::NeedsYou => "needs_you",
    AgentStateGroup::Autonomous => "autonomous",
    AgentStateGroup::Delivered => "delivered",
};

// KEEP: lines 111-114 (hook_event_context) — still needed, move to top of function
let hook_event_context: Option<serde_json::Value> = payload
    .tool_input
    .clone()
    .or_else(|| payload.error.as_ref().map(|e| serde_json::json!({"error": e})));

// DELETE: lines 116-123
let hook_event = build_hook_event(
    now,
    &payload.hook_event_name,
    payload.tool_name.as_deref(),
    &agent_state.label,
    group_str,
    hook_event_context.as_ref(),
);
```

**Step 2: Rewrite the hook event appending block (lines 453-470)**

Replace the existing block with one that builds the hook event using the session's actual group:

```rust
// ── Append hook event to session (unified, after all match arms) ──
// SessionEnd removes the session, so skip appending for it.
// IMPORTANT: Build the hook event HERE (after match arms), using the
// session's actual agent_state.group. For metadata-only events
// (TaskCompleted, SubagentStop, TeammateIdle), the resolved state from
// resolve_state_from_hook is never applied to session.agent_state.
// Recording the resolved group would create visual false positives
// in the hook event log (e.g., TaskCompleted showing as "needs_you"
// when the session is still autonomous).
if payload.hook_event_name != "SessionEnd" {
    let mut sessions = state.live_sessions.write().await;
    if let Some(session) = sessions.get_mut(&payload.session_id) {
        let actual_group = match &session.agent_state.group {
            AgentStateGroup::NeedsYou => "needs_you",
            AgentStateGroup::Autonomous => "autonomous",
            AgentStateGroup::Delivered => "delivered",
        };

        let hook_event = build_hook_event(
            now,
            &payload.hook_event_name,
            payload.tool_name.as_deref(),
            &agent_state.label,
            actual_group,
            hook_event_context.as_ref(),
        );

        if session.hook_events.len() >= MAX_HOOK_EVENTS_PER_SESSION {
            session.hook_events.drain(..100); // drop oldest 100
        }
        session.hook_events.push(hook_event.clone());
        drop(sessions);

        // Broadcast to any connected WS listeners
        let channels = state.hook_event_channels.read().await;
        if let Some(tx) = channels.get(&payload.session_id) {
            let _ = tx.send(hook_event);
        }
    }
}
```

**Step 3: Also refactor Task 1's test to use `make_autonomous_session` helper**

The first test (Task 1) manually inlined the LiveSession construction. Refactor it to use `make_autonomous_session` for consistency.

**Step 4: Run all tests**

Run: `cargo test -p claude-view-server -- --nocapture`

Expected: ALL PASS — the 3 failing tests from Tasks 1-2 now pass, the positive test from Task 3 still passes, and all existing tests remain green.

**Step 5: Commit**

```
git add crates/server/src/routes/hooks.rs
git commit -m "fix: hook event log records session's actual group, not resolved group

TaskCompleted, SubagentStop, and TeammateIdle are metadata-only events
whose resolved state is never applied to session.agent_state. Previously,
the hook event log recorded the resolved group (e.g., needs_you for
TaskCompleted), creating visual false positives in the Log tab. Now,
hook events are built after the match arms run, using the session's
actual agent_state.group."
```

---

### Task 5: Verify end-to-end with running app

**Files:** None (manual verification)

**Step 1: Start the dev server**

Run: `bun run dev:server`

**Step 2: Open the app and trigger a TaskCompleted hook manually**

```bash
# First, start a session via a SessionStart hook
curl -X POST http://localhost:47892/api/live/hook \
  -H 'content-type: application/json' \
  -d '{"session_id":"test-verify","hook_event_name":"SessionStart","cwd":"/tmp/test"}'

# Send UserPromptSubmit to transition to autonomous
curl -X POST http://localhost:47892/api/live/hook \
  -H 'content-type: application/json' \
  -d '{"session_id":"test-verify","hook_event_name":"UserPromptSubmit","prompt":"test"}'

# Send TaskCompleted — should NOT show as needs_you in the log
curl -X POST http://localhost:47892/api/live/hook \
  -H 'content-type: application/json' \
  -d '{"session_id":"test-verify","hook_event_name":"TaskCompleted","task_id":"1","task_subject":"Test task"}'
```

**Step 3: Verify in the UI**

Open the session detail panel → Log tab → look at the TaskCompleted event. It should show with the `autonomous` group color (not amber `needs_you`).

**Step 4: Verify no notification sound**

Confirm no ding was played on the TaskCompleted hook.

**Step 5: Clean up**

```bash
curl -X POST http://localhost:47892/api/live/hook \
  -H 'content-type: application/json' \
  -d '{"session_id":"test-verify","hook_event_name":"SessionEnd"}'
```

---

## Why this is not a band-aid

The root cause is an **architectural conflation**: `resolve_state_from_hook()` produces one `AgentState` used for two different purposes:

1. **State mutation** — applied (or not) by each match arm in `handle_hook`
2. **Event logging** — recorded in hook_events for the Log tab

For metadata-only arms, purpose #1 is skipped but purpose #2 still uses the same data. This fix separates the concerns by building the log entry from the session's actual state post-mutation, ensuring the log always reflects reality. The resolved state from `resolve_state_from_hook` remains unchanged — it still drives the match arms and tracing logs correctly.

## What this does NOT change

- `resolve_state_from_hook()` — untouched, still maps events to theoretical states
- Server tracing logs (line 91-97) — still log the resolved state for debugging
- Frontend `useNotificationSound` — already correct (watches session SSE, not hook log)
- The SSE `session_updated` event payload — still carries session's actual state
