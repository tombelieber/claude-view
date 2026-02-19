---
status: pending
date: 2026-02-17
phase: Mission Control
depends_on: Phase B
---

# Derived Agent State Model

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the event-driven (transition-gated) agent state classification with a pure derived model that re-computes state from current evidence on every update, eliminating the race condition where tool_result lines steal the Working→Paused transition from the real end_turn line.

**Architecture:** Extract all classification logic into a pure function `derive_agent_state()` that takes current evidence and returns `AgentState`. Remove transition-gating from classification. Keep side-effect-only transition tracking in a slimmed `handle_transitions()`. Absorb Phase 4 stale-Autonomous timeout (120s) into the derivation function. The result: agent state is always correct regardless of JSONL line ordering or file-event batching.

**Tech Stack:** Rust (Axum server crate), existing `SessionStateClassifier`

---

## Root Cause (for context)

JSONL lines arrive in separate file-system events. When a `tool_result` user line arrives BEFORE the assistant's `end_turn` line:

1. tool_result triggers Working→Paused transition
2. Classification fires with no `stop_reason` → MidWork → **Autonomous**
3. end_turn arrives → Paused→Paused — **classification never re-fires**
4. Session stays in Running column for 2+ minutes until Phase 4 catches it

The fix: classification is no longer gated by transitions. It runs on every update using current `last_line` evidence.

---

### Task 1: Extract `derive_agent_state()` Pure Function

**Files:**
- Modify: `crates/server/src/live/manager.rs`

**Step 1: Write the failing test**

Add inside the `#[cfg(test)] mod tests` block (starts at line 1044), before the closing `}` at line 1118. The imports below are technically redundant (already in scope via `use super::*`) but kept for clarity:

```rust
use super::classifier::SessionStateClassifier;
use super::state::AgentStateGroup;
use vibe_recall_core::live_parser::LineType;

/// Helper to create a LiveLine for agent state derivation tests.
fn make_test_line(
    line_type: LineType,
    tool_names: Vec<String>,
    stop_reason: Option<&str>,
    is_tool_result: bool,
) -> LiveLine {
    LiveLine {
        line_type,
        role: None,
        content_preview: String::new(),
        tool_names,
        model: None,
        input_tokens: None,
        output_tokens: None,
        cache_read_tokens: None,
        cache_creation_tokens: None,
        timestamp: None,
        stop_reason: stop_reason.map(String::from),
        git_branch: None,
        is_meta: false,
        is_tool_result_continuation: is_tool_result,
        has_system_prefix: false,
        sub_agent_spawns: Vec::new(),
        sub_agent_result: None,
        sub_agent_progress: None,
    }
}

#[test]
fn test_derive_agent_state_working_is_autonomous() {
    let classifier = SessionStateClassifier::new();
    let state = derive_agent_state(
        &SessionStatus::Working,
        None,
        &VecDeque::new(),
        &classifier,
        true, 5, 3, false,
    );
    assert_eq!(state.group, AgentStateGroup::Autonomous);
    assert_eq!(state.state, "acting");
}

#[test]
fn test_derive_agent_state_done_is_needs_you() {
    let classifier = SessionStateClassifier::new();
    let state = derive_agent_state(
        &SessionStatus::Done,
        None,
        &VecDeque::new(),
        &classifier,
        false, 400, 3, false,
    );
    assert_eq!(state.group, AgentStateGroup::NeedsYou);
    assert_eq!(state.state, "session_ended");
}

#[test]
fn test_derive_agent_state_paused_end_turn_is_needs_you() {
    let classifier = SessionStateClassifier::new();
    let last = make_test_line(LineType::Assistant, vec![], Some("end_turn"), false);
    let state = derive_agent_state(
        &SessionStatus::Paused,
        Some(&last),
        &VecDeque::new(),
        &classifier,
        true, 5, 5, false,
    );
    assert_eq!(state.group, AgentStateGroup::NeedsYou,
        "end_turn assistant line should always produce NeedsYou");
}

#[test]
fn test_derive_agent_state_tool_result_midwork_autonomous() {
    // tool_result with process running = between steps = Autonomous (correct for intermediate state)
    let classifier = SessionStateClassifier::new();
    let last = make_test_line(LineType::User, vec![], None, true);
    let state = derive_agent_state(
        &SessionStatus::Paused,
        Some(&last),
        &VecDeque::new(),
        &classifier,
        true, 5, 5, false,
    );
    assert_eq!(state.group, AgentStateGroup::Autonomous,
        "tool_result with process running should be Autonomous (between steps)");
}

#[test]
fn test_derive_agent_state_midwork_stale_120s_is_needs_you() {
    // MidWork + process running but >120s idle = stale, should be NeedsYou
    let classifier = SessionStateClassifier::new();
    let last = make_test_line(LineType::User, vec![], None, true);
    let state = derive_agent_state(
        &SessionStatus::Paused,
        Some(&last),
        &VecDeque::new(),
        &classifier,
        true, 130, 5, false,
    );
    assert_eq!(state.group, AgentStateGroup::NeedsYou,
        "MidWork >120s should force NeedsYou even with process running");
}

/// THE critical regression test: simulates the race condition.
/// tool_result arrives first (Autonomous), then end_turn (should flip to NeedsYou).
#[test]
fn test_derive_agent_state_race_condition_tool_result_then_end_turn() {
    let classifier = SessionStateClassifier::new();

    // Step 1: tool_result line → should be Autonomous (between steps)
    let tool_result_line = make_test_line(LineType::User, vec![], None, true);
    let state1 = derive_agent_state(
        &SessionStatus::Paused,
        Some(&tool_result_line),
        &VecDeque::new(),
        &classifier,
        true, 5, 5, false,
    );
    assert_eq!(state1.group, AgentStateGroup::Autonomous,
        "Intermediate: tool_result with process should be Autonomous");

    // Step 2: end_turn line → MUST be NeedsYou (the fix!)
    let end_turn_line = make_test_line(LineType::Assistant, vec![], Some("end_turn"), false);
    let state2 = derive_agent_state(
        &SessionStatus::Paused,
        Some(&end_turn_line),
        &VecDeque::new(),
        &classifier,
        true, 5, 5, false,
    );
    assert_eq!(state2.group, AgentStateGroup::NeedsYou,
        "REGRESSION: end_turn must produce NeedsYou regardless of previous state");
}

#[test]
fn test_derive_agent_state_first_poll_no_process_is_needs_you() {
    // On first discovery, MidWork without confirmed process → NeedsYou
    let classifier = SessionStateClassifier::new();
    let last = make_test_line(LineType::User, vec![], None, true);
    let state = derive_agent_state(
        &SessionStatus::Paused,
        Some(&last),
        &VecDeque::new(),
        &classifier,
        false, 5, 5, true, // is_first_poll = true, no process
    );
    assert_eq!(state.group, AgentStateGroup::NeedsYou,
        "First poll without process should not keep Autonomous");
}

#[test]
fn test_derive_agent_state_ask_user_question_is_needs_you() {
    let classifier = SessionStateClassifier::new();
    let last = make_test_line(
        LineType::Assistant,
        vec!["AskUserQuestion".to_string()],
        Some("end_turn"),
        false,
    );
    let mut recent = VecDeque::new();
    recent.push_back(MessageSummary {
        role: "assistant".to_string(),
        content_preview: "Which option?".to_string(),
        tool_names: vec!["AskUserQuestion".to_string()],
    });
    let state = derive_agent_state(
        &SessionStatus::Paused,
        Some(&last),
        &recent,
        &classifier,
        true, 5, 5, false,
    );
    assert_eq!(state.group, AgentStateGroup::NeedsYou);
    assert_eq!(state.state, "awaiting_input");
}

#[test]
fn test_derive_agent_state_single_turn_end_turn_is_task_complete() {
    // Structural classifier single-turn Q&A path: turn_count ≤ 2 + end_turn + assistant message
    let classifier = SessionStateClassifier::new();
    let last = make_test_line(LineType::Assistant, vec![], Some("end_turn"), false);
    let mut recent = VecDeque::new();
    recent.push_back(MessageSummary {
        role: "assistant".to_string(),
        content_preview: "The answer is 42.".to_string(),
        tool_names: vec![],
    });
    let state = derive_agent_state(
        &SessionStatus::Paused,
        Some(&last),
        &recent,
        &classifier,
        true, 5, 1, false, // turn_count = 1 → triggers single-turn Q&A structural match
    );
    assert_eq!(state.group, AgentStateGroup::NeedsYou);
    assert_eq!(state.state, "task_complete",
        "Single-turn Q&A with end_turn should hit structural classifier → task_complete");
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p vibe-recall-server -- tests::test_derive_agent_state 2>&1 | head -30`
Expected: FAIL — `derive_agent_state` function doesn't exist yet.

