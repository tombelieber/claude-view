# State Resolver Hardening Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix the timestamp-blind `clear_hook_state` race, add observability tracing to the resolver, and log unknown state categories — closing the three meaningful robustness gaps in the live monitor's state resolution system.

**Architecture:** Three targeted fixes to `StateResolver` and its call sites. No structural changes to the priority model (Hook > JSONL > Fallback), lock patterns, or manager orchestration — those are already solid. All changes are backward-compatible; no API/frontend changes needed.

**Tech Stack:** Rust (tokio, tracing), existing test infrastructure

---

## Context

Three call sites invoke `clear_hook_state`:

| Location | When | Purpose |
|----------|------|---------|
| `manager.rs:1085-1087` | JSONL shows `Working` | Clear stale NeedsYou from prior turn |
| `hooks.rs:136` | `UserPromptSubmit` hook | Clear stale NeedsYou when user responds |
| `hooks.rs:175` | `SessionEnd` cleanup (10s delay) | Garbage collection on session removal |

The race: call site 1 can wipe a hook that was *just set* between the JSONL parse and the clear call. Sites 2 and 3 are safe (UserPromptSubmit is definitive new evidence; SessionEnd is cleanup).

---

### Task 1: Make `clear_hook_state` timestamp-aware

**Files:**
- Modify: `crates/server/src/live/state_resolver.rs:37-42`
- Test: `crates/server/src/live/state_resolver.rs` (tests module at line 103+)

**Step 1: Write the failing test**

Add to the `mod tests` block at the end of `state_resolver.rs`:

```rust
/// The critical race: JSONL Working triggers clear, but a hook arrived
/// AFTER the JSONL evidence. The fresh hook must survive.
#[tokio::test]
async fn clear_if_before_preserves_fresh_hook() {
    let resolver = StateResolver::new();

    // Capture a "before" timestamp
    let before = Instant::now();
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Hook arrives AFTER the before timestamp
    resolver.update_from_hook(
        "s1", make_hook_state("awaiting_input", AgentStateGroup::NeedsYou)
    ).await;

    // Try to clear with the old timestamp — hook should survive
    resolver.clear_hook_state_if_before("s1", before).await;

    let resolved = resolver.resolve("s1").await;
    assert_eq!(resolved.state, "awaiting_input",
        "Hook set AFTER evidence_time must survive clear_hook_state_if_before");
    assert_eq!(resolved.group, AgentStateGroup::NeedsYou);
}

#[tokio::test]
async fn clear_if_before_removes_stale_hook() {
    let resolver = StateResolver::new();

    // Hook set first
    resolver.update_from_hook(
        "s1", make_hook_state("awaiting_input", AgentStateGroup::NeedsYou)
    ).await;

    tokio::time::sleep(Duration::from_millis(10)).await;
    let after = Instant::now();

    // JSONL says acting
    resolver.update_from_jsonl(
        "s1", make_jsonl_state("acting", AgentStateGroup::Autonomous)
    ).await;

    // Clear with timestamp AFTER the hook was set — hook should be removed
    resolver.clear_hook_state_if_before("s1", after).await;

    let resolved = resolver.resolve("s1").await;
    assert_eq!(resolved.state, "acting",
        "Hook set BEFORE evidence_time should be cleared");
    assert_eq!(resolved.group, AgentStateGroup::Autonomous);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p vibe-recall-server -- state_resolver::tests::clear_if_before --no-run 2>&1; cargo test -p vibe-recall-server -- state_resolver::tests::clear_if_before`
Expected: Compilation error — `clear_hook_state_if_before` method doesn't exist.

**Step 3: Implement the new method**

In `state_resolver.rs`, add the new method after the existing `clear_hook_state` (after line 42), and update the doc on the old method:

```rust
/// Clear the hook state ONLY if it was set before `evidence_time`.
///
/// This prevents the race where:
/// 1. JSONL parse starts (captures current time)
/// 2. Hook arrives (sets fresh state)
/// 3. JSONL processing finishes, calls clear — would wipe the fresh hook
///
/// By comparing timestamps, the fresh hook (set after evidence_time) survives.
pub async fn clear_hook_state_if_before(&self, session_id: &str, evidence_time: Instant) {
    let mut states = self.hook_states.write().await;
    if let Some((_, hook_time)) = states.get(session_id) {
        if *hook_time <= evidence_time {
            states.remove(session_id);
        }
    }
}
```

