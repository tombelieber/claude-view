# JSONL Ground Truth Recovery v2 — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace stale snapshot agent_state at startup with JSONL-derived ground truth, and remove the 300s runtime staleness hack.

**Architecture:** At startup recovery, read the tail of each session's JSONL file and derive the true agent_state from the last meaningful line (assistant/user/result). Remove the runtime staleness check — PID liveness + hooks cover all runtime failure modes.

**Tech Stack:** Rust (Axum), `claude_view_core::live_parser`, `claude_view_core::tail`

---

### Task 1: Make `parse_single_line` public

**Files:**
- Modify: `crates/core/src/live_parser.rs:248`

**Step 1: Change visibility**

Change line 248 from:
```rust
fn parse_single_line(raw: &[u8], finders: &TailFinders) -> LiveLine {
```
to:
```rust
pub fn parse_single_line(raw: &[u8], finders: &TailFinders) -> LiveLine {
```

**Step 2: Verify it compiles**

Run: `cargo check -p claude-view-core`
Expected: compiles with no errors

**Step 3: Commit**

```bash
git add crates/core/src/live_parser.rs
git commit -m "refactor: make parse_single_line public for startup state derivation"
```

---

### Task 2: Write failing tests for `derive_agent_state_from_jsonl`

**Files:**
- Modify: `crates/server/src/live/manager.rs` (test module at bottom, before line 1782)

**Step 1: Write test for assistant end_turn → NeedsYou/idle**

Add to the `mod tests` block at the bottom of `manager.rs`:

```rust
    #[tokio::test]
    async fn test_derive_state_assistant_end_turn() {
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"text","text":"Done!"}}],"stop_reason":"end_turn"}}}}"#).unwrap();
        f.flush().unwrap();

        let state = derive_agent_state_from_jsonl(&path).await;
        let state = state.expect("should derive a state");
        assert_eq!(state.group, AgentStateGroup::NeedsYou);
        assert_eq!(state.state, "idle");
    }
```

**Step 2: Write test for assistant tool_use → Autonomous/acting**

```rust
    #[tokio::test]
    async fn test_derive_state_assistant_tool_use() {
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"tool_use","name":"Read","id":"x","input":{{}}}}],"stop_reason":"tool_use"}}}}"#).unwrap();
        f.flush().unwrap();

        let state = derive_agent_state_from_jsonl(&path).await;
        let state = state.expect("should derive a state");
        assert_eq!(state.group, AgentStateGroup::Autonomous);
        assert_eq!(state.state, "acting");
    }
```

**Step 3: Write test for user with tool_result → Autonomous/thinking**

```rust
    #[tokio::test]
    async fn test_derive_state_user_tool_result() {
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, r#"{{"type":"user","message":{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"x","content":"ok"}}]}}}}"#).unwrap();
        f.flush().unwrap();

        let state = derive_agent_state_from_jsonl(&path).await;
        let state = state.expect("should derive a state");
        assert_eq!(state.group, AgentStateGroup::Autonomous);
        assert_eq!(state.state, "thinking");
    }
```

**Step 4: Write test for result line → NeedsYou/idle**

```rust
    #[tokio::test]
    async fn test_derive_state_result_line() {
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, r#"{{"type":"result","subtype":"success","cost_usd":0.05,"duration_ms":1234,"is_error":false,"session_id":"abc"}}"#).unwrap();
        f.flush().unwrap();

        let state = derive_agent_state_from_jsonl(&path).await;
        let state = state.expect("should derive state from result line");
        assert_eq!(state.group, AgentStateGroup::NeedsYou);
        assert_eq!(state.state, "idle");
    }
```

**Step 5: Write test for skipping progress lines**

```rust
    #[tokio::test]
    async fn test_derive_state_skips_progress_lines() {
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        // Real assistant line first, then progress lines after it
        writeln!(f, r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"text","text":"Done"}}],"stop_reason":"end_turn"}}}}"#).unwrap();
        writeln!(f, r#"{{"type":"progress","data":{{"type":"usage","usage":{{}}}}}}"#).unwrap();
        writeln!(f, r#"{{"type":"progress","data":{{"type":"usage","usage":{{}}}}}}"#).unwrap();
        f.flush().unwrap();

        let state = derive_agent_state_from_jsonl(&path).await;
        let state = state.expect("should derive state from assistant line, not progress");
        assert_eq!(state.group, AgentStateGroup::NeedsYou);
        assert_eq!(state.state, "idle");
    }
```

**Step 6: Write test for empty file → None**