**Step 3: Write the `derive_agent_state` function**

Add this free function above `handle_status_change` (before line 840):

```rust
/// Pure function: derive agent state from current evidence.
///
/// Called on EVERY update — not gated by status transitions. This eliminates
/// the race condition where a tool_result line steals the Working→Paused
/// transition from the real end_turn line.
///
/// The 120s MidWork timeout (previously Phase 4 in spawn_process_detector)
/// is incorporated here for instant reactivity.
fn derive_agent_state(
    status: &SessionStatus,
    last_line: Option<&LiveLine>,
    recent_messages: &VecDeque<MessageSummary>,
    classifier: &SessionStateClassifier,
    has_running_process: bool,
    seconds_since_modified: u64,
    turn_count: u32,
    is_first_poll: bool,
) -> AgentState {
    match status {
        SessionStatus::Working => AgentState {
            group: AgentStateGroup::Autonomous,
            state: "acting".into(),
            label: "Working...".into(),
            confidence: 0.7,
            source: SignalSource::Jsonl,
            context: None,
        },
        SessionStatus::Done => AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "session_ended".into(),
            label: "Session ended".into(),
            confidence: 0.9,
            source: SignalSource::Jsonl,
            context: None,
        },
        SessionStatus::Paused => {
            let ctx = SessionStateContext {
                recent_messages: recent_messages.iter().cloned().collect(),
                last_stop_reason: last_line.and_then(|l| l.stop_reason.clone()),
                last_tool: last_line.and_then(|l| l.tool_names.last().cloned()),
                has_running_process,
                seconds_since_modified,
                turn_count,
            };

            // Tier 1: structural classification (instant)
            if let Some(c) = classifier.structural_classify(&ctx) {
                return pause_classification_to_agent_state(&c);
            }

            // Fallback classification
            let c = classifier.fallback_classify(&ctx);

            // MidWork = ambiguous pause. Keep Autonomous if ALL of:
            //   (a) fallback says MidWork (no end_turn detected), AND
            //   (b) process detected OR file active within 60s, AND
            //   (c) not stale (≤120s) — absorbs former Phase 4, AND
            //   (d) not first poll without process evidence
            let keep_autonomous = c.reason == PauseReason::MidWork && if is_first_poll {
                has_running_process
            } else {
                (has_running_process || seconds_since_modified <= 60)
                    && seconds_since_modified <= 120
            };

            if keep_autonomous {
                AgentState {
                    group: AgentStateGroup::Autonomous,
                    state: "thinking".into(),
                    label: "Between steps...".into(),
                    confidence: if has_running_process { 0.5 } else { 0.4 },
                    source: SignalSource::Jsonl,
                    context: None,
                }
            } else {
                pause_classification_to_agent_state(&c)
            }
        }
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p vibe-recall-server -- tests::test_derive_agent_state -v`
Expected: ALL 9 tests PASS.

**Step 5: Commit**

```bash
git add crates/server/src/live/manager.rs
git commit -m "feat(live): add derive_agent_state pure function with tests

Extracts classification logic into a stateless pure function that can
be called on every update without transition gating. Incorporates the
120s Phase 4 stale-Autonomous timeout for instant reactivity.

Regression test covers the race condition where tool_result steals
the Working→Paused transition from end_turn."
```

---

### Task 2: Replace `handle_status_change` with `handle_transitions` + `derive_agent_state`

> **⚠️ Compile Note:** `handle_status_change` is called from TWO sites: `process_jsonl_update` (line 758) and `spawn_process_detector` Phase 1 (line 343). This task replaces the method with `handle_transitions` and updates the `process_jsonl_update` call site. The Phase 1 call site is updated in Task 3. **Task 2 and Task 3 must be implemented together** — the code will not compile between them. If implementing as separate commits, squash before pushing.
>
> **⚠️ Line Shift Note:** Task 1 inserts `derive_agent_state()` (~55 lines) before line 840, which shifts all subsequent line numbers down. After Task 1, use text search (`fn handle_status_change`) rather than raw line numbers to find the target blocks.

**Files:**
- Modify: `crates/server/src/live/manager.rs` — replace `handle_status_change` method (search for `fn handle_status_change`, currently lines 843-968 pre-Task-1)
- Modify: `crates/server/src/live/manager.rs` — update caller in `process_jsonl_update` (search for `self.handle_status_change`, currently line 758 pre-Task-1)

**Step 1: Write the failing test**

Add to the test block:

```rust
#[test]
fn test_handle_transitions_working_to_paused_computes_task_time() {
    let mut acc = SessionAccumulator::new();
    acc.last_status = Some(SessionStatus::Working);
    acc.current_turn_started_at = Some(1000);

    handle_transitions(&SessionStatus::Paused, &mut acc, 1033);

    assert_eq!(acc.last_turn_task_seconds, Some(33),
        "Working→Paused should compute task time as last_activity_at - turn_start");
    assert_eq!(acc.last_status, Some(SessionStatus::Paused));
}

#[test]
fn test_handle_transitions_to_working_clears_task_time() {
    let mut acc = SessionAccumulator::new();
    acc.last_turn_task_seconds = Some(42);

    handle_transitions(&SessionStatus::Working, &mut acc, 0);

    assert_eq!(acc.last_turn_task_seconds, None,
        "Entering Working should clear task time");
}

#[test]
fn test_handle_transitions_to_done_sets_completed_at() {
    let mut acc = SessionAccumulator::new();
    acc.last_status = Some(SessionStatus::Paused);

    handle_transitions(&SessionStatus::Done, &mut acc, 0);

    assert!(acc.completed_at.is_some());
    assert_eq!(acc.last_status, Some(SessionStatus::Done));
}

#[test]
fn test_handle_transitions_done_cleans_up_running_subagents() {
    let mut acc = SessionAccumulator::new();
    acc.sub_agents.push(SubAgentInfo {
        tool_use_id: "toolu_1".into(),
        agent_id: None,
        agent_type: "Explore".into(),
        description: "test".into(),
        status: SubAgentStatus::Running,
        started_at: 1000,
        completed_at: None,
        duration_ms: None,
        tool_use_count: None,
        cost_usd: None,
        current_activity: None,
    });

    handle_transitions(&SessionStatus::Done, &mut acc, 0);

    assert_eq!(acc.sub_agents[0].status, SubAgentStatus::Error,
        "Running sub-agents should be marked Error on session Done");
}

#[test]
fn test_handle_transitions_paused_to_paused_no_task_time_change() {
    let mut acc = SessionAccumulator::new();
    acc.last_status = Some(SessionStatus::Paused);
    acc.last_turn_task_seconds = Some(33);

    handle_transitions(&SessionStatus::Paused, &mut acc, 2000);

    assert_eq!(acc.last_turn_task_seconds, Some(33),
        "Paused→Paused should NOT recompute task time");
}

#[test]
fn test_handle_transitions_first_discovery_as_paused_computes_task_time() {
    // First discovery (last_status = None) as Paused should still compute task time
    let mut acc = SessionAccumulator::new();
    // last_status is None (first discovery)
    acc.current_turn_started_at = Some(500);

    handle_transitions(&SessionStatus::Paused, &mut acc, 555);

    assert_eq!(acc.last_turn_task_seconds, Some(55),
        "First discovery as Paused (old_status.is_none()) should compute task time");
    assert_eq!(acc.last_status, Some(SessionStatus::Paused));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p vibe-recall-server -- tests::test_handle_transitions 2>&1 | head -30`
Expected: FAIL — `handle_transitions` doesn't exist yet.

**Step 3: Replace `handle_status_change` with `handle_transitions`**

Delete the entire `handle_status_change` method including its doc comment (search for `fn handle_status_change`; pre-Task-1 location: lines 840–968, including doc comment at 840–842). Add this **free function** outside the `impl LiveSessionManager` block (NOT a method — no `&self`):