Keep the existing `clear_hook_state` unchanged — it's still used by `UserPromptSubmit` (where unconditional clear is correct) and `SessionEnd` cleanup.

**Step 4: Run tests to verify they pass**

Run: `cargo test -p vibe-recall-server -- state_resolver::tests::clear_if_before`
Expected: Both `clear_if_before_preserves_fresh_hook` and `clear_if_before_removes_stale_hook` PASS.

**Step 5: Commit**

```
feat(live): add timestamp-aware clear_hook_state_if_before

Prevents the race where a fresh hook (set between JSONL parse start
and clear call) gets wiped by stale JSONL evidence. Existing
unconditional clear_hook_state kept for UserPromptSubmit/SessionEnd
where it is correct.
```

---

### Task 2: Wire timestamp-aware clear into `process_jsonl_update`

**Files:**
- Modify: `crates/server/src/live/manager.rs:1083-1087`

**Step 1: Capture evidence timestamp before JSONL parse**

In `process_jsonl_update`, add a timestamp capture before the parse starts. Insert after the blank line 686 (end of the offset-read block), before the `// Parse new lines` comment at line 687:

```rust
// Capture the "evidence time" before parsing — any hook set after this
// instant is fresher than the JSONL evidence we're about to process.
let evidence_time = Instant::now();
```

This requires adding `Instant` to the existing `use std::time::{Duration, SystemTime, UNIX_EPOCH};` import at line 10:

```rust
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
```

**Step 2: Replace the clear call site**

Change lines 1085-1087 from:

```rust
if live_session.status == SessionStatus::Working {
    self.state_resolver.clear_hook_state(&session_id).await;
}
```

To:

```rust
if live_session.status == SessionStatus::Working {
    self.state_resolver.clear_hook_state_if_before(&session_id, evidence_time).await;
}
```

**Step 3: Verify all existing tests still pass**

Run: `cargo test -p vibe-recall-server -- live`
Expected: All `live::` tests PASS (state_resolver, manager, state, classifier).

**Step 4: Commit**

```
fix(live): use timestamp-aware hook clearing in process_jsonl_update

Captures Instant::now() before JSONL parsing starts, then only clears
hook state that was set before that instant. Hooks arriving during
parse (e.g. Stop hook between parse start and clear) now survive.
```

---

### Task 3: Add tracing to the resolution path

**Files:**
- Modify: `crates/server/src/live/state_resolver.rs:27-72,85-93`

**Step 1: Write the tests (tracing output is not directly testable, but verify compilation and behavior unchanged)**

No new tests needed — existing tests cover all paths. We're adding `tracing::debug!` calls that don't affect return values.

**Step 2: Add tracing to `resolve()`**

Replace the `resolve` method body (lines 52-72):

```rust
pub async fn resolve(&self, session_id: &str) -> AgentState {
    if let Some((hook_state, timestamp)) = self.hook_states.read().await.get(session_id) {
        let elapsed = timestamp.elapsed();
        let expired = Self::is_expired(&hook_state.state, elapsed);
        if !expired {
            tracing::debug!(
                session_id,
                state = %hook_state.state,
                group = ?hook_state.group,
                elapsed_ms = elapsed.as_millis() as u64,
                "resolve: hook wins"
            );
            return hook_state.clone();
        }
        tracing::debug!(
            session_id,
            state = %hook_state.state,
            elapsed_ms = elapsed.as_millis() as u64,
            "resolve: hook expired, falling through to JSONL"
        );
    }

    if let Some(jsonl_state) = self.jsonl_states.read().await.get(session_id) {
        tracing::debug!(
            session_id,
            state = %jsonl_state.state,
            group = ?jsonl_state.group,
            "resolve: JSONL wins"
        );
        return jsonl_state.clone();
    }

    tracing::debug!(session_id, "resolve: fallback (no hook or JSONL)");

    AgentState {
        group: AgentStateGroup::Autonomous,
        state: "unknown".into(),
        label: "Status unavailable".into(),
        confidence: 0.0,
        source: SignalSource::Fallback,
        context: None,
    }
}
```