```rust
    #[tokio::test]
    async fn test_derive_state_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        std::fs::File::create(&path).unwrap();

        let state = derive_agent_state_from_jsonl(&path).await;
        assert!(state.is_none());
    }
```

**Step 7: Run tests to verify they fail**

Run: `cargo test -p claude-view-server derive_agent_state -- --nocapture`
Expected: FAIL — `derive_agent_state_from_jsonl` does not exist yet

**Step 8: Commit**

```bash
git add crates/server/src/live/manager.rs
git commit -m "test: add failing tests for JSONL ground truth state derivation"
```

---

### Task 3: Implement `derive_agent_state_from_jsonl`

**Files:**
- Modify: `crates/server/src/live/manager.rs` (insert after `build_recovered_session` at line 170, before `apply_jsonl_metadata` at line 172)

**Step 1: Add the function**

Insert between lines 170 and 172 (between `build_recovered_session` and `apply_jsonl_metadata`):

```rust
/// Derive agent state from the JSONL file's tail — ground truth for startup recovery.
///
/// Reads the last 10 lines, finds the last meaningful line (assistant, user, or result),
/// and derives the agent state. Returns None if the file is empty, unreadable, or has
/// no meaningful lines (falls back to NeedsYou/idle at the call site).
async fn derive_agent_state_from_jsonl(path: &Path) -> Option<AgentState> {
    let lines = claude_view_core::tail::tail_lines(path, 10).await.ok()?;
    if lines.is_empty() {
        return None;
    }

    let finders = claude_view_core::live_parser::TailFinders::new();

    // Scan from the end to find the last meaningful line
    for raw in lines.iter().rev() {
        let parsed = claude_view_core::live_parser::parse_single_line(raw.as_bytes(), &finders);

        match parsed.line_type {
            LineType::Progress | LineType::System | LineType::Summary | LineType::Other => {
                continue; // Skip non-meaningful lines
            }
            LineType::Result => {
                return Some(AgentState {
                    group: AgentStateGroup::NeedsYou,
                    state: "idle".into(),
                    label: "Waiting for your next prompt".into(),
                    context: None,
                });
            }
            LineType::Assistant => {
                let has_tool_use = !parsed.tool_names.is_empty();
                let stop = parsed.stop_reason.as_deref();
                let is_tool_active = stop == Some("tool_use") || has_tool_use;

                return Some(if stop == Some("end_turn") {
                    AgentState {
                        group: AgentStateGroup::NeedsYou,
                        state: "idle".into(),
                        label: "Waiting for your next prompt".into(),
                        context: None,
                    }
                } else if is_tool_active {
                    AgentState {
                        group: AgentStateGroup::Autonomous,
                        state: "acting".into(),
                        label: format!(
                            "Using {}",
                            parsed.tool_names.first().map(|s| s.as_str()).unwrap_or("tool")
                        ),
                        context: None,
                    }
                } else {
                    // Unknown stop_reason (max_tokens, etc.) — safe default
                    AgentState {
                        group: AgentStateGroup::NeedsYou,
                        state: "idle".into(),
                        label: "Waiting for your next prompt".into(),
                        context: None,
                    }
                });
            }
            LineType::User => {
                return Some(AgentState {
                    group: AgentStateGroup::Autonomous,
                    state: "thinking".into(),
                    label: if parsed.is_tool_result_continuation {
                        "Processing tool result...".into()
                    } else {
                        "Processing prompt...".into()
                    },
                    context: None,
                });
            }
        }
    }

    None
}
```

**Step 2: Run the tests**

Run: `cargo test -p claude-view-server derive_agent_state -- --nocapture`
Expected: all 6 tests PASS

**Step 3: Commit**

```bash
git add crates/server/src/live/manager.rs
git commit -m "feat: derive agent state from JSONL ground truth at startup"
```

---

### Task 4: Wire into startup recovery

**Files:**
- Modify: `crates/server/src/live/manager.rs` (inside `spawn_file_watcher`, after `apply_jsonl_metadata` call at ~line 484, before session insertion at ~line 489)

**Step 1: Add JSONL state override**

Find this block (after the `apply_jsonl_metadata` call and `drop(accumulators)` in the else branch):

```rust
                            } else {
                                drop(accumulators);
                            }

                            manager
                                .sessions
```

Insert between `}` and `manager.sessions`:

```rust
                            // Override snapshot agent_state with JSONL ground truth.
                            // The snapshot may be stale if the Stop hook fired while
                            // the server was down. The JSONL file is authoritative.
                            if let Some(derived) = derive_agent_state_from_jsonl(path).await {
                                if derived.group != session.agent_state.group
                                    || derived.state != session.agent_state.state
                                {
                                    info!(
                                        session_id = %session_id,
                                        snapshot = %session.agent_state.state,
                                        derived = %derived.state,
                                        "JSONL ground truth overrides snapshot agent_state"
                                    );
                                }
                                session.status = status_from_agent_state(&derived);
                                session.current_activity = derived.label.clone();
                                session.agent_state = derived;
                            } else {
                                // No meaningful JSONL lines — safe default to idle
                                session.agent_state = AgentState {
                                    group: AgentStateGroup::NeedsYou,
                                    state: "idle".into(),
                                    label: "Waiting for your next prompt".into(),
                                    context: None,
                                };
                                session.status = SessionStatus::Paused;
                            }

```

**Step 2: Verify it compiles**

Run: `cargo check -p claude-view-server`
Expected: compiles

**Step 3: Commit**

```bash
git add crates/server/src/live/manager.rs
git commit -m "feat: wire JSONL ground truth into startup recovery path"
```

---

### Task 5: Remove 300s staleness check and old tests

**Files:**
- Modify: `crates/server/src/live/manager.rs`

**Step 1: Update the Phase 1 comment**

Change line 628 from:
```rust
                // Phase 1: Lightweight liveness + staleness (every tick = 10s)
```
to:
```rust
                // Phase 1: Lightweight liveness check (every tick = 10s)
```

**Step 2: Delete the staleness constant and now_secs calculation**

Delete lines 633-643 (the entire block from the `// Staleness threshold:` comment through the `as_secs() as i64;` line):
```rust
                // Staleness threshold: autonomous sessions without hook/file
                // activity for 5+ minutes are almost certainly idle. During
                // real autonomous work, hooks fire every few seconds (PreToolUse,
                // PostToolUse) and streaming writes update the JSONL file mtime.
                // A 5-minute gap means the Stop hook was lost (e.g., server was
                // down when it fired, or curl failed silently).
                const AUTONOMOUS_STALE_SECS: i64 = 300;
                let now_secs = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;
```

**Step 3: Delete the "1b. Staleness downgrade" block**

Delete lines 677-703 (the entire `// 1b. Staleness downgrade:` comment through the closing `}`):
```rust
                        // 1b. Staleness downgrade: autonomous sessions with no
                        //     hook/file activity for 5+ minutes → downgrade to idle.
                        //     If the session IS truly active, the next hook event
                        //     (within seconds) will re-promote it to autonomous.
                        if session.agent_state.group == AgentStateGroup::Autonomous {
                            let idle_secs = now_secs - session.last_activity_at;
                            if idle_secs > AUTONOMOUS_STALE_SECS {
                                info!(
                                    session_id = %session_id,
                                    idle_secs = idle_secs,
                                    last_state = %session.agent_state.state,
                                    "Autonomous session stale for {}s — downgrading to idle",
                                    idle_secs
                                );
                                session.agent_state = AgentState {
                                    group: AgentStateGroup::NeedsYou,
                                    state: "idle".into(),
                                    label: "Waiting for your next prompt".into(),
                                    context: None,
                                };
                                session.status = SessionStatus::Paused;
                                let _ = manager.tx.send(SessionEvent::SessionUpdated {
                                    session: session.clone(),
                                });
                                snapshot_dirty = true;
                            }
                        }
```

**Step 4: Delete the 3 old staleness tests**

Delete these 3 test functions (lines 1670-1780):
- `test_staleness_downgrade_autonomous_to_idle`
- `test_staleness_does_not_downgrade_active_session`
- `test_staleness_skips_paused_sessions`

**Step 5: Run all server tests**

Run: `cargo test -p claude-view-server -- --nocapture`
Expected: all tests pass

**Step 6: Commit**

```bash
git add crates/server/src/live/manager.rs
git commit -m "fix: remove 300s staleness hack — startup JSONL recovery handles root cause

The 300s last_activity_at check was a band-aid for stale agent_state after
missed Stop hooks during server downtime. Now that startup recovery derives
state from the JSONL ground truth, the runtime check is unnecessary.
PID liveness + hooks cover all runtime failure modes."
```

---

### Task 6: Final verification

**Files:** None (verification only)

**Step 1: Run full server tests**

Run: `cargo test -p claude-view-server -- --nocapture`
Expected: all tests pass

**Step 2: Run core tests (parse_single_line is now public)**

Run: `cargo test -p claude-view-core -- --nocapture`
Expected: all tests pass

**Step 3: Compile release build**

Run: `cargo build --release`
Expected: compiles with no warnings related to our changes