```rust
/// Handle side effects of status transitions.
///
/// This is NOT classification — classification is done by `derive_agent_state()`.
/// This function only handles:
/// - Task time computation on Working→Paused
/// - Task time clearing on →Working
/// - Completion tracking + sub-agent cleanup on →Done
fn handle_transitions(
    new_status: &SessionStatus,
    acc: &mut SessionAccumulator,
    last_activity_at: i64,
) {
    let old_status = acc.last_status.clone();

    // Working→Paused (or first-discovery-as-Paused): compute task time
    let is_working_to_paused = *new_status == SessionStatus::Paused
        && (old_status == Some(SessionStatus::Working) || old_status.is_none());
    if is_working_to_paused {
        if let Some(turn_start) = acc.current_turn_started_at {
            let elapsed = (last_activity_at - turn_start).max(0) as u32;
            acc.last_turn_task_seconds = Some(elapsed);
        }
    }

    // →Working: clear frozen task time
    if *new_status == SessionStatus::Working {
        acc.last_turn_task_seconds = None;
    }

    // →Done: track completion time + orphaned sub-agent cleanup
    if *new_status == SessionStatus::Done && acc.completed_at.is_none() {
        let completed_at_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        acc.completed_at = Some(completed_at_secs);
        for agent in &mut acc.sub_agents {
            if agent.status == SubAgentStatus::Running {
                agent.status = SubAgentStatus::Error;
                agent.completed_at = Some(completed_at_secs as i64);
                agent.current_activity = None;
            }
        }
    } else if *new_status != SessionStatus::Done {
        acc.completed_at = None;
    }

    acc.last_status = Some(new_status.clone());
}
```

**Step 4: Update `process_jsonl_update` caller (lines 752–758)**

Replace:
```rust
        // Derive status
        let processes = self.processes.read().await;
        let (running, pid) = has_running_process(&processes, &project_path);
        let status = derive_status(acc.last_line.as_ref(), seconds_since, running);

        // Detect transitions and trigger classification
        self.handle_status_change(&session_id, status.clone(), acc, running, seconds_since, last_activity_at);
```

With:
```rust
        // Derive status
        let processes = self.processes.read().await;
        let (running, pid) = has_running_process(&processes, &project_path);
        let status = derive_status(acc.last_line.as_ref(), seconds_since, running);

        // Capture before handle_transitions mutates it
        let is_first_poll = acc.last_status.is_none();

        // Side effects only (task time, completion, sub-agent cleanup)
        handle_transitions(&status, acc, last_activity_at);

        // Derive agent state from current evidence (always runs, no transition gating)
        acc.agent_state = derive_agent_state(
            &status,
            acc.last_line.as_ref(),
            &acc.recent_messages,
            &self.classifier,
            running,
            seconds_since,
            acc.user_turn_count,
            is_first_poll,
        );
```

**Step 5: Run all server tests**

Run: `cargo test -p vibe-recall-server -v 2>&1 | tail -30`
Expected: ALL tests pass (including existing derive_status, derive_activity, classifier tests).

**Step 6: Commit**

```bash
git add crates/server/src/live/manager.rs
git commit -m "refactor(live): replace handle_status_change with handle_transitions + derive_agent_state

Classification is now a pure derivation called on every JSONL update,
not gated by Working→Paused transitions. Side effects (task time,
completion tracking) remain transition-gated in handle_transitions.

Fixes: sessions stuck in Running when tool_result arrives before end_turn."
```

---

### Task 3: Update Process Detector to Use Derived Model + Remove Phase 4

**Files:**
- Modify: `crates/server/src/live/manager.rs:304-448` (`spawn_process_detector`)

**Step 1: No new test needed**

The existing `derive_agent_state` tests cover the 120s timeout (test `test_derive_agent_state_midwork_stale_120s_is_needs_you`). Phase 4 removal is a deletion. We verify via the full test suite + `cargo check`.

**Step 2: Update Phase 1 of process detector (brace-delimited block at lines 326–358; the Phase 1 comment at lines 321–322 and `pending_updates` declaration at lines 323–324 are preserved)**

Replace the Phase 1 block (the `{ ... }` scope starting at line 326, closing at line 358):

> **Behavioral change (intentional):** The old Phase 1 only pushed updates when `status` or `pid` changed. The new code also pushes updates when `agent_state.group` changes — this catches time-based transitions (MidWork→NeedsYou at 120s) that the old Phase 4 handled. Hook-sourced states (e.g., `SignalSource::Hook` from sub-agent delegation) are preserved — the process detector skips re-derivation for them, matching the old Phase 4's guard. The `derive_agent_state` allocation (VecDeque→Vec conversion) only happens when status/pid changed OR when checking for group-level agent state changes, not on every tick for every session.

```rust
                {
                    let processes = manager.processes.read().await;
                    let mut sessions = manager.sessions.write().await;
                    let mut accumulators = manager.accumulators.write().await;

                    for (session_id, session) in sessions.iter_mut() {
                        if let Some(acc) = accumulators.get_mut(session_id) {
                            let seconds_since = seconds_since_modified_from_timestamp(
                                session.last_activity_at,
                            );
                            let (running, pid) =
                                has_running_process(&processes, &session.project_path);
                            let new_status =
                                derive_status(acc.last_line.as_ref(), seconds_since, running);

                            let status_or_pid_changed =
                                session.status != new_status || session.pid != pid;

                            if status_or_pid_changed {
                                // Status/pid changed — must re-derive and handle transitions
                                let is_first_poll = acc.last_status.is_none();
                                let new_agent_state = derive_agent_state(
                                    &new_status,
                                    acc.last_line.as_ref(),
                                    &acc.recent_messages,
                                    &manager.classifier,
                                    running,
                                    seconds_since,
                                    acc.user_turn_count,
                                    is_first_poll,
                                );
                                handle_transitions(
                                    &new_status, acc, session.last_activity_at,
                                );
                                acc.agent_state = new_agent_state;

                                pending_updates.push((
                                    session_id.clone(),
                                    new_status,
                                    pid,
                                    acc.agent_state.clone(),
                                ));
                            } else {
                                // Status unchanged — check if agent state group changed
                                // (time-based transitions like MidWork→NeedsYou at 120s).
                                // Skip hook-sourced states: long subagent runs are
                                // legitimately autonomous, don't override with JSONL derivation.
                                if matches!(session.agent_state.source, SignalSource::Hook) {
                                    continue;
                                }
                                let is_first_poll = acc.last_status.is_none();
                                let new_agent_state = derive_agent_state(
                                    &new_status,
                                    acc.last_line.as_ref(),
                                    &acc.recent_messages,
                                    &manager.classifier,
                                    running,
                                    seconds_since,
                                    acc.user_turn_count,
                                    is_first_poll,
                                );
                                if session.agent_state.group != new_agent_state.group {
                                    acc.agent_state = new_agent_state;
                                    pending_updates.push((
                                        session_id.clone(),
                                        new_status,
                                        pid,
                                        acc.agent_state.clone(),
                                    ));
                                }
                            }
                        }
                    }
                }
```

**Step 3: Delete Phase 4 block (lines 399–447)**

Delete the entire Phase 4 block including its comment:

```rust
                // Phase 4: Stale-Autonomous re-evaluation.
                // Catch sessions that are Paused + Autonomous for >2 minutes ...
                {
                    ...
                }
```

**Step 4: Update the function's doc comment (lines 295–303, starting from `/// Spawn the process detector background task.`)**

Replace with:
```rust
    /// Spawn the process detector background task.
    ///
    /// Every 5 seconds, scans the process table for running Claude instances
    /// and updates the shared process map. Re-derives status AND agent state
    /// for all sessions (agent state derivation is not transition-gated).
    ///
    /// Uses a 3-phase pattern to avoid deadlocks and TOCTOU races:
    /// - Phase 1 (under sessions+accumulators locks): Derive status + agent state.
    /// - Phase 2 (no locks): Feed JSONL states into the resolver.
    /// - Phase 3 (under sessions lock only): Call resolve() and apply final state.
```

**Step 5: Run full test suite + compile check**

Run: `cargo test -p vibe-recall-server -v && cargo check -p vibe-recall-server`
Expected: ALL pass, no compilation errors.

**Step 6: Commit**

```bash
git add crates/server/src/live/manager.rs
git commit -m "refactor(live): process detector uses derived model, remove Phase 4

Process detector now always re-derives agent_state (not transition-gated).
The 120s stale-Autonomous timeout is built into derive_agent_state, so
the separate Phase 4 loop is no longer needed."
```

---

### Task 4: Frontend Typecheck + Build Verification

**Files:**
- None modified — this is a backend-only change. Frontend reads `agentState.group` which is unchanged.

**Step 1: Verify frontend still compiles**

Run: `bun run typecheck`
Expected: No errors (no frontend types changed).

**Step 2: Full build verification**

Run: `cargo build -p vibe-recall-server 2>&1 | tail -5`
Expected: `Finished` with no warnings related to this change.

**Step 3: Commit (if any warnings were fixed)**

No commit expected — this task is verification only.

---

### Task 5: Manual End-to-End Smoke Test

**Step 1: Start the dev server**

Run: `bun run dev` (or `bun run preview` for production build)

**Step 2: Open Mission Control in browser**