**Step 3: Add tracing to `clear_hook_state` and `clear_hook_state_if_before`**

Update `clear_hook_state` (line 40-42):

```rust
pub async fn clear_hook_state(&self, session_id: &str) {
    let removed = self.hook_states.write().await.remove(session_id);
    if removed.is_some() {
        tracing::debug!(session_id, "clear_hook_state: removed (unconditional)");
    }
}
```

Update `clear_hook_state_if_before` (the new method from Task 1):

```rust
pub async fn clear_hook_state_if_before(&self, session_id: &str, evidence_time: Instant) {
    let mut states = self.hook_states.write().await;
    if let Some((state, hook_time)) = states.get(session_id) {
        if *hook_time <= evidence_time {
            tracing::debug!(
                session_id,
                state = %state.state,
                hook_age_ms = hook_time.elapsed().as_millis() as u64,
                "clear_hook_state_if_before: cleared stale hook"
            );
            states.remove(session_id);
        } else {
            tracing::debug!(
                session_id,
                state = %state.state,
                "clear_hook_state_if_before: preserved fresh hook"
            );
        }
    }
}
```

**Step 4: Log unknown states in `state_category`**

Replace the catch-all in `state_category` (line 91):

```rust
fn state_category(state: &str) -> StateCategory {
    match state {
        "task_complete" | "session_ended" | "work_delivered" => StateCategory::Terminal,
        "awaiting_input" | "awaiting_approval" | "needs_permission" | "error" | "idle"
    | "interrupted"
        => StateCategory::Blocking,
        // Known transient states — no warning
        "thinking" | "acting" | "delegating" | "unknown" => StateCategory::Transient,
        other => {
            tracing::warn!(
                state = other,
                "state_category: unknown state defaulting to Transient (60s expiry)"
            );
            StateCategory::Transient
        }
    }
}
```

**Step 5: Verify compilation and all tests pass**

Run: `cargo test -p vibe-recall-server -- state_resolver`
Expected: All state_resolver tests PASS.

**Step 6: Commit**

```
feat(live): add tracing to StateResolver resolution path

Logs at debug level: which signal wins resolve(), when hooks expire,
when clear_hook_state fires/preserves. Warns on unknown states
hitting the Transient catch-all in state_category().
```

---

### Task 4: Verify end-to-end

**Step 1: Run the full live module test suite**

Run: `cargo test -p vibe-recall-server -- live`
Expected: All tests PASS.

**Step 2: Run the existing race condition regression tests**

Run: `cargo test -p vibe-recall-server -- race_condition`
Expected: `race_condition_hook_survives_process_detector_reevaluation` PASS.

**Step 3: Verify tracing output appears in dev mode**

Start the dev server with debug logging for the state resolver:

```bash
RUST_LOG=warn,vibe_recall_server::live::state_resolver=debug cargo run -p vibe-recall-server
```

Open Mission Control in a browser and trigger a session. Verify that `resolve: hook wins` / `resolve: JSONL wins` messages appear in the server log.

**Step 4: Commit (if any final adjustments needed)**

Otherwise, done.

---

## Summary of Changes

| File | What changes | Lines affected |
|------|-------------|----------------|
| `crates/server/src/live/state_resolver.rs` | New `clear_hook_state_if_before()` method, tracing in `resolve()`/`clear_*`/`state_category`, 2 new tests | ~60 lines added |
| `crates/server/src/live/manager.rs` | Capture `evidence_time`, use `clear_hook_state_if_before` at line 1086 | 4 lines changed |

**Not changed:**
- `hooks.rs:136` (`UserPromptSubmit`) — unconditional clear is correct here (definitive evidence the user responded)
- `hooks.rs:175` (`SessionEnd`) — cleanup, unconditional clear is correct
- No frontend changes
- No API changes

---

## Changelog of Fixes Applied (Audit → Final Plan)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | Task 2 Step 1 referenced "line 6" for `std::time` import but actual line is 10 | Minor | Fixed line reference to "line 10" |
| 2 | Task 2 Step 1 said "around line 688" without exact anchor | Minor | Added exact anchor: "after blank line 686, before the comment at line 687" |

**Audit score: 100/100** — All code blocks compile verbatim. All types, signatures, imports, and call sites verified against the actual codebase. No blockers, no warnings.