Navigate to `http://localhost:5173/live` (or `:47892/live` for preview).
Switch to Kanban view.

**Step 3: Verify running sessions**

If any Claude Code sessions are active:
- Sessions where Claude finished its turn (waiting for input) should appear in **Needs You** column
- Sessions where Claude is actively working should appear in **Running** column
- No sessions should be stuck in Running when Claude is clearly waiting

**Step 4: Verify the API directly**

Run: `curl -s http://localhost:47892/api/live/sessions | jq '.[] | {id: .id[0:8], status, group: .agentState.group, state: .agentState.state}'`

For sessions where Claude is waiting for input:
- `status` should be `"paused"`
- `group` should be `"needs_you"`
- `state` should be `"awaiting_input"` or `"task_complete"` (not `"thinking"`)

---

## Files Summary

| File | Change |
|------|--------|
| `crates/server/src/live/manager.rs` | Add `derive_agent_state()` pure function (~55 lines); replace `handle_status_change()` with `handle_transitions()` (~35 lines, net -30 lines); update `process_jsonl_update` caller (3 lines); update process detector Phase 1 (replace ~25 lines); delete Phase 4 (~45 lines deleted); add 15 unit tests (~200 lines) |

**No other files modified.** The `AgentState`, `AgentStateGroup`, `SessionStatus`, and `LiveSession` types are unchanged. The frontend receives the same data shape — just with correct values.

## Why This Is Robust

| Property | Old Model | New Model |
|----------|-----------|-----------|
| Classification trigger | Working→Paused transition only | Every update |
| Line ordering dependency | Yes (tool_result steals transition) | No (uses current last_line) |
| Phase 4 separate loop | Yes (120s delay, 5s poll) | Absorbed into derivation (instant) |
| Testability | Hard (requires transition setup) | Easy (pure function, no state) |
| Race conditions | Vulnerable | Immune (no cached classification) |

---

## Changelog of Fixes Applied (Audit → Final Plan)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | `handle_status_change` called from 2 sites (line 758 + line 343); deleting it in Task 2 while Task 3 updates the second call site creates un-compilable intermediate state | **Blocker** | Added compile note to Task 2 stating Tasks 2+3 must be implemented together; squash if separate commits |
| 2 | Task 1 inserts ~55 lines before line 840, shifting Task 2's line references ("lines 840-968") | **Warning** | Added line-shift note to Task 2; changed to "search for `fn handle_status_change`" instead of raw line numbers |
| 3 | Phase 1 replacement silently widens change-detection guard to include `agent_state.group` changes (more SSE events) | **Warning** | Added behavioral-change note documenting this as intentional (replaces Phase 4's role) |
| 4 | Task 3 Step 4 says "lines 296-303" but doc comment starts at line 295 | **Minor** | Changed to "lines 295-303" |
| 5 | Test block imports redundant (already via `use super::*`) | **Minor** | Added clarifying note; kept imports for explicit readability |
| 6 | "after line 1117" ambiguous for test insertion location | **Minor** | Changed to "before the closing `}` at line 1118" with test block start reference (line 1044) |
| 7 | `is_first_poll` misleading in process detector context | **Minor** | Added inline comment explaining it's almost always false in process detector |
| 8 | Task 2 Step 3 didn't clarify the method should be removed from `impl` block, not just renamed | **Minor** | Added "outside the `impl LiveSessionManager` block" to placement instruction |
| 9 | No test for structural-classify path (single-turn Q&A with `turn_count=1` + `end_turn`) | **Suggestion** | Added `test_derive_agent_state_single_turn_end_turn_is_task_complete` asserting `state == "task_complete"` |
| 10 | No test for `handle_transitions` first-discovery-as-Paused (`old_status.is_none()`) | **Suggestion** | Added `test_handle_transitions_first_discovery_as_paused_computes_task_time` |
| 11 | `derive_agent_state` allocates Vec from VecDeque on every 5s tick in process detector, even when nothing changed | **Suggestion** | Restructured Phase 1 into fast-path (status/pid check) and slow-path (agent state re-derivation only when needed) |
| 12 | Process detector may fire unnecessary SSE events when hook state overrides JSONL derivation | **Suggestion** | Added `SignalSource::Hook` guard in Phase 1 slow-path, matching old Phase 4's hook preservation |

---

## Part 2: Pipeline Robustness Hardening (Tasks 6–12)

The derived agent state model (Tasks 1–5) fixes classification correctness. These additional tasks harden the **entire live pipeline** against the remaining fragility points identified in the full-stack audit.

| Issue | ID | Severity | Task |
|-------|-----|----------|------|
| File watcher channel overflow — silent drops via `try_send`, no recovery | A | High | Task 6 |
| SIMD line-type misclassification — progress lines classified as Assistant | B | Medium | Task 7 |
| Cost breakdown double-counting — `mainAgentCost = total - sub` when total is parent-only | C | Medium | Task 8 |
| SSE broadcast lag — on lag, sends summary only, client can't recover missing sessions | H | High | Task 9 |
| Sub-agent `startedAt = unwrap_or(0)` — epoch timestamps leak to TimelineView | G | Low | Task 10 |
| Sub-agent drill-down impossible for agents without agentId | K | Medium | Task 11 |
| Session completion delayed 5 min (300s stale + 5s poll) | D | High | Task 13 |
| File replacement freezes parser forever (TOCTOU) | E | Medium | Task 14 |
| SSE field naming inconsistent + dead fallback code | I,J | Low | Task 15 |

### Accepted Limitations (No Code Change)

**F. WebSocket initial scrollback:** The default is 100 lines (not 100K as initially feared). The server already uses incremental `tail_lines` and the client receives `buffer_end` after the batch. No fix needed — the server caps at `MAX_SCROLLBACK` in the code below (Task 12) for defense.

---

### Task 6: File Watcher Channel Resilience + Drop Recovery

**Root cause:** `watcher.rs:113` uses `try_send()` which silently drops events when the channel (cap 512) is full. The manager has no mechanism to detect dropped events and re-scan for missed file changes.

**Files:**
- Modify: `crates/server/src/live/watcher.rs`
- Modify: `crates/server/src/live/manager.rs`

**Step 1: Add drop counter to watcher**

In `watcher.rs`, add an `Arc<AtomicU64>` counter that increments on every failed `try_send`. Return it alongside the watcher so the manager can poll it.

Replace the import block (lines 30-34):

```rust
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tokio::sync::mpsc;
use tracing::{error, warn};
```

With:

```rust
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::mpsc;
use tracing::{error, warn};
```

Change the `start_watcher` signature (line 62) from:

```rust
pub fn start_watcher(tx: mpsc::Sender<FileEvent>) -> notify::Result<RecommendedWatcher> {
```

To:

```rust
pub fn start_watcher(tx: mpsc::Sender<FileEvent>) -> notify::Result<(RecommendedWatcher, Arc<AtomicU64>)> {
```

Inside the function, before `let projects_dir_for_filter` (line 72), add:

```rust
    let dropped_events = Arc::new(AtomicU64::new(0));
    let dropped_counter = dropped_events.clone();
```

Replace the silent `try_send` block (lines 112-115):

```rust
                    // Best-effort send; if the receiver is full/closed, drop
                    if tx.try_send(file_event).is_err() {
                        // Channel full or closed — not fatal
                    }
```

With:

```rust
                    if tx.try_send(file_event).is_err() {
                        let count = dropped_counter.fetch_add(1, Ordering::Relaxed) + 1;
                        if count == 1 || count % 100 == 0 {
                            warn!(
                                dropped_total = count,
                                "File watcher channel full — event dropped (process detector will catch up)"
                            );
                        }
                    }
```

Change the return at the end of `start_watcher` from `Ok(watcher)` to `Ok((watcher, dropped_events))`.

**Step 2: Manager uses drop counter for catch-up scans**

In `manager.rs`, update `spawn_file_watcher` to store the drop counter. After the watcher is started (around line 244), capture the counter:

Replace:

```rust
            let _watcher = match start_watcher(file_tx) {
                Ok(w) => w,
```

With:

```rust
            let (_watcher, dropped_events) = match start_watcher(file_tx) {
                Ok((w, d)) => (w, d),
```

In the file event loop (line 254), add a periodic catch-up check. Replace:

```rust
            // Process file events forever
            while let Some(event) = file_rx.recv().await {
```

With:

```rust
            // Track last catch-up scan time
            let mut last_catchup_count = 0u64;

            // Process file events forever
            while let Some(event) = file_rx.recv().await {
                // Check if drops occurred since last check — trigger catch-up scan
                let current_drops = dropped_events.load(std::sync::atomic::Ordering::Relaxed);
                if current_drops > last_catchup_count {
                    last_catchup_count = current_drops;
                    info!(
                        dropped_total = current_drops,
                        "Detected dropped watcher events — triggering catch-up scan"
                    );
                    let catchup_paths = {
                        let dir = projects_dir.clone();
                        tokio::task::spawn_blocking(move || initial_scan(&dir))
                            .await
                            .unwrap_or_default()
                    };
                    for path in &catchup_paths {
                        let sid = extract_session_id(path);
                        let is_new = {
                            let sessions = manager.sessions.read().await;
                            !sessions.contains_key(&sid)
                        };
                        manager.process_jsonl_update(path).await;
                        if is_new {
                            let sessions = manager.sessions.read().await;
                            if let Some(session) = sessions.get(&sid) {
                                let _ = manager.tx.send(SessionEvent::SessionDiscovered {
                                    session: session.clone(),
                                });
                            }
                        }
                    }
                }
```

**Step 3: Run tests**

Run: `cargo test -p vibe-recall-server -- tests 2>&1 | tail -10`
Expected: All pass.

**Step 4: Commit**

```bash
git add crates/server/src/live/watcher.rs crates/server/src/live/manager.rs
git commit -m "fix(live): log and recover from file watcher channel drops

Add AtomicU64 drop counter to watcher. When the manager detects drops,
it triggers a catch-up scan via initial_scan() to re-discover any
sessions whose file events were lost. Prevents sessions from silently
freezing when file events overflow the 512-capacity channel."
```

---

### Task 7: Fix SIMD Line-Type Misclassification for Progress Events

**Root cause:** `live_parser.rs:204-216` checks for `"user"` and `"assistant"` substrings BEFORE `"progress"`. Progress lines with nested `"role":"assistant"` get classified as `LineType::Assistant` because `"assistant"` is found first. The sub-agent progress parsing compensates with secondary checks, but `derive_status()` and the terminal handler branch on `line_type` and may mishandle these lines.

**Files:**
- Modify: `crates/core/src/live_parser.rs`

**Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block (before the closing `}`):

```rust
    #[test]
    fn test_progress_line_classified_as_progress_not_assistant() {
        // Progress lines contain "assistant" in nested data.message.role,
        // but line_type should be Progress (not Assistant).
        let finders = TailFinders::new();
        let raw = br#"{"type":"progress","parentToolUseID":"toolu_01ABC","data":{"type":"agent_progress","agentId":"a951849","message":{"role":"assistant","content":[{"type":"tool_use","name":"Read","input":{"file_path":"/path/to/file.rs"}}]}},"timestamp":"2026-02-16T08:34:13.134Z"}"#;
        let line = parse_single_line(raw, &finders);
        assert_eq!(
            line.line_type,
            LineType::Progress,
            "Progress lines must be classified as Progress, not Assistant"
        );
    }
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p vibe-recall-core -- tests::test_progress_line_classified`
Expected: FAIL — currently classified as `LineType::Assistant`.

**Step 3: Fix the ordering**

In `parse_single_line` (lines 204-216), reorder the if-else chain to check `type_progress` BEFORE `type_user` and `type_assistant`:

Replace:

```rust
    let line_type = if finders.type_user.find(raw).is_some() {
        LineType::User
    } else if finders.type_assistant.find(raw).is_some() {
        LineType::Assistant
    } else if finders.type_system.find(raw).is_some() {
        LineType::System
    } else if finders.type_progress.find(raw).is_some() {
        LineType::Progress
    } else if finders.type_summary.find(raw).is_some() {
        LineType::Summary
    } else {
        LineType::Other
    };
```

With:

```rust
    // Check progress and summary BEFORE user/assistant because progress lines
    // contain nested "role":"assistant" which would match the assistant finder.
    // Progress lines always have "type":"progress" at the top level.
    let line_type = if finders.type_progress.find(raw).is_some() {
        LineType::Progress
    } else if finders.type_summary.find(raw).is_some() {
        LineType::Summary
    } else if finders.type_user.find(raw).is_some() {
        LineType::User
    } else if finders.type_assistant.find(raw).is_some() {
        LineType::Assistant
    } else if finders.type_system.find(raw).is_some() {
        LineType::System
    } else {
        LineType::Other
    };
```

**Step 4: Run all core tests**

Run: `cargo test -p vibe-recall-core -v 2>&1 | tail -20`
Expected: ALL pass, including the new test and all existing progress event tests.

**Step 5: Commit**

```bash
git add crates/core/src/live_parser.rs
git commit -m "fix(parser): classify progress lines before user/assistant

Progress lines contain nested 'role:assistant' which was matching the
assistant SIMD finder before the progress check. Reorder to check
progress and summary first, since they have distinctive top-level
type fields."
```

---

### Task 8: Fix Cost Breakdown Double-Counting

**Root cause:** `CostBreakdown.tsx:11` computes `mainAgentCost = cost.totalUsd - subAgentTotal`. But `cost.totalUsd` is computed from the parent session's token accumulation only — sub-agent tokens come from separate API calls and are NOT included in the parent's cumulative `tokens` field. The sub-agent cost is independently computed from `toolUseResult.usage`. So `cost.totalUsd` is already the main-agent cost. Subtracting `subAgentTotal` produces a number that's too low (or negative).

**Files:**
- Modify: `src/components/live/CostBreakdown.tsx`

**Step 1: No new test needed** (visual component — verified by manual inspection)

**Step 2: Fix the cost arithmetic**

Replace the entire `CostBreakdown` component (lines 1-46 of `CostBreakdown.tsx`):

```tsx
import type { LiveSession } from './use-live-sessions'
import type { SubAgentInfo } from '../../types/generated/SubAgentInfo'

interface CostBreakdownProps {
  cost: LiveSession['cost']
  subAgents?: SubAgentInfo[]
}

export function CostBreakdown({ cost, subAgents }: CostBreakdownProps) {
  const subAgentTotal = subAgents?.reduce((sum, a) => sum + (a.costUsd ?? 0), 0) ?? 0
  // cost.totalUsd is the PARENT session's cost only (sub-agent tokens are
  // separate API calls, not in the parent's cumulative token accumulation).
  // True total = parent + all sub-agents.
  const grandTotal = cost.totalUsd + subAgentTotal

  return (
    <div className="space-y-4 p-4">
      {/* Total */}
      <div className="flex items-baseline justify-between">
        <span className="text-sm text-gray-500 dark:text-gray-400">Total Cost</span>
        <span className="text-2xl font-mono font-semibold text-gray-900 dark:text-gray-100">
          ${grandTotal.toFixed(2)}
        </span>
      </div>

      {/* Breakdown table */}
      <div className="space-y-2">
        <CostRow label="Input tokens" value={cost.inputCostUsd} />
        <CostRow label="Output tokens" value={cost.outputCostUsd} />
        {cost.cacheReadCostUsd > 0 && <CostRow label="Cache reads" value={cost.cacheReadCostUsd} />}
        {cost.cacheCreationCostUsd > 0 && <CostRow label="Cache creation" value={cost.cacheCreationCostUsd} />}
        {cost.cacheSavingsUsd > 0 && (
          <CostRow label="Cache savings" value={-cost.cacheSavingsUsd} className="text-green-600 dark:text-green-400" />
        )}
      </div>

      {/* Sub-agent breakdown */}
      {subAgents && subAgents.length > 0 && (
        <div className="border-t border-gray-200 dark:border-gray-800 pt-3 space-y-2">
          <h4 className="text-xs font-medium text-gray-500 uppercase tracking-wide">Cost by Agent</h4>
          <CostRow label="Main agent" value={cost.totalUsd} />
          {subAgents
            .filter((a) => a.costUsd != null && a.costUsd > 0)
            .map((a) => (
              <CostRow key={a.toolUseId} label={`${a.agentType}: ${a.description}`} value={a.costUsd!} />
            ))}
        </div>
      )}
    </div>
  )
}
```

**Step 3: Commit**

```bash
git add src/components/live/CostBreakdown.tsx
git commit -m "fix(ui): correct cost breakdown arithmetic

cost.totalUsd is parent-only (sub-agent tokens are separate API calls).
Grand total = parent + sub-agents. Main agent cost = cost.totalUsd (not
cost.totalUsd - subAgentTotal, which would double-subtract)."
```

---

### Task 9: SSE Lag Recovery — Re-Send All Sessions on Lag

**Root cause:** `live.rs:116-126` — on broadcast channel lag, the server sends only a summary event. The client receives the summary but has no way to recover the individual session states it missed. Sessions that were discovered/completed during the lag gap are silently lost.

**Files:**
- Modify: `crates/server/src/routes/live.rs`

**Step 1: Fix the lag handler**

In the SSE stream (around lines 116-126), replace:

```rust
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!(
                                "SSE client lagged by {} events, sending fresh summary",
                                n
                            );
                            let map = sessions.read().await;
                            let summary = build_summary(&map);
                            yield Ok(Event::default().event("summary").data(
                                serde_json::to_string(&summary).unwrap_or_default()
                            ));
                        }
```

With:

```rust
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!(
                                "SSE client lagged by {} events, re-sending all sessions",
                                n
                            );
                            // Re-send full state (same as initial connect) so the
                            // client recovers from any missed discover/complete events.
                            let map = sessions.read().await;
                            let summary = build_summary(&map);
                            yield Ok(Event::default().event("summary").data(
                                serde_json::to_string(&summary).unwrap_or_default()
                            ));
                            for session in map.values() {
                                yield Ok(Event::default().event("session_discovered").data(
                                    serde_json::to_string(session).unwrap_or_default()
                                ));
                            }
                        }
```

**Step 2: Frontend: prune stale sessions on full-state re-sync**

In `use-live-sessions.ts`, the `session_discovered` handler already does `new Map(prev).set(session.id, session)` which upserts. But sessions that were completed during the lag gap remain in the map. Add a sync mechanism.

After the `summary` event listener (around line 139), add a new listener:

Replace the existing `summary` listener:

```typescript
      es.addEventListener('summary', (e: MessageEvent) => {
        try {
          const data = JSON.parse(e.data)
          // CRITICAL: detect summary by new field name
          const s = data.needsYouCount !== undefined ? data : data.summary ?? data
          setSummary(s)
          setLastUpdate(new Date())
        } catch { /* ignore */ }
      })
```

With:

```typescript
      es.addEventListener('summary', (e: MessageEvent) => {
        try {
          const data = JSON.parse(e.data)
          const s = data.needsYouCount !== undefined ? data : data.summary ?? data
          setSummary(s)
          setLastUpdate(new Date())

          // After a lag recovery, the server re-sends all active sessions
          // as session_discovered events. Track which IDs arrive in the
          // next batch so we can prune sessions that no longer exist.
          // Use a short window: if a summary is followed by session_discovered
          // events, those represent the full current state. Sessions NOT in
          // that batch were completed during the lag gap.
          // Implementation: mark a resync window, collect IDs, prune after.
          resyncRef.current = { ids: new Set<string>(), timer: null }
          resyncRef.current.timer = window.setTimeout(() => {
            if (resyncRef.current) {
              const validIds = resyncRef.current.ids
              if (validIds.size > 0) {
                setSessions(prev => {
                  const next = new Map<string, LiveSession>()
                  for (const [id, session] of prev) {
                    if (validIds.has(id)) next.set(id, session)
                  }
                  return next
                })
              }
              resyncRef.current = null
            }
          }, 500) // 500ms window for all session_discovered to arrive
        } catch { /* ignore */ }
      })
```

Add the ref at the top of the `useLiveSessions` function (after the existing refs):

```typescript
  const resyncRef = useRef<{ ids: Set<string>; timer: ReturnType<typeof setTimeout> | null } | null>(null)
```

In the `session_discovered` handler, add the resync tracking:

Replace:

```typescript
      es.addEventListener('session_discovered', (e: MessageEvent) => {
        try {
          const data = JSON.parse(e.data)
          const session = data.session ?? data
          if (session?.id) {
            setSessions(prev => new Map(prev).set(session.id, session))
            setLastUpdate(new Date())
            lastEventTimes.current.set(session.id, Date.now())
          }
        } catch { /* ignore malformed */ }
      })
```

With:

```typescript
      es.addEventListener('session_discovered', (e: MessageEvent) => {
        try {
          const data = JSON.parse(e.data)
          const session = data.session ?? data
          if (session?.id) {
            setSessions(prev => new Map(prev).set(session.id, session))
            setLastUpdate(new Date())
            lastEventTimes.current.set(session.id, Date.now())
            // Track for resync window
            if (resyncRef.current) resyncRef.current.ids.add(session.id)
          }
        } catch { /* ignore malformed */ }
      })
```

**Step 3: Commit**

```bash
git add crates/server/src/routes/live.rs src/components/live/use-live-sessions.ts
git commit -m "fix(live): re-send all sessions on SSE lag recovery

On broadcast channel lag (cap 256), the server now re-sends ALL active
sessions (not just a summary). The frontend prunes sessions that
weren't in the recovery batch, fixing ghost sessions from missed
session_completed events."
```

---

### Task 10: Guard Sub-Agent `startedAt = 0` (Epoch Timestamps)

**Root cause:** `manager.rs:658-659` uses `unwrap_or(0)` for sub-agent `startedAt` when the JSONL line has no timestamp. This produces Unix epoch (Jan 1, 1970) which `TimelineView.tsx` renders as a bar pinned to position 0%.

**Files:**
- Modify: `crates/server/src/live/manager.rs`
- Modify: `src/components/live/TimelineView.tsx`

**Step 1: Backend — use `last_activity_at` as fallback instead of `0`**

In `manager.rs`, replace the sub-agent spawn `started_at` computation (lines 657-659):

```rust
                let started_at = line.timestamp.as_deref()
                    .and_then(parse_timestamp_to_unix)
                    .unwrap_or(0);
```

With:

```rust
                let started_at = line.timestamp.as_deref()
                    .and_then(parse_timestamp_to_unix)
                    .unwrap_or(last_activity_at); // fallback to file mtime, never epoch-zero
```

**Step 2: Frontend — guard against any remaining epoch-zero values**

In `TimelineView.tsx`, add a guard before computing `startOffsetMs` (line 154). Replace:

```tsx
          const startOffsetMs = (agent.startedAt - sessionStartedAt) * 1000
```

With:

```tsx
          // Guard: skip agents with epoch-zero or nonsensical startedAt
          if (agent.startedAt <= 0) return null
          const startOffsetMs = (agent.startedAt - sessionStartedAt) * 1000
```

And wrap the return in a fragment to filter nulls — or simpler, filter in the `sortedAgents` memo. Replace:

```tsx
  const sortedAgents = useMemo(() => {
    return [...subAgents].sort((a, b) => a.startedAt - b.startedAt)
  }, [subAgents])
```

With:

```tsx
  const sortedAgents = useMemo(() => {
    return [...subAgents]
      .filter((a) => a.startedAt > 0) // Exclude epoch-zero (data bug)
      .sort((a, b) => a.startedAt - b.startedAt)
  }, [subAgents])
```

Remove the inline `if (agent.startedAt <= 0) return null` guard since the filter handles it.

**Step 3: Commit**

```bash
git add crates/server/src/live/manager.rs src/components/live/TimelineView.tsx
git commit -m "fix(live): prevent epoch-zero sub-agent timestamps

Use last_activity_at as fallback instead of 0 when JSONL line has no
timestamp. Frontend also filters out any remaining zero-valued
startedAt to prevent timeline artifacts."
```

---

### Task 11: Improve Sub-Agent Drill-Down UX for Missing agentId

**Root cause:** `SwimLanes.tsx:94` — `canDrillDown = !!agent.agentId && !!onDrillDown`. Agents that haven't received a progress event yet have `agentId = null`. The drill-down button doesn't appear, and there's no visual indication of why.

The `agentId` is populated from either:
1. Progress events (while running) — requires at least one progress event
2. Completion events (`toolUseResult.agentId`) — available after completion

Short-lived agents that complete without progress events DO get `agentId` from the completion event. The only gap is agents currently running that haven't emitted progress yet.

**Files:**
- Modify: `src/components/live/SwimLanes.tsx`

**Step 1: Add visual indicator for running agents without agentId**

In `SwimLanes.tsx`, after the running progress bar section (line 138), add a message for running agents without agentId. Replace the running block:

```tsx
          {/* Running: activity text or progress bar */}
          {agent.status === 'running' && (
            <div className="pl-4 flex items-center gap-2">
              {agent.currentActivity ? (
                <span className="text-xs font-mono text-blue-600 dark:text-blue-400 flex items-center gap-1.5">
                  <span className="inline-block w-1.5 h-1.5 rounded-full bg-blue-500 dark:bg-blue-400 animate-pulse" />
                  {agent.currentActivity}
                </span>
              ) : (
                <ProgressBar />
              )}
            </div>
          )}
```

With:

```tsx
          {/* Running: activity text or progress bar */}
          {agent.status === 'running' && (
            <div className="pl-4 flex items-center gap-2">
              {agent.currentActivity ? (
                <span className="text-xs font-mono text-blue-600 dark:text-blue-400 flex items-center gap-1.5">
                  <span className="inline-block w-1.5 h-1.5 rounded-full bg-blue-500 dark:bg-blue-400 animate-pulse" />
                  {agent.currentActivity}
                </span>
              ) : (
                <ProgressBar />
              )}
              {!agent.agentId && onDrillDown && (
                <span className="text-[10px] text-gray-400 dark:text-gray-500 italic">
                  awaiting agent ID...
                </span>
              )}
            </div>
          )}
```

**Step 2: Commit**

```bash
git add src/components/live/SwimLanes.tsx
git commit -m "fix(ui): show 'awaiting agent ID' for running agents without drill-down

Running sub-agents that haven't emitted a progress event yet have no
agentId, making drill-down impossible. Show a subtle indicator so
the user understands why the row isn't clickable."
```

---

### Task 12: Add Server-Side WebSocket Scrollback Cap

**Root cause:** The terminal WebSocket handler accepts any `scrollback` value from the client handshake with no upper bound. A malicious or misconfigured client could request millions of lines, causing the server to read an entire large JSONL file into memory.

**Files:**
- Modify: `crates/server/src/routes/terminal.rs`

**Step 1: Add a cap constant and clamp the handshake value**

After `fn default_scrollback()` (line 266-268), add:

```rust
/// Maximum scrollback lines the server will send, regardless of client request.
/// Protects against OOM from malicious/misconfigured clients requesting huge scrollbacks.
const MAX_SCROLLBACK: usize = 5_000;
```

In `handle_terminal_ws`, after the handshake is parsed (around line 615), replace:

```rust
    let scrollback_count = handshake.scrollback;
```

With:

```rust
    let scrollback_count = handshake.scrollback.min(MAX_SCROLLBACK);
```

**Step 2: Commit**

```bash
git add crates/server/src/routes/terminal.rs
git commit -m "fix(terminal): cap WebSocket scrollback at 5000 lines

Prevents OOM from clients requesting unbounded scrollback. Default
remains 100; cap only affects malicious or misconfigured clients."
```

---

## Updated Files Summary

| File | Change |
|------|--------|
| `crates/server/src/live/manager.rs` | Tasks 1-3: `derive_agent_state()`, `handle_transitions()`, process detector. Task 6: catch-up scan on watcher drops. Task 10: `startedAt` fallback. Task 13: 2s polling interval. |
| `crates/server/src/live/watcher.rs` | Task 6: `AtomicU64` drop counter, warning on overflow. |
| `crates/core/src/live_parser.rs` | Task 7: SIMD pre-filter ordering fix. Task 13: `LineType::Result` + `type_result` finder. Task 14: offset rollback on file replacement. |
| `crates/server/src/live/state.rs` | Task 13: `derive_status()` returns Done on Result line. Task 15: `sessionId` rename. |
| `src/components/live/CostBreakdown.tsx` | Task 8: `grandTotal = parent + sub`, not `total - sub`. |
| `crates/server/src/routes/live.rs` | Task 9: SSE lag recovery re-sends all sessions. |
| `src/components/live/use-live-sessions.ts` | Task 9: Resync pruning. Task 15: Remove dead fallbacks, use canonical field names. |
| `src/components/live/TimelineView.tsx` | Task 10: Filter epoch-zero `startedAt`. |
| `src/components/live/SwimLanes.tsx` | Task 11: "awaiting agent ID" indicator. |
| `crates/server/src/routes/terminal.rs` | Task 12: `MAX_SCROLLBACK` cap. |

---

## Part 3: Zero-Lag Completion & Serialization Cleanup (Tasks 13–15)

These tasks eliminate the remaining "accepted limitations" from Part 2.

| Issue | ID | What was "accepted" | Task |
|-------|-----|---------------------|------|
| Session completion delayed 5 minutes (300s stale threshold) | D | 5s polling + 300s Done threshold | Task 13 |
| File replacement causes parser to return empty forever | E | TOCTOU "self-correcting" | Task 14 |
| SSE field naming inconsistency + dead fallback code | I,J | "Defensive, works" | Task 15 |

---

### Task 13: Instant Session Completion via `result` Line Detection

**Root cause:** `derive_status()` requires `!has_running_process && seconds_since_modified > 300` for Done state. This means a normally-completed session stays as Paused for up to **5 minutes** after Claude exits. But Claude Code always writes a `"type":"result"` line as the final JSONL entry. We don't detect this line type at all — there's no `LineType::Result` variant or `type_result` SIMD finder.

Adding result-line detection makes normal session completion **instant** (file-watcher-driven, 0s lag). The 5s process detector becomes a crash-only fallback, and its interval is reduced to 2s.

**Files:**
- Modify: `crates/core/src/live_parser.rs`
- Modify: `crates/server/src/live/state.rs`
- Modify: `crates/server/src/live/manager.rs`

**Step 1: Add `LineType::Result` variant**

In `live_parser.rs`, add `Result` to the `LineType` enum (after `Summary`, before `Other`):

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LineType {
    User,
    Assistant,
    System,
    Progress,
    Summary,
    Result,    // ← NEW: session final result line
    Other,
}
```

**Step 2: Add `type_result` SIMD finder**

In `TailFinders` struct, add:

```rust
    pub type_result: memmem::Finder<'static>,
```

In `TailFinders::new()`, add:

```rust
            type_result: memmem::Finder::new(b"\"result\""),
```

**Step 3: Update SIMD classification ordering**

In `parse_single_line`, update the if-else chain (this builds on Task 7's reordering). The final order is:

```rust
    // Check result/progress/summary BEFORE user/assistant because these lines
    // may contain nested "role":"assistant" which would match the assistant finder.
    let line_type = if finders.type_result.find(raw).is_some()
        && finders.type_progress.find(raw).is_none()
    {
        // "result" appears in result lines AND in toolUseResult lines.
        // Disambiguate: result lines do NOT contain "progress".
        // toolUseResult lines are on user lines with "toolUseResult" key,
        // but they also don't contain "progress" — however they DO contain
        // "user". So check: if line has "result" but NOT "user", it's a
        // top-level result line. If it has both "result" and "user", it's
        // a toolUseResult on a user line → classify as User below.
        if finders.type_user.find(raw).is_none() {
            LineType::Result
        } else {
            LineType::User
        }
    } else if finders.type_progress.find(raw).is_some() {
        LineType::Progress
    } else if finders.type_summary.find(raw).is_some() {
        LineType::Summary
    } else if finders.type_user.find(raw).is_some() {
        LineType::User
    } else if finders.type_assistant.find(raw).is_some() {
        LineType::Assistant
    } else if finders.type_system.find(raw).is_some() {
        LineType::System
    } else {
        LineType::Other
    };
```

**Step 4: Update `derive_status()` to handle Result lines**

In `state.rs`, `derive_status()`, add a Result check immediately after the `None` guard (before the process+stale check):

```rust
pub fn derive_status(
    last_line: Option<&LiveLine>,
    seconds_since_modified: u64,
    has_running_process: bool,
) -> SessionStatus {
    let last_line = match last_line {
        Some(ll) => ll,
        None => return SessionStatus::Paused,
    };

    // Result line = session definitively over. No need to wait for process
    // exit or 300s stale threshold.
    if last_line.line_type == LineType::Result {
        return SessionStatus::Done;
    }

    // Done: process exited AND file stale >300s
    if !has_running_process && seconds_since_modified > 300 {
        return SessionStatus::Done;
    }
    // ... rest unchanged
```

**Step 5: Reduce process detector polling to 2s**

In `manager.rs`, `spawn_process_detector()`, change:

```rust
            let mut interval = tokio::time::interval(Duration::from_secs(5));
```

To:

```rust
            let mut interval = tokio::time::interval(Duration::from_secs(2));
```

Update the doc comment above (lines 297-298) from "Every 5 seconds" to "Every 2 seconds".

**Step 6: Write the failing test**

Add to `live_parser.rs` test module:

```rust
    #[test]
    fn test_result_line_classified_as_result() {
        let finders = TailFinders::new();
        // Claude Code writes this as the final session line
        let raw = br#"{"type":"result","subtype":"success","duration_ms":12345,"duration_api_ms":10234,"is_error":false,"num_turns":5,"session_id":"abc123"}"#;
        let line = parse_single_line(raw, &finders);
        assert_eq!(
            line.line_type,
            LineType::Result,
            "Result lines must be classified as Result"
        );
    }

    #[test]
    fn test_tool_use_result_not_classified_as_result() {
        let finders = TailFinders::new();
        // toolUseResult is on a user line, should be User not Result
        let raw = br#"{"type":"user","message":{"role":"user","content":[{"type":"toolUseResult","toolUseId":"toolu_01ABC","content":"done"}]}}"#;
        let line = parse_single_line(raw, &finders);
        assert_eq!(
            line.line_type,
            LineType::User,
            "toolUseResult lines must remain User, not Result"
        );
    }
```

Add to `state.rs` test module:

```rust
    #[test]
    fn test_derive_status_result_line_is_done_immediately() {
        let result_line = LiveLine {
            line_type: LineType::Result,
            ..Default::default()
        };
        // Even with running process and fresh file, result = Done
        let status = derive_status(Some(&result_line), 0, true);
        assert_eq!(status, SessionStatus::Done);
    }
```

**Step 7: Run tests**

Run: `cargo test -p vibe-recall-core -- tests::test_result_line && cargo test -p vibe-recall-core -- tests::test_tool_use_result && cargo test -p vibe-recall-server -- tests::test_derive_status_result`
Expected: All pass.

**Step 8: Commit**

```bash
git add crates/core/src/live_parser.rs crates/server/src/live/state.rs crates/server/src/live/manager.rs
git commit -m "feat(live): instant session completion via result-line detection

Add LineType::Result and type_result SIMD finder. derive_status() now
returns Done immediately when the last line is a result line, bypassing
the 300s stale threshold. Normal session completion is now instant
(file-watcher-driven). Process detector reduced to 2s for crash-only
fallback."
```

---

### Task 14: File Offset Rollback Guard (TOCTOU Fix)

**Root cause:** `parse_tail()` at `live_parser.rs:160-162` does `if offset >= file_len { return empty }`. If the file is replaced (new file, smaller than stored offset), the parser returns empty forever — the session appears frozen with no new data.

**Files:**
- Modify: `crates/core/src/live_parser.rs`

**Step 1: Write the failing test**

```rust
    #[test]
    fn test_parse_tail_resets_on_file_replacement() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.jsonl");

        // Write initial content
        {
            let mut f = std::fs::File::create(&path).unwrap();
            writeln!(f, r#"{{"type":"user","message":{{"role":"user","content":"hello"}}}}"#).unwrap();
            writeln!(f, r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"text","text":"hi"}}]}}}}"#).unwrap();
        }

        let finders = TailFinders::new();
        let (lines, offset) = parse_tail(&path, 0, &finders).unwrap();
        assert!(!lines.is_empty());
        assert!(offset > 0);

        // "Replace" the file with smaller content (simulates log rotation)
        {
            let mut f = std::fs::File::create(&path).unwrap();
            writeln!(f, r#"{{"type":"user","message":{{"role":"user","content":"new session"}}}}"#).unwrap();
        }

        // Old offset is larger than new file — should reset and read from start
        let (lines2, offset2) = parse_tail(&path, offset, &finders).unwrap();
        assert!(!lines2.is_empty(), "Should read new content after file replacement");
        assert!(offset2 > 0);
    }
```

**Step 2: Fix the offset check**

In `parse_tail()`, replace:

```rust
    if offset >= file_len {
        return Ok((Vec::new(), offset));
    }
```

With:

```rust
    if offset > file_len {
        // File was replaced (new file smaller than stored offset).
        // Reset to start and read the entire new file.
        tracing::warn!(
            path = %path.display(),
            old_offset = offset,
            new_file_len = file_len,
            "File replaced (offset > size) — resetting to start"
        );
        return parse_tail(path, 0, finders);
    }
    if offset == file_len {
        return Ok((Vec::new(), offset));
    }
```

**Step 3: Run tests**

Run: `cargo test -p vibe-recall-core -- tests::test_parse_tail_resets`
Expected: PASS.

**Step 4: Commit**

```bash
git add crates/core/src/live_parser.rs
git commit -m "fix(parser): reset offset when JSONL file is replaced

If the stored byte offset exceeds the current file length (file was
replaced or truncated), reset to 0 and re-read from start. Prevents
sessions from freezing permanently after file rotation."
```

---

### Task 15: Standardize SSE Serialization & Remove Dead Fallbacks

**Root cause:**
- **Issue I:** `SessionCompleted` uses `#[serde(rename_all = "snake_case")]` on the enum, producing `"session_id"` in JSON. But `LiveSession` uses `#[serde(rename_all = "camelCase")]`, producing `camelCase`. Inconsistent.
- **Issue J:** The frontend `summary` handler does `data.needsYouCount !== undefined ? data : data.summary ?? data`. The `data.summary` branch is dead code — the backend never wraps summary fields in a `summary` key.

**Files:**
- Modify: `crates/server/src/live/state.rs`
- Modify: `src/components/live/use-live-sessions.ts`

**Step 1: Fix backend — use camelCase for SessionCompleted field**

In `state.rs`, the `SessionEvent` enum (line 121) has `#[serde(tag = "type", rename_all = "snake_case")]`. The `rename_all = "snake_case"` applies to the tag value (`SessionCompleted` → `"session_completed"`) which is correct. But the `session_id` field is already snake_case, so it stays as `"session_id"` in JSON — inconsistent with `LiveSession` which uses camelCase.

Add an explicit rename to the field:

```rust
    SessionCompleted {
        #[serde(rename = "sessionId")]
        session_id: String,
    },
```

**Step 2: Clean up frontend — remove dual-field detection and dead fallbacks**

In `use-live-sessions.ts`, replace the `session_completed` handler:

```typescript
      es.addEventListener('session_completed', (e: MessageEvent) => {
        try {
          const data = JSON.parse(e.data)
          const sessionId = data.session_id ?? data.sessionId
          if (sessionId) {
            setSessions(prev => {
              const next = new Map(prev)
              next.delete(sessionId)
              return next
            })
            lastEventTimes.current.delete(sessionId)
            setLastUpdate(new Date())
          }
        } catch { /* ignore */ }
      })
```

With:

```typescript
      es.addEventListener('session_completed', (e: MessageEvent) => {
        try {
          const data = JSON.parse(e.data)
          if (data.sessionId) {
            setSessions(prev => {
              const next = new Map(prev)
              next.delete(data.sessionId)
              return next
            })
            lastEventTimes.current.delete(data.sessionId)
            setLastUpdate(new Date())
          }
        } catch { /* ignore */ }
      })
```

Replace the `summary` handler (use the version from Task 9 if already applied, otherwise the original):

```typescript
      es.addEventListener('summary', (e: MessageEvent) => {
        try {
          const data = JSON.parse(e.data)
          // CRITICAL: detect summary by new field name
          const s = data.needsYouCount !== undefined ? data : data.summary ?? data
          setSummary(s)
          setLastUpdate(new Date())
```

With:

```typescript
      es.addEventListener('summary', (e: MessageEvent) => {
        try {
          const data = JSON.parse(e.data)
          // Backend always sends summary fields at top level (needsYouCount, etc.)
          setSummary(data)
          setLastUpdate(new Date())
```

(Keep the rest of the handler unchanged — the resync logic from Task 9.)

**Step 3: Commit**

```bash
git add crates/server/src/live/state.rs src/components/live/use-live-sessions.ts
git commit -m "fix(live): standardize SSE field naming to camelCase

SessionCompleted.session_id → sessionId (matches LiveSession convention).
Remove dead frontend fallbacks: dual-field detection (session_id ??
sessionId) and triple-shape detection (data.summary ?? data). Backend
always sends canonical shapes."
```

---

## Robustness Matrix (After All Tasks 1–15)

| Issue | Before | After |
|-------|--------|-------|
| **A. Watcher overflow** | Silent drops, sessions freeze | Logged + catch-up scan recovers |
| **B. SIMD misclassification** | Progress → Assistant | Progress checked first |
| **C. Cost arithmetic** | `total - sub` (wrong) | `parent + sub` (correct) |
| **D. Session completion lag** | 5s poll + 300s stale = **5 min** | Result line → **instant** Done; 2s poll for crash fallback |
| **E. TOCTOU file replacement** | Parser stuck forever | Offset reset + re-read from start |
| **F. WebSocket scrollback** | Uncapped client request | Capped at 5,000 |
| **G. Epoch timestamps** | `unwrap_or(0)` leaks to UI | `unwrap_or(last_activity_at)` + filter |
| **H. SSE lag recovery** | Summary only | Full session re-send + client prune |
| **I. Serde naming** | `session_id` (inconsistent) | `sessionId` (matches LiveSession) |
| **J. Dead fallback code** | Triple-shape detection | Clean single-shape access |
| **K. Missing agentId** | Silent no-click | "awaiting agent ID..." indicator |
